"""Exercise the real stdio backend against a local provider and fake desktop tools."""
import http.server
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[2]
BINARY = Path(os.environ.get('LUCAS_NATIVE_BACKEND', ROOT / 'crates/lucas-omarchy/target/debug/lucas-translate-omarchy-backend'))


class Provider(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        text = body['messages'][-1]['content']
        data = json.dumps({'choices': [{'message': {'content': 'translated: ' + text}}]}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


class NativeBackend(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='lucas-native-backend-test-')
        self.home = Path(self.directory.name)
        self.provider = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Provider)
        threading.Thread(target=self.provider.serve_forever, daemon=True).start()
        data = self.home / '.local/share/lucas-translate-omarchy'
        data.mkdir(parents=True)
        (data / 'config.json').write_text(json.dumps({
            'enabled': True, 'base_url': f'http://127.0.0.1:{self.provider.server_port}/v1', 'model': 'local-test',
            'services': {key: False for key in ['youdao', 'deepl', 'bing', 'google', 'deepl_api']},
        }))
        tools = self.home / 'bin'
        tools.mkdir()
        programs = {
            'wl-paste': 'print("selected text")',
            'slurp': 'print("0,0 32x32")',
            'grim': 'from pathlib import Path; import sys; Path(sys.argv[-1]).write_bytes(b"test-image")',
            'tesseract': 'print("recognized text")',
            'wl-copy': 'from pathlib import Path; import os,sys; Path(os.environ["HOME"],"clipboard").write_bytes(sys.stdin.buffer.read())',
        }
        for name, source in programs.items():
            path = tools / name
            path.write_text('#!/usr/bin/env python3\n' + source + '\n')
            path.chmod(0o755)
        environment = {**os.environ, 'HOME': str(self.home), 'PATH': str(tools) + os.pathsep + os.environ['PATH']}
        for name in ['XDG_DATA_HOME', 'XDG_CONFIG_HOME', 'XDG_STATE_HOME']:
            environment.pop(name, None)
        self.process = subprocess.Popen([str(BINARY), '--stdio', '--plugin-dir', str(ROOT / 'omarchy')],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=environment)
        self.messages = queue.Queue()
        def read():
            for line in self.process.stdout:
                self.messages.put(json.loads(line))
        threading.Thread(target=read, daemon=True).start()
        self.assertEqual(self.messages.get(timeout=5)['event'], 'ready')
        self.sequence = 0

    def tearDown(self):
        self.process.terminate()
        self.process.communicate(timeout=5)
        self.provider.shutdown()
        self.provider.server_close()
        self.directory.cleanup()

    def send(self, method, params=None):
        self.sequence += 1
        identity = str(self.sequence)
        self.process.stdin.write(json.dumps({'id': identity, 'method': method, 'params': params or {}}) + '\n')
        self.process.stdin.flush()
        return identity

    def reply(self, identity):
        while True:
            message = self.messages.get(timeout=8)
            if message.get('id') == identity:
                return message

    def call(self, method, params=None):
        reply = self.reply(self.send(method, params))
        self.assertTrue(reply['ok'], reply)
        return reply['data']

    def test_translation_events_and_history(self):
        identity = self.send('translate', {'text': 'quotes " and\nnewlines', 'from': 'en', 'to': 'zh-Hans'})
        events = []
        while True:
            message = self.messages.get(timeout=10)
            if message.get('event') == 'translation':
                self.assertEqual(message['data']['request_id'], identity)
                events.append(message['data'])
                if events[-1]['kind'] == 'done':
                    break
        result = next(event['data'] for event in events if event['kind'] == 'result')
        self.assertEqual(result['paragraphs'], ['translated: quotes " and', 'newlines'])
        history = self.call('history.list')
        self.assertEqual(history[0]['text'], 'quotes " and\nnewlines')

    def test_native_preferences_reject_app_theme_and_secrets_are_not_returned(self):
        settings = self.call('settings')
        self.assertEqual(settings['preferences'], {'language': 'auto'})
        self.assertNotIn('api_key', settings['ai'])
        self.assertFalse(self.reply(self.send('preferences.save', {'language': 'en', 'theme': 'dark'}))['ok'])
        settings = self.call('preferences.save', {'language': 'zh-CN'})
        self.assertEqual(settings['preferences'], {'language': 'zh-CN'})

    def test_favorites_and_copy_preserve_literal_text(self):
        value = '<img src="https://invalid.test/">\n$(echo literal)'
        params = {'text': value, 'result': value, 'service': 'AI'}
        self.call('favorite.add', params)
        self.call('favorite.add', params)
        records = self.call('favorites.list')
        self.assertEqual(len(records), 1)
        self.assertEqual(records[0]['result'], value)
        self.call('copy', {'text': value})
        self.assertEqual((self.home / 'clipboard').read_text(), value)
        self.call('favorite.remove', {'id': records[0]['id']})
        self.assertEqual(self.call('favorites.list'), [])

    def test_ordered_language_rules_persist_and_invalid_edits_preserve_saved_rules(self):
        routing = {'rules': [{'from': 'ja', 'to': 'en'}, {'from': 'en', 'to': 'zh-Hans'}], 'fallback': 'fr'}
        self.assertEqual(self.call('routing.save', routing)['routing'], routing)
        invalid = {'rules': [{'from': 'ja', 'to': 'en'}, {'from': 'ja', 'to': 'fr'}], 'fallback': 'fr'}
        self.assertFalse(self.reply(self.send('routing.save', invalid))['ok'])
        self.assertEqual(self.call('settings')['routing'], routing)

    def test_diagnostics_entry_and_official_test_without_saved_credentials(self):
        self.assertTrue(self.call('diagnostics')['available'])
        directory = Path(self.call('logs.directory'))
        self.assertTrue(directory.is_relative_to(self.home))
        self.assertTrue(directory.is_dir())
        result = self.reply(self.send('official.test'))
        self.assertFalse(result['ok'])
        self.assertIn('DeepL API Key', result['error'])

    def test_selection_screenshot_and_silent_ocr(self):
        self.assertEqual(self.call('capture', {'action': 'selection'})['text'], 'selected text\n')
        self.assertEqual(self.call('capture', {'action': 'screenshot'})['text'], 'recognized text\n')
        result = self.call('capture', {'action': 'ocr'})
        self.assertTrue(result['copied'])
        self.assertNotIn('text', result)
        self.assertEqual((self.home / 'clipboard').read_text(), 'recognized text\n')
        self.assertEqual(self.call('history.list'), [])

    def test_capture_cancellation_does_not_modify_clipboard(self):
        (self.home / 'bin/slurp').write_text('#!/usr/bin/env python3\nraise SystemExit(1)\n')
        result = self.call('capture', {'action': 'screenshot'})
        self.assertTrue(result['cancelled'])
        self.assertFalse((self.home / 'clipboard').exists())

    def test_native_annotation_is_scoped_and_discard_preserves_clipboard(self):
        annotation = self.call('capture', {'action': 'annotate'})
        image = Path(annotation['path'])
        self.assertTrue(image.exists())
        self.assertFalse((self.home / 'clipboard').exists())
        self.assertFalse(self.reply(self.send('annotation.copy', {'annotation_id': 'unowned'}))['ok'])
        output = Path(annotation['output_path'])
        output.write_bytes(b'\x89PNG\r\n\x1a\n' + b'synthetic-png-data')
        self.call('annotation.copy', {'annotation_id': annotation['annotation_id']})
        self.assertTrue((self.home / 'clipboard').read_bytes().startswith(b'\x89PNG'))
        self.assertFalse(image.exists())
        another = self.call('capture', {'action': 'annotate'})
        old_clipboard = (self.home / 'clipboard').read_bytes()
        self.call('annotation.discard', {'annotation_id': another['annotation_id']})
        self.assertFalse(Path(another['path']).exists())
        self.assertEqual((self.home / 'clipboard').read_bytes(), old_clipboard)

    def test_process_exit_removes_temporary_annotation(self):
        annotation = self.call('capture', {'action': 'annotate'})
        image = Path(annotation['path'])
        self.assertTrue(image.exists())
        self.process.terminate()
        self.process.wait(timeout=5)
        self.assertFalse(image.exists())


if __name__ == '__main__':
    unittest.main()
