#!/usr/bin/env node
// Optional media build: Node + sharp + ffmpeg, never runtime dependencies.
// node tools/render-animation.cjs <actual-buffer frame directory> <output.gif>
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const sharp = require('sharp');

async function main() {
  const args = process.argv.slice(2);
  if (args.length !== 2 || !args[1].endsWith('.gif')) throw new Error('usage: render-animation.cjs <frame directory> <output.gif>');
  const [source, output] = args;
  const manifest = fs.readFileSync(path.join(source, 'manifest.json'));
  const frames = JSON.parse(manifest);
  if (!Array.isArray(frames) || frames.length < 2 || frames.length > 600) throw new Error('expected 2–600 exported frames');
  const first = frames[0];
  const profiles = ['dark-truecolor', 'dark-graphite', 'light-truecolor', 'dark-256', 'light-256', 'ansi-16', 'unknown-ground', 'no-color', 'ascii'];
  const integer = (value, min, max) => Number.isSafeInteger(value) && value >= min && value <= max;
  if (!integer(first.width, 1, 160) || !integer(first.height, 1, 80) || !integer(first.elapsed_ms, 0, Number.MAX_SAFE_INTEGER) || !profiles.includes(first.profile)) throw new Error('invalid frame dimensions, profile or time');
  const frameMs = frames[1].elapsed_ms - first.elapsed_ms;
  if (!integer(frameMs, 20, 1000) || frameMs % 10) throw new Error('frame interval must be 20–1000 ms in GIF centiseconds');
  const width = first.width * 10, height = first.height * 20;
  const digest = crypto.createHash('sha256').update(manifest);
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'codewhale-animation-'));
  try {
    for (const [index, frame] of frames.entries()) {
      if (Object.keys(frame).sort().join(',') !== 'elapsed_ms,file,height,profile,width' || frame.file !== `frame-${String(index).padStart(3, '0')}.svg` || frame.width !== first.width || frame.height !== first.height || frame.profile !== first.profile || !integer(frame.elapsed_ms, 0, Number.MAX_SAFE_INTEGER) || frame.elapsed_ms !== first.elapsed_ms + index * frameMs) throw new Error('unexpected frame manifest');
      const svg = fs.readFileSync(path.join(source, frame.file));
      if (/href\s*=|<script|<foreignObject|<!DOCTYPE|\bon\w+\s*=/i.test(svg.toString())) throw new Error('active or external frame content');
      digest.update(frame.file).update('\0').update(svg);
      const sourceMetadata = await sharp(svg).metadata();
      if (sourceMetadata.width !== width || sourceMetadata.height !== height) throw new Error('SVG does not preserve its declared cell dimensions');
      await sharp(svg).png().toFile(path.join(temporary, `frame-${String(index).padStart(3, '0')}.png`));
    }
    fs.mkdirSync(path.dirname(output), { recursive: true });
    const result = spawnSync('ffmpeg', ['-v', 'error', '-y', '-framerate', String(1000 / frameMs), '-i', path.join(temporary, 'frame-%03d.png'), '-filter_complex', '[0:v]split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=none', '-loop', '0', output], { stdio: 'inherit' });
    if (result.error || result.status !== 0) throw result.error || new Error('ffmpeg failed');
    const metadata = await sharp(output, { animated: true }).metadata();
    if (metadata.width !== width || metadata.pageHeight !== height || metadata.pages !== frames.length || metadata.delay?.some(delay => delay !== frameMs)) throw new Error('GIF does not preserve the exported frame dimensions, count or timing');
    const record = { source_sha256: digest.digest('hex'), gif_sha256: crypto.createHash('sha256').update(fs.readFileSync(output)).digest('hex'), frames: frames.length, width, height, frame_ms: frameMs };
    fs.writeFileSync(`${output}.json`, JSON.stringify(record, null, 2) + '\n');
    process.stdout.write(`Rendered ${record.frames} actual buffer frames to ${output}\n`);
  } finally { fs.rmSync(temporary, { recursive: true, force: true }); }
}
main().catch(error => { process.stderr.write(error.message + '\n'); process.exitCode = 1; });
