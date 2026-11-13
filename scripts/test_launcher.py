#!/usr/bin/env python3
"""Real HTTP/Git checks, with desktop/agent commands replaced by logging stubs."""
import json, os, signal, socket, subprocess, time, urllib.error, urllib.request
from pathlib import Path
import test_outbox

fixture = test_outbox.OutboxTests()
fixture.setUp()
server = None
try:
    root = Path(__file__).resolve().parents[1]
    log = fixture.d / 'desktop.jsonl'
    stub = '''#!/usr/bin/env python3
import json,sys,os
with open(os.environ['DESKTOP_LOG'],'a') as f:f.write(json.dumps(sys.argv)+'\\n')
'''
    for name in ('osascript', 'open', 'xdg-open', 'x-terminal-emulator', 'codex'):
        p = fixture.d / 'bin' / name
        p.write_text(stub); p.chmod(0o755)
    env = dict(fixture.env, DESKTOP_LOG=str(log), DISPLAY=':fake')
    test_outbox.run('git', '--git-dir', fixture.mirror, 'update-ref', 'refs/remotes/origin/main', fixture.sha)
    receipt = fixture.d / 'url'
    with socket.socket() as occupied:
        occupied.bind(('127.0.0.1', 0)); occupied.listen()
        port = occupied.getsockname()[1]
        with (fixture.d / 'http.log').open('w') as stream:
            server = subprocess.Popen([str(root/'target/debug/ogh'), 'serve', '--port', str(port),
                                       '--url-file', str(receipt), '--web-dir', str(root/'web/dist')],
                                      env=env, stdout=stream, stderr=stream, start_new_session=True)
        for _ in range(100):
            if receipt.exists(): break
            if server.poll() is not None: raise RuntimeError((fixture.d/'http.log').read_text())
            time.sleep(.05)
        url = receipt.read_text()
        assert url != f'http://127.0.0.1:{port}'
        def request(data=None, origin=None):
            headers={'Content-Type':'application/json'}
            if origin: headers['Origin']=origin
            req=urllib.request.Request(url+'/api/repos/test/repo/open', data=json.dumps(data).encode() if data is not None else None, headers=headers)
            with urllib.request.urlopen(req, timeout=15) as r: return json.load(r)
        context=request()
        assert not context['checkout_exists']
        for bad in ({'tool':'arbitrary'}, {'tool':'codex','path':'/tmp/foreign-repo'}):
            try: request(bad); raise AssertionError('Accepted invalid launch')
            except urllib.error.HTTPError as e: assert e.code==400
        try: request({'tool':'terminal'}, 'https://foreign.example'); raise AssertionError('Cross-origin launch')
        except urllib.error.HTTPError as e: assert e.code==403
        result=request({'tool':'codex','path':context['path']})
        path=Path(result['path']); assert (path/'README.md').read_text()=='base\n'
        assert test_outbox.run('git','-C',path,'branch','--show-current').stdout.startswith('offline/work-')
        assert request()['checkout_exists']
        for _ in range(100):
            calls=[json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            if any(url in call for call in calls) and any(any('OGH_REPO=' in arg for arg in call) for call in calls):break
            time.sleep(.05)
        else: raise AssertionError('Browser or terminal did not receive actual launch URL/context')
        # Paths with spaces and quotes must survive worktree discovery unchanged.
        another=fixture.d / "project ' with spaces"
        test_outbox.run('git','--git-dir',fixture.mirror,'worktree','add','--detach',another,fixture.sha)
        assert str(another.resolve()) in request()['workspaces']
        assert request({'tool':'terminal','path':str(another.resolve())})['path']==str(another.resolve())
        print('Launcher passed: occupied port, actual browser URL, safe worktree creation, path validation, quoted paths, cross-origin rejection; all desktop commands stubbed.')
finally:
    if server is not None:
        try: os.killpg(server.pid, signal.SIGTERM)
        except ProcessLookupError: pass
        server.wait(timeout=10)
    fixture.tearDown()
