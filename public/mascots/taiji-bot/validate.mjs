import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(
  fs.readFileSync(path.join(root, "manifest.json"), "utf8"),
);
const u24 = (buffer, offset) => buffer.readUIntLE(offset, 3);
const results = [];
for (const [state, entry] of Object.entries(manifest.states)) {
  const data = fs.readFileSync(path.join(root, entry.animation));
  if (
    data.toString("ascii", 0, 4) !== "RIFF" ||
    data.toString("ascii", 8, 12) !== "WEBP"
  )
    throw Error(`Invalid WebP: ${state}`);
  if (data.readUInt32LE(4) + 8 !== data.length)
    throw Error(`Incomplete WebP: ${state}`);
  const frames = [];
  let width, height, alpha, loop;
  for (let offset = 12; offset + 8 <= data.length;) {
    const tag = data.toString("ascii", offset, offset + 4);
    const length = data.readUInt32LE(offset + 4);
    const start = offset + 8;
    if (start + length > data.length) throw Error(`Truncated chunk: ${state}`);
    if (tag === "VP8X") {
      alpha = Boolean(data[start] & 0x10);
      width = u24(data, start + 4) + 1;
      height = u24(data, start + 7) + 1;
    }
    if (tag === "ANIM") loop = data.readUInt16LE(start + 4);
    if (tag === "ANMF")
      frames.push({
        durationMs: u24(data, start + 12),
        hash: crypto
          .createHash("sha256")
          .update(data.subarray(start + 16, start + length))
          .digest("hex"),
      });
    offset += 8 + length + (length % 2);
  }
  const distinctFrames = new Set(frames.map((frame) => frame.hash)).size;
  if (
    width !== 222 ||
    height !== 222 ||
    !alpha ||
    loop !== 0 ||
    frames.length !== 4 ||
    distinctFrames < 2 ||
    frames.some((frame) => frame.durationMs !== 250)
  )
    throw Error(`Animation contract failed: ${state}`);
  const png = fs.readFileSync(path.join(root, entry.image));
  if (png.readUInt32BE(16) !== 222 || png.readUInt32BE(20) !== 222)
    throw Error(`Invalid PNG size: ${state}`);
  results.push({
    state,
    width,
    height,
    alpha,
    loop,
    frameCount: frames.length,
    distinctFrames,
    frames,
  });
}
const atlas = fs.readFileSync(path.join(root, manifest.spritesheet.file));
if (atlas.readUInt32BE(16) !== 888 || atlas.readUInt32BE(20) !== 1776)
  throw Error("Invalid atlas size");
fs.writeFileSync(
  path.join(root, "validation.json"),
  JSON.stringify({ ok: true, results }, null, 2) + "\n",
);
console.log(
  `PASS: ${results.length} animated WebP files, transparent, 4 distinct-frame animations, 222x222; atlas 888x1776.`,
);
