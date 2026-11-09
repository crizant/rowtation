// Production-package export smoke test. Node 24+, no npm packages. Synthetic data only.
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readFile, mkdtemp, rm, stat } from 'node:fs/promises';
import { resolve, extname, join } from 'node:path';
import { tmpdir } from 'node:os';

const args = {};
for (let i = 2; i < process.argv.length; i++) {
  const key = process.argv[i];
  if (!key.startsWith('--')) throw new Error('Expected an option: ' + key);
  const next = process.argv[i + 1];
  args[key.slice(2)] = next && !next.startsWith('--') ? process.argv[++i] : true;
}
if (!args.build || !args.browser) throw new Error('Usage: node scripts/test-export-package.mjs --build dist/Rowtation --browser <Chrome>');
const root = resolve(args.build);
const browser = args.browser ?? '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser';
const sleep = ms => new Promise(r => setTimeout(r, ms));
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.json': 'application/json' };
const server = createServer(async (req, res) => {
  try {
    const path = resolve(root, '.' + new URL(req.url, 'http://local').pathname.replace(/\/$/, '/index.html'));
    if (!path.startsWith(root + '/')) throw new Error('outside build');
    let data = await readFile(path);
    res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' }); res.end(data);
  } catch { res.writeHead(404); res.end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const profile = await mkdtemp(join(tmpdir(), 'rowtation-export-browser-'));
const child = spawn(browser, ['--headless', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-sync', '--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--blink-settings=primaryPointerType=4,availablePointerTypes=4,primaryHoverType=2,availableHoverTypes=2', 'about:blank'], { stdio: 'ignore' });
let ws, nextId = 0; const pending = new Map(), listeners = new Map();
function send(method, params = {}) {
  const id = ++nextId;
  return new Promise((resolve, reject) => { pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params })); });
}
function event(name) { return new Promise((resolve,reject) => { const timer=setTimeout(()=>{listeners.delete(name);reject(new Error('Timed out: '+name));},15000);listeners.set(name,value=>{clearTimeout(timer);resolve(value);}); }); }
async function evaluate(expression) {
  const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r.result.value;
}
async function until(expression, timeout = 120000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) { if (await evaluate(expression)) return; await sleep(100); }
  throw new Error('Timed out: ' + expression);
}
async function click(text, clickCount = 1) {
  const p = await evaluate(`(()=>{const el=[...document.querySelectorAll('[role="button"], [role="checkbox"], .nsh-run')].find(el=>el.getAttribute('aria-label')===${JSON.stringify(text)}||el.textContent.trim()===${JSON.stringify(text)});if(!el)return null;el.scrollIntoView({block:"nearest"});const r=el.getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`);
  if (!p) throw new Error('Button missing: ' + text);
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', ...p, button: 'left', clickCount });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...p, button: 'left', clickCount });
  await sleep(100);
}
try {
  let port;
  for (let i = 0; i < 100; i++) { try { port = (await readFile(join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]; break; } catch { await sleep(100); } }
  const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  ws = new WebSocket(pages.find(p => p.type === 'page').webSocketDebuggerUrl);
  await new Promise(r => ws.addEventListener('open', r, { once: true }));
  ws.addEventListener('message', e => { const m = JSON.parse(e.data); if (m.id) { const p = pending.get(m.id); pending.delete(m.id); m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result); } else if (listeners.has(m.method)) { listeners.get(m.method)(m.params); listeners.delete(m.method); } });
  ws.addEventListener('close', () => {
    for (const request of pending.values()) request.reject(new Error('Browser connection closed'));
    pending.clear();
  });
  await send('Page.enable'); await send('Runtime.enable'); await send('Performance.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 800, deviceScaleFactor: Number(args.dpr ?? 2), mobile: false });
  await send('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
  await until(`!!document.querySelector('#rowtation-file-input')`);
  // The home must not scroll through empty space, but short screens must still
  // reach the final action. Compare actual browser scroll bounds, not snapshots.
  for (const [width, height] of [[1280, 800], [390, 844], [320, 568], [1280, 480]]) {
    await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
    await sleep(150);
    const bounds = await evaluate(`(() => {
      const scroll = document.querySelector('[data-testid="home-scroll"]');
      const content = [...scroll.children].find(el => el.classList.contains('nsh-v'));
      scroll.scrollTop = 100000;
      const link = document.querySelector('[data-testid="project-link"]');
      return { viewport: scroll.clientHeight, content: content.getBoundingClientRect().height,
        extent: scroll.scrollHeight, width: scroll.clientWidth, scrollWidth: scroll.scrollWidth,
        top: scroll.scrollTop, bottom: link.getBoundingClientRect().bottom,
        edge: scroll.getBoundingClientRect().bottom };
    })()`);
    if (Math.abs(bounds.extent - Math.max(bounds.viewport, bounds.content)) > 1)
      throw new Error(`Home has empty scroll space at ${width}×${height}: ${JSON.stringify(bounds)}`);
    if (bounds.scrollWidth !== bounds.width) throw new Error('Home overflows horizontally');
    if (bounds.content <= bounds.viewport && bounds.top !== 0) throw new Error('Fitting home can still scroll');
    if (bounds.content > bounds.viewport && (bounds.top === 0 || bounds.bottom > bounds.edge + 1))
      throw new Error('Short home cannot reach GitHub');
    console.log(`PASS home scroll bounds ${width}×${height}: ${bounds.extent}px`);
  }
  await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 800, deviceScaleFactor: Number(args.dpr ?? 2), mobile: false });
  await evaluate(`document.querySelector('[data-testid="home-scroll"]').scrollTop = 0`);
  await sleep(100);
  await send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: profile, eventsEnabled: true });
  await send('Browser.grantPermissions', {origin: `http://127.0.0.1:${server.address().port}`, permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite']});
  await evaluate(`globalThis.__exportUrls=[];globalThis.__exportRevoked=[];
    const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL);
    URL.createObjectURL=blob=>{const url=create(blob);__exportUrls.push({url,type:blob.type});return url;};
    URL.revokeObjectURL=url=>{__exportRevoked.push(url);revoke(url);};`);
  await click('Paste from clipboard');
  await evaluate(`(()=>{const el=document.querySelector('[data-testid="paste-input"]');(el.matches('input,textarea')?el:el.querySelector('input,textarea')).focus()})()`);
  await send('Input.insertText', {text:'id,city\n001,Brisbane\n002,Sydney'});
  await click('Review import'); await until(`document.body.textContent.includes('Import document')`);
  await click('Import document'); await until(`!!document.querySelector('[data-testid="data-grid"]')`);
  await sleep(600);
  await click('Brisbane');
  await click('Brisbane');
  await until(`!!document.querySelector('[data-testid="cell-draft"]')`);
  // Select the draft without platform-specific OS shortcuts, then type through CDP.
  await evaluate(`(()=>{const el=document.querySelector('[data-testid="cell-draft"]');const input=el.matches('input,textarea')?el:el.querySelector('input,textarea');input.focus();input.select();})()`);
  await send('Input.insertText',{text:'Perth'}); await click('Save cell');
  await until(`document.body.textContent.includes('1 modified · Unsaved')`);
  await click('Export…'); await until(`document.body.textContent.includes('Export 2 rows')`);
  // Radio labels have semantic accessibility names in the production DOM.
  async function choose(format) {
    const p=await evaluate(`(()=>{const el=[...document.querySelectorAll('[role="radio"]')].find(e=>e.getAttribute('aria-label')===${JSON.stringify(format+' · '+format)});if(!el)return null;el.scrollIntoView({block:"nearest"});const r=el.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
    if(!p)throw new Error('Format radio missing: '+format);
    await send('Input.dispatchMouseEvent',{type:'mousePressed',...p,button:'left',clickCount:1});
    await send('Input.dispatchMouseEvent',{type:'mouseReleased',...p,button:'left',clickCount:1});
    await sleep(100);
  }
  await choose('JSON');
  await click('Readable formatting');
  await click('Copy'); await until(`document.body.textContent.includes('Copied export text')`);
  const copied=await evaluate(`navigator.clipboard.readText()`);
  if(copied!=='[{"id":"001","city":"Perth"},{"id":"002","city":"Sydney"}]')throw new Error('Clipboard mismatch: '+copied);
  for(const [format,extension,mime] of [['JSON','json','application/json'],['NDJSON','ndjson','application/x-ndjson'],['XML','xml','application/xml'],['CSV','csv','text/csv'],['TSV','tsv','text/tab-separated-values']]){
    await choose(format);
    if(format==='XML'||format==='CSV'||format==='TSV') await click('Accept these mapping changes');
    const started=event('Browser.downloadWillBegin');
    await click('Download'); const download=await started;
    if(download.suggestedFilename!=='export.'+extension)throw new Error('Filename mismatch: '+download.suggestedFilename);
    const path=join(profile,download.suggestedFilename);
    await until(`document.body.textContent.includes('Download requested:')`);
    for(let i=0;i<100;i++){try{await stat(path);break;}catch{await sleep(50);}}
    const text=await readFile(path,'utf8');
    if(!text.includes('001')||!text.includes('Perth'))throw new Error('Downloaded content mismatch: '+format);
    if(format==='NDJSON'&&(text.trimEnd().split('\n').length!==2||!text.endsWith('\n')))throw new Error('NDJSON line mismatch');
    const type=await evaluate(`__exportUrls.at(-1).type`);
    if(!type.startsWith(mime))throw new Error('MIME mismatch: '+type);
    await until(`__exportRevoked.includes(__exportUrls.at(-1).url)`);
    if(await evaluate(`!!document.querySelector('a[download]')`))throw new Error('Temporary download anchor leaked');
    console.log('PASS download',format,download.suggestedFilename,type);
  }
  await click('Cancel');await click('Undo');await until(`document.body.textContent.includes('Unmodified')`);
  await send('Emulation.setDeviceMetricsOverride', {width:390,height:844,deviceScaleFactor:2,mobile:true});
  await send('Emulation.setTouchEmulationEnabled', {enabled:true,maxTouchPoints:1});
  await sleep(200);
  async function tap(label) {
    const point=await evaluate(`(()=>{const el=[...document.querySelectorAll('[role="button"],[role="radio"]')].find(e=>e.getAttribute('aria-label')===${JSON.stringify(label)}||e.textContent.trim()===${JSON.stringify(label)});if(!el)return null;el.scrollIntoView({block:'nearest'});const r=el.getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2,id:1};})()`);
    if(!point)throw new Error('Touch target missing: '+label);
    await send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[point]});
    await send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
    await sleep(100);
  }
  await tap('Export…');await until(`document.body.textContent.includes('Export 2 rows')`);
  await tap('NDJSON · NDJSON');await tap('Copy');await until(`document.body.textContent.includes('Copied export text')`);
  if(!(await evaluate(`navigator.clipboard.readText()`)).includes('Brisbane'))throw new Error('Touch copy did not reflect undo');
  await tap('Cancel');await until(`!document.querySelector('[data-testid="app-dialog"]')`);
  console.log('PASS mobile touch format selection, copy and cancellation');
  console.log('PASS production copy, five downloads, MIME/extensions, object URL cleanup and undo/dirty lifecycle');
} finally {
  ws?.close();child.kill();server.close();await rm(profile,{recursive:true,force:true,maxRetries:5,retryDelay:100});
}
