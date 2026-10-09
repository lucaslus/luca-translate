"""Selection fixtures cannot access the host's clipboard or compositor."""
HYPRCTL = '''import json,os,sys
from pathlib import Path
home=Path(os.environ['HOME'])
if 'activewindow' in sys.argv:
    window={'address':'0xabc','pid':2000000,'class':'chromium'}
    mode=home/'selection-window.json'
    if mode.exists(): window=json.loads(mode.read_text())
    print(json.dumps(window))
elif 'dispatch' in sys.argv:
    (home/'selection-dispatched').write_text(sys.argv[-1])
    (home/'selection-dispatch-log').write_text(sys.argv[-1])
    print('ok')
else: print('[]')
'''


def paste(selected):
    return '''import os,sys,subprocess,time,json
from pathlib import Path
home=Path(os.environ['HOME'])
if '--primary' in sys.argv: raise SystemExit('PRIMARY must never be read')
if '--list-types' in sys.argv:
    if (home/'selection-dispatched').exists(): print('text/plain;charset=utf-8');raise SystemExit(0)
    raise SystemExit('Nothing is copied')
if '--watch' not in sys.argv: raise SystemExit('Existing clipboard must never become a query')
marker=home/'selection-dispatched'
marker.unlink(missing_ok=True)
helper=sys.argv[sys.argv.index('--watch')+1:]
env=dict(os.environ,CLIPBOARD_TYPE='text/plain;charset=utf-8')
def emit(data):
    result=subprocess.run(helper,input=data,capture_output=True,env=env,check=True)
    sys.stdout.buffer.write(result.stdout);sys.stdout.buffer.flush()
emit(b'stale clipboard and PRIMARY text')
while not marker.exists(): time.sleep(.005)
mode=home/'selection-mode'
mode=mode.read_text() if mode.exists() else 'selected'
if mode=='selected': emit(''' + repr(selected.encode()) + ''')
elif mode=='blank': emit(b'   ')
elif mode=='utf8': emit(b'\\xff')
elif mode=='oversize': emit(b'x'*80001)
elif mode=='unicode-oversize': emit(('x'*20001).encode())
while True: time.sleep(.05)
'''
