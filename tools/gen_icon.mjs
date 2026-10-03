// Generate flat app icons for the A980 console: PNG set (512/256/128/32) + multi-size ICO.
// Node built-ins only (zlib). Superellipse tile + SDF glyphs, 3x3 supersampling AA,
// every output size rendered natively (not downscaled).
//
// v3 design: "dial" metaphor -- no device drawing. A flat white knob with a pointer
// set to stage 4 of 5; five tick marks arc above it, the active one amber. The DPI
// five-stage select is the console's core action, so the icon IS the action.
import { writeFileSync, mkdirSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { join } from "node:path";

const C = {
  tile:   [28, 28, 35],    // #1C1C23 near-black field (ChatGPT-style)
  ring:   [38, 38, 48],    // #262630 inner squircle
  knob:   [255, 255, 255], // #FFFFFF knob face
  tick:   [200, 200, 210], // #C8C8D2 idle ticks
  accent: [255, 255, 255], // #FFFFFF active tick
};

const N = 5; // superellipse exponent: 5 reads as an iOS squircle
const G = {
  outer: 0.500, outerN: N,
  inner: 0.430, innerN: N,
  knobR: 0.205,                          // knob face
  ptrR0: 0.060, ptrR1: 0.150, ptrW: 0.028, // pointer capsule (punched in tile color)
  tickR0: 0.275, tickR1: 0.335, tickW: 0.022, // five scale ticks, arc -60..+60 deg
  active: 3,                              // stage 4 of 5 (matches the real mouse)
};

function inSquircle(x, y, r, n) {
  const dx = (x - 0.5) / r, dy = (y - 0.5) / r;
  const ax = Math.abs(dx), ay = Math.abs(dy);
  if (ax >= 1 || ay >= 1) return false;
  return Math.pow(ax, n) + Math.pow(ay, n) <= 1;
}
const dist2 = (x, y, cx, cy) => Math.hypot(x - cx, y - cy);
function segDist(px, py, x1, y1, x2, y2) {
  const dx = x2 - x1, dy = y2 - y1;
  const t = Math.max(0, Math.min(1, ((px - x1) * dx + (py - y1) * dy) / (dx * dx + dy * dy)));
  return Math.hypot(px - (x1 + t * dx), py - (y1 + t * dy));
}
// y-down coords: 0 deg = 12 o'clock, positive = clockwise
const dirOf = (deg) => {
  const th = (deg * Math.PI) / 180;
  return [Math.sin(th), -Math.cos(th)];
};

function pixelColor(x, y) {
  if (!inSquircle(x, y, G.outer, G.outerN)) return [0, 0, 0, 0];
  const field = inSquircle(x, y, G.inner, G.innerN) ? C.ring : C.tile;
  for (let i = 0; i < 5; i++) {
    const [dx, dy] = dirOf((i - 2) * 30);
    if (segDist(x, y, 0.5 + G.tickR0 * dx, 0.5 + G.tickR0 * dy, 0.5 + G.tickR1 * dx, 0.5 + G.tickR1 * dy) <= G.tickW)
      return i === G.active ? [...C.accent, 255] : [...C.tick, 255];
  }
  if (dist2(x, y, 0.5, 0.5) <= G.knobR) {
    const [dx, dy] = dirOf((G.active - 2) * 30);
    if (segDist(x, y, 0.5 + G.ptrR0 * dx, 0.5 + G.ptrR0 * dy, 0.5 + G.ptrR1 * dx, 0.5 + G.ptrR1 * dy) <= G.ptrW)
      return [...C.tile, 255];
    return [...C.knob, 255];
  }
  return [...field, 255];
}

function render(S) {
  const px = Buffer.alloc(S * S * 4, 0);
  for (let j = 0; j < S; j++)
    for (let i = 0; i < S; i++) {
      let r = 0, g = 0, b = 0, a = 0;
      for (let sy = 0; sy < 3; sy++)
        for (let sx = 0; sx < 3; sx++) {
          const c = pixelColor((i + (sx + 0.5) / 3) / S, (j + (sy + 0.5) / 3) / S);
          r += (c[0] * c[3]) / 255; g += (c[1] * c[3]) / 255; b += (c[2] * c[3]) / 255; a += c[3];
        }
      const o = (j * S + i) * 4;
      px[o] = Math.round(r / 9); px[o + 1] = Math.round(g / 9);
      px[o + 2] = Math.round(b / 9); px[o + 3] = Math.round(a / 9);
    }
  return px;
}

const crcT = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1; t[n] = c; }
  return t;
})();
const crc32 = (buf) => { let c = ~0; for (const byte of buf) c = crcT[(c ^ byte) & 0xff] ^ (c >>> 8); return ~c >>> 0; };
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
function png(S, rgba) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(S, 0); ihdr.writeUInt32BE(S, 4);
  ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  const stride = S * 4 + 1;
  const raw = Buffer.alloc(S * stride);
  for (let y = 0; y < S; y++) {
    raw[y * stride] = 1; // Sub filter
    for (let x = 0; x < S * 4; x++) {
      const p = y * S * 4 + x, left = x >= 4 ? rgba[p - 4] : 0;
      raw[y * stride + 1 + x] = (rgba[p] - left) & 255;
    }
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
    chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 9 })), chunk("IEND", Buffer.alloc(0)),
  ]);
}
function ico(entries) {
  const n = entries.length;
  const head = Buffer.alloc(6 + n * 16);
  head.writeUInt16LE(0, 0); head.writeUInt16LE(1, 2); head.writeUInt16LE(n, 4);
  let off = head.length;
  entries.forEach((e, i) => {
    const p = 6 + i * 16;
    head[p] = e.S >= 256 ? 0 : e.S; head[p + 1] = e.S >= 256 ? 0 : e.S;
    head[p + 2] = 0; head[p + 3] = 0; head.writeUInt16LE(1, p + 4); head.writeUInt16LE(32, p + 6);
    head.writeUInt32LE(e.png.length, p + 8); head.writeUInt32LE(off, p + 12);
    off += e.png.length;
  });
  return Buffer.concat([head, ...entries.map((e) => e.png)]);
}

const outDir = process.argv[2];
if (!outDir) { console.error("usage: node gen_icon.mjs <outdir> [singleSize]"); process.exit(1); }
mkdirSync(outDir, { recursive: true });
if (process.argv[3]) {
  const S = Number(process.argv[3]);
  writeFileSync(join(outDir, `icon-${S}.png`), png(S, render(S)));
  console.log(`wrote ${S}px to`, outDir);
} else {
  writeFileSync(join(outDir, "icon.png"), png(512, render(512)));
  writeFileSync(join(outDir, "128x128@2x.png"), png(256, render(256)));
  writeFileSync(join(outDir, "128x128.png"), png(128, render(128)));
  writeFileSync(join(outDir, "32x32.png"), png(32, render(32)));
  const icoPngs = [16, 24, 32, 48, 64, 128, 256].map((S) => ({ S, png: png(S, render(S)) }));
  writeFileSync(join(outDir, "icon.ico"), ico(icoPngs));
  console.log("wrote icon set to", outDir);
}
