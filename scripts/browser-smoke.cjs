const {chromium,expect}=require('../web/node_modules/@playwright/test');
const fs=require('fs');const path=require('path');
(async()=>{
 const browser=await chromium.launch();const errors=[],external=[];
 try{
 const context=await browser.newContext({viewport:{width:1440,height:1000},acceptDownloads:true});
 const base=process.env.OGH_TEST_URL;
 await context.route('**/*',r=>r.request().url().startsWith(base+'/')?r.continue():(external.push(r.request().url()),r.abort()));
 const page=await context.newPage();page.on('pageerror',e=>errors.push(String(e)));
 const artifacts=process.env.OGH_TEST_ARTIFACTS;fs.mkdirSync(artifacts,{recursive:true});
 // Mock launch only: never open a real agent or desktop during browser QA.
 const launchPath='/tmp/offline project';let launched;
 await page.route('**/api/repos/test/repo/open',async route=>{
   if(route.request().method()==='POST') { launched=route.request().postDataJSON(); await route.fulfill({json:{message:'Launch requested in a local terminal'}}); }
   else await route.fulfill({json:{path:launchPath,workspaces:[launchPath],checkout_exists:true,tools:[{id:'claude',label:'Claude Code',available:false,reason:'Command not found on the server PATH'},{id:'codex',label:'Codex CLI',available:true,reason:''},{id:'ogh',label:'ogh CLI',available:true,reason:''},{id:'terminal',label:'Terminal',available:true,reason:''}]}});
 });
 await page.goto(base);
 await expect(page.locator('.checkout-path')).toContainText(launchPath);
 await expect(page.getByRole('button',{name:'Remove offline repo…',exact:true})).toBeHidden();
 await page.getByLabel('Project options',{exact:true}).click();
 await page.getByRole('button',{name:'Remove offline repo…',exact:true}).click();
 await expect(page.getByRole('dialog')).toContainText('Checkout files, commits, cached data and outgoing changes are retained');
 await page.getByRole('button',{name:'Cancel',exact:true}).click();
 await expect(page.getByRole('button',{name:'Remove offline repo…',exact:true})).toBeHidden();
 await page.getByLabel('Open repository').click();
 await expect(page.getByRole('button',{name:'Claude Code'})).toBeDisabled();
 await page.getByRole('button',{name:'Codex CLI',exact:true}).click();
 await expect(page.getByRole('status').filter({hasText:'Launch requested'})).toBeVisible();
 if(launched?.tool!=='codex'||launched?.path!==launchPath)throw Error('Wrong launch target');
 await page.screenshot({path:path.join(artifacts,'local-launcher.png')});
 await page.getByRole('button',{name:'Push changes',exact:true}).click();await expect(page.getByRole('dialog')).toContainText('No outgoing changes to push');await expect(page.getByRole('button',{name:'Push 0 outgoing changes',exact:true})).toBeDisabled();await page.getByRole('button',{name:'Cancel',exact:true}).click();
 // Numeric searches include closed items, match exactly and Enter opens either kind.
 const search=page.getByLabel('Search cached items');
 await search.fill('#12');await expect(page.locator('.item-row')).toHaveCount(1);await expect(page.locator('.item-row')).toContainText('Fixture 12');
 await search.press('Enter');await expect(page).toHaveURL(base+'/test/repo/issues/12');
 await page.reload();await expect(page.getByLabel('Add a comment to the local queue')).toBeVisible();await expect(page).toHaveURL(base+'/test/repo/issues/12');
 await page.goBack();await expect(page).toHaveURL(base+'/test/repo/issues');
 await page.goForward();await expect(page).toHaveURL(base+'/test/repo/issues/12');
 await page.goto(base+'/test/repo/issues');await search.fill('2');await search.press('Enter');await expect(page).toHaveURL(base+'/test/repo/pull/2');
 await page.reload();await expect(page.getByLabel('Add a comment to the local queue')).toBeVisible();await expect(page).toHaveURL(base+'/test/repo/pull/2');
 await page.goto(base+'/test/repo/issues/1');await expect(page.getByLabel('Add a comment to the local queue')).toBeVisible();

 await expect(page.locator('.cached-image')).toHaveCount(2);
 await expect.poll(()=>page.locator('.cached-image').evaluateAll(images=>images.every(img=>img.complete && img.naturalWidth===1))).toBe(true);
 await page.getByLabel('Add a comment to the local queue').fill('Offline browser notes: preserve this comment.');
 await page.reload();await expect(page.getByLabel('Add a comment to the local queue')).toBeVisible();
 await expect(page.getByLabel('Add a comment to the local queue')).toHaveValue('Offline browser notes: preserve this comment.');
 // Commit locally, lose the local HTTP reply, then retry the same client UUID.
 let dropReply=true;const incomingPulls=[];
 page.on('request',r=>{if(r.url().endsWith('/api/repos/test/repo/sync'))incomingPulls.push(r.url());});
 await page.route('**/api/outbox',async route=>{
   if(route.request().method()==='POST' && dropReply){dropReply=false;await route.fetch();await route.abort();}else await route.continue();
 });
 await page.getByRole('button',{name:'Queue comment',exact:true}).click();
 await expect(page.getByRole('button',{name:'Queue comment',exact:true})).toBeEnabled();
 await expect(page.getByLabel('Add a comment to the local queue')).toHaveValue('Offline browser notes: preserve this comment.');
 await page.getByRole('button',{name:'Queue comment',exact:true}).click();
 await expect(page.locator('.local-comment')).toContainText('Offline browser notes',{timeout:15000});
 await expect(page.locator('.local-comment')).toContainText('pending');
 await page.reload();await expect(page.getByLabel('Add a comment to the local queue')).toBeVisible();await expect(page.locator('.local-comment')).toContainText('Offline browser notes');
 await page.getByRole('button',{name:'Add / remove labels',exact:true}).click();await page.getByRole('dialog').getByLabel('Add',{exact:true}).fill('airplane');await page.getByRole('dialog').getByRole('button',{name:'Queue change'}).click();
 await expect(page.locator('.queued-field')).toContainText('airplane');
 await page.getByRole('button',{name:/Outgoing/}).click();await expect(page.locator('.queue-card')).toHaveCount(2);await page.screenshot({path:path.join(artifacts,'outgoing-pending.png')});
 for(const [button,ext] of [['Download JSON','json'],['Download Markdown','md']]){const downloading=page.waitForEvent('download');await page.getByRole('button',{name:button,exact:true}).click();const d=await downloading;await d.saveAs(path.join(artifacts,'outgoing.'+ext));}
 if(!fs.readFileSync(path.join(artifacts,'outgoing.json'),'utf8').includes('Offline browser notes'))throw Error('JSON export lost local body');
 if(!fs.readFileSync(path.join(artifacts,'outgoing.md'),'utf8').includes('Offline browser notes'))throw Error('MD export lost local body');
 await page.getByRole('button',{name:'Push changes',exact:true}).click();await expect(page.getByRole('dialog')).toContainText('Offline browser notes');await page.getByRole('dialog').getByRole('button',{name:'Push 2 outgoing changes',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0,{timeout:30000});await expect(page.locator('.notice')).toContainText('Push completed',{timeout:15000});
 await expect(page.locator('.queue-marker.queue-applied')).toHaveCount(2);if(incomingPulls.length)throw Error('Push unexpectedly pulled');
 await page.getByRole('button',{name:'Pull from GitHub',exact:true}).click();await expect(page.locator('.notice')).toContainText('Pull completed',{timeout:30000});
 await page.getByRole('button',{name:/Issues/}).click();await page.locator('.item-row').first().click();await expect(page.locator('.local-comment')).toHaveCount(0);await expect(page.locator('.comment').filter({hasText:'Offline browser notes'})).toHaveCount(1);
 await page.getByRole('button',{name:'Edit title and body',exact:true}).click();await page.getByRole('dialog').getByLabel('Title',{exact:true}).fill('Offline revised title');await page.getByRole('dialog').getByRole('button',{name:'Queue change'}).click();
 // Independent remote edit before Sync must be shown as a conflict, never overwritten.
 const remoteFile=path.join(process.env.OGH_TEST_DIR,'remote.json');let remote=JSON.parse(fs.readFileSync(remoteFile,'utf8'));remote.items['1'].title='Independent GitHub title';fs.writeFileSync(remoteFile,JSON.stringify(remote));
 await page.getByRole('button',{name:'Push changes',exact:true}).click();await page.getByRole('dialog').getByRole('button',{name:'Push 1 outgoing changes',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0,{timeout:30000});
 await page.getByRole('button',{name:/Outgoing/}).click();await expect(page.locator('.queue-marker.queue-conflict')).toHaveCount(1);await expect(page.locator('.queue-card').filter({hasText:'Offline revised title'})).toContainText('Independent GitHub title');
 await page.getByRole('combobox').selectOption('conflict');
 await expect(page.locator('.notice-warning')).toBeVisible();
 await page.screenshot({path:path.join(artifacts,'outgoing-conflict.png')});
 remote=JSON.parse(fs.readFileSync(remoteFile,'utf8'));if(remote.items['1'].title!=='Independent GitHub title')throw Error('Remote edit was overwritten');
 await page.getByRole('button',{name:/Issues/}).click();
 await page.getByLabel('Filter by label').selectOption('airplane');await expect(page.locator('.item-row')).toHaveCount(1);
 await page.getByLabel('Filter by label').selectOption('');
 // New offline issues and PRs are first-class local discussions, including reloads.
 async function createDraft(kind,title) {
   await page.getByRole('button',{name:kind==='pr'?'New pull request':'New issue',exact:true}).click();
   const dialog=page.getByRole('dialog');
   await dialog.getByLabel('Title',{exact:true}).fill(title);
   await dialog.getByLabel('Body',{exact:true}).fill('Local draft description');
   if(kind==='pr'){await dialog.getByLabel('Head',{exact:true}).fill('main');await dialog.getByLabel('Base',{exact:true}).fill('main');}
   await dialog.getByRole('button',{name:'Queue change',exact:true}).click();
   await expect(dialog).toBeHidden();
   if(kind==='pr')await page.goto(base+'/test/repo/pulls');
   const row=page.locator('.item-row').filter({hasText:title});
   await expect(row).toContainText('pending');await row.click();
   await expect(page).toHaveURL(/local:[a-f0-9-]+$/);
   await expect(page.locator('.detail')).toContainText('GitHub assigns a number after Push');
   await page.getByLabel('Add a comment to the local queue').fill(title+' comment');
   await page.getByRole('button',{name:'Queue comment',exact:true}).click();
   await expect(page.locator('.local-comment')).toContainText(title+' comment');
   await page.reload();await expect(page.locator('.local-comment')).toContainText(title+' comment');
 }
 await createDraft('issue','Unpublished issue');
 await page.getByRole('button',{name:'Add / remove labels',exact:true}).click();
 await page.getByRole('dialog').getByLabel('Add',{exact:true}).fill('offline-new-label');
 await page.getByRole('dialog').getByRole('button',{name:'Queue change',exact:true}).click();
 await expect(page.locator('.detail .label')).toContainText('offline-new-label');
 await page.getByRole('button',{name:'Edit title and body',exact:true}).click();
 await page.getByRole('dialog').getByLabel('Title',{exact:true}).fill('Revised unpublished issue');
 await page.getByRole('dialog').getByRole('button',{name:'Queue change',exact:true}).click();
 await expect(page.locator('.detail h2')).toContainText('Revised unpublished issue');
 await page.getByRole('button',{name:'Back to issues',exact:true}).click();
 await page.getByLabel('Filter by label').selectOption('offline-new-label');
 await expect(page.locator('.item-row')).toHaveCount(1);
 await expect(page.locator('.item-row')).toContainText('Revised unpublished issue');
 await page.getByLabel('Filter by label').selectOption('');
 await page.getByLabel('Search cached items').fill('Revised unpublished');
 await expect(page.locator('.item-row')).toHaveCount(1);
 await page.getByLabel('Search cached items').fill('');
 await createDraft('pr','Unpublished PR');
 await page.goto(base+'/test/repo/pulls');
 await expect(page.locator('.item-row').filter({hasText:'Unpublished PR'})).toHaveCount(1);
 await page.screenshot({path:path.join(artifacts,'local-pr-draft.png')});
 // Local comments/labels/edits retain automatic creation dependencies.
 const queue=await (await context.request.get(base+'/api/outbox?repo=test/repo')).json();
 const issue=queue.events.find(e=>e.kind==='issue_create'&&e.payload.title==='Unpublished issue');
 const children=queue.events.filter(e=>e.target==='local:'+issue.id);
 if(children.length!==3)throw Error('Draft updates lost their durable local target');
 if(children.some(e=>!e.depends_on.includes(issue.id)))throw Error('Draft update missing publication dependency');
 await page.getByLabel('Project options',{exact:true}).click();await page.getByRole('button',{name:'Remove offline repo…',exact:true}).click();
 await expect(page.getByRole('dialog')).toContainText('7 outgoing changes have not been pushed');
 await page.screenshot({path:path.join(artifacts,'remove-preview.png')});
 await page.getByRole('button',{name:'Remove from app',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0);await expect(page.locator('.repo-link')).toHaveCount(0);
 await page.setViewportSize({width:390,height:844});await page.waitForTimeout(300);if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Outgoing mobile horizontal overflow');await page.screenshot({path:path.join(artifacts,'outgoing-mobile.png')});
 if(errors.length||external.length)throw Error(JSON.stringify({errors,external}));
 console.log(JSON.stringify({passed:['exact number search including closed items','issue and PR deep links, reload, back and forward','HTTP enqueue with lost local reply idempotency','unsubmitted browser draft recovery','pending comment/label markers','local issue/PR drafts with comments, labels, edits and durable URLs','reload durability','JSON/MD downloads','bound preview and explicit fake send','applied history','confirmed comment deduplication','remote conflict preserved','mobile layout','no external requests'],errors,external},null,2));
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exit(1)});
