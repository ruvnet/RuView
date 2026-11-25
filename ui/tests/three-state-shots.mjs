// Tri-state screenshot verification (Task 4): drives the REAL banner/badge
// code paths in ui/tests/three-state-shots.html through simulated /
// rssi-only / live and captures one screenshot per state.
// Run: PLAYWRIGHT_BROWSERS_PATH=/opt/data/cache/scratch/.pw-browsers \
//   node ui/tests/three-state-shots.mjs   (serves ui/ on 127.0.0.1:8321)
import { createRequire } from 'node:module';

const require = createRequire(process.env.PLAYWRIGHT_CORE_PKG || '/opt/data/profiles/dev/home/.npm/_npx/f0a362733743bae2/node_modules/playwright-core/package.json');
const { chromium } = require('playwright-core');
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json' };

const server = createServer(async (req, res) => {
  try {
    const path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    const data = await readFile(join(ROOT, path === '/' ? '/index.html' : path));
    res.writeHead(200, { 'Content-Type': MIME[extname(path)] || 'application/octet-stream' });
    res.end(data);
  } catch {
    res.writeHead(404); res.end('nope');
  }
});
await new Promise((r) => server.listen(8321, '127.0.0.1', r));

const browser = await chromium.launch({
  executablePath: process.env.PLAYWRIGHT_CHROME_EXE || '/opt/data/cache/scratch/.pw-browsers/chromium-1148/chrome-linux/chrome',
});
const page = await browser.newPage({ viewport: { width: 860, height: 700 } });
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
await page.goto('http://127.0.0.1:8321/tests/three-state-shots.html');
await page.waitForFunction('window.__ready === true', null, { timeout: 15000 });

const report = {};
for (const name of ['simulated', 'rssi-only', 'live']) {
  const info = await page.evaluate((n) => window.setShotState(n), name);
  report[name] = info;
  await page.screenshot({ path: `ui/tests/shot-${name}.png` });
  console.log(name, JSON.stringify(info));
}
await browser.close();
server.close();
if (errors.length) {
  console.error('PAGE ERRORS:', errors);
  process.exit(1);
}
console.log('SHOTS OK');
