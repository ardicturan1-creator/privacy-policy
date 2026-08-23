import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const distDir = join(root, 'dist');

let html = readFileSync(join(distDir, 'index.html'), 'utf8');

// inline the built JS/CSS bundle so the page is fully self-contained
html = html.replace(/<link rel="stylesheet"[^>]*href="\/assets\/([^"]+\.css)"[^>]*>/, (_, file) => {
  const css = readFileSync(join(distDir, 'assets', file), 'utf8');
  return `<style>${css}</style>`;
});
html = html.replace(/<script type="module"[^>]*src="\/assets\/([^"]+\.js)"[^>]*><\/script>/, (_, file) => {
  const js = readFileSync(join(distDir, 'assets', file), 'utf8');
  return `<script type="module">${js}</script>`;
});

// drop PWA bits that don't apply inside a sandboxed artifact iframe
html = html.replace(/\s*<link rel="manifest"[^>]*>\n?/, '\n');
html = html.replace(/\s*<link rel="icon"[^>]*>\n?/, '\n');
html = html.replace(/\s*<link rel="apple-touch-icon"[^>]*>\n?/, '\n');
html = html.replace(
  /`serviceWorker`in navigator&&window\.addEventListener\(`load`,\(\)=>\{navigator\.serviceWorker\.register\(`\/sw\.js`\)\.catch\(\(\)=>\{\}\)\}\);/,
  '',
);

writeFileSync(join(root, 'artifact.html'), html);
console.log('Wrote', join(root, 'artifact.html'), `(${(html.length / 1024).toFixed(0)} KB)`);
