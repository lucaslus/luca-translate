"""Verify platform dispatch and lock protection without changing the desktop."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


class Integration(unittest.TestCase):
    def test_selection_launcher_pins_source_without_window_title(self):
        with tempfile.TemporaryDirectory(prefix='lucas-selection-launch-') as directory:
            tools=Path(directory)
            source={'address':'0xabc','pid':123,'class':'chrome-test','title':'private title omitted'}
            hyprctl=tools/'hyprctl';hyprctl.write_text('#!/usr/bin/env python3\nimport json\nprint('+repr(json.dumps(source))+')\n');hyprctl.chmod(0o755)
            shell=tools/'omarchy-shell';shell.write_text('#!/usr/bin/env python3\nimport json,sys\nprint(json.dumps(sys.argv[1:]))\n');shell.chmod(0o755)
            environment=dict(os.environ,PATH=str(tools)+os.pathsep+os.environ['PATH'])
            result=subprocess.run(['bash',str(ROOT/'omarchy/scripts/launch.sh'),'--selection'],capture_output=True,text=True,env=environment,check=True)
            arguments=json.loads(result.stdout)
            self.assertEqual(arguments[:3],['shell','summon','lucas.translate'])
            self.assertEqual(json.loads(arguments[3]),{'action':'selection','source':{key:source[key] for key in ['address','pid','class']}})

    def installer_fixture(self, failure=None, initially_installed=True):
        """Run the actual transaction with every Shell/Hyprland command intercepted."""
        spec = importlib.util.spec_from_file_location('native_installer', ROOT / 'scripts/install-omarchy-native.py')
        installer = importlib.util.module_from_spec(spec); spec.loader.exec_module(installer)
        with tempfile.TemporaryDirectory(prefix='lucas-native-install-transaction-') as directory:
            scratch = Path(directory); home = scratch / 'home'; home.mkdir()
            config = home / '.config'; destination = home / '.local/bin'; destination.mkdir(parents=True)
            desktop = home / '.local/share/applications/lucas-translate.desktop'
            runtime = scratch / 'runtime/shell/Ui'; runtime.mkdir(parents=True); (runtime / 'BorderSurface.qml').write_text('fixture')
            binary = scratch / 'backend'; binary.write_text('#!/bin/sh\nexit 0\n'); binary.chmod(0o755)
            target = config / 'omarchy/plugins/lucas.translate'
            paths = [(target, 'plugin'), (destination / 'lucas-translate-omarchy-backend', 'backend'),
                     (destination / 'lucas-translate-native', 'launcher'), (desktop, 'desktop'),
                     (destination / 'lucas-translate', 'dispatch'), (config / 'omarchy/shell.json', 'shell.json'),
                     (config / 'hypr/hyprland.lua', 'hyprland.lua'), (config / 'hypr/lucas-translate.lua', 'lucas-translate.lua'),
                     (config / 'hypr/lucas-translate-shortcuts.json', 'shortcuts.json')]
            if initially_installed:
                for path, name in paths:
                    if name == 'plugin':
                        path.mkdir(parents=True); (path / 'sentinel').write_text('old plugin')
                    else:
                        path.parent.mkdir(parents=True, exist_ok=True); path.write_text('original ' + name)
            else:
                (config / 'hypr').mkdir(parents=True); (config / 'hypr/hyprland.lua').write_text('original hyprland.lua')
            previous = {name: ('old plugin' if name == 'plugin' else path.read_text()) for path, name in paths if path.exists()}
            listed = 0; calls = []
            def run(*arguments, **options):
                nonlocal listed
                calls.append(arguments)
                output = 'ok'
                if arguments[:3] == ('omarchy-shell', 'shell', 'setPluginEnabled'):
                    state = config / 'omarchy/shell.json'; state.parent.mkdir(parents=True, exist_ok=True); state.write_text('modified shell')
                    if arguments[-1] == 'true' and failure == 'enable': output = 'refused'
                if arguments[:3] == ('omarchy-shell', 'shell', 'rescanPlugins') and failure == 'rescan':
                    raise subprocess.CalledProcessError(1, arguments)
                if arguments[:3] == ('omarchy', 'plugin', 'validate') and failure == 'validate':
                    raise subprocess.CalledProcessError(1, arguments)
                if arguments[:3] == ('omarchy-shell', 'shell', 'listPlugins'):
                    listed += 1
                    output = 'not json' if failure == 'discover' else json.dumps([{'id':'lucas.translate', 'enabled':not (failure == 'final' and listed > 1)}])
                if arguments[:3] == ('omarchy-shell', 'shell', 'call'): output = 'ready'
                if arguments[:2] == ('hyprctl', 'configerrors'): output = 'invalid config' if failure == 'shortcuts' else ''
                return subprocess.CompletedProcess(arguments, 0, stdout=output, stderr='')
            environment = dict(HOME=str(home),XDG_CONFIG_HOME=str(config),XDG_DATA_HOME=str(home / '.local/share'),
                               XDG_STATE_HOME=str(home / '.local/state'),OMARCHY_PATH=str(runtime.parents[1]))
            with patch.dict(os.environ, environment), patch.object(installer, 'run', side_effect=run), \
                 patch.object(installer, 'require_unlocked'), patch.object(installer.os, 'geteuid', return_value=1000), \
                 patch('sys.argv', ['installer','--binary',str(binary)]):
                if failure:
                    with self.assertRaises((RuntimeError,subprocess.CalledProcessError,json.JSONDecodeError)):
                        installer.main()
                    for path, name in paths:
                        if name in previous:
                            self.assertTrue(path.exists(), name)
                            self.assertEqual((path / 'sentinel').read_text() if name == 'plugin' else path.read_text(), previous[name])
                        else:
                            self.assertFalse(path.exists(), name)
                else:
                    installer.main()
                    self.assertTrue((target / 'manifest.json').is_file())
                    self.assertEqual((destination / 'lucas-translate-omarchy-backend').read_text(), binary.read_text())
                    self.assertNotIn('tests', [value.name for value in target.iterdir()])
                    backups = list((home / '.local/state/lucas-translate-omarchy/install-backups').iterdir())
                    self.assertEqual(len(backups), 1)
                    self.assertEqual((backups[0] / 'hyprland.lua').read_text(), 'original hyprland.lua')
                self.assertTrue(all(str(home) in str(path) for path, _ in paths))

    def test_installer_success_preserves_backup_and_excludes_fixtures(self):
        self.installer_fixture()

    def test_installer_rolls_back_existing_resources_at_each_failure(self):
        for failure in ['validate','rescan','discover','enable','shortcuts','final']:
            with self.subTest(failure=failure): self.installer_fixture(failure)

    def test_installer_rolls_back_first_install_without_leaving_managed_files(self):
        self.installer_fixture('final', initially_installed=False)

    def test_launcher_dispatch_is_limited_to_omarchy_sessions(self):
        with tempfile.TemporaryDirectory(prefix='lucas-native-dispatch-test-') as directory:
            shell = Path(directory) / 'omarchy/shell'
            shell.mkdir(parents=True)
            (shell / 'shell.qml').write_text('')
            tools = Path(directory) / 'bin'
            tools.mkdir()
            ipc = tools / 'omarchy-shell'
            ipc.write_text('#!/usr/bin/env bash\n[[ "$*" == "shell ping" ]] || exit 2\nexit "${NATIVE_TEST_SHELL_STATUS:-0}"\n')
            ipc.chmod(0o755)
            environment = dict(os.environ, HOME=directory, PATH=str(tools) + os.pathsep + os.environ['PATH'])
            # Intercept exec so neither native nor Tauri opens a real window.
            wrapper = 'exec() { printf "exec\\0"; printf "%s\\0" "$@"; exit 0; }; export -f exec; bash "$@"'
            arguments = ['--selection', 'quotes " and $(literal)']
            desktop = next((path for path in ['/usr/bin/lucas-translate', '/usr/local/bin/lucas-translate']
                            if os.access(path, os.X_OK)), None)
            for hyprland, omarchy, shell_status, native in [
                ('instance', str(shell.parent), '0', True),
                ('instance', str(shell.parent), '1', False),
                ('', str(shell.parent), '0', False),
                ('instance', '', '0', False),
                ('instance', directory, '0', False),
                ('', '', '0', False),
            ]:
                with self.subTest(hyprland=hyprland, omarchy=omarchy):
                    result = subprocess.run(['bash', '-c', wrapper, 'test', str(ROOT / 'omarchy/scripts/dispatch.sh'), *arguments],
                        capture_output=True, env=dict(environment, HYPRLAND_INSTANCE_SIGNATURE=hyprland, OMARCHY_PATH=omarchy,
                                                      NATIVE_TEST_SHELL_STATUS=shell_status))
                    if native or desktop:
                        self.assertEqual(result.returncode, 0, result.stderr)
                        expected = str(Path(directory) / '.local/bin/lucas-translate-native') if native else desktop
                        self.assertEqual(result.stdout.decode().split('\0')[:-1], ['exec', expected, *arguments])
                    else:
                        self.assertEqual(result.returncode, 1)
                        self.assertEqual(result.stdout, b'')

    def test_installer_refuses_locked_or_unknown_lock_state(self):
        spec = importlib.util.spec_from_file_location('native_installer', ROOT / 'scripts/install-omarchy-native.py')
        installer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(installer)
        for code, state in [(0, 'true\n'), (1, ''), (0, 'unknown\n')]:
            with self.subTest(code=code, state=state), patch.object(installer.subprocess, 'run',
                    return_value=subprocess.CompletedProcess([], code, stdout=state, stderr='')):
                with self.assertRaises(RuntimeError):
                    installer.require_unlocked()
        with patch.object(installer.subprocess, 'run',
                return_value=subprocess.CompletedProcess([], 0, stdout='false\n', stderr='')):
            installer.require_unlocked()


if __name__ == '__main__':
    unittest.main()
