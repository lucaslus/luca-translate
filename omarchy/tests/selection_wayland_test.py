"""Real Rust/Wayland clipboard transfers against a private data-control server.

Implements only wl_display, wl_registry, wl_seat and wlr-data-control v2.
There are no windows, desktop input, or connections to the user's compositor.
"""
import array
import json
import os
from pathlib import Path
import selectors
import socket
import struct
import subprocess
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT/'crates/lucas-omarchy/target/debug/lucas-translate-omarchy-backend'


def word(value): return struct.pack('<I',value)


def string(value):
    data=value.encode()+b'\0'
    return word(len(data))+data+b'\0'*((-len(data))%4)


def read_string(data, offset=0):
    length=struct.unpack_from('<I',data,offset)[0]
    return data[offset+4:offset+4+length-1].decode(), offset+4+(length+3)//4*4


class Server:
    def __init__(self, directory, clipboard):
        self.directory=directory
        self.clipboard=clipboard
        self.selection=None
        self.owner=None
        self.errors=[]
        self.copy_requested=threading.Event()
        self.release_copy=threading.Event()
        self.delay_copy=False
        self.pending_copy=False
        self.stopped=threading.Event()
        self.selector=selectors.DefaultSelector()
        self.listener=socket.socket(socket.AF_UNIX)
        self.path=directory/'wayland-native-test'
        self.listener.bind(str(self.path));self.listener.listen();self.listener.setblocking(False)
        self.selector.register(self.listener,selectors.EVENT_READ,None)
        self.clients={}
        self.thread=threading.Thread(target=self.run,daemon=True);self.thread.start()

    def event(self, client, identity, opcode, data=b'', fd=None):
        packet=struct.pack('<II',identity,(len(data)+8)<<16|opcode)+data
        if fd is None: client['socket'].sendall(packet)
        else: client['socket'].sendmsg([packet],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[fd]))])

    def offer(self, client, device):
        if self.clipboard is None:
            self.event(client,device,1,word(0));return
        identity=client['next'];client['next']+=1
        contents=self.clipboard.copy() if isinstance(self.clipboard,dict) else self.clipboard
        client['objects'][identity]=('offer',contents)
        self.event(client,device,0,word(identity))
        types=contents.keys() if isinstance(contents,dict) else contents[0]['objects'][contents[1]][1]
        for mime in types: self.event(client,identity,0,string(mime))
        self.event(client,device,1,word(identity))

    def replace(self, contents):
        if self.owner:
            client,identity=self.owner
            try: self.event(client,identity,1)
            except (BrokenPipeError,ConnectionResetError): pass
        self.clipboard=contents
        self.owner=contents if isinstance(contents,tuple) else None
        for client in list(self.clients.values()):
            for identity in list(client['devices']):
                try: self.offer(client,identity)
                except (BrokenPipeError,ConnectionResetError): pass

    def request(self, client, identity, opcode, body):
        kind,data=client['objects'].get(identity,('unknown',None))
        words=lambda offset=0:struct.unpack_from('<I',body,offset)[0]
        if kind=='display':
            if opcode==1:
                registry=words();client['objects'][registry]=('registry',None)
                self.event(client,registry,0,word(1)+string('wl_seat')+word(7))
                self.event(client,registry,0,word(2)+string('zwlr_data_control_manager_v1')+word(2))
            elif opcode==0:
                callback=words();self.event(client,callback,0,word(1));self.event(client,1,1,word(callback))
        elif kind=='registry':
            interface,offset=read_string(body,4);newid=words(offset+4)
            if interface=='wl_seat':
                client['objects'][newid]=('seat',None)
                self.event(client,newid,0,word(0));self.event(client,newid,1,string('default'))
            else: client['objects'][newid]=('manager',None)
        elif kind=='manager':
            if opcode==0: client['objects'][words()]=('source',[])
            elif opcode==1:
                device=words();client['objects'][device]=('device',None);client['devices'].add(device)
                self.offer(client,device);self.event(client,device,3,word(0))
        elif kind=='source':
            if opcode==0: data.append(read_string(body)[0])
        elif kind=='device':
            if opcode==0:
                source=words();self.replace((client,source) if source else None)
            elif opcode==1: client['devices'].discard(identity)
        elif kind=='offer' and opcode==0:
            mime=read_string(body)[0];fd=client['fds'].pop(0)
            try:
                if isinstance(data,dict): os.write(fd,data[mime])
                else: self.event(data[0],data[1],0,string(mime),fd)
            finally: os.close(fd)

    def run(self):
        try:
            while not self.stopped.is_set():
                trigger=self.directory/'copy-request'
                if trigger.exists():
                    trigger.unlink()
                    self.copy_requested.set()
                    if self.delay_copy: self.pending_copy=True
                    elif self.selection is not None: self.replace({'text/plain;charset=utf-8':self.selection})
                if self.pending_copy and self.release_copy.is_set():
                    self.pending_copy=False
                    if self.selection is not None: self.replace({'text/plain;charset=utf-8':self.selection})
                for key,_ in self.selector.select(.005):
                    if key.data is None:
                        connection,_=self.listener.accept()
                        client={'socket':connection,'buffer':bytearray(),'fds':[],'devices':set(),'objects':{1:('display',None)},'next':0xff000000}
                        self.clients[connection]=client;self.selector.register(connection,selectors.EVENT_READ,client)
                    else:
                        client=key.data
                        try: data,ancillary,_,_=key.fileobj.recvmsg(65536,socket.CMSG_SPACE(32))
                        except ConnectionResetError: data=b'';ancillary=[]
                        if not data:
                            self.selector.unregister(key.fileobj);self.clients.pop(key.fileobj,None);key.fileobj.close();continue
                        client['buffer'].extend(data)
                        for level,kind,fds in ancillary:
                            if level==socket.SOL_SOCKET and kind==socket.SCM_RIGHTS:
                                received=array.array('i');received.frombytes(fds);client['fds'].extend(received)
                        buffer=client['buffer']
                        while len(buffer)>=8:
                            identity,header=struct.unpack_from('<II',buffer);size=header>>16
                            if len(buffer)<size: break
                            body=bytes(buffer[8:size]);del buffer[:size]
                            try: self.request(client,identity,header&0xffff,body)
                            except (BrokenPipeError,ConnectionResetError):
                                self.selector.unregister(key.fileobj);self.clients.pop(key.fileobj,None);key.fileobj.close();break
        except Exception as error: self.errors.append(repr(error))

    def close(self):
        if self.owner:
            try: self.event(self.owner[0],self.owner[1],1)
            except (BrokenPipeError,ConnectionResetError): pass
        self.stopped.set();self.thread.join(timeout=2)
        for client in list(self.clients.values()): client['socket'].close()
        self.listener.close();self.selector.close()


class SelectionWayland(unittest.TestCase):
    def setUp(self):
        self.scratch=tempfile.TemporaryDirectory(prefix='lucas-selection-wayland-')
        self.directory=Path(self.scratch.name)
        self.original={'text/plain;charset=utf-8':b'old clipboard','text/html':b'<b>old clipboard</b>',
            'image/png':b'\x89PNG\x00\xff\x13', 'application/x-private-native-test':b'\x00\x01\xff'}
        self.server=Server(self.directory,self.original)
        tools=self.directory/'bin';tools.mkdir()
        tool=tools/'hyprctl'
        tool.write_text('''#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
root=Path(os.environ['XDG_RUNTIME_DIR'])
if 'activewindow' in sys.argv: print(json.dumps({'address':'0xabc','pid':2000000,'class':'chromium'}))
else:
    (root/'copy-request').touch()
    print('ok')
''');tool.chmod(0o755)
        self.environment=dict(os.environ,XDG_RUNTIME_DIR=str(self.directory),WAYLAND_DISPLAY=str(self.server.path),
            DBUS_SESSION_BUS_ADDRESS='unix:path=/missing-native-test-bus',PATH=str(tools)+os.pathsep+os.environ['PATH'])

    def tearDown(self):
        self.server.close();self.scratch.cleanup()
        self.assertEqual(self.server.errors,[])

    def read(self):
        result=subprocess.run([str(BINARY),'--read-selection','null'],capture_output=True,text=True,env=self.environment,timeout=6)
        self.assertEqual(result.returncode,0,result.stderr)
        return json.loads(result.stdout)

    def contents(self):
        types=subprocess.check_output(['wl-paste','--list-types'],env=self.environment,text=True,timeout=2).splitlines()
        return {mime:subprocess.check_output(['wl-paste','--no-newline','--type',mime],env=self.environment,timeout=2) for mime in types}

    def test_current_selection_restores_all_clipboard_formats(self):
        self.server.selection='本次鼠标选区'.encode()
        self.assertEqual(self.read(),{'text':'本次鼠标选区'})
        self.assertEqual(self.contents(),self.original)

    def test_no_selection_does_not_translate_existing_clipboard(self):
        self.assertEqual(self.read(),{'text':''})
        self.assertEqual(self.contents(),self.original)

    def test_identical_selection_can_be_translated_again(self):
        self.server.selection=b'old clipboard'
        self.assertEqual(self.read(),{'text':'old clipboard'})
        self.assertEqual(self.read(),{'text':'old clipboard'})
        self.assertEqual(self.contents(),self.original)

    def test_deselection_after_success_does_not_reuse_previous_query(self):
        self.server.selection=b'live selected text'
        self.assertEqual(self.read(),{'text':'live selected text'})
        self.server.selection=None
        self.assertEqual(self.read(),{'text':''})
        self.assertEqual(self.contents(),self.original)

    def test_invalid_utf8_restores_clipboard_without_translating(self):
        self.server.selection=b'\xff'
        self.assertIn('UTF-8',self.read()['error'])
        self.assertEqual(self.contents(),self.original)

    def test_image_only_clipboard_is_not_a_readiness_deadlock(self):
        image={'image/png':b'\x89PNG\x00\xff'}
        self.server.replace(image)
        self.server.selection=b'live selection over an image clipboard'
        self.assertEqual(self.read(),{'text':'live selection over an image clipboard'})
        self.assertEqual(self.contents(),image)

    def test_empty_clipboard_is_restored_after_current_selection(self):
        self.server.replace(None)
        self.server.selection=b'live selection'
        self.assertEqual(self.read(),{'text':'live selection'})
        result=subprocess.run(['wl-paste','--list-types'],capture_output=True,env=self.environment,timeout=2)
        self.assertNotEqual(result.returncode,0)

    def test_backend_shutdown_still_restores_clipboard(self):
        import time
        self.server.delay_copy=True
        self.server.selection=b'live selected text'
        home=self.directory/'home';home.mkdir()
        process=subprocess.Popen([str(BINARY),'--stdio'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
            text=True,env=dict(self.environment,HOME=str(home),XDG_DATA_HOME=str(home/'.local/share'),XDG_STATE_HOME=str(home/'.local/state')))
        try:
            self.assertEqual(json.loads(process.stdout.readline())['event'],'ready')
            process.stdin.write(json.dumps({'id':'capture','method':'capture','params':{'action':'selection'}})+'\n');process.stdin.flush()
            self.assertTrue(self.server.copy_requested.wait(timeout=3),'Selection did not reach the copy request')
            process.terminate();process.wait(timeout=1)
            self.server.release_copy.set()
            deadline=time.monotonic()+2
            while self.server.owner is None and time.monotonic()<deadline: time.sleep(.01)
            self.assertIsNotNone(self.server.owner,'Clipboard restoration stopped with backend')
            self.assertEqual(self.contents(),self.original)
        finally:
            if process.poll() is None: process.terminate()
            process.communicate(timeout=2)


if __name__=='__main__': unittest.main()
