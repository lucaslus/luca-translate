#!/usr/bin/env python3
"""Load the real native components and verify live theme bindings without mapping windows."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import sys

ROOT = Path(__file__).resolve().parents[1]
SHELL = Path(os.environ.get('OMARCHY_PATH', '/usr/share/omarchy')) / 'shell'
with tempfile.TemporaryDirectory(prefix='lucas-native-qml-test-') as directory:
    root = Path(directory)
    plugin = root / 'Native'
    shutil.copytree(ROOT / 'omarchy', plugin)
    # qs imports are relative to the entry shell's configuration root.
    for module in ['Commons', 'Ui']:
        (root / module).symlink_to(SHELL / module, target_is_directory=True)
    source = (ROOT / 'omarchy/tests/qml/shell.qml').read_text().replace('import "../../" as Native', 'import "Native" as Native').replace('"../../" + file', '"Native/" + file')
    (root / 'shell.qml').write_text(source)
    result = subprocess.run(['quickshell', '-p', str(root), '--no-color'], capture_output=True, text=True, timeout=25, env=dict(os.environ,QT_QPA_PLATFORM='offscreen',QSG_RHI_BACKEND='opengl',LIBGL_ALWAYS_SOFTWARE='1'))
    output = result.stdout + result.stderr
    print(output)
    failures = ['NATIVE_QML_FAIL', 'ReferenceError:', 'TypeError:', 'Binding loop', 'ERROR:', 'Failed to load configuration', 'Unable to assign']
    if result.returncode != 0 or 'NATIVE_QML_PASS' not in output or any(item in output for item in failures):
        raise SystemExit(1)
    preview = ROOT / 'dist/omarchy/preview'
    preview.mkdir(parents=True, exist_ok=True)
    (root / 'shell.qml').write_text((ROOT / 'omarchy/tests/qml/preview.qml').read_text())
    environment = dict(os.environ, QT_QPA_PLATFORM='offscreen', NATIVE_PREVIEW_DIR=str(preview))
    # Mesa software OpenGL preserves the same scenegraph transforms as the
    # desktop renderer; Qt's software scenegraph misplaces the header text here.
    environment.pop('QT_QUICK_BACKEND', None)
    environment['QSG_RHI_BACKEND'] = 'opengl'
    environment['LIBGL_ALWAYS_SOFTWARE'] = '1'
    for page, theme in [(page, theme) for theme in ['current', 'light'] for page in ['translate', 'empty', 'settings', 'overflow', 'loading']]:
        environment['NATIVE_PREVIEW_PAGE'] = page
        environment['NATIVE_PREVIEW_THEME'] = theme
        result = subprocess.run(['quickshell','-p',str(root),'--no-color'],capture_output=True,text=True,timeout=15,env=environment)
        output = result.stdout + result.stderr
        print(output)
        if result.returncode != 0 or 'NATIVE_PREVIEW_PASS' not in output or any(item in output for item in failures):
            raise SystemExit(1)
    (root / 'shell.qml').write_text((ROOT / 'omarchy/tests/qml/icons.qml').read_text())
    result = subprocess.run(['quickshell','-p',str(root),'--no-color'],capture_output=True,text=True,timeout=15,env=environment)
    output=result.stdout+result.stderr
    print(output)
    if result.returncode!=0 or 'NATIVE_ICON_PASS' not in output or any(item in output for item in failures):
        raise SystemExit(1)
    sys.path.insert(0,str(ROOT/'omarchy/tests'))
    from icon_test import verify
    verify(preview)
