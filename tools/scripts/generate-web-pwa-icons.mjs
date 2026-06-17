#!/usr/bin/env node
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "../..");
const outDir = resolve(repoRoot, "apps/web/public");

function renderPng(size) {
  const scale = 3;
  const hi = size * scale;
  const hiPixels = new Uint8Array(hi * hi * 4);

  for (let y = 0; y < hi; y += 1) {
    for (let x = 0; x < hi; x += 1) {
      const nx = (x + 0.5) / hi;
      const ny = (y + 0.5) / hi;
      const color = pixelColor(nx, ny);
      const offset = (y * hi + x) * 4;
      hiPixels[offset] = color[0];
      hiPixels[offset + 1] = color[1];
      hiPixels[offset + 2] = color[2];
      hiPixels[offset + 3] = 255;
    }
  }

  const pixels = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const acc = [0, 0, 0, 0];
      for (let sy = 0; sy < scale; sy += 1) {
        for (let sx = 0; sx < scale; sx += 1) {
          const hiOffset = ((y * scale + sy) * hi + x * scale + sx) * 4;
          for (let channel = 0; channel < 4; channel += 1) {
            acc[channel] += hiPixels[hiOffset + channel];
          }
        }
      }
      const offset = (y * size + x) * 4;
      for (let channel = 0; channel < 4; channel += 1) {
        pixels[offset + channel] = Math.round(acc[channel] / (scale * scale));
      }
    }
  }

  return encodePng(size, size, pixels);
}

function pixelColor(x, y) {
  const blue = [0, 102, 204];
  const white = [255, 255, 255];
  const pale = [245, 245, 247];

  const mark =
    roundedRect(x, y, 0.28, 0.23, 0.16, 0.55, 0.025) ||
    (roundedRect(x, y, 0.28, 0.23, 0.48, 0.34, 0.17) &&
      !roundedRect(x, y, 0.43, 0.33, 0.22, 0.14, 0.075));
  const horizon =
    Math.abs(y - (0.69 - 0.18 * (x - 0.5))) < 0.018 &&
    x > 0.42 &&
    x < 0.75;
  const spark = Math.hypot(x - 0.73, y - 0.58) < 0.035;

  if (mark) return white;
  if (horizon || spark) return pale;
  return blue;
}

function roundedRect(x, y, left, top, width, height, radius) {
  const right = left + width;
  const bottom = top + height;
  const cx = clamp(x, left + radius, right - radius);
  const cy = clamp(y, top + radius, bottom - radius);
  return Math.hypot(x - cx, y - cy) <= radius;
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function encodePng(width, height, rgba) {
  const raw = new Uint8Array((width * 4 + 1) * height);
  for (let y = 0; y < height; y += 1) {
    const rowStart = y * (width * 4 + 1);
    raw[rowStart] = 0;
    raw.set(rgba.subarray(y * width * 4, (y + 1) * width * 4), rowStart + 1);
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr(width, height)),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function ihdr(width, height) {
  const buffer = Buffer.alloc(13);
  buffer.writeUInt32BE(width, 0);
  buffer.writeUInt32BE(height, 4);
  buffer[8] = 8;
  buffer[9] = 6;
  buffer[10] = 0;
  buffer[11] = 0;
  buffer[12] = 0;
  return buffer;
}

function chunk(type, data) {
  const typeBuffer = Buffer.from(type);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k += 1) {
    c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  }
  return c >>> 0;
});

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) {
    c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

mkdirSync(outDir, { recursive: true });

for (const [file, size] of [
  ["icon-512.png", 512],
  ["icon-192.png", 192],
  ["apple-touch-icon.png", 180],
  ["favicon-32.png", 32],
]) {
  writeFileSync(resolve(outDir, file), renderPng(size));
}

console.log("Generated web PWA icons.");
