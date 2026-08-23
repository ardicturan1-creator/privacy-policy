import { chromium } from 'playwright';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const svgFull = readFileSync(join(root, 'public/icon-source.svg'), 'utf8');
const svgTransparent = svgFull.replace('<rect width="512" height="512" fill="url(#bg)"/>', '');

const densities = [
  { name: 'mdpi', legacy: 48, foreground: 108 },
  { name: 'hdpi', legacy: 72, foreground: 162 },
  { name: 'xhdpi', legacy: 96, foreground: 216 },
  { name: 'xxhdpi', legacy: 144, foreground: 324 },
  { name: 'xxxhdpi', legacy: 192, foreground: 432 },
];

const resDir = join(root, 'android/app/src/main/res');
const browser = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium-1194/chrome-linux/chrome' });
const page = await browser.newPage();

async function render(svg, size, pad, bg) {
  await page.setViewportSize({ width: size, height: size });
  const html = `<!doctype html><html><head><style>
    html,body{margin:0;padding:0;width:${size}px;height:${size}px;background:${bg};overflow:hidden;}
    svg{display:block;width:${size - pad * 2}px;height:${size - pad * 2}px;margin:${pad}px;}
  </style></head><body>${svg}</body></html>`;
  await page.setContent(html);
  return page.screenshot({ omitBackground: bg === 'transparent' });
}

for (const d of densities) {
  const dir = join(resDir, `mipmap-${d.name}`);
  mkdirSync(dir, { recursive: true });

  // legacy square launcher icon (opaque background baked in)
  const legacyBuf = await render(svgFull, d.legacy, 0, '#05030f');
  writeFileSync(join(dir, 'ic_launcher.png'), legacyBuf);
  writeFileSync(join(dir, 'ic_launcher_round.png'), legacyBuf);

  // adaptive icon foreground layer (transparent, padded to adaptive safe zone)
  const pad = Math.round(d.foreground * 0.24);
  const fgBuf = await render(svgTransparent, d.foreground, pad, 'transparent');
  writeFileSync(join(dir, 'ic_launcher_foreground.png'), fgBuf);
}

await browser.close();
console.log('Android launcher icons written to', resDir);
