#!/usr/bin/env node
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pngToIco from "png-to-ico";
import pngjs from "pngjs";

const { PNG } = pngjs;

// The Glyphi mark is the glass Eye of Horus tile supplied by the project's art
// director: a rounded square already cut out of its frame, so every size is a
// straight reduction of that one transparent master.
const appPngSize = 1024;
const webLogoSize = 512;
const icoSizes = [16, 24, 32, 48, 64, 128, 256];
// The favicon is fetched on every page view and Windows only reads the small
// representations of the installer's `iconUrl`, so the web copy stops at 48.
const webIcoSizes = [16, 32, 48];
const icnsRepresentations = [
  ["icp4", 16],
  ["icp5", 32],
  ["icp6", 64],
  ["ic07", 128],
  ["ic08", 256],
  ["ic09", 512],
  ["ic10", 1024]
];

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const brandingRoot = path.join(repoRoot, "branding");
const masterPath = path.join(brandingRoot, "glyphi-primary.png");
const brandingLogoPath = path.join(brandingRoot, "glyphi-logo.png");
const destinationRoot = path.join(repoRoot, "apps", "desktop", "resources", "icons");
const pngPath = path.join(destinationRoot, "glyphi.png");
const icoPath = path.join(destinationRoot, "glyphi.ico");
const installerRoot = path.join(repoRoot, "apps", "desktop", "resources", "installer");
// The packaged app ships everything under resources/icons, so the icns, which
// only the macOS build reads, sits with the other build-time artwork.
const icnsPath = path.join(installerRoot, "glyphi.icns");
const dmgBackgroundPath = path.join(installerRoot, "dmg-background.png");
const webPublicRoot = path.join(repoRoot, "apps", "web", "public");
const webLogoPath = path.join(webPublicRoot, "logo.png");
const webIconPath = path.join(webPublicRoot, "icon.ico");

await mkdir(destinationRoot, { recursive: true });
await mkdir(installerRoot, { recursive: true });
await mkdir(webPublicRoot, { recursive: true });

const master = PNG.sync.read(await readFile(masterPath));
if (master.width !== master.height) throw new Error("Glyphi application icon must be square");
// The supplied artwork is 500 px square, so the 1024 px macOS representation is
// the only size interpolated upwards; every other output is a reduction.
if (master.width < 256) throw new Error("Glyphi master must be at least 256px");

const appIcon = resample(master, appPngSize);

await writeFile(brandingLogoPath, PNG.sync.write(resample(master, webLogoSize)));
await writeFile(pngPath, PNG.sync.write(appIcon));
await writeFile(icoPath, await renderIco(icoSizes));
await writeFile(webLogoPath, PNG.sync.write(resample(master, webLogoSize)));
await writeFile(webIconPath, await renderIco(webIcoSizes));
await writeFile(icnsPath, renderIcns());
await writeFile(dmgBackgroundPath, PNG.sync.write(createDmgBackground()));

console.log(
  `Generated ${[brandingLogoPath, pngPath, icoPath, webLogoPath, webIconPath, icnsPath, dmgBackgroundPath]
    .map((file) => path.relative(repoRoot, file))
    .join(", ")}`
);

/** Box filter over premultiplied colour, so the feathered tile edge stays clean. */
function resample(source, size) {
  const image = new PNG({ width: size, height: size });
  const scale = source.width / size;
  for (let y = 0; y < size; y += 1) {
    const startY = Math.floor(y * scale);
    const endY = Math.min(source.height, Math.max(startY + 1, Math.ceil((y + 1) * scale)));
    for (let x = 0; x < size; x += 1) {
      const startX = Math.floor(x * scale);
      const endX = Math.min(source.width, Math.max(startX + 1, Math.ceil((x + 1) * scale)));
      let red = 0;
      let green = 0;
      let blue = 0;
      let alpha = 0;
      let samples = 0;
      for (let sourceY = startY; sourceY < endY; sourceY += 1) {
        for (let sourceX = startX; sourceX < endX; sourceX += 1) {
          const offset = (sourceY * source.width + sourceX) * 4;
          const coverage = source.data[offset + 3] / 255;
          red += source.data[offset] * coverage;
          green += source.data[offset + 1] * coverage;
          blue += source.data[offset + 2] * coverage;
          alpha += coverage;
          samples += 1;
        }
      }
      const target = (y * size + x) * 4;
      if (alpha > 0) {
        image.data[target] = clampByte(red / alpha);
        image.data[target + 1] = clampByte(green / alpha);
        image.data[target + 2] = clampByte(blue / alpha);
      }
      image.data[target + 3] = clampByte((alpha / samples) * 255);
    }
  }
  return image;
}

function clampByte(value) {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function renderIco(sizes) {
  return pngToIco(sizes.map((size) => PNG.sync.write(size === appPngSize ? appIcon : resample(master, size))));
}

function renderIcns() {
  const chunks = icnsRepresentations.map(([type, size]) => (
    createIcnsChunk(type, PNG.sync.write(size === appPngSize ? appIcon : resample(master, size)))
  ));
  const totalLength = 8 + chunks.reduce((length, chunk) => length + chunk.length, 0);
  const header = Buffer.alloc(8);
  header.write("icns", 0, "ascii");
  header.writeUInt32BE(totalLength, 4);
  return Buffer.concat([header, ...chunks], totalLength);
}

function createIcnsChunk(type, data) {
  const chunk = Buffer.alloc(8 + data.length);
  chunk.write(type, 0, "ascii");
  chunk.writeUInt32BE(chunk.length, 4);
  data.copy(chunk, 8);
  return chunk;
}

// 658x498 is the stock dmg window; the icons sit at (192, 344) and (448, 344).
function createDmgBackground() {
  const width = 658;
  const height = 498;
  const top = [250, 239, 221];
  const bottom = [239, 220, 192];
  const arrow = [120, 92, 58];
  const arrowOpacity = 0.5;
  const image = new PNG({ width, height });

  for (let y = 0; y < height; y += 1) {
    const shade = y / (height - 1);
    for (let x = 0; x < width; x += 1) {
      const offset = (y * width + x) * 4;
      const ink = arrowCoverage(x, y) * arrowOpacity;
      for (let channel = 0; channel < 3; channel += 1) {
        const base = top[channel] + (bottom[channel] - top[channel]) * shade;
        image.data[offset + channel] = Math.round(base * (1 - ink) + arrow[channel] * ink);
      }
      image.data[offset + 3] = 255;
    }
  }
  return image;
}

// Supersampled so the arrow keeps clean edges without an image library.
function arrowCoverage(x, y) {
  const samples = 4;
  let covered = 0;
  for (let sampleY = 0; sampleY < samples; sampleY += 1) {
    for (let sampleX = 0; sampleX < samples; sampleX += 1) {
      const pointX = x + (sampleX + 0.5) / samples;
      const pointY = y + (sampleY + 0.5) / samples;
      if (isInsideArrow(pointX, pointY)) covered += 1;
    }
  }
  return covered / (samples * samples);
}

function isInsideArrow(x, y) {
  const centerY = 344;
  const shaftStart = 286;
  const shaftEnd = 330;
  const shaftRadius = 5;
  const headBase = 326;
  const headTip = 358;
  const headHalfHeight = 17;

  const nearestX = Math.min(Math.max(x, shaftStart), shaftEnd);
  const shaftDistance = (x - nearestX) ** 2 + (y - centerY) ** 2;
  if (shaftDistance <= shaftRadius ** 2) return true;

  if (x < headBase || x > headTip) return false;
  const alongHead = (x - headBase) / (headTip - headBase);
  return Math.abs(y - centerY) <= headHalfHeight * (1 - alongHead);
}
