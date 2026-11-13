#!/usr/bin/env python3
"""Install reentry helpers and start only a verified local ogh server."""
import json
import os
import shlex
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
    manifest = workspace / '.ogh-workspace.json'
    repository = json.loads(manifest.read_text())['repository'] if manifest.exists() else 'open-mercato/open-mercato'
    launcher = Path.home() / '.local/bin/ogh-work'
    launcher.parent.mkdir(parents=True, exist_ok=True)
    content = ('#!/usr/bin/env bash\nset -e\nexport PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"\n'
               f'export OGH_REPO={shlex.quote(repository)}\n'
               f'export OGH_DATA_DIR={shlex.quote(data_dir)}\n'
               f'cd {shlex.quote(str(workspace))}\nexec "${{SHELL:-/bin/bash}}" -i\n')
    install_helper(launcher, content)
    url = f'http://127.0.0.1:{port}'

    def healthy():
        try:
            with urllib.request.urlopen(url + '/api/health', timeout=.5) as response:
                data = json.load(response)
            return (data.get('status') == 'ok' and data.get('outgoing_queue') is True
                    and data.get('data_dir') == data_dir)
        except (OSError, ValueError):
            return False

    root.mkdir(parents=True, exist_ok=True)
    receipt = root / 'server.url'
    if healthy():
        receipt.write_text(url)
        print(f'Reusing existing ogh server at {url}.')
        return
    receipt.unlink(missing_ok=True)
    with (root / 'server.log').open('ab') as log:
        process = subprocess.Popen(['ogh', 'serve', '--port', str(port), '--no-browser', '--url-file', str(receipt)], cwd=workspace,
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                   start_new_session=True)
    (root / 'server.pid').write_text(str(process.pid) + '\n')
    for _ in range(40):
        if receipt.exists():
            url = receipt.read_text().strip()
        if receipt.exists() and healthy():
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
