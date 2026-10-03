'use strict';
/* ============================================================
   A980 控制台 · 前端
   Rust 负责 HID 收发，这一层只管界面和状态。桥接优先用
   __TAURI__.core.invoke，退化到 internals.invoke；都不在就跑
   合成鼠标，方便在没有真机的浏览器里看界面。
   ============================================================ */

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const sleep = ms => new Promise(r => setTimeout(r, ms));

/* ---------------- Tauri 桥 ----------------
   Tauri v2 只在页面里注入 window.__TAURI_INTERNALS__（带 invoke），
   window.__TAURI__ 要配置里开 withGlobalTauri 才有。早先只探了 __TAURI__，
   打包出来的应用里必然探不到，整套界面会静默退化成合成鼠标：所有写入都
   “成功”，鼠标一动不动——所以三种形状都要认，认不到才跑合成鼠标。 */
const TA = window.__TAURI__ || {};
const INV = TA.core || TA.internals || window.__TAURI_INTERNALS__;
const IN_TAURI = !!(INV && typeof INV.invoke === 'function');

function call(cmd, args) {
  if (IN_TAURI) return INV.invoke(cmd, args || {});
  return mockCall(cmd, args || {});
}

/* ---------------- 合成鼠标（只在不带 Tauri 时用） ---------------- */
const mock = {
  connected: true,
  fw: '1.2.1', hw: '8.x', sn: '',
  sensor: { id: 1, model: 12, max: 26000, step: 50 },
  dpi: { count: 5, stage: 3, x: [400, 800, 1600, 3200, 6400], y: [400, 800, 1600, 3200, 6400] },
  rate: { usb: 1, rate: 8 },
  buttons: [1, 2, 3, 10, 11, 4, 5, 9, 8, 14].map(id => ({ id, code: 0, data: [] })),
  leds: [0, 1, 2, 3].map(id => ({
    id, kind: 2, fps: 60, effects: [0, 1, 2, 3, 4, 5, 6, 9, 11, 12, 19, 20, 29, 31],
    bright: 60 + id * 10,
    effect: { effect: 1, flag: 0, speed: 2, colors: [[255, 122, 0], [0, 122, 255]] }
  })),
  macros: [{ id: 1, name: '压枪', size: 48 }, { id: 2, name: '一键跳投', size: 32 }],
  profiles: [0, 1, 2], profile_active: 1,
  lcd_lang: 0, lcd_bright: null, battery: null, interface: 'mock [0c/1] mi_03',
  log: []
};
let mockTimer = null;

async function mockCall(cmd, args) {
  await sleep(60);
  if (cmd === 'refresh') {
    mock.log.push(new Date().toLocaleTimeString('zh-CN', { hour12: false }) + '  读了一遍状态');
    if (mock.log.length > 40) mock.log.shift();
    return clone(mock);
  }
  if (cmd === 'connect' || cmd === 'disconnect') {
    if (cmd === 'disconnect') { clearInterval(mockTimer); mockTimer = null; }
    return clone(mock);
  }
  if (cmd === 'set_dpi') {
    mock.dpi.x = args.values.map(v => v[0]);
    mock.dpi.y = args.values.map(v => v[1]);
    mock.dpi.stage = args.stage;
    mock.log.push('DPI 已写入（合成）');
    return clone(mock);
  }
  if (cmd === 'set_rate') {
    mock.rate.rate = args.rate;
    mock.log.push('回报率已改成 ' + args.rate + ' Hz（合成）');
    return clone(mock);
  }
  if (cmd === 'set_profile') {
    mock.profile_active = args.id;
    mock.log.push('已切到配置 ' + args.id + '（合成）');
    return clone(mock);
  }
  if (cmd === 'set_led_effect' || cmd === 'set_led_bright') {
    const l = mock.leds.find(l => l.id === args.region);
    if (l && cmd === 'set_led_effect') l.effect = { effect: args.effect, flag: args.flag, speed: args.speed, colors: args.colors };
    if (l && cmd === 'set_led_bright') l.bright = args.bright;
    mock.log.push((cmd === 'set_led_bright' ? '灯区亮度 ' + args.bright + '（合成）' : '灯效 ' + args.effect + '（合成）'));
    return clone(mock);
  }
  if (cmd === 'set_button') {
    const b = mock.buttons.find(b => b.id === args.btn);
    if (b) { b.code = args.code; b.data = args.data; }
    mock.log.push('按键 ' + args.btn + ' 改成 ' + args.code + '（合成）');
    return clone(mock);
  }
  if (cmd === 'set_lcd_lang') {
    mock.lcd_lang = args.lang;
    mock.log.push('屏幕语言已改成 ' + args.lang + '（合成）');
    return clone(mock);
  }
  if (cmd === 'factory_reset') {
    mock.log.push('已下发恢复出厂（合成）');
    return clone(mock);
  }
  if (cmd === 'forget_interface') {
    mock.log.push('已忘掉上次的接口（合成）');
    return clone(mock);
  }
  return clone(mock);
}

function clone(o) { return JSON.parse(JSON.stringify(o)); }

/* ---------------- 常量表 ---------------- */
const RATES = [[64, 125], [32, 250], [16, 500], [8, 1000], [4, 2000], [2, 4000], [1, 8000]];
const EFFECTS = {
  0: '关闭', 1: '常亮', 2: '呼吸', 3: '霓虹', 4: '激击', 5: '波浪', 6: '雨滴', 7: '聚集', 8: '涟漪',
  9: '跑马灯', 10: '旋转', 11: '星空', 12: '升温', 19: '圈速', 20: '彩虹', 21: '光波', 22: '稳流',
  23: '启动', 24: '区域激击', 25: '线性激击', 26: '瀑布', 27: '扫描', 28: '心跳', 29: '流光',
  30: '月光呼吸', 31: '星光呼吸'
};
const BTN_NAMES = {
  1: '左键', 2: '右键', 3: '中键（滚轮按下）', 4: '侧键 · 后', 5: '侧键 · 前',
  8: 'DPI － 键', 9: 'DPI ＋ 键', 10: '滚轮上', 11: '滚轮下', 14: '狙击键', 15: '底部模式键',
  16: '按键 16', 17: '按键 17', 18: '按键 18', 19: '按键 19'
};
const LANGS = [[0, '中文'], [1, 'English']];
const MEDIA = [[181, '下一曲'], [182, '上一曲'], [183, '停止'], [205, '播放 / 暂停'], [226, '静音'], [233, '音量 +'], [234, '音量 −']];

function fnDesc(fn, d) {
  d = d || [];
  switch (fn) {
    case 0: return '禁用';
    case 1: return '鼠标 · ' + ({ 1: '左键', 2: '右键', 3: '中键', 4: '侧键后', 5: '侧键前' }[d[0]] || '#' + d[0]);
    case 2: return '双击';
    case 4: return '滚轮 · ' + (d[0] === 255 ? '下' : d[0] === 1 ? '上' : '±' + ((d[0] << 24) >> 24));
    case 8: return '多媒体 · ' + (MEDIA.find(m => m[0] === ((d[0] << 8) | d[1])) || [0, '#' + ((d[0] << 8) | d[1])])[1];
    case 12: { const m = S.macros.find(m => m.id === d[0]); return '宏 · ' + (m ? m.name : '#' + d[0]); }
    case 16: return 'DPI 档位 +';
    case 17: return 'DPI 档位 −';
    /* 档位号就是从 1 起数的，和写入侧 `18|n` 一致 */
    case 18: return '切换到第 ' + (d[0] || 1) + ' 档';
    case 22: return '狙击 ' + ((d[0] << 8) | d[1]) + ' DPI';
    case 30: return '灯效切换';
    case 44: return '恢复出厂';
    case 47: return 'Win 锁';
    default: return '功能 #' + fn;
  }
}

/* ---------------- 状态 ---------------- */
let S = blank();
let ui = { page: 'ov', led: -1, speed: 2, colors: [], effect: 0, flag: 0, dpiDraft: null };

function blank() {
  return {
    connected: false, busy: false, interface: '', fw: '', hw: '', sn: '',
    sensor: { id: 1, model: 12, max: 26000, step: 50 },
    dpi: { count: 5, stage: 3, x: [], y: [] },
    rate: { usb: 1, rate: 8 },
    buttons: [], leds: [], macros: [], profiles: [], profile_active: 0,
    lcd_lang: null, lcd_bright: null, battery: null, problem: null, log: []
  };
}

/* ---------------- 小工具 ---------------- */
function toast(msg, kind) {
  const t = document.createElement('div');
  t.className = 'toast' + (kind ? ' ' + kind : '');
  t.textContent = msg;
  $('#toasts').appendChild(t);
  requestAnimationFrame(() => t.classList.add('on'));
  setTimeout(() => {
    t.classList.remove('on');
    setTimeout(() => t.remove(), 250);
  }, kind === 'err' ? 3200 : 1800);
}

/* 写命令失败要把原因说出来：固件答状态 0x03 时设备不会变，
   界面还弹"已写入"就是在骗人 */
function err(e) {
  const m = e && (e.message || e.payload || e);
  return (typeof m === 'string' && m) || '未知错误';
}
async function tryWrite(name, fn) {
  try {
    await fn();
    return true;
  } catch (e) {
    toast(name + '没写进去：' + err(e), 'err');
    return false;
  }
}

function sheet(title, items) {
  return new Promise(resolve => {
    $('#sheetTitle').textContent = title;
    const body = $('#sheetBody');
    body.className = 'sheetbody';
    body.innerHTML = '';
    items.forEach(it => {
      const b = document.createElement('button');
      b.className = 'sheetitem' + (it.danger ? ' danger' : '') + (it.on ? ' on' : '');
      b.textContent = it.text;
      if (it.val) {
        const v = document.createElement('span');
        v.className = 'val';
        v.textContent = it.val;
        b.appendChild(v);
      }
      b.onclick = () => { closeSheet(); resolve(it); };
      body.appendChild(b);
    });
    $('#sheet').classList.add('on');
    $('#scrim').classList.add('on', 'sel');
  });
}
function closeSheet() {
  $('#sheet').classList.remove('on');
  $('#scrim').classList.remove('on', 'sel');
}
$('#sheetCancel').onclick = closeSheet;
$('#scrim').onclick = closeSheet;

/* ---------------- 标签栏 ---------------- */
$$('.tab').forEach(b => (b.onclick = () => {
  $$('.tab').forEach(x => x.classList.toggle('on', x === b));
  const p = b.dataset.page;
  ui.page = p;
  $$('.page').forEach(s => s.classList.toggle('on', s.dataset.page === p));
  $('#pages').scrollTop = 0;
  setTitle(p);
}));

const TITLES = { ov: 'A980', dpi: '性能', led: '灯光', sys: '系统' };
function setTitle(p) {
  $('#bigTitle').textContent = TITLES[p] || 'A980';
  $('#sub').textContent = p === 'sys'
    ? '排障、维护和日志'
    : IN_TAURI ? '插上鼠标就会自动连上' : '合成鼠标模式（未在桌面应用里运行）';
}

/* ---------------- 概览 ---------------- */
function renderOv() {
  $('#ovState').textContent = S.connected ? '已连接' : '未连接';
  $('#ovState').className = 'val' + (S.connected ? '' : ' dim');
  $('#ovIface').textContent = S.interface || '—';
  $('#ovFw').textContent = S.fw || '—';
  $('#ovHw').textContent = S.hw || '—';
  $('#ovSn').textContent = S.sn ? S.sn.replace(/(FF ){15}FF/, '未写入') : '未写入';
  $('#ovSensor').textContent = '型号 ' + S.sensor.model;
  $('#ovStep').textContent = S.sensor.max + ' / ' + S.sensor.step;
  $('#ovDpi').textContent = (S.dpi.x.length && S.dpi.stage >= 1)
    ? S.dpi.x[S.dpi.stage - 1] + ' × ' + S.dpi.y[S.dpi.stage - 1] + '（第 ' + S.dpi.stage + ' 档）'
    : '—';
  $('#ovRate').textContent = rateName(S.rate.rate) + ' Hz';
  $('#ovProfile').textContent = S.profiles.length
    ? '配置 ' + S.profile_active + ' / 共 ' + S.profiles.length + ' 个'
    : '—';
  $('#ovBat').textContent = S.battery != null ? S.battery + '%' : '有线 · 无电池';

  const bg = $('#btnGroup');
  bg.innerHTML = '';
  if (!S.buttons.length) {
    bg.innerHTML = '<div class="empty">' + (S.connected ? '没读到按键' : '连接后可见') + '</div>';
  } else {
    S.buttons.forEach(b => {
      const row = document.createElement('button');
      row.className = 'row sheetitem';
      row.innerHTML = '<span class="lab">' + (BTN_NAMES[b.id] || '按键 ' + b.id) + '</span>' +
        '<span class="val">' + fnDesc(b.code, b.data) + '</span>';
      row.onclick = () => pickFunction(b);
      bg.appendChild(row);
    });
  }
}

/* ---------------- 性能 ---------------- */
function rateName(rate) {
  const f = RATES.find(r => r[0] === rate);
  return f ? f[1] : '?';
}
function renderDpi() {
  $('#rateNow').textContent = rateName(S.rate.rate) + ' Hz';
  const seg = $('#rateSeg');
  seg.innerHTML = '';
  RATES.forEach(([, hz]) => {
    const b = document.createElement('button');
    b.textContent = hz;
    b.className = (S.rate.rate !== 0 && RATES.find(r => r[1] === hz)[0] === S.rate.rate) ? 'on' : '';
    b.onclick = async () => {
      const it = RATES.find(r => r[1] === hz);
      if (await tryWrite('回报率', () => call('set_rate', { usb: S.rate.usb, rate: it[0] }))) {
        toast('回报率已改成 ' + it[1] + ' Hz', 'ok');
      }
      pull();
    };
    seg.appendChild(b);
  });

  const box = $('#dpiBox');
  box.innerHTML = '';
  const draft = ui.dpiDraft || S.dpi;
  const max = S.sensor.max || 26000;
  const step = S.sensor.step || 50;
  for (let i = 0; i < draft.count; i++) {
    const cur = draft.stage === i + 1;
    const row = document.createElement('div');
    row.className = 'stage' + (cur ? ' cur' : '');
    row.innerHTML =
      '<span class="stg"><b></b>档位 ' + (i + 1) + '</span>' +
      '<span class="xy">X <input class="num" type="number" data-i="' + i + '" data-xy="x" min="50" max="' + max + '" step="' + step + '" value="' + draft.x[i] + '"></span>' +
      '<span class="xy">Y <input class="num" type="number" data-i="' + i + '" data-xy="y" min="50" max="' + max + '" step="' + step + '" value="' + draft.y[i] + '"></span>' +
      (cur ? '<span class="val">使用中</span>' : '<button class="btn sm" data-stage="' + (i + 1) + '">设为当前</button>');
    box.appendChild(row);
  }
  $$('input', box).forEach(inp => {
    inp.oninput = () => {
      const i = +inp.dataset.i;
      const link = $('#dpiLink').classList.contains('on');
      if (inp.dataset.xy === 'x') {
        draft.x[i] = clamp(+inp.value, 50, max);
        if (link) draft.y[i] = draft.x[i];
      } else {
        draft.y[i] = clamp(+inp.value, 50, max);
        if (link) draft.x[i] = draft.y[i];
      }
      if (link) $$('[data-i="' + i + '"]', box).forEach(x => (x.value = draft.x[i]));
    };
  });
  $$('[data-stage]', box).forEach(b => {
    b.onclick = async () => {
      const stage = +b.dataset.stage;
      draft.stage = stage;
      if (await tryWrite('DPI', () =>
        call('set_dpi', { values: draft.x.map((x, i) => [x, draft.y[i]]), stage }))) {
        toast('已切到第 ' + stage + ' 档');
      }
      pull();
    };
  });

  const prf = $('#prfSeg');
  prf.innerHTML = '';
  (S.profiles.length ? S.profiles : [0, 1, 2]).forEach(id => {
    const b = document.createElement('button');
    b.textContent = '配置 ' + id;
    b.className = id === S.profile_active ? 'on' : '';
    b.onclick = async () => {
      if (await tryWrite('切换配置', () => call('set_profile', { id }))) {
        toast('已切到配置 ' + id, 'ok');
      }
      pull();
    };
    prf.appendChild(b);
  });
  $('#prfNote').textContent = S.profiles.length
    ? '配置里存的是整套键位和灯效，切换不会改这里显示的值。'
    : '';
}

function clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, Math.round(v / 50) * 50)); }

function draftDpi() {
  return ui.dpiDraft || S.dpi;
}

/* ---------------- 灯光 ---------------- */
function renderLed() {
  if (!S.leds.length) {
    $('#ledSeg').innerHTML = '<span class="val" style="padding:12px 14px;color:var(--sub)">' +
      (S.connected ? '没读到灯区' : '连接后可见') + '</span>';
    $('#effChips').innerHTML = '';
    $('#colorBox').innerHTML = '';
    return;
  }
  if (ui.led < 0) {
    ui.led = S.leds[0].id;
    const l = S.leds[0];
    ui.effect = l.effect.effect;
    ui.speed = l.effect.speed;
    ui.colors = l.effect.colors.map(c => c.join(','));
    ui.flag = l.effect.flag;
  }
  const seg = $('#ledSeg');
  seg.innerHTML = '';
  S.leds.forEach(l => {
    const b = document.createElement('button');
    b.textContent = '灯区 ' + l.id;
    b.className = l.id === ui.led ? 'on' : '';
    b.onclick = () => {
      ui.led = l.id;
      ui.effect = l.effect.effect;
      ui.speed = l.effect.speed;
      ui.colors = l.effect.colors.map(c => c.join(','));
      ui.flag = l.effect.flag;
      renderLed();
    };
    seg.appendChild(b);
  });
  const cur = S.leds.find(l => l.id === ui.led) || S.leds[0];

  const chips = $('#effChips');
  chips.innerHTML = '';
  const list = cur.effects.length ? cur.effects : [0, 1, 2, 3, 4, 5, 6, 9, 11, 12, 19, 20, 29, 31];
  list.forEach(e => {
    const b = document.createElement('button');
    b.className = 'chip' + (e === ui.effect ? ' on' : '');
    b.textContent = EFFECTS[e] || ('#' + e);
    b.onclick = () => { ui.effect = e; renderLed(); };
    chips.appendChild(b);
  });

  $('#ledSpeed').value = ui.speed;
  $('#ledSpeedVal').textContent = ui.speed;
  $('#ledBright').value = cur.bright;
  $('#ledBrightVal').textContent = Math.round(cur.bright / 255 * 100) + '%';

  const cb = $('#colorBox');
  cb.innerHTML = '';
  ui.colors.forEach((c, i) => {
    const d = document.createElement('label');
    d.className = 'swatch on';
    d.style.background = 'rgb(' + c + ')';
    d.innerHTML = '<input type="color" value="#' + c.split(',').map(x => (+x).toString(16).padStart(2, '0')).join('') + '">';
    /* × 角标：label 里子元素的点击会冒泡到 input，必须挡掉，否则点删除弹取色器 */
    const del = document.createElement('span');
    del.className = 'swdel';
    del.textContent = '×';
    del.title = '删掉这色';
    del.onclick = ev => {
      ev.preventDefault();
      ev.stopPropagation();
      ui.colors.splice(i, 1);
      renderLed();
      saveLed();
    };
    d.appendChild(del);
    d.querySelector('input').oninput = ev => {
      const hex = ev.target.value;
      const rgb = [1, 3, 5].map(k => parseInt(hex.substr(k, 2), 16));
      ui.colors[i] = rgb.join(',');
      d.style.background = 'rgb(' + ui.colors[i] + ')';
    };
    d.querySelector('input').onchange = saveLed;
    cb.appendChild(d);
  });
  if (ui.colors.length < 7) {
    const add = document.createElement('button');
    add.className = 'addcolor';
    add.textContent = '+';
    add.onclick = () => { ui.colors.push('255,255,255'); renderLed(); };
    cb.appendChild(add);
  }
}

function saveLed() {
  /* 把 tryWrite 的结果交出去，调用方才知道该不该弹"已写入" */
  return tryWrite('灯效', () => call('set_led_effect', {
    region: ui.led,
    effect: ui.effect,
    flag: ui.flag,
    speed: +$('#ledSpeed').value,
    colors: ui.colors.map(c => c.split(',').map(x => +x))
  }));
}

/* ---------------- 系统 ---------------- */
function renderSys() {
  const seg = $('#langSeg');
  seg.innerHTML = '';
  LANGS.forEach(([v, name]) => {
    const b = document.createElement('button');
    b.textContent = name;
    b.className = S.lcd_lang === v ? 'on' : '';
    b.onclick = async () => {
      if (await tryWrite('屏幕语言', () => call('set_lcd_lang', { lang: v }))) {
        toast('屏幕语言已改', 'ok');
      }
      pull();
    };
    seg.appendChild(b);
  });
  const row = $('#brightRow');
  if (S.lcd_bright != null) {
    row.classList.remove('hidden');
    $('#lcdBright').value = S.lcd_bright;
    $('#lcdBrightVal').textContent = S.lcd_bright;
  } else {
    row.classList.add('hidden');
  }
  $('#memIface').textContent = S.interface || '—';
  $('#logBox').innerHTML = (S.log && S.log.length)
    ? S.log.map(l => esc(l)).join('\n')
    : '还没有日志';
}

function esc(s) {
  return String(s).replace(/[&<>]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
}

/* ---------------- 按键功能编辑 ---------------- */
function pickFunction(b) {
  const cur = { code: b.code, data: b.data.slice() };
  const sameAs = (code, data) => code === cur.code &&
    data.every((v, i) => v === (cur.data[i] || 0));
  const sniper = dpi => {
    const hi = dpi >> 8, lo = dpi & 255;
    return { code: 22, data: [hi, lo, hi, lo, 0] };
  };
  /* 列表里没有“照现在这样”那一项：选中的项照亮点出来，再点一次会提示“没改”，
     不会往设备里写一遍现值 */
  const items = [
    { text: '禁用', on: sameAs(0, [0]), code: 0, data: [0, 0, 0, 0, 0] },
    { text: '左键', on: sameAs(1, [1]), code: 1, data: [1, 0, 0, 0, 0] },
    { text: '右键', on: sameAs(1, [2]), code: 1, data: [2, 0, 0, 0, 0] },
    { text: '中键', on: sameAs(1, [3]), code: 1, data: [3, 0, 0, 0, 0] },
    { text: '侧键 · 后', on: sameAs(1, [4]), code: 1, data: [4, 0, 0, 0, 0] },
    { text: '侧键 · 前', on: sameAs(1, [5]), code: 1, data: [5, 0, 0, 0, 0] },
    { text: '双击', on: sameAs(2, [0]), code: 2, data: [0, 0, 0, 0, 0] },
    { text: '滚轮上', on: sameAs(4, [1]), code: 4, data: [1, 0, 0, 0, 0] },
    { text: '滚轮下', on: sameAs(4, [255]), code: 4, data: [255, 0, 0, 0, 0] },
    { text: 'DPI 档位 +', on: sameAs(16, [0]), code: 16, data: [0, 0, 0, 0, 0] },
    { text: 'DPI 档位 −', on: sameAs(17, [0]), code: 17, data: [0, 0, 0, 0, 0] }
  ];
  for (let n = 1; n <= 5; n++) {
    items.push({ text: '切换到第 ' + n + ' 档', on: sameAs(18, [n]), code: 18, data: [n, 0, 0, 0, 0] });
  }
  [400, 800, 1600, 3200].forEach(d => {
    const s = sniper(d);
    items.push({ text: '狙击键 · ' + d + ' DPI', on: sameAs(s.code, s.data), code: s.code, data: s.data });
  });
  MEDIA.forEach(m => items.push({
    text: '多媒体 · ' + m[1], code: 8,
    data: [m[0] >> 8 & 255, m[0] & 255, 0, 0, 0]
  }));
  S.macros.forEach(m => items.push({
    text: '宏 · ' + m.name, code: 12, data: [m.id, 0, 0, 0, 0]
  }));
  items.push({ text: '灯效切换', on: sameAs(30, [0]), code: 30, data: [0, 0, 0, 0, 0] });
  items.push({ text: 'Win 键锁定', on: sameAs(47, [0]), code: 47, data: [0, 0, 0, 0, 0] });
  items.push({ text: '恢复出厂', on: sameAs(44, [0]), code: 44, data: [0, 0, 0, 0, 0] });

  sheet(BTN_NAMES[b.id] || '按键 ' + b.id, items).then(async it => {
    if (!it || it.code == null) return;
    if (sameAs(it.code, it.data)) { toast('没改'); return; }
    if (await tryWrite(BTN_NAMES[b.id] || '按键 ' + b.id,
      () => call('set_button', { btn: b.id, code: it.code, data: it.data }))) {
      toast((BTN_NAMES[b.id] || '按键') + ' 已改成 ' + it.text, 'ok');
      pull();
    }
  });
}

/* ---------------- 渲染入口 ---------------- */
function render() {
  $('#pill').className = 'pill' + (S.connected ? ' on' : '');
  $('#pillTxt').textContent = S.connected ? '已连接 · 固件 ' + (S.fw || '?') : '未连接';
  $('#sub').textContent = S.problem
    || (S.connected ? S.interface : (IN_TAURI ? '插上鼠标就会自动连上' : '合成鼠标模式（未在桌面应用里运行）'));
  $('#btnConn').textContent = S.connected ? '断开连接' : '连接鼠标';
  renderOv();
  renderDpi();
  renderLed();
  renderSys();
}

function apply(snap) {
  /* 三秒一次的轮询不能把正在编辑的草稿冲掉：设备那边的值只要没变，就保留草稿。
     写入成功后设备返回值会变，草稿自然清掉。 */
  const key = JSON.stringify([snap.dpi, snap.rate, snap.profile_active]);
  const changed = key !== lastDev;
  S = Object.assign(blank(), snap);
  if (changed) ui.dpiDraft = null;
  lastDev = key;
  /* 灯区只挑一次，轮询不该把用户选中的灯区弹回第一个 */
  if (ui.led >= 0 && !S.leds.some(l => l.id === ui.led)) ui.led = -1;
  render();
}
let lastDev = null;

async function pull() {
  try {
    const snap = await call('refresh', {});
    if (snap) apply(snap);
    return true;
  } catch (e) {
    toast('读状态失败：' + err(e), 'err');
    return false;
  }
}

/* ---------------- 事件 ---------------- */
let manualDisconnect = false;
$('#btnConn').onclick = async () => {
  if (S.connected) {
    manualDisconnect = true;
    if (await tryWrite('断开', () => call('disconnect', {}))) {
      toast('已断开，鼠标已拔出或关闭', 'ok');
    }
  } else {
    manualDisconnect = false;
    toast('正在探测接口…');
    const s = await call('connect', {});
    if (s && s.problem) toast('连不上：' + s.problem, 'err');
    else toast('已连上', 'ok');
  }
  pull();
};
$('#btnRefresh').onclick = async () => {
  const btn = $('#btnRefresh');
  const prev = btn.textContent;
  btn.textContent = '正在读取…';
  btn.disabled = true;
  const ok = await pull();
  btn.textContent = ok ? '已读取' : '读取失败';
  setTimeout(() => { btn.textContent = prev; btn.disabled = false; }, 1200);
  if (ok) toast('已读取当前状态', 'ok');
};
$('#btnDpiSave').onclick = async () => {
  const d = draftDpi();
  if (await tryWrite('DPI', () =>
    call('set_dpi', { values: d.x.map((x, i) => [x, d.y[i]]), stage: d.stage }))) {
    toast('DPI 已写入', 'ok');
  }
  pull();
};
$('#dpiLink').onclick = () => $('#dpiLink').classList.toggle('on');
$('#ledSpeed').oninput = () => ($('#ledSpeedVal').textContent = $('#ledSpeed').value);
$('#ledSpeed').onchange = () => { ui.speed = +$('#ledSpeed').value; saveLed(); };
$('#ledBright').oninput = () => ($('#ledBrightVal').textContent = Math.round(+$('#ledBright').value / 255 * 100) + '%');
$('#ledBright').onchange = async () => {
  if (await tryWrite('亮度', () => call('set_led_bright', { region: ui.led, bright: +$('#ledBright').value }))) {
    toast('亮度已改', 'ok');
  }
};
$('#btnLedSave').onclick = async () => {
  if (await saveLed()) toast('灯效已写入', 'ok');
  pull();
};
$('#lcdBright').oninput = () => ($('#lcdBrightVal').textContent = $('#lcdBright').value);
$('#lcdBright').onchange = async () => {
  if (await tryWrite('屏幕亮度', () => call('set_lcd_bright', { bright: +$('#lcdBright').value }))) {
    toast('屏幕亮度已改', 'ok');
  }
  pull();
};
$('#btnForget').onclick = async () => {
  await call('forget_interface', {});
  toast('已重新探测');
  pull();
};
$('#btnReset').onclick = async () => {
  const it = await sheet('恢复出厂设置？', [
    { text: '确认恢复出厂', danger: true },
    { text: '取消' }
  ]);
  if (it && it.danger) {
    if (await tryWrite('恢复出厂', () => call('factory_reset', {}))) {
      toast('已下发，鼠标需要重启一下');
    }
    pull();
  }
};
document.addEventListener('keydown', e => { if (e.key === 'Escape') closeSheet(); });

/* ---------------- 启动 ---------------- */
setTitle('ov');
render();
if (IN_TAURI) {
  pull();
  setInterval(pull, 3000);
} else {
  apply(mock);
             }
