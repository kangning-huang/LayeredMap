// Renders the film frame by frame with headless Chrome.
//   node render.mjs stills <t1,t2,...> <outdir>      -> PNG stills at those times
//   node render.mjs video  <out.mkv> [workers] [fps]  -> lossless FFV1 master (no audio)
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import puppeteer from 'puppeteer-core';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../..');            // LayeredMap repo root (fonts + logo live there)
const CHROME = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const FFMPEG = process.env.FFMPEG || '/Users/kangninghuang/.local/bin/ffmpeg';
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.otf': 'font/otf', '.png': 'image/png' };

function serve() {
  return new Promise(res => {
    const srv = http.createServer((req, rsp) => {
      const p = path.join(ROOT, decodeURIComponent(new URL(req.url, 'http://x').pathname));
      if (!p.startsWith(ROOT) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) { rsp.writeHead(404); rsp.end(); return; }
      rsp.writeHead(200, { 'Content-Type': MIME[path.extname(p)] || 'application/octet-stream' });
      fs.createReadStream(p).pipe(rsp);
    });
    srv.listen(0, '127.0.0.1', () => res(srv));
  });
}

async function openPage(browser, port) {
  const page = await browser.newPage();
  await page.setViewport({ width: 1920, height: 1080, deviceScaleFactor: 1 });
  page.on('console', m => { if (m.type() === 'error') console.error('[page]', m.text()); });
  page.on('pageerror', e => console.error('[pageerror]', e.message));
  await page.goto(`http://127.0.0.1:${port}/design/launch-video/index.html`, { waitUntil: 'load' });
  await page.waitForFunction('window.__ready === true', { timeout: 60000 });
  return page;
}
async function grab(page, t) {
  await page.evaluate(tt => window.renderFrame(tt), t);
  return page.screenshot({ type: 'png', clip: { x: 0, y: 0, width: 1920, height: 1080 }, optimizeForSpeed: true });
}

const [mode, a1, a2, a3] = process.argv.slice(2);
const srv = await serve();
const port = srv.address().port;
// Software rendering (no GPU) keeps output identical from run to run.
const browser = await puppeteer.launch({ executablePath: CHROME, headless: 'new', args: ['--disable-gpu', '--force-color-profile=srgb', '--font-render-hinting=none', '--hide-scrollbars'] });

if (mode === 'stills') {
  const times = a1.split(',').map(Number);
  fs.mkdirSync(a2, { recursive: true });
  const page = await openPage(browser, port);
  for (const t of times) {
    const buf = await grab(page, t);
    fs.writeFileSync(path.join(a2, `f_${t.toFixed(3).padStart(6, '0')}.png`), buf);
  }
} else if (mode === 'video') {
  const out = a1, workers = Number(a2 || 4), fps = Number(a3 || 60);
  const total = Math.round(20 * fps);
  const partsDir = path.join(path.dirname(out), 'parts'); fs.mkdirSync(partsDir, { recursive: true });
  const t0 = Date.now(); let done = 0;
  const job = async w => {
    const s = Math.floor(w * total / workers), e = Math.floor((w + 1) * total / workers);
    const page = await openPage(browser, port);
    const part = path.join(partsDir, `part_${w}.mkv`);
    const ff = spawn(FFMPEG, ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(fps), '-c:v', 'png', '-i', '-', '-c:v', 'ffv1', '-level', '3', '-pix_fmt', 'yuv444p', part], { stdio: ['pipe', 'inherit', 'inherit'] });
    const closed = new Promise(r => ff.on('close', r));
    for (let f = s; f < e; f++) {
      const buf = await grab(page, f / fps);
      if (!ff.stdin.write(buf)) await new Promise(r => ff.stdin.once('drain', r));
      done++;
      if (done % 60 === 0) console.log(`${done}/${total} frames, ${((Date.now() - t0) / 1000).toFixed(0)} s`);
    }
    ff.stdin.end(); await closed; await page.close();
    return part;
  };
  const parts = await Promise.all([...Array(workers).keys()].map(job));
  const list = path.join(partsDir, 'list.txt');
  fs.writeFileSync(list, parts.map(p => `file '${path.resolve(p)}'`).join('\n'));
  await new Promise(r => spawn(FFMPEG, ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', list, '-c', 'copy', out], { stdio: 'inherit' }).on('close', r));
  console.log(`master written: ${out} (${((Date.now() - t0) / 1000).toFixed(0)} s)`);
}
await browser.close();
srv.close();
