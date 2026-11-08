// Production-build scroll probe. Node 24+, no npm packages. Artifacts stay local.
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readFile, writeFile, mkdtemp, rm, stat } from 'node:fs/promises';
import { resolve, extname, join } from 'node:path';
import { tmpdir } from 'node:os';

const args = {};
for (let i = 2; i < process.argv.length; i++) {
  const key = process.argv[i];
  if (!key.startsWith('--')) throw new Error('Expected an option: ' + key);
  const next = process.argv[i + 1];
  args[key.slice(2)] = next && !next.startsWith('--') ? process.argv[++i] : true;
}
function checkProfile(report, budget) {
  if (!report.instrumented) throw new Error('Slice checks require an --instrument recording');
  const calls = report.stats.calls.filter(call => call.kind === 'profileStep');
  const last = calls.at(-1);
  const expected = Array.from({ length: 5 }, (_, i) => [String(i).padStart(7, '0'), 1]);
  if (!last?.done || last.unique !== 2000000 || JSON.stringify(last.common) !== JSON.stringify(expected)) {
    throw new Error('The 2M-row unique ID profile did not finish with the expected values');
  }
  const longest = Math.max(...calls.map(call => call.ms));
  if (longest > budget) throw new Error(`Inspector slice ${longest.toFixed(1)} ms exceeded ${budget} ms`);
}
if (args.check) {
  checkProfile(JSON.parse(await readFile(args.check, 'utf8')), Number(args['max-profile-ms'] ?? 50));
  console.log('Profile budget and exact results passed');
  process.exit(0);
}
if (!args.build || !args.csv || !args.out) throw new Error('Usage: node scripts/profile-scroll.mjs --build <production-directory> --csv <fixture.csv> --out <result.json> [--instrument] [--trace] [--cpu] [--search] [--dpr 2] [--browser <Chrome/Brave>]');
const root = resolve(args.build), dataset = resolve(args.csv), output = resolve(args.out);
const browser = args.browser ?? '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser';
const sleep = ms => new Promise(r => setTimeout(r, ms));
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.json': 'application/json' };
const server = createServer(async (req, res) => {
  try {
    const path = resolve(root, '.' + new URL(req.url, 'http://local').pathname.replace(/\/$/, '/index.html'));
    if (!path.startsWith(root + '/')) throw new Error('outside build');
    let data = await readFile(path);
    // Expose the existing player for measurement; no runtime source is changed.
    if (path.endsWith('/index.html')) data = Buffer.from(data.toString().replace('player = loaded;', 'player = loaded; globalThis.__scrollPlayer = player;'));
    res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' }); res.end(data);
  } catch { res.writeHead(404); res.end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const profile = await mkdtemp(join(tmpdir(), 'rowtation-scroll-browser-'));
const child = spawn(browser, ['--headless', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-sync', '--disable-background-timer-throttling', '--disable-renderer-backgrounding', 'about:blank'], { stdio: 'ignore' });
let ws, nextId = 0; const pending = new Map(), listeners = new Map();
function send(method, params = {}) {
  const id = ++nextId;
  return new Promise((resolve, reject) => { pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params })); });
}
function event(name) { return new Promise(resolve => listeners.set(name, resolve)); }
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
async function click(text) {
  const p = await evaluate(`(()=>{const el=[...document.querySelectorAll('[role="button"], .nsh-run')].find(el=>el.getAttribute('aria-label')===${JSON.stringify(text)}||el.textContent.trim()===${JSON.stringify(text)});if(!el)return null;const r=el.getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`);
  if (!p) throw new Error('Button missing: ' + text);
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', ...p, button: 'left', clickCount: 1 });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...p, button: 'left', clickCount: 1 });
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
  const doc = await send('DOM.getDocument');
  const input = await send('DOM.querySelector', { nodeId: doc.root.nodeId, selector: '#rowtation-file-input' });
  await send('DOM.setFileInputFiles', { nodeId: input.nodeId, files: [dataset] });
  await until(`document.body.textContent.includes('Read file')`); await click('Read file');
  await until(`document.body.textContent.includes('Import document') || document.body.textContent.includes('Import anyway')`);
  await click(await evaluate(`document.body.textContent.includes('Import anyway')`) ? 'Import anyway' : 'Import document');
  await until(`!!document.querySelector('[data-testid="data-grid"]')`);
  console.log('Imported', args.label ?? '', await evaluate(`document.querySelector('[data-testid="data-grid"]').textContent.slice(0,80)`));
  await sleep(1500);
  // Optional PR #8 highlight workload: 20,000 ID matches, below the search
  // limits, with a match in every 100 rows throughout the scroll protocol.
  if (args.search) {
    await click('id'); await click('Find'); await click('All'); await click('.*');
    await evaluate(`(()=>{const el=document.querySelector('[data-testid="find-input"]');(el.matches('input,textarea')?el:el.querySelector('input,textarea')).focus()})()`);
    await send('Input.insertText', { text: '00$' });
    await until(`document.body.textContent.includes('1 of 20000 matches')`);
    await until(`!!document.querySelector('[data-testid="search-highlight"]')`);
    await sleep(1500);
  }
  const setup = function (instrument) {
    const player = globalThis.__scrollPlayer;
    const grid = document.querySelector('[data-testid="data-grid"]');
    const scroll = [...grid.querySelectorAll('.nsh-scroll')].find(el => el.scrollHeight > 100000);
    const realm = [...document.querySelectorAll('iframe')].map(f => f.contentWindow).find(w => w.__nsh_browser_realm);
    const modules = realm.__nsh_browser_realm.modules;
    const stats = globalThis.__scrollStats = { calls: [], events: [], frames: [], positions: [], longtasks: [], samples: [], wheels: [], desktop: player.desktopUI, dpr: devicePixelRatio, extent: scroll.scrollHeight, viewport: scroll.clientHeight };
    globalThis.__scrollElement = scroll;
    const save = (kind, start, extra = {}) => stats.calls.push({ kind, start, ms: performance.now() - start, ...extra });
    if (instrument) {
      const Session = modules.get('rowtation/native').DocumentSession;
      for (const name of ['rows', 'profileStep']) { const fn = Session.prototype[name]; Session.prototype[name] = function (...args) { const t = performance.now(); const result = fn.apply(this, args); save(name, t, name === 'rows' ? { rows: result.rows.length, columns: result.rows[0]?.cells.length, offset: args[0].offset } : { scanned: result.scanned, total: result.total, done: !!result.profile, unique: result.profile?.uniqueCount, common: result.profile?.commonValues.map(v => [v.value, v.count]) }); return result; }; }
      const call = player.wasm.nsh_browser_call;
      player.wasm.nsh_browser_call = function (op, id) { const t = performance.now(); const result = call(op, id); if (op === 206) save('componentProp', t); return result; };
      for (const method of ['scrolled', 'scrollEnded', 'ownEnd', 'apply']) if (typeof player[method] === 'function') { const fn = player[method]; player[method] = function (...args) { const t = performance.now(); const result = fn.apply(this, args); save(method, t); return result; }; }
    }
    new PerformanceObserver(list => { stats.longtasks.push(...list.getEntries().map(e => ({ start: e.startTime, ms: e.duration }))); }).observe({ type: 'longtask' });
    scroll.addEventListener('wheel', e => stats.wheels.push({ t: performance.now(), delta: e.deltaY, mode: e.deltaMode }), { passive: true });
    scroll.addEventListener('scroll', () => stats.events.push({ t: performance.now(), y: scroll.scrollTop }));
    let previous = performance.now(), previousRaf = previous;
    const frame = now => { const wall = performance.now(); stats.frames.push({ t: wall, ms: wall - previous, rafMs: now - previousRaf, lag: wall - now }); previous = wall; previousRaf = now; const cell = [...grid.querySelectorAll('.nsh-run')].find(el => /^\d{7}$/.test(el.textContent)); let y = Number.parseFloat(cell?.style.top) || 0; for (let el = cell?.parentElement; el && el !== grid; el = el.parentElement) y += (el._frame?.[1] ?? 0) - el.scrollTop; stats.positions.push({ t: now, scroll: scroll.scrollTop, glide: scroll._glide?.at?.[1] ?? null, target: scroll._glide?.y ?? null, row: cell?.textContent, y, content: cell ? Number(cell.textContent) * (player.desktopUI ? 34 : 44) - y : null }); requestAnimationFrame(frame); };
    requestAnimationFrame(frame);
    globalThis.__scrollMark = name => { performance.mark("rowtation-" + name); stats.samples.push({ name, t: performance.now() }); };
    return { desktop: player.desktopUI, extent: scroll.scrollHeight, viewport: scroll.clientHeight, nodes: player.views.size, memory: player.wasm.memory.buffer.byteLength };
  };
  console.log('Setup', await evaluate(`(${setup.toString()})(${!!args.instrument})`));
  if (args.trace) await send('Tracing.start', { categories: 'devtools.timeline,blink.user_timing,v8', transferMode: 'ReturnAsStream' });
  if (args.cpu) { await send('Profiler.enable'); await send('Profiler.start'); }
  const point = await evaluate(`(()=>{const r=__scrollElement.getBoundingClientRect();return{x:r.x+200,y:r.y+100}})()`);
  for (let run = 0; run < 3; run++) {
    await evaluate(`__scrollMark('wheel-${run}-start')`);
    for (const delta of [160,160,120,80,50,30,18,10,6,3,1]) { await send('Input.dispatchMouseEvent', { type: 'mouseWheel', ...point, deltaX: 0, deltaY: delta }); await sleep(30); }
    await sleep(1200);
    await evaluate(`__scrollMark('wheel-${run}-end')`);
  }
  await evaluate(`__scrollMark('tail-start')`);
  for (let i = 0; i < 16; i++) { await send('Input.dispatchMouseEvent', { type: 'mouseWheel', ...point, deltaX: 0, deltaY: 0.5 }); await sleep(30); }
  await sleep(400);
  await evaluate(`__scrollMark('tail-end')`);
  await evaluate(`__scrollMark('profile-start')`);
  // Click the unique ID column to measure concurrent Inspector counting.
  await click('id');
  await sleep(50);
  for (const delta of [160,120,80,40,20,10,5,2,1]) { await send('Input.dispatchMouseEvent', { type: 'mouseWheel', ...point, deltaX: 0, deltaY: delta }); await sleep(30); }
  await sleep(2000);
  await evaluate(`__scrollMark('profile-end')`);
  const stats = await evaluate('__scrollStats');
  const phase = (start, end) => { const a = stats.samples.find(x => x.name === start).t, b = stats.samples.find(x => x.name === end).t; const frames = stats.frames.filter(x => x.t >= a && x.t <= b).map(x => x.ms).sort((a,b) => a-b); return { frames: frames.length, p50: frames[Math.floor(frames.length * .5)], p95: frames[Math.floor(frames.length * .95)], max: Math.max(...frames), over25: frames.filter(x => x > 25).length }; };
  const summary = { scrolling: phase('wheel-0-start', 'wheel-2-end'), inspector: phase('profile-start', 'profile-end'), tail: phase('tail-start', 'tail-end') };
  const tailStart = stats.samples.find(x => x.name === 'tail-start').t;
  const tailEnd = stats.samples.find(x => x.name === 'tail-end').t;
  const wheels = stats.wheels.filter(x => x.t >= tailStart && x.t <= tailEnd);
  const positions = stats.positions.filter(x => x.t >= tailStart && x.t <= wheels.at(-1).t + 50);
  const steps = [], holds = [];
  let last = positions[0];
  for (const position of positions.slice(1)) {
    if (position.content != null && last?.content != null && Math.abs(position.content - last.content) > 1) {
      steps.push(Math.abs(position.content - last.content));
      holds.push(position.t - last.t);
      last = position;
    }
  }
  summary.tailMotion = { wheelEvents: wheels.length, moves: steps.length, minStep: steps.length ? Math.min(...steps) : 0, maxStep: Math.max(0, ...steps), maxHold: Math.max(wheels.at(-1).t - (last?.t ?? tailStart), 0, ...holds) };
  for (const kind of ['rows','componentProp','profileStep']) { const calls = stats.calls.filter(x => x.kind === kind); summary[kind] = { count: calls.length, total: calls.reduce((a,c) => a+c.ms, 0), max: Math.max(0,...calls.map(x => x.ms)) }; }
  console.log('Summary', JSON.stringify(summary));
  const metrics = await send('Performance.getMetrics');
  if (args.cpu) { const { profile } = await send('Profiler.stop'); await writeFile(output + '.cpuprofile', JSON.stringify(profile)); }
  if (args.trace) { const done = event('Tracing.tracingComplete'); await send('Tracing.end'); const { stream } = await done; let text = ''; for (;;) { const r = await send('IO.read', { handle: stream }); text += r.data; if (r.eof) break; } await send('IO.close', { handle: stream }); await writeFile(output + '.trace.json', text); }
  if (args.screenshot) { const shot = await send('Page.captureScreenshot', { format: 'png' }); await writeFile(args.screenshot, Buffer.from(shot.data, 'base64')); }
  await writeFile(output, JSON.stringify({ label: args.label, browser: await send('Browser.getVersion'), instrumented: !!args.instrument, search: !!args.search, datasetBytes: (await stat(dataset)).size, summary, stats, metrics }, null, 2));
  console.log('Wrote', output);
  if (args['max-profile-ms']) checkProfile({ instrumented: !!args.instrument, stats }, Number(args['max-profile-ms']));
} finally {
  if (ws?.readyState === 1) { await send('Browser.close').catch(() => {}); ws.close(); }
  child.kill(); server.close(); await sleep(300); await rm(profile, { recursive: true, force: true }).catch(() => {});
}
