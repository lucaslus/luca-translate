#!/usr/bin/env python3
"""Actual QML -> private stdio -> Rust -> local HTTP fixture, isolated from the desktop."""
import http.server
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import struct
import tempfile
import threading
import time
import zlib

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('LUCAS_NATIVE_BACKEND',ROOT / 'crates/lucas-omarchy/target/debug/lucas-translate-omarchy-backend'))


class Provider(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):
        pass

    def do_POST(self):
        value=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        text=value['messages'][-1]['content']
        if text=='slow bridge':
            time.sleep(.25)
        data=json.dumps({'choices':[{'message':{'content':'translated: '+text}}]}).encode()
        try:
            self.send_response(200); self.send_header('Content-Length',str(len(data))); self.end_headers(); self.wfile.write(data)
        except (BrokenPipeError,ConnectionResetError):
            pass


def main():
    started=time.monotonic()
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Provider)
    threading.Thread(target=server.serve_forever,daemon=True).start()
    try:
        with tempfile.TemporaryDirectory(prefix='lucas-native-rust-bridge-') as directory:
            scratch=Path(directory); config=scratch / 'config'; config.mkdir()
            shutil.copytree(ROOT / 'omarchy',config / 'Native',ignore=shutil.ignore_patterns('__pycache__'))
            shell=Path(os.environ.get('OMARCHY_PATH','/usr/share/omarchy')) / 'shell'
            for module in ['Commons','Ui']:
                (config / module).symlink_to(shell / module,target_is_directory=True)
            (config / 'shell.qml').write_text((ROOT / 'omarchy/tests/qml/backend.qml').read_text())
            home=scratch / 'home'; data=home / '.local/share/lucas-translate-omarchy'; data.mkdir(parents=True)
            (data / 'config.json').write_text(json.dumps({'enabled':True,'base_url':f'http://127.0.0.1:{server.server_port}/v1',
                'model':'bridge-test','preferences':{'language':'en'},
                'services':{key:False for key in ['youdao','bing','deepl','google','deepl_api']}}))
            hypr=home / '.config/hypr'; hypr.mkdir(parents=True); (hypr / 'hyprland.lua').write_text('-- isolated test config\n')
            tools=home / 'bin'; tools.mkdir()
            image=scratch / 'capture.png'
            def chunk(kind, payload):
                return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload) & 0xffffffff)
            image.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',320,240,8,2,0,0,0))
                +chunk(b'IDAT',zlib.compress((b'\x00'+b'\xe0\xe0\xe0'*320)*240))+chunk(b'IEND',b''))
            programs={
                'hyprctl':'print("[]")',
                'wl-paste':'print("bridge selection")',
                'slurp':'print("0,0 32x32")',
                'grim':'import os,sys,shutil; shutil.copyfile(os.environ["NATIVE_CAPTURE_PNG"],sys.argv[-1])',
                'tesseract':'print("bridge OCR")',
                'wl-copy':'import os,sys; from pathlib import Path; Path(os.environ["HOME"],"clipboard").write_bytes(sys.stdin.buffer.read())',
            }
            for name,source in programs.items():
                path=tools / name; path.write_text('#!/usr/bin/env python3\n'+source+'\n'); path.chmod(0o755)
            environment=dict(os.environ,HOME=str(home),XDG_CONFIG_HOME=str(home / '.config'),XDG_DATA_HOME=str(home / '.local/share'),
                XDG_STATE_HOME=str(home / '.local/state'),PATH=str(tools)+os.pathsep+os.environ['PATH'],
                QT_QPA_PLATFORM='offscreen',QSG_RHI_BACKEND='opengl',LIBGL_ALWAYS_SOFTWARE='1',
                NATIVE_REAL_BACKEND=str(BINARY),NATIVE_REAL_PLUGIN=str(config / 'Native'),NATIVE_CAPTURE_PNG=str(image))
            environment.pop('QT_QUICK_BACKEND',None)
            result=subprocess.run(['quickshell','-p',str(config),'--no-color'],capture_output=True,text=True,timeout=40,env=environment)
            output=result.stdout+result.stderr
            cases=re.findall(r'NATIVE_BRIDGE_CASE_PASS ([\w-]+)',output)
            match=re.search(r'NATIVE_BRIDGE_PASS cases=(\d+)',output)
            failures=['NATIVE_BRIDGE_FAIL','ReferenceError:','TypeError:','Binding loop','ERROR:','Unable to assign','Error decoding:','libpng error:']
            passed=result.returncode==0 and match is not None and len(cases)==int(match[1]) and not any(value in output for value in failures)
            if passed:
                copied=(home / 'clipboard').read_bytes()
                passed=copied.startswith(b'\x89PNG') and struct.unpack('>II',copied[16:24])==(320,240)
            report={'passed':passed,'count':len(cases),'cases':cases,'duration_seconds':round(time.monotonic()-started,2),
                    'backend':str(BINARY),'network':'localhost fixture only','desktop_tools':'isolated executables',
                    'copied_annotation_dimensions':[320,240] if passed else None}
            reports=ROOT / 'dist/omarchy/test-results'; reports.mkdir(parents=True,exist_ok=True)
            (reports / 'bridge.json').write_text(json.dumps(report,indent=2)+'\n'); (reports / 'bridge.log').write_text(output)
            print(output); print(json.dumps(report,indent=2))
            if not passed:
                raise SystemExit(1)
    finally:
        server.shutdown(); server.server_close()


if __name__=='__main__':
    main()
