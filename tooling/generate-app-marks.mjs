#!/usr/bin/env node
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pngToIco from "png-to-ico";
import pngjs from "pngjs";

const { PNG } = pngjs;

// The three marks Settings → Appearance offers for the title bar, the taskbar
// and the tray. They are built from the installer's own master, so picking one
// never produces artwork the icon pipeline does not know about.
const MARK_SIZE = 256;
// Windows draws the window and taskbar icon from the ICO, so each mark ships one
// with the sizes the shell asks for instead of a single 256 px bitmap.
const MARK_ICO_SIZES = [16, 24, 32, 48, 64, 128, 256];
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const iconRoot = path.join(repoRoot, "apps", "desktop", "resources", "icons");
const markRoot = path.join(iconRoot, "marks");
const masterPath = path.join(repoRoot, "branding", "glyphi-primary.png");

await mkdir(markRoot, { recursive: true });

const master = resample(PNG.sync.read(await readFile(masterPath)), MARK_SIZE);
await writeMark("glyph", master);

// Warm sand: the palette of the tile itself, pushed towards the desert light.
await writeMark("ember", tinted(master, [206, 148, 74], 0.55));

// Night: a cool glass reading for a dark taskbar.
await writeMark("night", tinted(master, [62, 88, 122], 0.6));

console.log(`Generated app marks in ${path.relative(repoRoot, markRoot)}`);

async function writeMark(name, image) {
  await writeFile(path.join(markRoot, `mark-${name}.png`), PNG.sync.write(image));
  await writeFile(
    path.join(markRoot, `mark-${name}.ico`),
    await pngToIco(MARK_ICO_SIZES.map((size) => PNG.sync.write(size === MARK_SIZE ? image : resample(image, size))))
  );
}

/** Mixes the artwork towards a flat tint while keeping its light and shade. */
function tinted(source, tint, amount) {
  const image = new PNG({ width: MARK_SIZE, height: MARK_SIZE });
  for (let pixel = 0; pixel < MARK_SIZE * MARK_SIZE; pixel += 1) {
    const offset = pixel * 4;
    const luminance = (0.2126 * source.data[offset] + 0.7152 * source.data[offset + 1] + 0.0722 * source.data[offset + 2]) / 255;
    for (let channel = 0; channel < 3; channel += 1) {
      const tintedChannel = tint[channel] * (0.55 + luminance * 0.6);
      image.data[offset + channel] = clampByte(source.data[offset + channel] * (1 - amount) + tintedChannel * amount);
    }
    image.data[offset + 3] = source.data[offset + 3];
  }
  return image;
}

/** Averages every source pixel inside a destination pixel, keeping the alpha channel. */
function resample(source, size) {
  const image = new PNG({ width: size, height: size });
  const scale = source.width / size;
  for (let y = 0; y < size; y += 1) {
    const startY = Math.floor(y * scale);
    const endY = Math.min(source.height, Math.max(startY + 1, Math.ceil((y + 1) * scale)));
    for (let x = 0; x < size; x += 1) {
      const startX = Math.floor(x * scale);
      const endX = Math.min(source.width, Math.max(startX + 1, Math.ceil((x + 1) * scale)));
      const total = [0, 0, 0, 0];
      let samples = 0;
      for (let sourceY = startY; sourceY < endY; sourceY += 1) {
        for (let sourceX = startX; sourceX < endX; sourceX += 1) {
          const offset = (sourceY * source.width + sourceX) * 4;
          const alpha = source.data[offset + 3] / 255;
          total[0] += source.data[offset] * alpha;
          total[1] += source.data[offset + 1] * alpha;
          total[2] += source.data[offset + 2] * alpha;
          total[3] += alpha;
          samples += 1;
        }
      }
      const target = (y * size + x) * 4;
      const coverage = total[3] / samples;
      if (coverage > 0) {
        image.data[target] = clampByte(total[0] / total[3]);
        image.data[target + 1] = clampByte(total[1] / total[3]);
        image.data[target + 2] = clampByte(total[2] / total[3]);
      }
      image.data[target + 3] = clampByte(coverage * 255);
    }
  }
  return image;
}

function clampByte(value) {
  return Math.max(0, Math.min(255, Math.round(value)));
}
