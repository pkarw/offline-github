#!/usr/bin/env python3
"""Offline regression checks. Requires `cargo build`; no GitHub access needed."""
import json, os, pathlib, subprocess, tempfile, unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
BIN = ROOT / 'target/debug/ogh'

def run(*args, env=None, ok=True):
    p = subprocess.run([str(a) for a in args], text=True, capture_output=True, timeout=45, env=env)
    if ok and p.returncode:
        raise AssertionError(p.stderr)
    return p

class SyncTests(unittest.TestCase):
    def test_delta_full_and_failure_preservation(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = pathlib.Path(tmp)
            env = dict(os.environ, OGH_DATA_DIR=str(d/'cache'))
            run(BIN, 'repo', 'add', 'test/repo', env=env)
            source = d/'source'
            run('git', 'init', '-b', 'main', source)
            run('git', '-C', source, 'config', 'user.email', 'test@example.org')
            run('git', '-C', source, 'config', 'user.name', 'Test')
            (source/'README.md').write_text('hello\n')
            run('git', '-C', source, 'add', '.')
            run('git', '-C', source, 'commit', '-m', 'first')
            run('git', 'clone', '--bare', source, d/'cache/test/repo/repo.git')
            fake = d/'bin'
            fake.mkdir()
            gh = fake/'gh'
            gh.write_text('''#!/usr/bin/env python3
import json, os, sys
endpoint=sys.argv[-1]
assert sys.argv[1]=='api' and sys.argv[sys.argv.index('--method')+1]=='GET', 'Upstream writes forbidden'
with open(os.environ['REQUEST_LOG'],'a') as f:f.write(endpoint+'\\n')
if os.environ.get('FAIL_SYNC'):sys.exit(1)
data=json.load(open(os.environ['FIXTURE']))
key=endpoint.split('?')[0]
if key not in data: print('unexpected endpoint: '+key,file=sys.stderr);sys.exit(2)
value=data[key]
print(json.dumps([value] if '--slurp' in sys.argv else value))
''')
            gh.chmod(0o755)
            fixture=d/'fixture.json'
            env.update(PATH=str(fake)+os.pathsep+env['PATH'], FIXTURE=str(fixture), REQUEST_LOG=str(d/'requests'))
            item={'number':1,'title':'Initial','body':'Cached','state':'open','updated_at':'2099-01-01T00:00:00Z','comments':1,'user':{'login':'test'}}
            comment={'id':99,'issue_url':'https://api.github.com/repos/test/repo/issues/1','body':'before','user':{'login':'test'}}
            data={'repos/test/repo':{'description':'Test','default_branch':'main'},'repos/test/repo/issues':[item], 'repos/test/repo/pulls':[], 'repos/test/repo/issues/comments':[comment], 'repos/test/repo/pulls/comments':[]}
            fixture.write_text(json.dumps(data))
            run(BIN,'sync','-R','test/repo',env=env)
            snapshot=d/'cache/test/repo/snapshot.json'
            first=snapshot.read_bytes()
            self.assertIsNotNone(json.loads(first)['last_sync'])
            self.assertEqual(json.loads(first)['details']['1']['comments'][0]['body'],'before')
            run(BIN,'sync','-R','test/repo',env=dict(env,FAIL_SYNC='1'),ok=False)
            self.assertEqual(snapshot.read_bytes(),first,'failed sync must preserve cache and watermark')
            item['state']='closed'
            comment['body']='edited'
            fixture.write_text(json.dumps(data))
            run(BIN,'sync','-R','test/repo',env=env)
            updated=json.loads(snapshot.read_bytes())
            self.assertEqual(updated['items']['1']['state'],'closed')
            self.assertEqual(len(updated['details']['1']['comments']),1)
            self.assertEqual(updated['details']['1']['comments'][0]['body'],'edited')
            self.assertIn('&since=',(d/'requests').read_text())
            data['repos/test/repo/issues']=[]
            data['repos/test/repo/issues/comments']=[]
            fixture.write_text(json.dumps(data))
            run(BIN,'sync','-R','test/repo','--full',env=env)
            self.assertEqual(json.loads(snapshot.read_bytes())['items'],{})
            self.assertEqual(run('git','--git-dir',d/'cache/test/repo/repo.git','remote','get-url','--push','origin').stdout.strip(),'disabled://read-only')
            # A sync must not overwrite or refuse a user branch checked out in a worktree.
            mirror=d/'cache/test/repo/repo.git'
            work=d/'working'
            run('git','--git-dir',mirror,'worktree','add',work,'main')
            run('git','-C',work,'config','user.name','Test')
            run('git','-C',work,'config','user.email','test@example.org')
            (work/'local.txt').write_text('local work')
            run('git','-C',work,'add','.')
            run('git','-C',work,'commit','-m','local work')
            local_sha=run('git','-C',work,'rev-parse','HEAD').stdout
            run(BIN,'sync','-R','test/repo',env=env)
            self.assertEqual(run('git','-C',work,'rev-parse','HEAD').stdout,local_sha)
            # An intervening normal sync must invalidate cached review summaries.
            item['pull_request']={}
            data['repos/test/repo/issues']=[item]
            data['repos/test/repo/pulls/1/reviews']=[{'id':10,'body':'first review'}]
            fixture.write_text(json.dumps(data))
            run(BIN,'sync','-R','test/repo','--reviews',env=env)
            self.assertTrue(json.loads(snapshot.read_bytes())['details']['1']['reviews_cached'])
            item['updated_at']='2099-02-01T00:00:00Z'
            fixture.write_text(json.dumps(data))
            run(BIN,'sync','-R','test/repo',env=env)
            saved=json.loads(snapshot.read_bytes())
            self.assertFalse(saved['details']['1']['reviews_cached'])
            saved['last_sync']='2100-01-01T00:00:00Z'
            snapshot.write_text(json.dumps(saved))
            data['repos/test/repo/pulls/1/reviews'][0]['body']='updated review'
            fixture.write_text(json.dumps(data))
            run(BIN,'sync','-R','test/repo','--reviews',env=env)
            self.assertEqual(json.loads(snapshot.read_bytes())['details']['1']['reviews'][0]['body'],'updated review')


if __name__ == '__main__': unittest.main(verbosity=2)
