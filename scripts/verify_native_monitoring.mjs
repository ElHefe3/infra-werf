// Actual WebView2/Rust proof. Loopback fixture and disposable store only.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const root=process.cwd(),directory=path.join(root,'.tools',`monitoring-proof-${randomUUID()}`);
await mkdir(directory,{recursive:true});
const fixture=spawn(path.join(root,'.tools/ssh-fixture-venv/Scripts/python.exe'),['scripts/monitoring_fixture.py',path.join(directory,'fixture')],{cwd:root,windowsHide:true,stdio:['pipe','pipe','pipe']});
const lines=createInterface({input:fixture.stdout});let diagnostic='';fixture.stderr.on('data',data=>diagnostic+=data.toString());
const nextLine=()=>new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error(`Fixture timeout: ${diagnostic}`)),20000);lines.once('line',line=>{clearTimeout(timer);resolve(JSON.parse(line));});});
const control=async command=>{const ack=nextLine();fixture.stdin.write(command+'\n');expect((await ack).ack).toBe(command);};
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const database=path.join(directory,'native.sqlite3'),endpoint='http://127.0.0.1:9237';
const results=[];let app,browser,page,info;const errors=[];
const invoke=async(command,args)=>{
  let timer;
  const timeout=new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error(`Native command timed out: ${command}`)),45000);timer.unref();});
  try{return await Promise.race([page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args}),timeout]);}finally{clearTimeout(timer);}
};
async function launch(){
  const {ATLAS_NATIVE_DB,ATLAS_LEGACY_DB,ATLAS_REPORTS_DIR,...environment}=process.env;
  app=spawn(process.env.ATLAS_DESKTOP_BINARY ?? path.join(root,'target/release/atlas-native.exe'),[],{cwd:root,windowsHide:true,env:{...environment,ATLAS_NATIVE_DB:database,ATLAS_SSH_CONFIG:info.config,WEBVIEW2_USER_DATA_FOLDER:path.join(directory,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=9237 --remote-debugging-address=127.0.0.1'},stdio:'ignore'});
  for(let n=0;n<100;n++){if(app.exitCode!==null)throw new Error('Native application exited');try{if((await fetch(endpoint+'/json/version')).ok)break;}catch{}await delay(200);}
  browser=await chromium.connectOverCDP(endpoint);page=browser.contexts()[0].pages()[0];page.setDefaultTimeout(15000);page.on('pageerror',e=>errors.push(e.message));await page.waitForLoadState('domcontentloaded');await page.waitForFunction(()=>Boolean(window.__TAURI_INTERNALS__?.invoke));
}
async function quit(){await page.getByRole('button',{name:'Quit InfraWerf',exact:true}).click();await expect.poll(()=>app.exitCode,{timeout:20000}).not.toBe(null);await browser.close();browser=null;}
const policy={lifecycle:'active',enabled:true,owner_offline:false,inventory_enabled:true,interval_seconds:30,inventory_interval_seconds:300};
try{
  info=await nextLine();await launch();await invoke('monitoring_discover');
  let state=await invoke('map_state');const node=state.servers.find(s=>s.host==='monitor-node'),offline=state.servers.find(s=>s.host==='monitor-offline');
  expect(node).toBeTruthy();expect(offline).toBeTruthy();expect((await invoke('monitoring_state')).records.every(r=>r.policy.lifecycle==='candidate')).toBe(true);
  expect(JSON.parse(await readFile(info.stats,'utf8')).connections).toBe(0);results.push('configured aliases become candidates without any connection');
  console.log('PASS: candidate discovery makes no network connection');
  await invoke('monitoring_configure',{serverId:offline.id,policy:{...policy,owner_offline:true}});
  await invoke('monitoring_configure',{serverId:node.id,policy});
  const record=async()=> (await invoke('monitoring_state')).records.find(r=>r.server_id===node.id);
  await expect.poll(async()=>(await record()).inventory_detail,{timeout:20000}).toBe('Automatic inventory collection complete');
  await expect(page.locator(`[data-server-id="${node.id}"] .live-status`)).toHaveText('Reachable');
  await expect(page.locator(`[data-server-id="${offline.id}"] .live-status`)).toContainText('owner reported');
  state=await invoke('map_state');expect(state.servers.find(s=>s.id===node.id).inventory.services[0].name).toBe('worker.service');results.push('automatic SSH checks and approved collection run on topology; owner-offline stays visibly distinct');
  console.log('PASS: automatic check, complete inventory and owner-offline map display');
  await control('down');
  await expect.poll(async()=>(await record()).reachability.state,{timeout:45000,intervals:[1000]}).toBe('unreachable');
  await expect(page.locator(`[data-server-id="${node.id}"]`)).toHaveClass(/status-unreachable/);
  expect((await invoke('map_state')).servers.find(s=>s.id===node.id).inventory).toBeTruthy();results.push('periodic outage detection turns map card red and retains saved inventory');
  console.log('PASS: periodic outage detection');
  await control('up');
  await expect.poll(async()=>(await record()).reachability.state,{timeout:45000,intervals:[1000]}).toBe('reachable');
  results.push('returning host recovers under the original stable server ID');
  console.log('PASS: periodic recovery under stable ID');
  await control('failed');await invoke('monitoring_configure',{serverId:node.id,policy});
  await expect.poll(async()=>(await invoke('map_state')).events.some(e=>e.server_id===node.id && e.message.includes('worker.service') && e.message.includes('failed')),{timeout:20000}).toBe(true);
  const eventsBefore=(await invoke('map_state')).events.filter(e=>e.message.startsWith('Inventory:')).length;
  await invoke('inventory_refresh',{serverId:node.id});expect((await invoke('map_state')).events.filter(e=>e.message.startsWith('Inventory:')).length).toBe(eventsBefore);results.push('workload changes recorded by name/state; identical newer refresh adds no inventory-change event');
  const beforeReplacement=JSON.parse(await readFile(info.stats,'utf8')).collections;
  await control('replace');await control('trust-replacement');await invoke('monitoring_configure',{serverId:node.id,policy});
  await expect.poll(async()=>(await record()).pending_fingerprint,{timeout:15000}).toBe(info.replacement);
  expect(JSON.parse(await readFile(info.stats,'utf8')).collections).toBe(beforeReplacement);
  await control('original');await control('trust-original');await invoke('monitoring_configure',{serverId:node.id,policy});
  await expect.poll(async()=>(await record()).pending_fingerprint,{timeout:15000}).toBe(null);
  await control('replace');await control('trust-replacement');await invoke('monitoring_configure',{serverId:node.id,policy});
  await expect.poll(async()=>(await record()).pending_fingerprint,{timeout:15000}).toBe(info.replacement);
  const beforeReview=JSON.parse(await readFile(info.stats,'utf8')).collections;
  expect((await record()).fingerprint).toBe(info.fingerprint);
  await invoke('monitoring_accept_identity',{serverId:node.id,previous:info.fingerprint,accepted:info.replacement});
  await expect.poll(async()=>(await record()).fingerprint,{timeout:15000}).toBe(info.replacement);results.push('trusted replacement key pauses collection until explicit reviewed-identity acceptance');
  await invoke('monitoring_configure',{serverId:node.id,policy:{...policy,lifecycle:'archived'}});
  await expect(page.locator(`[data-server-id="${node.id}"]`)).toHaveCount(0);
  const archivedAt=JSON.parse(await readFile(info.stats,'utf8')).connections;await delay(1500);expect(JSON.parse(await readFile(info.stats,'utf8')).connections).toBe(archivedAt);
  await quit();await launch();expect((await record()).policy.lifecycle).toBe('archived');
  await page.getByRole('button',{name:'Servers & monitoring',exact:true}).first().click();
  const row=page.locator('details.managed-server').filter({hasText:'monitor-node'});await row.locator('summary').click();
  await row.getByLabel('Server lifecycle').selectOption('active');await row.getByRole('button',{name:'Save monitoring',exact:true}).click();
  await expect.poll(async()=>(await record()).reachability.state,{timeout:20000}).toBe('reachable');results.push('archive drains checks, hides map card, persists through restart and restores using native form');
  await invoke('monitoring_configure',{serverId:node.id,policy:{...policy,lifecycle:'ignored'}});
  const ignoredAt=JSON.parse(await readFile(info.stats,'utf8')).connections;await delay(32000);expect(JSON.parse(await readFile(info.stats,'utf8')).connections).toBe(ignoredAt);
  await invoke('remove_server',{id:node.id});expect((await invoke('map_state')).events.every(e=>e.server_id!==node.id)).toBe(true);
  await invoke('monitoring_discover');const rediscovered=(await invoke('map_state')).servers.find(s=>s.host==='monitor-node');expect(rediscovered.id).not.toBe(node.id);expect((await invoke('monitoring_state')).records.find(r=>r.server_id===rediscovered.id).policy.lifecycle).toBe('candidate');
  expect(JSON.parse(await readFile(info.stats,'utf8')).offline_auth).toBe(0);expect(errors).toEqual([]);results.push('ignore stops checks; deletion removes local history; rediscovery produces a fresh non-contacted candidate; offline host never authenticated');
  await page.screenshot({path:path.join(directory,'servers.png'),fullPage:true});
  await writeFile(path.join(directory,'result.json'),JSON.stringify({result:'pass',results,errors,directory,beforeReview},null,2));console.log(JSON.stringify({result:'pass',results,directory},null,2));await quit();
}catch(error){console.error(`Monitoring proof artifacts: ${directory}`);await page?.screenshot({path:path.join(directory,'failure.png')}).catch(()=>{});throw error;}
finally{await browser?.close().catch(()=>{});if(app?.exitCode===null){app.kill();await new Promise(resolve=>app.once('exit',resolve));}if(fixture.exitCode===null){fixture.stdin.end('quit\n');await Promise.race([new Promise(resolve=>fixture.once('exit',resolve)),delay(5000)]);if(fixture.exitCode===null)fixture.kill();}lines.close();}
