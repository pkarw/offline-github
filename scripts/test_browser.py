#!/usr/bin/env python3
"""Real browser + Rust HTTP server + local fake GitHub. No production writes."""
import json,os,pathlib,signal,socket,subprocess,sys,time,urllib.request
import test_outbox

root=pathlib.Path(__file__).resolve().parents[1]
fixture=test_outbox.OutboxTests();fixture.setUp();server=None
try:
 with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 url=f'http://127.0.0.1:{port}'
 log=(fixture.d/'server.log').open('w')
 server=subprocess.Popen([str(root/'target/debug/ogh'),'serve','--port',str(port),'--web-dir',str(root/'web/dist')],env=fixture.env,stdout=log,stderr=log,start_new_session=True)
 for attempt in range(80):
  try:
   with urllib.request.urlopen(url+'/api/health',timeout=1) as r:assert r.status==200
   break
  except Exception:
   if server.poll() is not None:raise RuntimeError((fixture.d/'server.log').read_text())
   time.sleep(.1)
 else:raise RuntimeError('Fixture server never became healthy')
 env=dict(fixture.env,OGH_TEST_URL=url,OGH_TEST_DIR=str(fixture.d),OGH_TEST_ARTIFACTS=os.environ.get('OGH_TEST_ARTIFACTS',str(fixture.d/'browser')))
 result=subprocess.run(['node',str(root/'scripts/browser-smoke.cjs')],env=env,timeout=120)
 if result.returncode:raise RuntimeError('Browser smoke failed')
 requests=[json.loads(s) for s in (fixture.d/'requests').read_text().splitlines()]
 assert all(r['path']=='user' or r['path'].startswith('repos/test/repo') for r in requests)
 assert all(r['method']!='PATCH' for r in requests),'Unsafe field patch was sent'
 print('HTTP/browser integration passed; all GitHub calls were fake test/repo operations.')
finally:
 if server is not None:
  try:os.killpg(server.pid,signal.SIGTERM)
  except ProcessLookupError:pass
  server.wait(timeout=10)
 fixture.tearDown()
