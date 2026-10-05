#!/usr/bin/env python3
"""Load the real native components and verify live theme bindings without mapping windows."""
from pathlib import Path
import os
import shutil
import subprocess
import struct
import tempfile
import zlib

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
    result = subprocess.run(['quickshell', '-p', str(root), '--no-color'], capture_output=True, text=True, timeout=25)
    output = result.stdout + result.stderr
    print(output)
    failures = ['NATIVE_QML_FAIL', 'ReferenceError:', 'TypeError:', 'Binding loop', 'ERROR:', 'Failed to load configuration', 'Unable to assign']
    if result.returncode != 0 or 'NATIVE_QML_PASS' not in output or any(item in output for item in failures):
        raise SystemExit(1)
    # Render and export the actual annotation Canvas through a software offscreen window.
    def chunk(kind, data):
        return struct.pack('>I',len(data)) + kind + data + struct.pack('>I',zlib.crc32(kind + data) & 0xffffffff)
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR',struct.pack('>IIBBBBB',128,96,8,2,0,0,0))
    png += chunk(b'IDAT',zlib.compress((b'\x00' + b'\xe0\xe0\xe0'*128)*96)) + chunk(b'IEND',b'')
    # Quickshell virtualizes its config directory; writable artifacts belong outside it.
    images = tempfile.TemporaryDirectory(prefix='lucas-native-image-test-')
    image = Path(images.name) / 'fixture.png'; image.write_bytes(png)
    exported = Path(images.name) / 'annotated.png'
    (root / 'shell.qml').write_text((ROOT / 'omarchy/tests/qml/annotation.qml').read_text())
    environment = {**os.environ,'QT_QPA_PLATFORM':'offscreen','QT_QUICK_BACKEND':'software',
        'NATIVE_TEST_IMAGE':str(image),'NATIVE_TEST_OUTPUT':str(exported)}
    annotation = subprocess.run(['quickshell','-p',str(root),'--no-color'],capture_output=True,text=True,timeout=15,env=environment)
    output = annotation.stdout + annotation.stderr
    print(output)
    if annotation.returncode != 0 or 'NATIVE_ANNOTATION_PASS' not in output or any(item in output for item in failures):
        raise SystemExit(1)
    result = exported.read_bytes()
    assert struct.unpack('>II',result[16:24]) == (128,96), 'Export did not preserve capture resolution'
    assert result != png, 'Annotation did not change the image'
    images.cleanup()
    preview = ROOT / 'dist/omarchy/preview'
    preview.mkdir(parents=True, exist_ok=True)
    (root / 'shell.qml').write_text((ROOT / 'omarchy/tests/qml/preview.qml').read_text())
    environment['NATIVE_PREVIEW_DIR'] = str(preview)
    # Mesa software OpenGL preserves the same scenegraph transforms as the
    # desktop renderer; Qt's software scenegraph misplaces the header text here.
    environment.pop('QT_QUICK_BACKEND', None)
    environment['QSG_RHI_BACKEND'] = 'opengl'
    environment['LIBGL_ALWAYS_SOFTWARE'] = '1'
    for page in ['translate', 'settings']:
        environment['NATIVE_PREVIEW_PAGE'] = page
        result = subprocess.run(['quickshell','-p',str(root),'--no-color'],capture_output=True,text=True,timeout=15,env=environment)
        output = result.stdout + result.stderr
        print(output)
        if result.returncode != 0 or 'NATIVE_PREVIEW_PASS' not in output or any(item in output for item in failures):
            raise SystemExit(1)
