import { chromium } from 'playwright';
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const svg = readFileSync(join(root, 'public/icon-source.svg'), 'utf8');

const size = 1200;
const html = `<!doctype html><html><head><style>
  html,body{margin:0;padding:0;width:${size}px;height:${size}px;background:#05030f;overflow:hidden;
    display:flex;align-items:center;justify-content:center;flex-direction:column;font-family:sans-serif;}
  .wrap{width:${size * 0.42}px;height:${size * 0.42}px;}
  h1{color:#fff;letter-spacing:6px;font-size:${size * 0.045}px;margin-top:${size * 0.04}px;text-shadow:0 0 30px #4bf5ff;}
  h1 span{color:#ff3d9a;text-shadow:0 0 30px #ff3d9a;}
</style></head><body>
  <div class="wrap">${svg}</div>
  <h1>NEBULA <span>DRIFT</span></h1>
</body></html>`;

const browser = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium-1194/chrome-linux/chrome' });
const page = await browser.newPage();
await page.setViewportSize({ width: size, height: size });
await page.setContent(html);
const buf = await page.screenshot();
await browser.close();

const resDir = join(root, 'android/app/src/main/res');
for (const entry of readdirSync(resDir, { withFileTypes: true })) {
  if (entry.isDirectory() && entry.name.startsWith('drawable')) {
    writeFileSync(join(resDir, entry.name, 'splash.png'), buf);
  }
}
console.log('Splash screens updated.');
