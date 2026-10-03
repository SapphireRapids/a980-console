// PNG -> ASCII 预览 / 调色板转储，仅依赖 node 内置模块
import { readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";

function decodePng(buf) {
  let pos = 8, w = 0, h = 0, bitDepth = 8, colorType = 6;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos); pos += 4;
    const type = buf.toString("ascii", pos, pos + 4); pos += 4;
    const data = buf.subarray(pos, pos + len); pos += len + 4;
    if (type === "IHDR") {
      w = data.readUInt32BE(0); h = data.readUInt32BE(4);
      bitDepth = data[8]; colorType = data[9];
    } else if (type === "IDAT") idat.push(data);
  }
  if (bitDepth !== 8 || (colorType !== 6 && colorType !== 2)) throw new Error(`unsupported png bd=${bitDepth} ct=${colorType}`);
  const ch = colorType === 6 ? 4 : 3;
  const raw = inflateSync(Buffer.concat(idat));
  const stride = w * ch;
  const out = Buffer.alloc(h * stride);
  let prev = Buffer.alloc(stride);
  for (let y = 0, p = 0; y < h; y++) {
    const ft = raw[p++];
    const line = raw.subarray(p, p + stride); p += stride;
    const cur = out.subarray(y * stride, (y + 1) * stride);
    for (let i = 0; i < stride; i++) {
      const a = i >= ch ? cur[i - ch] : 0, b = prev[i], c = i >= ch ? prev[i - ch] : 0;
      let v = line[i];
      if (ft === 1) v += a; else if (ft === 2) v += b; else if (ft === 3) v += (a + b) >> 1;
      else if (ft === 4) { const pp = a + b - c, pa = Math.abs(pp - a), pb = Math.abs(pp - b), pc = Math.abs(pp - c); v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      cur[i] = v & 255;
    }
    prev = cur;
  }
  return { w, h, ch, px: out };
}

const file = process.argv[2];
const cols = Number(process.argv[3] ?? 64);
const img = decodePng(readFileSync(file));
const { w, h, ch, px } = img;
const rows = Math.max(1, Math.round(cols * h / w));
const pal = new Map();
const ramp = "@%#*+=-:. ";
for (let r = 0; r < rows; r++) {
  let line = "";
  for (let c = 0; c < cols; c++) {
    let sr = 0, sg = 0, sb = 0, n = 0;
    const x0 = Math.floor(c * w / cols), x1 = Math.max(x0 + 1, Math.floor((c + 1) * w / cols));
    const y0 = Math.floor(r * h / rows), y1 = Math.max(y0 + 1, Math.floor((r + 1) * h / rows));
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
      const i = (y * w + x) * ch;
      sr += px[i]; sg += px[i + 1]; sb += px[i + 2]; n++;
    }
    sr /= n; sg /= n; sb /= n;
    const key = ((sr >> 4) << 8) | ((sg >> 4) << 4) | (sb >> 4);
    pal.set(key, (pal.get(key) ?? 0) + 1);
    const lum = (0.299 * sr + 0.587 * sg + 0.114 * sb) / 255;
    line += lum < 0.06 ? " " : ramp[Math.min(ramp.length - 1, Math.floor((1 - lum) * (ramp.length - 1)) + (lum < 0.5 ? 1 : 0))];
  }
  console.log(line);
}
console.log("\npalette:");
[...pal.entries()].sort((a, b) => b[1] - a[1]).slice(0, 14).forEach(([k, n]) => {
  const r = (k >> 8) << 4, g = ((k >> 4) & 15) << 4, b = (k & 15) << 4;
  console.log(`  #${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}  ${(100 * n / (cols * rows)).toFixed(1)}%`);
});
