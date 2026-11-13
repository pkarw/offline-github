#!/usr/bin/env python3
"""Local fake-GitHub acceptance tests. Never contacts or mutates real GitHub."""
import concurrent.futures, json, os, pathlib, sqlite3, subprocess, tempfile, unittest
ROOT=pathlib.Path(__file__).resolve().parents[1]
BIN=ROOT/'target/debug/ogh'

def run(*args, env=None, ok=True):
    p=subprocess.run([str(a) for a in args],env=env,text=True,capture_output=True,timeout=60)
    if ok and p.returncode: raise AssertionError(p.stderr)
    return p

FAKE_GH=r'''#!/usr/bin/env python3
import json,os,sys,urllib.parse
from pathlib import Path
args=sys.argv[1:]
state=Path(os.environ['MOCK_STATE']);data=json.loads(state.read_text())
if args[0]!='api': print('Unexpected gh command',file=sys.stderr);sys.exit(5)
method=args[args.index('--method')+1]
path=next(a for a in args if a=='user' or a.startswith('repos/'))
body=json.loads(Path(args[args.index('--input')+1]).read_text()) if '--input' in args else None
with open(os.environ['MOCK_LOG'],'a') as f:f.write(json.dumps({'method':method,'path':path,'body':body})+'\n')
p=urllib.parse.urlparse(path);c=urllib.parse.unquote(p.path).split('/');query=urllib.parse.parse_qs(p.query)
result=None
if os.environ.get('DROP_BEFORE_WRITE') and method=='POST' and not data.get('dropped_before'):
 data['dropped_before']=True;state.write_text(json.dumps(data));print('simulated ambiguous failure before remote write',file=sys.stderr);sys.exit(1)

def missing():
 print('gh: Not Found (HTTP 404)',file=sys.stderr);sys.exit(1)
def comment_rows(n):return [x for x in data['comments'] if x['issue_url'].endswith('/'+str(n))]
def save():state.write_text(json.dumps(data))
if path=='user':result={'login':'tester'}
elif c==['repos','test','repo']:result={'full_name':'test/repo','default_branch':'main','description':'Mock repository','private':False}
elif c[:3]!=['repos','test','repo']:print('Blocked non-test repo',file=sys.stderr);sys.exit(9)
elif c[3:] == ['issues']:
 if method=='POST':
  n=max(map(int,data['items']))+1
  result=dict(body,number=n,state='open',user={'login':'tester'},created_at='2026-11-13T00:00:00Z',updated_at='2026-11-13T00:00:00Z',comments=0,html_url='https://github.com/test/repo/issues/'+str(n))
  data['items'][str(n)]=result
 else:result=list(data['items'].values())
elif c[3:] == ['issues','comments']: result=data['comments']
elif c[3:] == ['pulls']:
 if method=='POST':
  n=max(map(int,data['items']))+1
  result=dict(body,number=n,state='open',user={'login':'tester'},pull_request={},html_url='https://github.com/test/repo/pull/'+str(n))
  data['items'][str(n)]=result
 else:result=[v for v in data['items'].values() if 'pull_request' in v]
elif c[3:]==['pulls','comments']:result=[]
elif len(c)==5 and c[3]=='issues':
 item=data['items'].get(c[4]);
 if item is None:missing()
 if method=='PATCH':item.update(body);data['items'][c[4]]=item
 result=dict(item)
 if method=='GET' and os.environ.get('EDIT_RACE'):
  data['items'][c[4]]['title']='Upstream edit raced after GET'
elif len(c)==6 and c[3]=='issues' and c[5]=='comments':
 if method=='POST':
  result={'id':len(data['comments'])+100,'body':body['body'],'user':{'login':'tester'},'created_at':'2026-11-13T00:00:00Z','updated_at':'2026-11-13T00:00:00Z','issue_url':'https://api.github.com/repos/test/repo/issues/'+c[4],'html_url':'https://github.com/test/repo/issues/'+c[4]+'#comment'}
  data['comments'].append(result)
 else: result=comment_rows(c[4])
elif len(c)>=6 and c[3]=='issues' and c[5]=='labels':
 item=data['items'][c[4]];labels=item.setdefault('labels',[])
 if method=='POST':
  for name in body['labels']:
   if not any(x['name']==name for x in labels):labels.append({'name':name,'color':'ededed'})
 if method=='DELETE':item['labels']=[x for x in labels if x['name']!=c[6]]
 result=item['labels']
elif c[3:]==['labels']:
 if method=='POST':data['labels'][body['name']]=dict(body,id=len(data['labels'])+1);result=data['labels'][body['name']]
 else:result=list(data['labels'].values())
elif len(c)==5 and c[3]=='labels':
 if c[4] not in data['labels']:missing()
 result=data['labels'][c[4]]
elif c[3:6]==['git','ref','heads']:
 branch='/'.join(c[6:]);sha=data['branches'].get(branch)
 if sha is None:missing()
 result={'ref':'refs/heads/'+branch,'object':{'sha':sha,'type':'commit'}}
else:print('Unexpected API path '+path,file=sys.stderr);sys.exit(5)
save()
if os.environ.get('DROP_AFTER_WRITE') and method=='POST' and c[-1]=='comments' and not data.get('dropped'):
 data['dropped']=True;save();print('simulated connection drop AFTER remote accepted',file=sys.stderr);sys.exit(1)
print(json.dumps([result] if '--slurp' in args else result))
'''
FAKE_GIT=r'''#!/usr/bin/env python3
import json,os,subprocess,sys
from pathlib import Path
args=sys.argv[1:]
if 'push' in args:
 assert '--force' not in args and not any(a.startswith('+') for a in args)
 idx=args.index('push')+1
 assert args[idx]=='https://github.com/test/repo.git','Blocked non-test Git push'
 args[idx]=os.environ['MOCK_REMOTE']
 p=subprocess.run(['/usr/bin/git']+args)
 if p.returncode==0:
  sha,ref=args[-1].split(':');state=Path(os.environ['MOCK_STATE']);data=json.loads(state.read_text());data['branches'][ref.removeprefix('refs/heads/')]=sha;state.write_text(json.dumps(data))
 sys.exit(p.returncode)
assert not any('https://github.com' in a for a in args),'Blocked unintended real network'
os.execv('/usr/bin/git',['git']+args)
'''

class OutboxTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.d=pathlib.Path(self.temp.name)
  self.env=dict(os.environ,OGH_DATA_DIR=str(self.d/'cache'),MOCK_STATE=str(self.d/'remote.json'),MOCK_LOG=str(self.d/'requests'),MOCK_REMOTE=str(self.d/'origin.git'))
  self.cmd('repo','add','test/repo')
  self.source=self.d/'source';run('git','init','-b','main',self.source)
  run('git','-C',self.source,'config','user.name','Test');run('git','-C',self.source,'config','user.email','test@example.org')
  (self.source/'README.md').write_text('base\n');run('git','-C',self.source,'add','.');run('git','-C',self.source,'commit','-m','initial')
  run('git','clone','--bare',self.source,self.d/'origin.git')
  self.mirror=self.d/'cache/test/repo/repo.git';run('git','clone','--bare',self.d/'origin.git',self.mirror)
  self.sha=run('git','-C',self.source,'rev-parse','HEAD').stdout.strip()
  item={'number':1,'title':'Initial','body':'Original body','state':'open','user':{'login':'tester'},'labels':[],'comments':0,'updated_at':'2026-11-13T00:00:00Z','created_at':'2026-11-13T00:00:00Z'}
  self.state={'items':{'1':item},'comments':[],'labels':{},'branches':{'main':self.sha}}
  self.write_state()
  snapshot=self.d/'cache/test/repo/snapshot.json';value=json.loads(snapshot.read_text());value.update(last_sync='2026-11-13T00:00:00Z',items={'1':item},details={'1':{'item':item,'comments':[],'reviews':[],'review_comments':[],'files':[],'diff':''}});snapshot.write_text(json.dumps(value))
  fake=self.d/'bin';fake.mkdir()
  for name,text in [('gh',FAKE_GH),('git',FAKE_GIT)]:p=fake/name;p.write_text(text);p.chmod(0o755)
  self.env['PATH']=str(fake)+os.pathsep+self.env['PATH']
 def tearDown(self):self.temp.cleanup()
 def cmd(self,*args,ok=True,env=None):return run(BIN,*args,env=env or self.env,ok=ok)
 def j(self,*args):return json.loads(self.cmd(*args).stdout)
 def write_state(self):(self.d/'remote.json').write_text(json.dumps(self.state))
 def load_state(self):self.state=json.loads((self.d/'remote.json').read_text());return self.state
 def queue(self,kind,target=None,payload=None):
  args=['outbox','enqueue','-R','test/repo','--kind',kind,'--payload',json.dumps(payload or {})]
  if target:args+=['--target',target]
  return self.j(*args)
 def sync(self,env=None):
  result=self.cmd('outbox','sync','--yes','-R','test/repo',env=env,ok=False)
  if not result.stdout:raise AssertionError(result.stderr)
  return json.loads(result.stdout)
 def events(self):return self.j('outbox','report')['events']
 def test_queue_survives_restart_and_preview_is_no_network(self):
  e=self.queue('comment','1',{'body':'Offline\n``` text'})
  self.assertEqual(self.events()[0]['payload']['body'],'Offline\n``` text')
  self.assertEqual(len(self.events()[0]['history']),1)
  self.j('outbox','sync','--dry-run','-R','test/repo')
  self.assertFalse((self.d/'requests').exists())
  self.assertIn('Offline',self.cmd('outbox','report','--format','md').stdout)
  self.assertNotEqual(self.cmd('outbox','sync',ok=False).returncode,0)
  token=self.j('outbox','sync','--dry-run','-R','test/repo')['plan_token']
  self.queue('comment','1',{'body':'Another offline intention'})
  p=self.cmd('outbox','sync','--yes','-R','test/repo','--plan-token',token,ok=False)
  self.assertNotEqual(p.returncode,0);self.assertIn('preview',p.stderr)
  self.assertFalse((self.d/'requests').exists())
 def test_connection_drop_reconciles_without_duplicate(self):
  self.queue('comment','1',{'body':'A durable comment'})
  result=self.sync(env=dict(self.env,DROP_AFTER_WRITE='1'))
  self.assertEqual(result['counts']['uncertain'],1)
  self.assertEqual(len(self.load_state()['comments']),1)
  result=self.sync();self.assertEqual(result['counts']['applied'],1)
  self.assertEqual(len(self.load_state()['comments']),1)
  posts=[json.loads(x) for x in (self.d/'requests').read_text().splitlines() if json.loads(x)['method']=='POST']
  self.assertEqual(len(posts),1)
  self.assertTrue(any(h['action']=='send_uncertain' for h in self.events()[0]['history']))
 def test_uncertain_absent_never_blindly_resends(self):
  e=self.queue('comment','1',{'body':'Conservative recovery'})
  self.assertEqual(self.sync(env=dict(self.env,DROP_BEFORE_WRITE='1'))['counts']['uncertain'],1)
  self.assertEqual(len(self.load_state()['comments']),0)
  self.assertEqual(self.sync()['counts']['uncertain'],1)
  posts=[json.loads(x) for x in (self.d/'requests').read_text().splitlines() if json.loads(x)['method']=='POST']
  self.assertEqual(len(posts),1,'uncertain absence must not auto-resend')
  self.j('outbox','resolve',e['id'],'--action','retry')
  self.assertEqual(self.sync()['counts']['applied'],1)
  self.assertEqual(len(self.load_state()['comments']),1)
 def test_uncertain_marker_removed_does_not_duplicate(self):
  self.queue('comment','1',{'body':'Original comment'})
  self.sync(env=dict(self.env,DROP_AFTER_WRITE='1'))
  self.state=self.load_state();self.state['comments'][0]['body']='Edited remotely; marker removed';self.write_state()
  self.assertEqual(self.sync()['counts']['uncertain'],1)
  self.assertEqual(len(self.load_state()['comments']),1)
 def test_conflict_preserves_all_versions_then_explicit_resolution(self):
  e=self.queue('edit','1',{'fields':{'title':'Local title'}})
  self.state['items']['1']['title']='Remote title';self.write_state()
  result=self.sync();self.assertEqual(result['counts']['conflict'],1)
  saved=self.events()[0];self.assertEqual(saved['base']['title'],'Initial');self.assertEqual(saved['payload']['fields']['title'],'Local title');self.assertEqual(saved['observed']['title'],'Remote title')
  self.j('outbox','resolve',e['id'],'--action','use_local')
  self.assertEqual(self.sync()['counts']['conflict'],1)
  self.assertEqual(self.load_state()['items']['1']['title'],'Remote title','unsafe issue PATCH must not overwrite fields')
  self.assertIn('Offline edit proposal',self.state['comments'][0]['body'])
  self.state['items']['1']['title']='Local title';self.write_state() # user applied the proposal on GitHub
  self.j('outbox','resolve',e['id'],'--action','use_local')
  self.assertEqual(self.sync()['counts']['applied'],1)
  self.assertTrue(any(h['action']=='resolved_use_local' for h in self.events()[0]['history']))
 def test_edit_race_keeps_upstream_content_and_retains_local_proposal(self):
  self.queue('edit','1',{'fields':{'title':'Offline title'}})
  result=self.sync(env=dict(self.env,EDIT_RACE='1'))
  self.assertEqual(result['counts']['conflict'],1)
  self.assertEqual(self.load_state()['items']['1']['title'],'Upstream edit raced after GET')
  self.assertIn('Offline title',self.state['comments'][0]['body'])
  writes=[json.loads(line)['method'] for line in (self.d/'requests').read_text().splitlines()]
  self.assertNotIn('PATCH',writes,'no unguarded field overwrite allowed')
 def test_consecutive_edits_depend_on_each_other(self):
  first=self.queue('edit','1',{'fields':{'title':'First title'}})
  second=self.queue('edit','1',{'fields':{'title':'Second title'}})
  self.assertIn(first['id'],second['depends_on']);self.assertEqual(second['base']['title'],'First title')
  self.assertEqual(self.sync()['counts']['conflict'],1)
  self.assertEqual(self.load_state()['items']['1']['title'],'Initial')
  self.state['items']['1']['title']='First title';self.write_state()
  self.j('outbox','resolve',first['id'],'--action','use_local')
  result=self.sync();self.assertEqual(result['counts']['applied'],1);self.assertEqual(result['counts']['conflict'],1)
  self.state=self.load_state();self.state['items']['1']['title']='Second title';self.write_state()
  self.j('outbox','resolve',second['id'],'--action','use_local')
  self.assertEqual(self.sync()['counts']['applied'],2)
 def test_new_issue_comment_dependency(self):
  created=self.queue('issue_create',payload={'title':'New offline issue','body':'Original draft','labels':[]})
  self.queue('comment','local:'+created['id'],{'body':'Draft follow-up'})
  self.assertEqual(self.sync()['counts']['applied'],2)
  remote=self.load_state();self.assertEqual(len(remote['items']),2);self.assertTrue(remote['comments'][0]['issue_url'].endswith('/2'))
 def test_label_union_and_conflicting_label_definition(self):
  self.queue('label_create',payload={'name':'offline','color':'a8f372','description':'Offline work'})
  self.queue('labels','1',{'add':['offline'],'remove':[]})
  self.assertEqual(self.sync()['counts']['applied'],2)
  self.assertEqual(self.load_state()['items']['1']['labels'][0]['name'],'offline')
  self.queue('label_create',payload={'name':'offline','color':'ffffff','description':'Different'})
  self.assertEqual(self.sync()['counts']['conflict'],1)
  self.assertEqual(self.load_state()['labels']['offline']['color'],'a8f372')
 def test_native_branch_commit_and_pinned_safe_publication(self):
  work=self.d/'work'
  self.j('branch','create','offline-work','--dir',work,'-R','test/repo')
  run('git','-C',work,'config','user.name','Test');run('git','-C',work,'config','user.email','test@example.org')
  (work/'README.md').write_text('local commit\n')
  self.cmd('commit','--dir',work,'--all','-m','Offline commit','-R','test/repo')
  queued=self.j('branch','publish','offline-work','-R','test/repo')
  pin=run('git','--git-dir',self.mirror,'rev-parse','refs/ogh/outbox/'+queued['id']).stdout.strip()
  self.assertEqual(pin,queued['payload']['sha'])
  self.assertEqual(self.sync()['counts']['applied'],1)
  remote=run('git','--git-dir',self.d/'origin.git','rev-parse','refs/heads/offline-work').stdout.strip()
  self.assertEqual(remote,pin)
 def test_offline_issue_branch_pr_publication_and_durable_history(self):
  work=self.d/'feature'
  self.j('branch','create','feature/offline','--dir',work,'-R','test/repo')
  run('git','-C',work,'config','user.name','Test');run('git','-C',work,'config','user.email','test@example.org')
  (work/'README.md').write_text('Offline feature\n')
  self.cmd('commit','--dir',work,'--all','-m','Build offline feature','-R','test/repo')
  issue=self.j('issue','create','--title','Offline issue','--body','Saved offline','-R','test/repo')
  branch=self.j('branch','publish','feature/offline','-R','test/repo')
  pr=self.j('pr','create','--head','feature/offline','--base','main','--draft','--title','Offline PR','--body','Offline implementation','-R','test/repo')
  self.assertIn(branch['id'],pr['depends_on'])
  self.assertFalse((self.d/'requests').exists(),'Creating drafts and branches must stay offline')
  preview=self.j('outbox','sync','--dry-run','-R','test/repo')
  self.assertEqual(preview['counts']['queued'],3)
  self.assertFalse((self.d/'requests').exists())
  result=self.j('outbox','sync','--yes','--plan-token',preview['plan_token'],'-R','test/repo')
  self.assertEqual(result['counts']['applied'],3);self.assertEqual(result['incoming_errors'],[])
  snapshot=json.loads((self.d/'cache/test/repo/snapshot.json').read_text())
  self.assertEqual(len(snapshot['items']),3,'Published issue and PR must return through Pull')
  remote=self.load_state();prs=[v for v in remote['items'].values() if 'pull_request' in v]
  self.assertEqual(len(prs),1);self.assertTrue(prs[0]['draft']);self.assertEqual(prs[0]['head'],'feature/offline')
  self.assertEqual(run('git','--git-dir',self.d/'origin.git','rev-parse','refs/heads/feature/offline').stdout.strip(),branch['payload']['sha'])
  self.assertEqual(self.sync()['counts']['applied'],3)
  self.assertEqual(len(self.load_state()['items']),3,'Repeated Push must not duplicate issue/PR creation')
  # Each call starts a fresh CLI process: receipts, payloads and history survive restarts.
  events={e['id']:e for e in self.events()}
  for queued in [issue,branch,pr]:
   event=events[queued['id']];self.assertEqual(event['status'],'applied');self.assertTrue(event['remote'])
   self.assertEqual(event['payload'],queued['payload'])
   self.assertIn('github_confirmed',[h['action'] for h in event['history']])
  report=self.cmd('outbox','report','--format','md','-R','test/repo').stdout
  self.assertIn(pr['id'],report);self.assertIn('Offline PR',report)
  self.assertIn('Build offline feature',run('git','-C',work,'log','--oneline').stdout)
 def test_pr_head_change_is_conflict_with_history_retained(self):
  self.j('branch','create','reviewed-head','--dir',self.d/'work','-R','test/repo')
  pr=self.j('pr','create','--head','reviewed-head','--base','main','--title','Pinned PR','--body','Keep draft','-R','test/repo')
  self.state['branches']['reviewed-head']='f'*40;self.write_state()
  self.assertEqual(self.sync()['counts']['conflict'],1)
  event=self.events()[0];self.assertEqual(event['id'],pr['id']);self.assertEqual(event['payload']['body'],'Keep draft')
  self.assertEqual(len(self.load_state()['items']),1,'Changed head must not open an unreviewed PR')
  self.assertTrue(event['history']);self.assertEqual(event['base']['head_sha'],self.sha)
 def test_remote_branch_collision_never_force_pushes(self):
  self.j('branch','create','taken','--dir',self.d/'work','-R','test/repo')
  self.j('branch','publish','taken','-R','test/repo')
  # A different commit appears under the previously absent name.
  (self.source/'README.md').write_text('remote work\n');run('git','-C',self.source,'commit','-am','remote')
  remote=run('git','-C',self.source,'rev-parse','HEAD').stdout.strip();self.state['branches']['taken']=remote;self.write_state()
  result=self.sync();self.assertEqual(result['counts']['conflict'],1)
  self.assertEqual(self.load_state()['branches']['taken'],remote)
 def test_concurrent_enqueue_cannot_escape_reviewed_plan(self):
  import time
  for index,delay in enumerate([0,.005,.01,.02]):
   first=self.queue('comment','1',{'body':'Reviewed '+str(index)})
   token=self.j('outbox','sync','--dry-run','-R','test/repo')['plan_token']
   with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    write=pool.submit(self.queue,'comment','1',{'body':'NOT PREVIEWED '+str(index)})
    time.sleep(delay)
    sync=pool.submit(self.cmd,'outbox','sync','--yes','-R','test/repo','--plan-token',token,ok=False)
    second=write.result();sync.result()
   self.assertFalse(any(c['body'].startswith('NOT PREVIEWED') for c in self.load_state()['comments']))
   for e in self.events():
    if e['status'] not in ('applied','cancelled'):self.j('outbox','resolve',e['id'],'--action','keep_remote')
 def test_client_retry_is_deduplicated_locally(self):
  import uuid
  client=str(uuid.uuid4())
  args=['outbox','enqueue','-R','test/repo','--kind','comment','--target','1','--payload',json.dumps({'body':'Same local request'}),'--client-id',client]
  with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:rows=list(pool.map(lambda _:self.j(*args),range(4)))
  self.assertTrue(all(r['id']==client for r in rows));self.assertEqual(len(self.events()),1);self.assertEqual(len(self.events()[0]['history']),1)
  changed=args.copy();changed[changed.index('--payload')+1]=json.dumps({'body':'Different intention'})
  self.assertNotEqual(self.cmd(*changed,ok=False).returncode,0)
  self.assertEqual(self.events()[0]['payload']['body'],'Same local request')
 def test_parallel_enqueues_and_cancelled_history_retained(self):
  with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:rows=list(pool.map(lambda i:self.queue('comment','1',{'body':'Concurrent '+str(i)}),range(8)))
  self.assertEqual(len(self.events()),8)
  self.j('outbox','resolve',rows[0]['id'],'--action','keep_remote')
  self.assertEqual(len(self.events()),8);self.assertEqual(self.j('outbox','report')['counts']['cancelled'],1)

if __name__=='__main__':unittest.main(verbosity=2)
