// Fixture-only native UI setup. The real OS picker is exercised by the operator.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';
const root=process.cwd(), directory=path.join(root,'.tools',`native-transfer-proof-${randomUUID()}`);
await mkdir(directory);
const fixture=spawn(path.join(root,'.tools/ssh-fixture-venv/Scripts/python.exe'),['scripts/ssh_fixture.py',path.join(directory,'fixture')],{cwd:root,windowsHide:true,stdio:['pipe','pipe','pipe']});
const info=await new Promise((resolve,reject)=>{createInterface({input:fixture.stdout}).once('line',line=>resolve(JSON.parse(line))); fixture.once('exit',()=>reject(new Error('Fixture exited')));});
const source=path.join(directory,'native-roundtrip.bin'), destination=path.join(directory,'native-downloaded.bin');
await writeFile(source,Buffer.alloc(8*1024*1024,127));
await writeFile(path.join(info.directory,'filesystem/home/fixture/write-area/native-source.bin'),Buffer.alloc(8*1024*1024,127));
const {ATLAS_LEGACY_DB,...environment}=process.env;
const app=spawn(path.join(root,'target/debug/atlas-native.exe'),[],{cwd:root,windowsHide:false,env:{...environment,ATLAS_NATIVE_DB:path.join(directory,'native.sqlite3'),ATLAS_SSH_CONFIG:info.config,WEBVIEW2_USER_DATA_FOLDER:path.join(directory,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=9231 --remote-debugging-address=127.0.0.1'},stdio:'ignore'});
let browser;
try {
  for(let n=0;n<100;n++) { try { browser=await chromium.connectOverCDP('http://127.0.0.1:9231');break; } catch {} await new Promise(resolve=>setTimeout(resolve,200)); }
  if(!browser) throw new Error('No native fixture WebView');
  const page=browser.contexts()[0].pages()[0]; await page.waitForLoadState('domcontentloaded'); await page.locator('.rail .brand').waitFor();
  const map=await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('add_server',{input:{name:'Transfer fixture',host:'atlas-trusted'}}));
  await page.reload();
  await writeFile(path.join(root,'.tools/transfer-ui-ready.json'),JSON.stringify({directory,source,destination,info,serverId:map.servers[0].id,pid:app.pid}));
  console.log(JSON.stringify({directory,source,destination,pid:app.pid}));
  await new Promise(resolve=>app.once('exit',resolve));
  const stats=JSON.parse(await readFile(info.stats,'utf8'));
  await writeFile(path.join(directory,'final-stats.json'),JSON.stringify(stats));
} finally { await browser?.close();fixture.stdin.end('\n'); }
