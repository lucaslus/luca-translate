#!/usr/bin/env python3
"""Drive actual QML controls with Qt Test in an isolated, offscreen window."""
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile
import time
import zlib

ROOT = Path(__file__).resolve().parents[1]
SHELL = Path(os.environ.get('OMARCHY_PATH','/usr/share/omarchy')) / 'shell'


def png(path, width=320, height=240):
    def chunk(kind, data):
        return struct.pack('>I',len(data)) + kind + data + struct.pack('>I',zlib.crc32(kind + data) & 0xffffffff)
    data = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,2,0,0,0))
    data += chunk(b'IDAT',zlib.compress((b'\x00'+b'\xe0\xe0\xe0'*width)*height)) + chunk(b'IEND',b'')
    path.write_bytes(data)


def main():
    with tempfile.TemporaryDirectory(prefix='lucas-native-interactions-') as directory:
        scratch = Path(directory)
        config = scratch / 'config'
        config.mkdir()
        shutil.copytree(ROOT / 'omarchy',config / 'Native',ignore=shutil.ignore_patterns('__pycache__'))
        for module in ['Commons','Ui']:
            (config / module).symlink_to(SHELL / module,target_is_directory=True)
        (config / 'shell.qml').write_text((ROOT / 'omarchy/tests/qml/ui.qml').read_text())
        home = scratch / 'home'; home.mkdir()
        image = scratch / 'fixture.png'; png(image)
        exported = scratch / 'annotated.png'
        environment = dict(os.environ,HOME=str(home),XDG_CONFIG_HOME=str(home / '.config'),
            XDG_DATA_HOME=str(home / '.local/share'),XDG_STATE_HOME=str(home / '.local/state'),
            QT_QPA_PLATFORM='offscreen',QSG_RHI_BACKEND='opengl',LIBGL_ALWAYS_SOFTWARE='1',
            NATIVE_UI_BACKEND_FIXTURE=str(ROOT / 'omarchy/tests/mock_ui_backend.py'),
            NATIVE_TEST_IMAGE=str(image),NATIVE_TEST_OUTPUT=str(exported))
        environment.pop('QT_QUICK_BACKEND',None)
        started = time.monotonic()
        result = subprocess.run(['quickshell','-p',str(config),'--no-color'],capture_output=True,text=True,timeout=100,env=environment)
        output = result.stdout + result.stderr
        cases = re.findall(r'NATIVE_UI_CASE_PASS ([\w-]+)',output)
        match = re.search(r'NATIVE_UI_PASS cases=(\d+)',output)
        failures = ['NATIVE_UI_FAIL','ReferenceError:','TypeError:','Binding loop','ERROR:','Unable to assign']
        passed = result.returncode == 0 and match is not None and len(cases) == int(match[1]) and not any(item in output for item in failures)
        report = {'passed':passed,'cases':cases,'count':len(cases),'duration_seconds':round(time.monotonic()-started,2),
                  'renderer':'offscreen Qt RHI / Mesa software OpenGL'}
        reports = ROOT / 'dist/omarchy/test-results'; reports.mkdir(parents=True,exist_ok=True)
        (reports / 'ui.json').write_text(json.dumps(report,indent=2)+'\n')
        (reports / 'ui.log').write_text(output)
        print(output)
        if not passed:
            raise SystemExit(1)
        print(json.dumps(report,indent=2))


if __name__ == '__main__':
    main()
