#!/usr/bin/env python3
"""Install reentry helpers and start only a verified local ogh server."""
import json
import os
import shlex
import socket
import subprocess
import sys
import time
import urllib.request
from pathlib import Path


def finish(workspace, root, port):
    workspace, root = Path(workspace), Path(root)
    port = int(port)
    if not 1 <= port <= 65535:
        raise ValueError('Invalid server port')
    launcher = Path.home() / '.local/bin/ogh-work'
    launcher.parent.mkdir(parents=True, exist_ok=True)
    launcher.write_text('#!/usr/bin/env bash\nset -e\nexport PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"\n'
                        'export OGH_REPO=open-mercato/open-mercato\n'
                        f'cd {shlex.quote(str(workspace))}\nexec "${{SHELL:-/bin/bash}}" -i\n')
    launcher.chmod(0o755)
    url = f'http://127.0.0.1:{port}/api/health'

    def healthy():
        try:
            with urllib.request.urlopen(url, timeout=2) as response:
                data = json.load(response)
            return data.get('status') == 'ok' and data.get('outgoing_queue') is True
        except (OSError, ValueError):
            return False

    if healthy():
        print(f'Reusing existing ogh server on {port}.')
        return
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', port))  # Refuse to replace an unrelated listener.
    root.mkdir(parents=True, exist_ok=True)
    with (root / 'server.log').open('ab') as log:
        process = subprocess.Popen(['ogh', 'serve', '--port', str(port)], cwd=workspace,
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                   start_new_session=True)
    (root / 'server.pid').write_text(str(process.pid) + '\n')
    for _ in range(40):
        if healthy():
            print(f'Started ogh server (PID {process.pid}, log {root / "server.log"}).')
            return
        if process.poll() is not None:
            break
        time.sleep(.25)
    if process.poll() is None:
        process.terminate()
        process.wait(timeout=5)
    raise RuntimeError(f'Server did not start; inspect {root / "server.log"}')


if __name__ == '__main__':
    finish(*sys.argv[1:])
