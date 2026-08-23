import { chromium, devices } from 'playwright';

const browser = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium-1194/chrome-linux/chrome' });
const context = await browser.newContext({ ...devices['Pixel 7'], permissions: [] });
const page = await context.newPage();

const errors = [];
page.on('pageerror', (e) => errors.push('pageerror: ' + e.message));
page.on('console', (msg) => {
  if (msg.type() === 'error') errors.push('console.error: ' + msg.text());
});

await page.goto('http://localhost:4173/', { waitUntil: 'networkidle' });
await page.screenshot({ path: '/tmp/shot-start.png' });

await page.click('#start-btn');
await page.waitForTimeout(600);
await page.screenshot({ path: '/tmp/shot-playing.png' });

// simulate steering drag + firing for a few seconds
for (let i = 0; i < 40; i++) {
  const x = 200 + Math.sin(i / 5) * 120;
  const y = 400 + Math.cos(i / 7) * 150;
  await page.mouse.move(x, y);
  if (i === 0) await page.mouse.down();
  await page.waitForTimeout(80);
}
await page.mouse.up();
await page.screenshot({ path: '/tmp/shot-mid.png' });

await page.tap('#fire-btn').catch(() => {});
await page.waitForTimeout(1500);
await page.screenshot({ path: '/tmp/shot-fire.png' });

await page.tap('#boost-btn').catch(() => {});
await page.waitForTimeout(1000);
await page.screenshot({ path: '/tmp/shot-boost.png' });

await page.click('#pause-btn');
await page.waitForTimeout(300);
await page.screenshot({ path: '/tmp/shot-pause.png' });

console.log('ERRORS:', JSON.stringify(errors, null, 2));
await browser.close();
