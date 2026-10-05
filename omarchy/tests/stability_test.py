"""Bounded stress and fault injection against the real native binary; no desktop access."""
import http.server
import json
from pathlib import Path
import threading
import time
import unittest

import backend_test as fixture


class FaultProvider(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        value = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        text = value['messages'][-1]['content']
        self.server.started.set()
        self.server.calls += 1
        time.sleep(self.server.delay)
        status = self.server.status
        data = self.server.body
        if data is None:
            data = json.dumps({'choices': [{'message': {'content': 'translated: ' + text}}]}).encode()
        try:
            self.send_response(status)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(data)))
            if status == 429:
                self.send_header('Retry-After', '3')
            self.end_headers()
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass


class Stability(fixture.NativeBackend):
    # Inherited functional cases are run separately, not counted twice.
    def setUp(self):
        super().setUp()
        self.provider.RequestHandlerClass = FaultProvider
        self.provider.started = threading.Event()
        self.provider.calls = 0
        self.provider.delay = 0
        self.provider.status = 200
        self.provider.body = None

    def tool(self, name, source):
        (self.home / 'bin' / name).write_text('#!/usr/bin/env python3\n' + source + '\n')

    def translated(self, text, only=None):
        identity = self.send('translate', {'text': text, 'from': 'en', 'to': 'zh-Hans', 'only': only})
        events = []
        while True:
            event = self.messages.get(timeout=8)
            if event.get('event') == 'translation' and event['data']['request_id'] == identity:
                events.append(event['data'])
                if events[-1]['kind'] == 'done':
                    return events

    def batch(self, method, params, count):
        identities = {self.send(method, params(i)) for i in range(count)}
        results = []
        while identities:
            value = self.messages.get(timeout=8)
            if value.get('id') in identities:
                identities.remove(value['id'])
                results.append(value)
        return results

    def fail(self, method, params):
        result = self.reply(self.send(method, params))
        self.assertFalse(result['ok'], result)
        self.assertTrue(result['error'])
        self.assertTrue(self.call('settings')['ai']['enabled'])
        return result

    def test_fault_malformed_frames_recover_without_echo(self):
        for data in ['{secret-sample', 'null', '[]', '{"id":"","method":"settings"}',
                     json.dumps({'id': 'a' * 129, 'method': 'settings'}),
                     '{"id":"x","method":"settings","extra":"synthetic-secret"}']:
            self.process.stdin.write(data + '\n'); self.process.stdin.flush()
            message = self.messages.get(timeout=5)
            self.assertEqual(message['event'], 'protocol_error')
            self.assertNotIn('secret', json.dumps(message))
        self.assertIn('services', self.call('settings'))

    def test_fault_oversized_frame_exits_cleanly(self):
        self.process.stdin.write(' ' * (128 * 1024 + 1)); self.process.stdin.flush()
        self.assertEqual(self.messages.get(timeout=5)['data']['code'], 'frame_too_large')
        self.assertEqual(self.process.wait(timeout=3), 0)

    def test_fault_unknown_and_invalid_parameters_remain_responsive(self):
        for method, params in [('unknown', {}), ('translate', {'text': 'hello', 'to': 'invalid'}),
                               ('translate', {'text': ' '}), ('translate', {'text': 'x' * 20001}),
                               ('favorite.add', {}), ('capture', {'action': 'invalid'}),
                               ('service.set', {'id': 'unknown', 'enabled': True})]:
            self.fail(method, params)

    def test_stress_300_concurrent_requests_are_bounded_and_answered(self):
        results = self.batch('settings', lambda _: {}, 300)
        self.assertEqual(len(results), 300)
        self.assertTrue(any(result['ok'] for result in results))
        self.assertTrue(all(result['ok'] or result['error'] == 'Too many pending operations' for result in results))
        self.assertIn('services', self.call('settings'))

    def test_stress_concurrent_favorites_are_deduplicated(self):
        params = {'text': 'same', 'result': 'same result', 'service': 'AI'}
        results = self.batch('favorite.add', lambda _: params, 100)
        self.assertTrue(any(result['ok'] for result in results))
        self.assertEqual(len(self.call('favorites.list')), 1)

    def test_stress_150_atomic_settings_writes_keep_valid_json(self):
        invalid = []; stop = threading.Event()
        path = self.home / '.local/share/lucas-translate-omarchy/config.json'
        def read():
            while not stop.is_set():
                try:
                    json.loads(path.read_text())
                except Exception as error:
                    invalid.append(str(error)); break
        reader = threading.Thread(target=read); reader.start()
        try:
            for i in range(150):
                self.call('preferences.save', {'language': 'en' if i % 2 else 'zh-CN'})
        finally:
            stop.set(); reader.join()
        self.assertEqual(invalid, [])
        self.assertEqual(self.call('settings')['preferences']['language'], 'en')

    def test_stress_100_translations_bound_memory_and_fds(self):
        for i in range(10):
            self.translated('warmup ' + str(i))
        def measure():
            status = Path(f'/proc/{self.process.pid}/status').read_text()
            rss = int(next(line for line in status.splitlines() if line.startswith('VmRSS:')).split()[1])
            return rss, len(list(Path(f'/proc/{self.process.pid}/fd').iterdir()))
        before = measure()
        for i in range(100):
            events = self.translated('stress ' + str(i))
            self.assertEqual(next(value['data']['paragraphs'] for value in events if value['kind'] == 'result'), ['translated: stress ' + str(i)])
        after = measure()
        metrics['backend_stress'] = {'translations': 100, 'rss_kib_before': before[0], 'rss_kib_after': after[0],
                                     'fds_before': before[1], 'fds_after': after[1]}
        self.assertLess(after[0] - before[0], 32 * 1024)
        self.assertLessEqual(after[1], before[1] + 4)
        rows = self.call('history.list', {'limit': 100})
        self.assertEqual(len(rows), 100)
        self.assertEqual(len(self.call('history.list', {'offset': 100})), 10)
        self.assertEqual(len({row['id'] for row in rows}), 100)

    def test_fault_http_429_is_structured_and_cools_down(self):
        self.provider.status = 429
        events = self.translated('rate limited')
        result = next(value['data'] for value in events if value['kind'] == 'result')
        self.assertTrue(result['error']); self.assertIsNotNone(result['failure'])
        calls = self.provider.calls
        events = self.translated('other limited')
        self.assertTrue(next(value['data'] for value in events if value['kind'] == 'result')['error'])
        self.assertEqual(self.provider.calls, calls)

    def test_fault_http_500_does_not_record_success_history(self):
        self.provider.status = 500
        events = self.translated('server broken')
        self.assertTrue(next(value['data'] for value in events if value['kind'] == 'result')['error'])
        self.assertEqual(self.call('history.list'), [])

    def test_fault_malformed_provider_json(self):
        self.provider.body = b'{invalid'
        events = self.translated('malformed')
        self.assertTrue(next(value['data'] for value in events if value['kind'] == 'result')['error'])
        self.assertEqual(self.call('history.list'), [])

    def test_fault_empty_provider_result(self):
        self.provider.body = b'{"choices":[{"message":{"content":""}}]}'
        events = self.translated('empty provider')
        self.assertTrue(next(value['data'] for value in events if value['kind'] == 'result')['error'])

    def test_fault_no_enabled_services_and_invalid_retry_finish(self):
        update = {key: value for key, value in self.call('settings')['ai'].items() if key != 'has_api_key'}
        self.call('ai.save', {**update, 'enabled': False})
        events = self.translated('disabled')
        self.assertIn('error', [value['kind'] for value in events])
        self.call('ai.save', {**update, 'enabled': True})
        events = self.translated('bad retry', 'MissingService')
        self.assertIn('error', [value['kind'] for value in events])

    def test_cancellation_suppresses_late_results_and_history(self):
        self.provider.delay = .4
        identity = self.send('translate', {'text': 'cancel private payload', 'from': 'en', 'to': 'zh-Hans'})
        self.assertTrue(self.provider.started.wait(3))
        started = time.monotonic()
        self.call('cancel', {'request_id': identity})
        self.assertLess(time.monotonic() - started, .5)
        time.sleep(.5)
        messages = []
        while not self.messages.empty():
            messages.append(self.messages.get())
        self.assertFalse(any(value.get('event') == 'translation' and value['data']['kind'] == 'result' for value in messages))
        self.assertEqual(self.call('history.list'), [])

    def test_replacement_discards_previous_request(self):
        self.provider.delay = .25
        identity = self.send('translate', {'text': 'superseded', 'from': 'en', 'to': 'zh-Hans'})
        self.assertTrue(self.provider.started.wait(3))
        events = self.translated('current')
        self.assertEqual(next(value['data']['text'] for value in events if value['kind'] == 'result'), 'current')
        self.assertEqual([row['text'] for row in self.call('history.list')], ['current'])

    def test_retry_uses_cache_without_duplicate_history(self):
        self.translated('cache fixture')
        self.translated('cache fixture', 'AI')
        self.assertEqual(self.provider.calls, 1)
        self.assertEqual(len(self.call('history.list')), 1)

    def test_sigterm_during_network_is_bounded(self):
        self.provider.delay = 3
        self.send('translate', {'text': 'exit in flight', 'from': 'en', 'to': 'zh-Hans'})
        self.assertTrue(self.provider.started.wait(3))
        started = time.monotonic(); self.process.terminate()
        self.process.wait(timeout=1.5)
        metrics['shutdown_seconds'] = round(time.monotonic() - started, 3)

    def test_fault_clipboard_failure_returns_error(self):
        self.tool('wl-copy', 'raise SystemExit(1)')
        self.fail('copy', {'text': 'sample'})
        self.assertFalse((self.home / 'clipboard').exists())

    def test_fault_selection_invalid_utf8_and_size(self):
        self.tool('wl-paste', 'import sys; sys.stdout.buffer.write(b"\\xff")')
        self.fail('capture', {'action': 'selection'})
        self.tool('wl-paste', 'print("x" * 80001)')
        self.fail('capture', {'action': 'selection'})
        self.tool('wl-paste', 'print("   ")')
        self.fail('capture', {'action': 'selection'})

    def test_fault_ocr_fallback_and_invalid_output(self):
        self.tool('tesseract', 'import sys\nif "-l" in sys.argv: raise SystemExit(1)\nprint("fallback OCR")')
        self.assertEqual(self.call('capture', {'action': 'screenshot'})['text'], 'fallback OCR\n')
        for source in ['print(" ")', 'print("x" * 20001)', 'raise SystemExit(1)']:
            self.tool('tesseract', source)
            self.fail('capture', {'action': 'screenshot'})

    def test_capture_concurrency_recovers_after_cancellation(self):
        self.tool('slurp', 'import time; time.sleep(.2); raise SystemExit(1)')
        results = self.batch('capture', lambda _: {'action': 'screenshot'}, 2)
        self.assertEqual(sum(value['ok'] for value in results), 1)
        self.assertIn('already running', next(value['error'] for value in results if not value['ok']))
        self.assertTrue(self.call('capture', {'action': 'screenshot'})['cancelled'])

    def test_stress_30_annotation_replacements_remove_old_files(self):
        previous = None
        for _ in range(30):
            value = self.call('capture', {'action': 'annotate'})
            if previous:
                self.assertFalse(Path(previous['path']).exists())
                self.assertFalse(self.reply(self.send('annotation.copy', {'annotation_id': previous['annotation_id']}))['ok'])
            previous = value
        self.call('annotation.discard', {'annotation_id': previous['annotation_id']})
        self.assertFalse(Path(previous['path']).exists())

    def test_fault_annotation_invalid_png_can_be_discarded(self):
        value = self.call('capture', {'action': 'annotate'})
        Path(value['output_path']).write_text('invalid')
        self.fail('annotation.copy', {'annotation_id': value['annotation_id']})
        self.assertFalse((self.home / 'clipboard').exists())
        self.call('annotation.discard', {'annotation_id': value['annotation_id']})
        self.assertFalse(Path(value['path']).exists())

    def test_fault_corrupt_database_preserves_original(self):
        path = self.home / '.local/share/lucas-translate-omarchy/lucas.db'
        path.write_bytes(b'original corrupt database')
        self.fail('history.list', {})
        self.assertEqual(path.read_bytes(), b'original corrupt database')

    def test_fault_corrupt_config_preserves_original(self):
        path = self.home / '.local/share/lucas-translate-omarchy/config.json'
        path.write_text('{broken')
        result = self.reply(self.send('preferences.save', {'language': 'en'}))
        self.assertFalse(result['ok'])
        self.assertEqual(path.read_text(), '{broken')

    def test_speech_rejects_untrusted_and_non_https_urls(self):
        for url in ['file:///etc/passwd', 'http://dict.youdao.com/dictvoice', 'https://evil.test/dictvoice',
                    'https://dict.youdao.com@evil.test/dictvoice', 'https://dict.youdao.com/other']:
            self.fail('speak', {'url': url})

    def test_logs_do_not_contain_source_result_or_keys(self):
        sample = 'unique-private-source-793511'
        self.translated(sample)
        self.call('copy', {'text': sample})
        directory = Path(self.call('logs.directory'))
        time.sleep(.05)
        for path in directory.glob('*'):
            self.assertNotIn(sample, path.read_text())
        self.process.terminate(); _, stderr = self.process.communicate(timeout=3)
        self.assertNotIn(sample, stderr)


metrics = {}

if __name__ == '__main__':
    suite = unittest.TestSuite(Stability(name) for name in sorted(Stability.__dict__) if name.startswith('test_'))
    started = time.monotonic()
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    report = {'passed': result.wasSuccessful(), 'count': result.testsRun, 'backend':str(fixture.BINARY), 'duration_seconds': round(time.monotonic()-started,2),
              'failures': [case.id() for case, _ in result.failures + result.errors], 'metrics': metrics}
    reports = fixture.ROOT / 'dist/omarchy/test-results'; reports.mkdir(parents=True, exist_ok=True)
    (reports / 'stability.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if result.wasSuccessful() else 1)
