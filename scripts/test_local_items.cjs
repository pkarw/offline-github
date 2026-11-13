// Exercise journal projection independently of the incoming snapshot and browser.
const assert = require('node:assert/strict');
const vm = require('node:vm');
const path = require('node:path');
const esbuild = require('../web/node_modules/esbuild');
const output = esbuild.buildSync({entryPoints:[path.join(__dirname,'../web/src/local-items.ts')],bundle:true,platform:'node',format:'cjs',write:false}).outputFiles[0].text;
const moduleContext = {exports:{}};
vm.runInNewContext(output, {module:moduleContext,exports:moduleContext.exports});
const {localItems,itemTarget,itemEvents} = moduleContext.exports;
const creation = {id:'draft-id',seq:1,kind:'pr_create',status:'queued',target:null,payload:{title:'Draft PR',body:'Original',head:'feature',base:'main'},created_at:'2026-11-13T10:00:00Z',remote:null};
const changes = [creation,
 {id:'comment',seq:2,kind:'comment',status:'queued',target:'local:draft-id',payload:{body:'Discussion'}},
 {id:'labels',seq:3,kind:'labels',status:'queued',target:'local:draft-id',payload:{add:['screenshots'],remove:[]}},
 {id:'edit',seq:4,kind:'edit',status:'queued',target:'local:draft-id',payload:{fields:{title:'Updated PR',body:'Updated body'}}},
 {id:'cancelled',seq:5,kind:'edit',status:'cancelled',target:'local:draft-id',payload:{fields:{title:'Withdrawn'}}}];
const [draft] = localItems(changes,[]);
assert.equal(draft.title,'Updated PR');
assert.equal(draft.body,'Updated body');
assert.equal(draft.comments,1);
assert.equal(draft.labels[0].name,'screenshots');
assert.equal(draft.head.ref,'feature');
assert.equal(itemTarget(draft),'local:draft-id');
assert.equal(itemEvents(draft,changes).length,5);
const sent = {...creation,status:'applied',remote:{number:42}};
const [published] = localItems([sent,...changes.slice(1)],[]);
assert.equal(published.number,42);
assert.equal(itemTarget(published),'local:draft-id','Keep dependency target before Pull caches the new item');
assert.equal(localItems([sent],[{number:42}]).length,0,'Pull must replace the local row without duplicating it');
assert.equal(itemEvents({number:42},[sent,...changes.slice(1)]).length,5,'Keep discussion history after Pull');
assert.equal(localItems([{...sent,history:[{action:'github_confirmed',at:'2026-11-13T10:01:00Z'}]}],[],'2026-11-13T10:02:00Z').length,0,'Filtered published rows must not reappear as open local drafts after Pull');
assert.equal(localItems([{...creation,status:'cancelled'}],[]).length,0);
assert.equal(localItems([{...creation,kind:'issue_create'}],[])[0].pull_request,undefined);
console.log('Local issue/PR journal projection: passed (drafts, updates, receipts, deduplication, cancellation).');
