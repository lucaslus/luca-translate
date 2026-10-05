"""Desktop integration tests use a fake compositor and temporary user files."""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import tempfile
import sys
sys.dont_write_bytecode = True
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
loader = importlib.machinery.SourceFileLoader('integration', str(ROOT / 'packaging/omarchy/lucas-translate-setup-omarchy'))
spec = importlib.util.spec_from_loader(loader.name, loader)
integration = importlib.util.module_from_spec(spec)
loader.exec_module(integration)
TEMPLATE = (ROOT / 'packaging/omarchy/lucas-translate.lua').read_text()


def bind(key, description='Another app', **extra):
    _, (mask, symbol) = integration.parse_key(key)
    return dict(modmask=mask, key=symbol, description=description, **extra)


class Shortcuts(unittest.TestCase):
    def test_conflicts_include_physical_codes_submaps_and_duplicates(self):
        defaults = integration.DEFAULTS
        bindings = [bind(defaults['input']), dict(modmask=69, key='', keycode=40, submap='custom', description='Custom selection')]
        conflicts = integration.find_conflicts(defaults, bindings, {})
        self.assertEqual(set(conflicts), {'input', 'selection'})
        desired = {**defaults, 'selection': defaults['input']}
        self.assertEqual(set(integration.find_conflicts(desired, [], {})), {'input', 'selection'})

    def test_recorder_escape_is_not_a_user_shortcut_conflict(self):
        desired = {**integration.DEFAULTS, 'input': 'Ctrl+Escape'}
        internal = dict(modmask=0, key='Escape', ignore_mods=True,
                        submap='lucas-translate-recording', description='Lucas Translate: finish shortcut recording')
        self.assertFalse(integration.find_conflicts(desired, [internal], integration.DEFAULTS))
        self.assertIn('input', integration.find_conflicts(desired, [internal], {}))

    def test_only_one_owned_binding_is_ignored(self):
        defaults = integration.DEFAULTS
        ours = bind(defaults['toggle'], integration.DESCRIPTIONS['toggle'])
        self.assertEqual(integration.find_conflicts(defaults, [ours], defaults), {})
        self.assertIn('toggle', integration.find_conflicts(defaults, [ours, bind(defaults['toggle'])], defaults))
        self.assertIn('toggle', integration.find_conflicts(defaults, [ours, ours], defaults))
        self.assertIn('toggle', integration.find_conflicts(defaults, [ours], {}))

    def test_aliases_and_unsafe_values(self):
        self.assertEqual(integration.parse_key('control + shift + super + i')[0], 'Super+Ctrl+Shift+I')
        for value in ['Super+I";os.execute("bad")', 'Shift+A', 'A', 'Ctrl+Ctrl+A']:
            with self.assertRaises(ValueError):
                integration.parse_key(value)

    def test_install_clear_conflicts_remember_and_reassign(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            (folder / 'hyprland.lua').write_text('-- User desktop\n')
            bindings = [bind(integration.DEFAULTS['input'], 'Existing input')]
            calls = []
            def hypr(*args):
                calls.append(args)
                return json.dumps(bindings) if args[0] == 'binds' else ''
            with patch.object(integration, 'hypr', hypr):
                result = integration.manage(folder, TEMPLATE, 'apply')
                self.assertEqual(result['shortcuts']['input'], '')
                self.assertIn('Existing input', result['conflicts']['input'])
                self.assertEqual(result['shortcuts']['toggle'], integration.DEFAULTS['toggle'])
                self.assertIn('input = ""', (folder / 'lucas-translate.lua').read_text())
                self.assertTrue(list(folder.glob('hyprland.lua.lucas-backup-*')))
                bindings.clear()
                calls.clear()
                again = integration.manage(folder, TEMPLATE, 'apply')
                self.assertEqual(result, again)
                self.assertNotIn(('reload',), calls)
                desired = {**result['shortcuts'], 'input': 'Super+Ctrl+Shift+N'}
                check = integration.manage(folder, TEMPLATE, 'check', desired)
                self.assertFalse(check['conflicts'])
                self.assertEqual(integration.manage(folder, TEMPLATE, 'status')['shortcuts']['input'], '')
                saved = integration.manage(folder, TEMPLATE, 'apply', desired)
                self.assertFalse(saved['conflicts'])
                self.assertEqual(saved['shortcuts']['input'], 'Super+Ctrl+Shift+N')

    def test_reload_failure_rolls_back_every_file(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            config = folder / 'hyprland.lua'
            config.write_text('-- untouched\n')
            def hypr(*args):
                return '[]' if args[0] == 'binds' else 'syntax error' if args[0] == 'configerrors' else ''
            with patch.object(integration, 'hypr', hypr), self.assertRaises(RuntimeError):
                integration.manage(folder, TEMPLATE, 'apply')
            self.assertEqual(config.read_text(), '-- untouched\n')
            self.assertFalse((folder / 'lucas-translate.lua').exists())
            self.assertFalse((folder / 'lucas-translate-shortcuts.json').exists())

    def test_custom_integration_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            (folder / 'hyprland.lua').write_text('-- user\n')
            target = folder / 'lucas-translate.lua'
            target.write_text('-- custom binding\n')
            with self.assertRaises(ValueError):
                integration.manage(folder, TEMPLATE, 'apply')
            self.assertEqual(target.read_text(), '-- custom binding\n')

    def test_upgrade_preserves_custom_shortcuts_and_disabled_choices(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            (folder / 'hyprland.lua').write_text('-- user\n')
            with patch.object(integration, 'hypr', lambda *args: '[]' if args[0] == 'binds' else ''):
                requested = {**integration.DEFAULTS, 'input': 'Super+N', 'ocr': ''}
                integration.manage(folder, TEMPLATE, 'apply', requested)
                result = integration.manage(folder, TEMPLATE + '\n-- upgraded\n', 'apply')
                self.assertEqual(result['shortcuts'], requested)
                self.assertIn('-- upgraded', (folder / 'lucas-translate.lua').read_text())


if __name__ == '__main__':
    unittest.main()
