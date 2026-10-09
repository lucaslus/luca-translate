#!/usr/bin/env python3
"""Opt-in selection smoke test using only a disposable browser document."""
import argparse
import http.server
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
TEXT = 'Lucas native live selection fixture'
state = {'selected': True, 'applied': None}


def unlocked():
    lock = json.loads(subprocess.check_output(['omarchy-shell', 'lock', 'status']))
    monitors = json.loads(subprocess.check_output(['hyprctl', '-j', 'monitors']))
    if any(lock.get(key) for key in ['locked', 'sessionLocked', 'pending', 'requested']) or any(
            'LOCK' in monitor.get('solitaryBlockedBy', []) for monitor in monitors):
        raise RuntimeError('Desktop locked; no desktop interaction permitted')


def active():
    return json.loads(subprocess.check_output(['hyprctl', '-j', 'activewindow']))


class Document(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        if self.path == '/state':
            data = json.dumps({'selected': state['selected']}).encode()
            mime = 'application/json'
        elif self.path.startswith('/applied?'):
            state['applied'] = self.path.endswith('true')
            data = b'ok'
            mime = 'text/plain'
        else:
            data = ('''<!doctype html><title>Lucas selection fixture</title>
                <p id="text">''' + TEXT + '''</p><script>
                let previous;
                setInterval(async () => {
                    const {selected} = await (await fetch('/state')).json();
                    if (previous === selected) return;
                    const selection = getSelection(); selection.removeAllRanges();
                    if (selected) { const range = document.createRange();
                        range.selectNodeContents(document.querySelector('#text')); selection.addRange(range); }
                    previous = selected;
                    await fetch('/applied?' + selected);
                }, 100);
                </script>''').encode()
            mime = 'text/html'
        self.send_response(200)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-desktop-test', action='store_true', required=True)
    parser.add_argument('--backend', type=Path, default=ROOT / 'crates/lucas-omarchy/target/debug/lucas-translate-omarchy-backend')
    args = parser.parse_args()
    assert args.run_desktop_test
    unlocked()
    previous = active()
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Document)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    browser = None
    source = None
    try:
        with tempfile.TemporaryDirectory(prefix='lucas-selection-desktop-') as scratch:
            unlocked()
            subprocess.run(['omarchy-shell', 'shell', 'hide', 'lucas.translate'], check=True, stdout=subprocess.DEVNULL)
            browser = subprocess.Popen(['chromium', '--ozone-platform=wayland', '--no-first-run',
                '--no-default-browser-check', '--disable-extensions', '--disable-background-networking',
                '--disable-component-update', '--disable-sync', '--no-proxy-server',
                '--user-data-dir=' + scratch, '--app=http://127.0.0.1:' + str(server.server_port)],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                current = active()
                if current.get('pid') == browser.pid and state['applied'] is True:
                    source = {key: current[key] for key in ['address', 'pid', 'class']}
                    break
                time.sleep(.1)
            if source is None:
                raise RuntimeError('Disposable browser did not become focused; no keys sent')
            binary = args.backend.resolve()
            environment = dict(os.environ, DBUS_SESSION_BUS_ADDRESS='unix:path=/missing-selection-fixture-bus')
            for selected in [True, True, False, True, False]:
                state['selected'] = selected
                deadline = time.monotonic() + 3
                while state['applied'] is not selected and time.monotonic() < deadline:
                    time.sleep(.05)
                assert state['applied'] is selected, 'Fixture did not update selection'
                unlocked()
                assert active().get('address') == source['address'], 'Focus changed; no keys sent'
                result = subprocess.run([str(binary), '--read-selection', json.dumps(source)],
                    capture_output=True, text=True, env=environment, timeout=8, check=True)
                response = json.loads(result.stdout)
                assert not response.get('error'), response.get('error')
                assert response.get('text') == (TEXT if selected else ''), 'Incorrect live selection result'
            print(json.dumps({'passed': True, 'cases': 5,
                'backend': str(binary),
                'scope': 'Disposable Chromium document; real Hyprland Copy and Wayland clipboard',
                'selected_then_deselected': True, 'same_text_repeated': True}))
    finally:
        server.shutdown()
        server.server_close()
        if browser is not None:
            try:
                os.killpg(browser.pid, signal.SIGTERM)
                browser.wait(timeout=5)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                try:
                    os.killpg(browser.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            # Restore focus only if closing our fixture left no active window.
            if previous.get('address') and not active().get('address'):
                unlocked()
                subprocess.run(['hyprctl', 'dispatch',
                    'hl.dsp.window.focus({window="address:' + previous['address'] + '"})'],
                    check=True, stdout=subprocess.DEVNULL)


if __name__ == '__main__':
    main()
