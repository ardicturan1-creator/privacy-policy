import { chromium } from 'playwright';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const svg = readFileSync(join(root, 'public/icon-source.svg'), 'utf8');
const outDir = join(root, 'public/icons');
mkdirSync(outDir, { recursive: true });

const sizes = [
  { name: 'icon-192.png', size: 192, maskable: false },
  { name: 'icon-512.png', size: 512, maskable: false },
  { name: 'maskable-192.png', size: 192, maskable: true },
  { name: 'maskable-512.png', size: 512, maskable: true },
  { name: 'apple-touch-icon.png', size: 180, maskable: false },
];

const browser = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium-1194/chrome-linux/chrome' });
const page = await browser.newPage();

for (const { name, size, maskable } of sizes) {
  const pad = maskable ? Math.round(size * 0.14) : 0;
  await page.setViewportSize({ width: size, height: size });
  const html = `<!doctype html><html><head><style>
    html,body{margin:0;padding:0;width:${size}px;height:${size}px;background:${maskable ? '#05030f' : 'transparent'};overflow:hidden;}
    svg{display:block;width:${size - pad * 2}px;height:${size - pad * 2}px;margin:${pad}px;}
  </style></head><body>${svg}</body></html>`;
  await page.setContent(html);
  await page.screenshot({ path: join(outDir, name), omitBackground: !maskable });
}

await browser.close();
console.log('Icons generated in', outDir);
