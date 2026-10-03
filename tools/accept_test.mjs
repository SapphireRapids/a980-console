// Live-hardware acceptance for the rebuilt Tauri shell.
// Verifies: real bridge (not synthetic), real snapshot from the mouse,
// same-value DPI write lands, UI renders real data.
const fs = await import('node:fs/promises');
const list = await (await fetch('http://127.0.0.1:9222/json/list')).json();
const page = list.find(p => p.type === 'page');
if (!page) { console.log('FAIL no page at debug port'); process.exit(1); }
const ws = new WebSocket(page.webSocketDebuggerUrl);
let id = 0; const pending = new Map();
ws.onmessage = (m) => { const msg = JSON.parse(m.data); if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); } };
await new Promise(r => ws.onopen = r);
const send = (method, params) => new Promise(res => { const i = ++id; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
const evalJs = async (expr) => {
  const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.result?.exceptionDetails) return 'EXC ' + JSON.stringify(r.result.exceptionDetails.exception?.description || r.result.exceptionDetails.text);
  return r.result?.result?.value;
};
const invoke = async (cmd, args = {}) => evalJs(`(async () => {
  try { return 'OK ' + JSON.stringify(await window.__TAURI_INTERNALS__.invoke('${cmd}', ${JSON.stringify(args)})); }
  catch (e) { return 'ERR ' + (e && (e.message || e.payload || JSON.stringify(e))); }
})()`);

console.log('== bridge ==');
console.log(await evalJs(`JSON.stringify({internals: typeof window.__TAURI_INTERNALS__})`));

console.log('== refresh ==');
const refresh = await invoke('refresh');
console.log(refresh);

console.log('== UI rendered ==');
console.log(await evalJs(`JSON.stringify({title: document.title, pill: (document.querySelector('#pillTxt')||{}).textContent, sub: (document.querySelector('#sub')||{}).textContent, btns: document.querySelectorAll('#pages .row').length})`));

// Same-value DPI write: take the snapshot, write it straight back, expect OK.
console.log('== dpi writeback ==');
const snap = await evalJs(`(async () => JSON.stringify(await window.__TAURI_INTERNALS__.invoke('refresh')))()`);
const s = JSON.parse(snap.replace(/^OK /, ''));
console.log(await invoke('set_dpi', { values: s.dpi.x.map((x, i) => [x, s.dpi.y[i]]), stage: s.dpi.stage }));

console.log('== disconnect ==');
console.log(await invoke('disconnect'));

console.log('== reconnect ==');
console.log(await invoke('connect'));
const after = await evalJs(`(async () => { try { return 'OK ' + JSON.stringify(await window.__TAURI_INTERNALS__.invoke('refresh')); } catch(e){ return 'ERR ' + e; } })()`);
const st = JSON.parse(after.replace(/^OK /, ''));
console.log('connected:', st.connected, 'iface:', st.interface, 'fw:', st.fw, 'problem:', st.problem, 'dpi stage:', st.dpi?.stage, 'log tail:', (st.log || []).slice(-3));
ws.close();
