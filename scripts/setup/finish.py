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
import uuid
import tempfile
from pathlib import Path


def install_helper(path, content):
    """Keep an existing file/link intact; atomically install a new regular file."""
    path = Path(path)
    if path.exists() or path.is_symlink():
        if path.is_dir() and not path.is_symlink():
            raise RuntimeError(f'Refusing helper directory: {path}')
        backup = path.with_name(path.name + '.ogh-backup-' + uuid.uuid4().hex)
        path.rename(backup)
        print(f'Previous helper retained at {backup}')
    fd, temporary = tempfile.mkstemp(prefix=path.name + '.', dir=path.parent)
    with os.fdopen(fd, 'w') as stream:
        stream.write(content)
    os.chmod(temporary, 0o755)
    os.replace(temporary, path)


def finish(workspace, root, port):
    workspace, root = Path(workspace), Path(root)
    port = int(port)
    if not 1 <= port <= 65535:
        raise ValueError('Invalid server port')
    data_dir = os.environ.get('OGH_DATA_DIR')
    if not data_dir:
        data_dir = json.loads(subprocess.check_output(['ogh', 'app', 'paths'], text=True, timeout=15))['data_dir']
    data_dir = str(Path(data_dir).expanduser().resolve())
    os.environ['OGH_DATA_DIR'] = data_dir
    launcher = Path.home() / '.local/bin/ogh-work'
    launcher.parent.mkdir(parents=True, exist_ok=True)
    content = ('#!/usr/bin/env bash\nset -e\nexport PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"\n'
               'export OGH_REPO=open-mercato/open-mercato\n'
               f'export OGH_DATA_DIR={shlex.quote(data_dir)}\n'
               f'cd {shlex.quote(str(workspace))}\nexec "${{SHELL:-/bin/bash}}" -i\n')
    install_helper(launcher, content)
    url = f'http://127.0.0.1:{port}/api/health'

    def healthy():
        try:
            with urllib.request.urlopen(url, timeout=2) as response:
                data = json.load(response)
            if data.get('status') == 'ok' and data.get('outgoing_queue') is True:
                if data.get('data_dir') != data_dir:
                    raise RuntimeError(f'Port {port} serves a different or unverifiable cache; choose OGH_SETUP_PORT or stop that server.')
                return True
            return False
        except (OSError, ValueError):
            return False

    if healthy():
        print(f'Reusing existing ogh server on {port}.')
        return
    with socket.socket() as sock:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
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
