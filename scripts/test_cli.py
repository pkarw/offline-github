#!/usr/bin/env python3
"""Skill-contract read regressions against a local fixture, never real GitHub."""
import base64,json,os,unittest
import test_outbox

class CliTests(unittest.TestCase):
 def setUp(self):
  self.f=test_outbox.OutboxTests();self.f.setUp();self.env=dict(self.f.env,OGH_REPO='test/repo')
  self.path=self.f.d/'cache/test/repo/snapshot.json';self.s=json.loads(self.path.read_text());sha=self.f.sha
  pr={'number':2,'title':'Fix offline sync','body':'A PR body','state':'closed','merged_at':'2026-11-02T00:00:00Z','updated_at':'2026-11-13T00:00:00Z','created_at':'2026-11-01T00:00:00Z','user':{'login':'tester'},'pull_request':{},'head':{'sha':sha,'ref':'feature'},'base':{'sha':sha,'ref':'main'},'labels':[{'name':'bug','color':'ededed'}],'draft':False,'commits':1}
  comment={'id':10,'body':'Discussed','user':{'login':'tester'},'created_at':'2026-11-01T00:00:00Z','html_url':'https://github.com/test/repo/issues/1#comment','issue_url':'https://api.github.com/repos/test/repo/issues/1'}
  self.s['details']['1']['comments']=[comment]
  self.s['items']['2']=pr
  self.s['details']['2']={'item':pr,'comments':[],'review_comments':[],'reviews':[],'reviews_cached':True,'files':[{'filename':'README.md','status':'modified','additions':1,'deletions':0,'patch':'+test'}],'commits':[{'sha':sha,'commit':{'message':'Fix'}}],'commit_count':1,'skills_head_sha':sha,'skills_item_updated_at':pr['updated_at'],'skills_prepared_at':'2026-11-13T00:00:00Z','gh':{'headRefOid':sha,'reviewDecision':'APPROVED','mergeable':'MERGEABLE','mergeStateStatus':'CLEAN','closingIssuesReferences':[{'number':1}]}}
  run={'id':7,'name':'CI','display_title':'Check PR','head_sha':sha,'head_branch':'feature','status':'completed','conclusion':'failure','html_url':'https://github.com/test/repo/actions/runs/7','created_at':'2026-11-13T00:00:00Z','jobs':[{'id':8,'name':'test','status':'completed','conclusion':'failure','steps':[{'name':'Run tests','conclusion':'failure','number':1}]}]}
  values={'repos/test/repo/labels':[{'name':'bug','color':'ededed'},{'name':'unused-label','color':'a8f372'}], 'user':{'login':'tester'},f'repos/test/repo/commits/{sha}/check-runs':{'check_runs':[{'name':'test','conclusion':'failure','status':'completed','html_url':'https://github.com/test/repo/checks/1'}]},f'repos/test/repo/commits/{sha}/status':{'statuses':[]},f'repos/test/repo/actions/runs/{sha}':{'total_count':1,'workflow_runs':[run]},'repos/test/repo/actions/runs/7':run,'logs/7':'test failed\n'}
  self.s['resources']={k:{'data':v,'cached_at':'2026-11-13T00:00:00Z'} for k,v in values.items()};self.save()
 def tearDown(self):self.f.tearDown()
 def save(self):self.path.write_text(json.dumps(self.s))
 def cmd(self,*args,ok=True):return self.f.cmd(*args,ok=ok,env=self.env)
 def j(self,*args):return json.loads(self.cmd(*args).stdout)
 def test_agent_provider_advertises_reviewed_writes(self):
  status=self.j('skills','status')
  self.assertTrue(status['local_queue'])
  self.assertTrue(status['reviewed_push'])
  self.assertFalse(status['direct_tracker_writes'])
  self.assertIn('comments',status['queued_write_operations'])
  self.assertIn('labels',status['queued_write_operations'])
  self.assertNotIn('GitHub mutations and publishing',status['blocked'])
  provider=self.cmd('skills','provider').stdout
  self.assertIn('ogh issue comment NUMBER --body-file FILE',provider)
  self.assertIn('ogh outbox sync --yes --plan-token TOKEN',provider)
  self.assertFalse((self.f.d/'requests').exists())
 def test_documented_agent_comments_and_labels_queue_then_push(self):
  body=self.f.d/'agent-comment.md';body.write_text('Comment queued by an offline agent')
  self.cmd('issue','comment','1','--body-file',body)
  self.cmd('label','create','agent-ready','--color','aabbcc','--description','Agent queued label')
  self.cmd('issue','edit','1','--add-label','agent-ready')
  report=self.j('outbox','report')
  self.assertEqual(len(report['events']),3)
  self.assertTrue(all(e['status']=='queued' for e in report['events']))
  self.assertFalse((self.f.d/'requests').exists(),'queueing must stay offline')
  plan=self.j('outbox','sync','--dry-run')
  self.assertFalse((self.f.d/'requests').exists(),'preview must stay offline')
  pushed=self.j('outbox','sync','--yes','--plan-token',plan['plan_token'])
  self.assertTrue(all(e['status']=='applied' for e in pushed['events']))
  remote=self.f.load_state()
  self.assertTrue(any('Comment queued by an offline agent' in c['body'] for c in remote['comments']))
  self.assertIn('agent-ready',remote['labels'])
  self.assertIn('agent-ready',[l['name'] for l in remote['items']['1']['labels']])
 def test_projection_jq_repo_identity_and_legacy_json(self):
  self.assertEqual(self.cmd('repo','view','--json','nameWithOwner,defaultBranchRef','--jq','.defaultBranchRef.name').stdout.strip(),'main')
  selected=self.j('issue','view','1','--json','number,author,state,comments');self.assertEqual(selected['author']['login'],'tester');self.assertEqual(selected['state'],'OPEN');self.assertEqual(selected['comments'][0]['author']['login'],'tester')
  self.assertIn('item',self.j('issue','view','1','--json'))
  self.assertEqual(self.cmd('issue','view','https://github.com/test/repo/issues/1','--json','number','--jq','.number').stdout.strip(),'1')
  self.assertNotEqual(self.cmd('issue','view','1','--json','notAField',ok=False).returncode,0)
 def test_state_search_filters_and_prepared_pr_fields(self):
  result=self.j('pr','list','--state','merged','--search','merged:>=2026-11-01 author:tester label:bug','--json','number,author,mergedAt');self.assertEqual(result[0]['number'],2)
  self.assertEqual(self.j('pr','list','--state','closed','--json','number'),[])
  self.assertEqual(self.j('search','prs','#2','--state','closed','--json','number'),[]) # closed excludes merged
  self.assertEqual(self.j('search','prs','#2','--state','merged','--json','number'),[{'number':2}])
  self.assertEqual(self.j('pr','view','2','--json','reviewDecision,closingIssuesReferences,commits,files')['reviewDecision'],'APPROVED')
  self.assertNotEqual(self.cmd('issue','list','--search','review:approved',ok=False).returncode,0)
 def test_rest_comments_pagination_taxonomy_and_contents(self):
  self.assertEqual(self.cmd('api','repos/test/repo/issues/comments/10','--jq','.user.login').stdout.strip(),'tester')
  self.assertEqual(self.j('api','repos/test/repo/issues/1/comments','--paginate','--slurp')[0][0]['id'],10)
  labels=self.j('api','repos/test/repo/labels','--paginate');self.assertEqual(len(labels),2)
  self.assertEqual(len(self.j('label','list','--json','name')),2)
  blob=self.j('api','repos/test/repo/contents/README.md?ref=HEAD');self.assertEqual(base64.b64decode(blob['content']),b'base\n')
 def test_checks_exit_status_and_stale_head_fail_closed(self):
  failed=self.cmd('pr','checks','2','--json','name,state,link',ok=False);self.assertEqual(failed.returncode,1);self.assertEqual(json.loads(failed.stdout)[0]['state'],'FAILURE')
  key=f'repos/test/repo/commits/{self.f.sha}/check-runs'
  self.s['resources'][key]['data']['check_runs'][0].update(conclusion=None,status='in_progress');self.save();self.assertEqual(self.cmd('pr','checks','2','--json','name,state',ok=False).returncode,8)
  self.s['resources'][key]['data']['check_runs'][0].update(conclusion='success',status='completed');self.save();self.assertEqual(self.cmd('pr','checks','2','--json','name,state').returncode,0)
  self.s['items']['2']['head']['sha']='f'*40;self.s['details']['2']['item']['head']['sha']='f'*40;self.save();self.assertNotEqual(self.cmd('pr','checks','2','--json','state',ok=False).returncode,0)
  self.assertNotEqual(self.cmd('api','repos/test/repo/pulls/2/commits',ok=False).returncode,0)
 def test_runs_logs_and_no_implicit_upstream_reads(self):
  self.assertEqual(self.j('run','list','--branch','feature','--json','databaseId,workflowName')[0]['databaseId'],7)
  self.assertEqual(self.j('run','view','7','--json','status,jobs')['jobs'][0]['name'],'test')
  self.assertIn('test failed',self.cmd('run','view','7','--log-failed').stdout)
  self.assertEqual(self.cmd('api','user','--jq','.login').stdout.strip(),'tester')
  self.assertNotEqual(self.cmd('api','repos/test/repo/issues','-X','POST',ok=False).returncode,0)
  self.assertNotEqual(self.cmd('api','graphql',ok=False).returncode,0)
  self.assertFalse((self.f.d/'requests').exists(),'cache reads must never call authenticated gh')

if __name__=='__main__':unittest.main(verbosity=2)
