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
 await page.getByLabel('Open repository').click();
 await expect(page.getByRole('button',{name:'Claude Code'})).toBeDisabled();
 await page.getByRole('button',{name:'Codex CLI',exact:true}).click();
 await expect(page.getByRole('status').filter({hasText:'Launch requested'})).toBeVisible();
 if(launched?.tool!=='codex'||launched?.path!==launchPath)throw Error('Wrong launch target');
 await page.screenshot({path:path.join(artifacts,'local-launcher.png')});
 await expect(page.locator('.item-row').first()).toBeVisible({timeout:20000});await page.locator('.item-row').first().click();
 await page.getByLabel('Add a comment to the local queue').fill('Offline browser notes: preserve this comment.');
 await page.reload();await expect(page.locator('.item-row').first()).toBeVisible();await page.locator('.item-row').first().click();
 await expect(page.getByLabel('Add a comment to the local queue')).toHaveValue('Offline browser notes: preserve this comment.');
 // Commit locally, lose the local HTTP reply, then retry the same client UUID.
 let dropReply=true;
 await page.route('**/api/outbox',async route=>{
   if(route.request().method()==='POST' && dropReply){dropReply=false;await route.fetch();await route.abort();}else await route.continue();
 });
 await page.getByRole('button',{name:'Queue comment',exact:true}).click();
 await expect(page.getByRole('button',{name:'Queue comment',exact:true})).toBeEnabled();
 await expect(page.getByLabel('Add a comment to the local queue')).toHaveValue('Offline browser notes: preserve this comment.');
 await page.getByRole('button',{name:'Queue comment',exact:true}).click();
 await expect(page.locator('.local-comment')).toContainText('Offline browser notes',{timeout:15000});
 await expect(page.locator('.local-comment')).toContainText('pending');
 await page.reload();await expect(page.locator('.item-row').first()).toBeVisible();await page.locator('.item-row').first().click();await expect(page.locator('.local-comment')).toContainText('Offline browser notes');
 await page.getByRole('button',{name:'Add / remove labels',exact:true}).click();await page.getByRole('dialog').getByLabel('Add',{exact:true}).fill('airplane');await page.getByRole('dialog').getByRole('button',{name:'Queue change'}).click();
 await expect(page.locator('.queued-field')).toContainText('airplane');
 await page.getByRole('button',{name:/Outgoing/}).click();await expect(page.locator('.queue-card')).toHaveCount(2);await page.screenshot({path:path.join(artifacts,'outgoing-pending.png')});
 for(const [button,ext] of [['Download JSON','json'],['Download Markdown','md']]){const downloading=page.waitForEvent('download');await page.getByRole('button',{name:button,exact:true}).click();const d=await downloading;await d.saveAs(path.join(artifacts,'outgoing.'+ext));}
 if(!fs.readFileSync(path.join(artifacts,'outgoing.json'),'utf8').includes('Offline browser notes'))throw Error('JSON export lost local body');
 if(!fs.readFileSync(path.join(artifacts,'outgoing.md'),'utf8').includes('Offline browser notes'))throw Error('MD export lost local body');
 await page.getByRole('button',{name:'Preview sync',exact:true}).click();await expect(page.getByRole('dialog')).toContainText('Offline browser notes');await page.getByRole('dialog').getByRole('button',{name:'Sync 2 changes',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0,{timeout:30000});await expect(page.locator('.notice')).toContainText('Outgoing changes reconciled',{timeout:15000});
 await expect(page.locator('.queue-marker.queue-applied')).toHaveCount(2);
 await page.getByRole('button',{name:/Issues/}).click();await page.locator('.item-row').first().click();await expect(page.locator('.local-comment')).toHaveCount(0);await expect(page.locator('.comment').filter({hasText:'Offline browser notes'})).toHaveCount(1);
 await page.getByRole('button',{name:'Edit title and body',exact:true}).click();await page.getByRole('dialog').getByLabel('Title',{exact:true}).fill('Offline revised title');await page.getByRole('dialog').getByRole('button',{name:'Queue change'}).click();
 // Independent remote edit before Sync must be shown as a conflict, never overwritten.
 const remoteFile=path.join(process.env.OGH_TEST_DIR,'remote.json');let remote=JSON.parse(fs.readFileSync(remoteFile,'utf8'));remote.items['1'].title='Independent GitHub title';fs.writeFileSync(remoteFile,JSON.stringify(remote));
 await page.getByRole('button',{name:'Preview sync',exact:true}).click();await page.getByRole('dialog').getByRole('button',{name:'Sync 1 changes',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0,{timeout:30000});
 await page.getByRole('button',{name:/Outgoing/}).click();await expect(page.locator('.queue-marker.queue-conflict')).toHaveCount(1);await expect(page.locator('.queue-card').filter({hasText:'Offline revised title'})).toContainText('Independent GitHub title');
 await page.getByRole('combobox').selectOption('conflict');
 await expect(page.locator('.notice-warning')).toBeVisible();
 await page.screenshot({path:path.join(artifacts,'outgoing-conflict.png')});
 remote=JSON.parse(fs.readFileSync(remoteFile,'utf8'));if(remote.items['1'].title!=='Independent GitHub title')throw Error('Remote edit was overwritten');
 await page.setViewportSize({width:390,height:844});await page.waitForTimeout(300);if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Outgoing mobile horizontal overflow');await page.screenshot({path:path.join(artifacts,'outgoing-mobile.png')});
 if(errors.length||external.length)throw Error(JSON.stringify({errors,external}));
 console.log(JSON.stringify({passed:['HTTP enqueue with lost local reply idempotency','unsubmitted browser draft recovery','pending comment/label markers','reload durability','JSON/MD downloads','bound preview and explicit fake send','applied history','confirmed comment deduplication','remote conflict preserved','mobile layout','no external requests'],errors,external},null,2));
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exit(1)});
