#!/usr/bin/env python3
"""Install only the native Omarchy variant; preserve the Tauri source and data."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
PLUGIN_ID = 'lucas.translate'


def run(*arguments, **options):
    return subprocess.run(arguments, check=True, text=True, **options)


def require_unlocked():
    result = subprocess.run(['omarchy-shell', 'lock', 'isLocked'], capture_output=True, text=True)
    if result.returncode == 0 and result.stdout.strip() == 'true':
        raise RuntimeError('Unlock the desktop before installing or updating a plugin; this Shell version can crash on a plugin reload while locked.')
    if result.returncode != 0 or result.stdout.strip() != 'false':
        raise RuntimeError('Cannot confirm the desktop is unlocked; no plugin files were modified.')
    monitors = run('hyprctl', '-j', 'monitors', capture_output=True)
    if not any(not monitor.get('disabled', False) for monitor in json.loads(monitors.stdout)):
        raise RuntimeError('No active compositor monitor; plugin reload refused.')


def stamp_revision(staging, binary):
    digest = hashlib.sha256()
    for path in sorted(staging.rglob('*')):
        if path.is_file() and path.name != 'BuildInfo.js':
            digest.update(str(path.relative_to(staging)).encode() + b'\0')
            digest.update(path.read_bytes() + b'\0')
    digest.update(binary.read_bytes())
    revision = digest.hexdigest()
    (staging / 'BuildInfo.js').write_text('var revision = ' + json.dumps(revision) + '\n')
    return revision


def wait_ready():
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        health = run('omarchy-shell', 'shell', 'call', PLUGIN_ID, 'health', '', capture_output=True).stdout.strip()
        if health == 'ready':
            return
        time.sleep(0.2)
    raise RuntimeError('Native panel or backend did not become ready; installation rolled back.')


def verify_revision(expected):
    wait_ready()
    def loaded():
        return run('omarchy-shell', 'shell', 'call', PLUGIN_ID, 'health', 'revision', capture_output=True).stdout.strip()
    if loaded() == expected:
        return
    # This Shell can retain QML/JS caches after rescanPlugins and enable/disable.
    require_unlocked()
    run('omarchy', 'restart', 'shell', capture_output=True)
    wait_ready()
    if loaded() != expected:
        raise RuntimeError('Shell is still running an outdated native UI; installation rolled back.')


def backup(path, destination):
    if not path.exists():
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    if path.is_dir():
        shutil.copytree(path, destination)
    else:
        shutil.copy2(path, destination)


def restore(paths, saved):
    require_unlocked()
    # Roll back only files managed by this installer.
    for path, name in paths:
        previous = saved / name
        if path.is_dir():
            shutil.rmtree(path)
        elif path.exists():
            path.unlink()
        if previous.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            if previous.is_dir():
                shutil.copytree(previous, path)
            else:
                shutil.copy2(previous, path)
    run('omarchy-shell', 'shell', 'rescanPlugins', capture_output=True)
    run('omarchy-shell', 'shell', 'reloadConfig', capture_output=True)
    run('hyprctl', 'reload', capture_output=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, help='Use an already built backend')
    parser.add_argument('--without-shortcuts', action='store_true')
    args = parser.parse_args()
    if os.geteuid() == 0:
        raise SystemExit('Run as the desktop user, without sudo.')
    shell = Path(os.environ.get('OMARCHY_PATH', '/usr/share/omarchy')) / 'shell'
    if not (shell / 'Ui/BorderSurface.qml').is_file():
        raise SystemExit('This variant requires the Omarchy Quickshell plugin runtime.')
    run('omarchy-shell', 'shell', 'ping', capture_output=True)
    require_unlocked()
    if args.binary:
        binary = args.binary.resolve()
    else:
        run('cargo', 'build', '--release', '--locked', '--manifest-path', str(ROOT / 'crates/lucas-omarchy/Cargo.toml'))
        binary = ROOT / 'crates/lucas-omarchy/target/release/lucas-translate-omarchy-backend'
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise SystemExit(f'Missing backend executable: {binary}')
    config = Path(os.environ.get('XDG_CONFIG_HOME', str(Path.home() / '.config')))
    data = Path(os.environ.get('XDG_DATA_HOME', str(Path.home() / '.local/share')))
    state = Path(os.environ.get('XDG_STATE_HOME', str(Path.home() / '.local/state')))
    plugins = config / 'omarchy/plugins'
    target = plugins / PLUGIN_ID
    plugins.mkdir(parents=True, exist_ok=True)
    saved = state / 'lucas-translate-omarchy/install-backups' / str(time.time_ns())
    destination = Path.home() / '.local/bin'
    destination.mkdir(parents=True, exist_ok=True)
    desktop = data / 'applications/lucas-translate.desktop'
    # Keep a complete pre-install snapshot, including legacy desktop shortcuts.
    paths = [(target, 'plugin'), (destination / 'lucas-translate-omarchy-backend', 'backend'),
             (destination / 'lucas-translate-native', 'launcher'), (desktop, 'desktop'),
             (destination / 'lucas-translate', 'dispatch'),
             (config / 'omarchy/shell.json', 'shell.json'),
             (config / 'hypr/hyprland.lua', 'hyprland.lua'),
             (config / 'hypr/lucas-translate.lua', 'lucas-translate.lua'),
             (config / 'hypr/lucas-translate-shortcuts.json', 'shortcuts.json')]
    for path, name in paths:
        backup(path, saved / name)
    try:
        install(binary, config, target, destination, desktop, args.without_shortcuts)
    except Exception:
        restore(paths, saved)
        raise
    print(f'Native Omarchy plugin installed. Previous files: {saved}')


def install(binary, config, target, destination, desktop, without_shortcuts):
    require_unlocked()
    with tempfile.TemporaryDirectory(prefix='lucas-native-install-', dir=target.parent.parent) as directory:
        staging = Path(directory) / PLUGIN_ID
        shutil.copytree(ROOT / 'omarchy', staging, ignore=shutil.ignore_patterns('tests', '__pycache__', '*.pyc'))
        revision = stamp_revision(staging, binary)
        run('omarchy', 'plugin', 'validate', str(staging))
        require_unlocked()
        run('omarchy-shell', 'shell', 'setPluginEnabled', PLUGIN_ID, 'false', capture_output=True)
        if target.exists():
            shutil.rmtree(target)
        staging.replace(target)
    # Atomic executable replacement works even while the old inode is running.
    for source, name in [(binary, 'lucas-translate-omarchy-backend'),
                         (ROOT / 'omarchy/scripts/launch.sh', 'lucas-translate-native'),
                         (ROOT / 'omarchy/scripts/dispatch.sh', 'lucas-translate')]:
        temp = destination / (name + '.installing')
        shutil.copy2(source, temp)
        temp.chmod(0o755)
        temp.replace(destination / name)
    desktop.parent.mkdir(parents=True, exist_ok=True)
    desktop.write_text('[Desktop Entry]\nType=Application\nName=Lucas Translate\nComment=Native Omarchy translation and local OCR\n'
                       f'Exec={destination / "lucas-translate"} --show\nIcon=lucas-translate\nTerminal=false\nCategories=Utility;\n')
    require_unlocked()
    run('omarchy-shell', 'shell', 'rescanPlugins', capture_output=True)
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        discovered = json.loads(run('omarchy-shell', 'shell', 'listPlugins', capture_output=True).stdout)
        if any(item.get('id') == PLUGIN_ID for item in discovered):
            break
        time.sleep(0.2)
    else:
        raise RuntimeError('Shell did not discover the native plugin; installation rolled back.')
    enabled = run('omarchy-shell', 'shell', 'setPluginEnabled', PLUGIN_ID, 'true', capture_output=True).stdout.strip()
    if enabled != 'ok':
        raise RuntimeError('Shell refused to enable the native plugin; installation rolled back.')
    verify_revision(revision)
    if not without_shortcuts:
        run('python3', str(target / 'scripts/shortcuts.py'))
        run('hyprctl', 'reload', capture_output=True)
        errors = run('hyprctl', 'configerrors', capture_output=True).stdout.strip()
        if errors:
            raise RuntimeError(errors)
    installed = json.loads(run('omarchy-shell', 'shell', 'listPlugins', capture_output=True).stdout)
    if not any(item.get('id') == PLUGIN_ID and item.get('enabled') for item in installed):
        raise RuntimeError('Plugin was not enabled; installation rolled back.')


if __name__ == '__main__':
    main()
