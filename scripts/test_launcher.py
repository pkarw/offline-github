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
        server_cwd=fixture.d/'disposable-server-cwd';server_cwd.mkdir()
        with (fixture.d / 'http.log').open('w') as stream:
            server = subprocess.Popen([str(root/'target/debug/ogh'), 'serve', '--port', str(port),
                                       '--url-file', str(receipt), '--web-dir', str(root/'web/dist')],
                                      env=env, cwd=server_cwd, stdout=stream, stderr=stream, start_new_session=True)
        for _ in range(100):
            if receipt.exists(): break
            if server.poll() is not None: raise RuntimeError((fixture.d/'http.log').read_text())
            time.sleep(.05)
        url = receipt.read_text()
        server_cwd.rmdir()  # Reproduce removal of the server's inherited checkout.
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
        def http(path, data=None):
            req=urllib.request.Request(url+path, data=json.dumps(data).encode() if data is not None else None, headers={'Content-Type':'application/json'})
            with urllib.request.urlopen(req, timeout=15) as response:return json.load(response)
        # Cached attachments are served locally with verified MIME, never fetched by GET.
        import hashlib, urllib.parse
        source='https://github.com/user-attachments/assets/offline-fixture'
        media=fixture.d/'cache/media'
        media.mkdir()
        png=b'\x89PNG\r\n\x1a\nfixture'
        (media/hashlib.sha256(source.encode()).hexdigest()).write_bytes(png)
        with urllib.request.urlopen(url+'/api/media?'+urllib.parse.urlencode({'url':source}),timeout=5) as image:
            assert image.read()==png and image.headers['Content-Type']=='image/png'
        for source in ('https://example.invalid/missing.png','../../snapshot.json'):
            try:
                urllib.request.urlopen(url+'/api/media?'+urllib.parse.urlencode({'url':source}),timeout=5)
                raise AssertionError('Missing attachment unexpectedly available')
            except urllib.error.HTTPError as error: assert error.code==404
        snapshot=fixture.d/'cache/test/repo/snapshot.json'
        data=json.loads(snapshot.read_text())
        data['items']['1']['labels']=[{'name':'bug','color':'ff0000'}]
        pr=dict(data['items']['1'],number=2,pull_request={},labels=[{'name':'enhancement','color':'00ff00'}])
        data['items']['2']=pr
        snapshot.write_text(json.dumps(data))
        assert http('/api/repos/test/repo/labels')==['bug','enhancement']
        assert len(http('/api/repos/test/repo/items?kind=issue&label=bug'))==1
        assert http('/api/repos/test/repo/items?kind=issue&label=enhancement')==[]
        assert len(http('/api/repos/test/repo/items?kind=pr&label=enhancement'))==1
        fixture.queue('comment','1',{'body':'Keep this offline work'})
        (path/'uncommitted.txt').write_text('keep me')
        preview=http('/api/repos/test/repo/remove')
        assert preview['outgoing_count']==1 and any('Uncommitted' in w for w in preview['warnings'])
        try:http('/api/repos/test/repo/remove',{'confirm':False});raise AssertionError('Missing confirmation accepted')
        except urllib.error.HTTPError as e:assert e.code==400
        http('/api/repos/test/repo/remove',{'confirm':True})
        assert http('/api/repos')==[]
        assert (path/'uncommitted.txt').read_text()=='keep me' and snapshot.exists()
        assert len(http('/api/outbox?repo=test/repo')['events'])==1
        http('/api/repos',{'full_name':'test/repo'})
        assert len(http('/api/repos'))==1
        assert not snapshot.with_name('removed.json').exists()
        # A second cached project must work even after the launch cwd disappeared.
        http('/api/repos',{'full_name':'test/second'})
        second=fixture.d/'cache/test/second/repo.git'
        test_outbox.run('git','clone','--bare',fixture.mirror,second)
        assert len(http('/api/repos'))==2
        assert isinstance(http('/api/repos/test/second/open')['workspaces'],list)
        assert request()['checkout_exists']
        print('Two-project launcher works after deletion of the server working directory.')
        print('Labels and safe removal passed: issue/PR filtering, pending/dirty warnings, confirmation, retained files/outbox, add restores.')
        print('Launcher passed: occupied port, actual browser URL, safe worktree creation, path validation, quoted paths, cross-origin rejection; all desktop commands stubbed.')
finally:
    if server is not None:
        try: os.killpg(server.pid, signal.SIGTERM)
        except ProcessLookupError: pass
        server.wait(timeout=10)
    fixture.tearDown()
