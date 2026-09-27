import base64
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time


BINARY = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parents[1] / 'target/debug/herdr-infobox').resolve()


class Pane:
    def __init__(self, state, session, env):
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 12, 30, 0, 0))
        self.child = subprocess.Popen([str(BINARY), '--state-dir', str(state), 'ui', '--session', session], stdin=slave, stdout=slave, stderr=slave, env=env)
        os.close(slave)
        self.capture = bytearray()

    def send(self, keys):
        os.write(self.master, keys)

    def until(self, condition):
        deadline = time.monotonic() + 10
        while not condition():
            if time.monotonic() >= deadline or self.child.poll() is not None:
                raise AssertionError(f'TUI condition not reached; status={self.child.poll()}; output={bytes(self.capture)!r}')
            if select.select([self.master], [], [], 0.02)[0]:
                try:
                    self.capture.extend(os.read(self.master, 65536))
                except OSError:
                    pass

    def close(self):
        try:
            if self.child.poll() is None:
                self.send(b'q')
                self.child.wait(timeout=10)
            assert self.child.returncode == 0, self.child.returncode
        finally:
            if self.child.poll() is None:
                self.child.kill()
                self.child.wait()
            os.close(self.master)


with tempfile.TemporaryDirectory(prefix='infobox-ui-runtime-') as tmp:
    root = Path(tmp)
    state = root / 'state'
    plan = root / 'plan.md'
    plan.write_text('# Expected plan\nDo the work.\n')
    calls = root / 'opened'
    env = {k: v for k, v in os.environ.items() if not k.startswith(('HERDR_', 'INFOBOX_'))}
    env.update(PATH=str(root) + ':' + env.get('PATH', ''), TERM='xterm-256color', INFOBOX_TEST_OPENED=str(calls))
    for name in ['open', 'xdg-open']:
        executable = root / name
        executable.write_text('#!/bin/sh\nprintf "%s\\n" "$1" >> "$INFOBOX_TEST_OPENED"\n')
        executable.chmod(0o700)

    def cli(*args):
        return subprocess.run([str(BINARY), '--state-dir', str(state), *args], env=env, check=True, capture_output=True).stdout.decode().strip()

    session = cli('session', 'add', '--provider', 'claude', '--native-id', 'ui-proof')
    cli('ref', 'add', '--session', session, '--url', 'https://unrelated.example.test')
    cli('plan', 'attach', '--session', session, '--file', str(plan))
    pane = Pane(state, session, env)
    try:
        pane.until(lambda: b'ui-proof' in pane.capture)
        pane.send(b'\t\ty')
        pane.until(lambda: re.search(rb'\x1b\]52;c;([^\x07]+)\x07', pane.capture))
        payload = re.findall(rb'\x1b\]52;c;([^\x07]+)\x07', pane.capture)
        assert base64.b64decode(payload[-1]).decode() == plan.read_text()
        pane.send(b'o')
        pane.until(lambda: calls.exists() and calls.read_text().strip() == str(plan.resolve()))
    finally:
        pane.close()
    print('PASS: narrow TUI copies selected Plan text and opens its canonical source')

    fake = root / 'herdr'
    fake.write_text('#!/bin/sh\nprintf \'%s\\n\' \'{"result":{"snapshot":{"panes":[],"focused_tab_id":"same-tab","focused_pane_id":null}}}\'\n')
    fake.chmod(0o700)
    for index in [1, 2]:
        socket = root / f'socket{index}'
        socket.touch()
        pane_env = env | {'HERDR_BIN_PATH': str(fake), 'HERDR_SOCKET_PATH': str(socket), 'HERDR_TAB_ID': 'same-tab'}
        pane = Pane(state, session, pane_env)
        try:
            pane.until(lambda: b'ui-proof' in pane.capture)
            pane.send(b'p')
            pane.until(lambda: sum(json.loads(path.read_text())['pinned'] for path in state.glob('ui-*.json')) == index)
        finally:
            pane.close()
    assert len(list(state.glob('ui-*.json'))) == 2
    print('PASS: distinct mocked Herdr instances keep independent pinned selections')
