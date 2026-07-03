// Generates a 1024x1024 source PNG for `tauri icon` to derive all app icons.
// Pure Node (zlib) — no external deps. A blue rounded tile with a photo frame,
// a sun, and a mountain, evoking a wallpaper slideshow.
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";

const S = 1024;

function blend(dst, sr, sg, sb, sa) {
  // dst: [r,g,b] opaque; src over
  const a = sa / 255;
  return [
    Math.round(sr * a + dst[0] * (1 - a)),
    Math.round(sg * a + dst[1] * (1 - a)),
    Math.round(sb * a + dst[2] * (1 - a)),
  ];
}

function inRoundedRect(x, y, x0, y0, x1, y1, r) {
  if (x < x0 || x > x1 || y < y0 || y > y1) return false;
  const cx = Math.min(Math.max(x, x0 + r), x1 - r);
  const cy = Math.min(Math.max(y, y0 + r), y1 - r);
  const dx = x - cx;
  const dy = y - cy;
  return dx * dx + dy * dy <= r * r;
}

const raw = Buffer.alloc((S * 4 + 1) * S);

for (let y = 0; y < S; y++) {
  const rowStart = y * (S * 4 + 1);
  raw[rowStart] = 0; // filter: None
  for (let x = 0; x < S; x++) {
    let px = [26, 26, 26]; // transparent-ish base (will set alpha)
    let alpha = 0;

    // Rounded blue tile
    if (inRoundedRect(x, y, 40, 40, S - 40, S - 40, 180)) {
      px = [74, 144, 226]; // brand blue
      alpha = 255;

      // Inner photo frame (white border)
      const f0 = 200, f1 = S - 200;
      const border = 26;
      const insideOuter = inRoundedRect(x, y, f0, f0, f1, f1, 40);
      const insideInner = inRoundedRect(
        x, y, f0 + border, f0 + border, f1 - border, f1 - border, 24,
      );
      if (insideOuter && !insideInner) {
        px = [255, 255, 255];
      } else if (insideInner) {
        // Sky
        px = [173, 205, 240];
        // Sun
        const sx = 400, sy = 400, sr = 70;
        if ((x - sx) ** 2 + (y - sy) ** 2 <= sr * sr) px = [255, 236, 150];
        // Mountain (triangle rising from the frame bottom)
        const baseY = f1 - border;
        const peakX = 560, peakY = 470;
        // left slope and right slope
        const leftOk = x >= peakX - (baseY - y) * 1.4;
        const rightOk = x <= peakX + (baseY - y) * 1.6;
        if (y <= baseY && y >= peakY && leftOk && rightOk) {
          px = [58, 110, 170];
        }
      }
    }

    const p = rowStart + 1 + x * 4;
    if (alpha === 0) {
      raw[p] = 0; raw[p + 1] = 0; raw[p + 2] = 0; raw[p + 3] = 0;
    } else {
      const out = blend([255, 255, 255], px[0], px[1], px[2], 255);
      raw[p] = out[0];
      raw[p + 1] = out[1];
      raw[p + 2] = out[2];
      raw[p + 3] = 255;
    }
  }
}

// --- PNG assembly ---
const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = (c >>> 8) ^ crcTable[(c ^ buf[i]) & 0xff];
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const tBuf = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([tBuf, data])), 0);
  return Buffer.concat([len, tBuf, data, crc]);
};

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0);
ihdr.writeUInt32BE(S, 4);
ihdr.writeUInt8(8, 8); // bit depth
ihdr.writeUInt8(6, 9); // RGBA
ihdr.writeUInt8(0, 10);
ihdr.writeUInt8(0, 11);
ihdr.writeUInt8(0, 12);

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

const out = "src-tauri/icons/source.png";
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, png);
console.log(`wrote ${out} (${png.length} bytes)`);
