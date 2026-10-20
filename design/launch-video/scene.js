/* NimbleMap launch film — deterministic Canvas 2D renderer.
 * window.renderFrame(t) draws the frame at time t (seconds) into #c.
 * Everything is a pure function of t (seeded RNG, no clocks), so any frame
 * can be rendered in any order by any worker.
 */
'use strict';
(() => {
const W = 1920, H = 1080, TAU = Math.PI * 2;
const BPM = 122, B = 60 / BPM;           // beat = 0.4918 s
const at = k => k * B;                    // time of beat k
const FPS = 60;

// ---------------------------------------------------------------- palette
const ACC = '#2563EB', AMBER = '#F2C94C', GREEN = '#4EBA86', GRAY = '#8A8F98', WHITE = '#FFFFFF';
const BG0 = '#0B0C10', BG1 = '#1A1B23';
const PANEL = '#17181F', PANEL_HI = '#1D1E27';
const BORDER = 'rgba(255,255,255,0.10)';
const MONO = 'GeistMono', SANS = 'Inter', CJK = 'NotoSC';

// ---------------------------------------------------------------- math / easing
const clamp = (x, a = 0, b = 1) => (x < a ? a : x > b ? b : x);
const seg = (t, a, b) => clamp((t - a) / (b - a));
const lerp = (a, b, u) => a + (b - a) * u;
const K10 = 1 - Math.pow(2, -10);
const expoOut = u => (u <= 0 ? 0 : u >= 1 ? 1 : (1 - Math.pow(2, -10 * u)) / K10);
const expoIn = u => (u <= 0 ? 0 : u >= 1 ? 1 : (Math.pow(2, 10 * u) - 1) / 1023);
const expoInOut = u => (u < 0.5 ? expoIn(u * 2) / 2 : 0.5 + expoOut(u * 2 - 1) / 2);
const quintOut = u => 1 - Math.pow(1 - clamp(u), 5);
const quintIn = u => Math.pow(clamp(u), 5);
const quintInOut = u => { u = clamp(u); return u < 0.5 ? 16 * u ** 5 : 1 - Math.pow(-2 * u + 2, 5) / 2; };
const cubicIn = u => clamp(u) ** 3;
const cubicOut = u => 1 - Math.pow(1 - clamp(u), 3);
const smooth = u => { u = clamp(u); return u * u * (3 - 2 * u); };
const ev = (t, t0, dur, e = expoOut) => e(seg(t, t0, t0 + dur));

function hex(h) { h = h.replace('#', ''); return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)]; }
const toRGB = c => (typeof c === 'string' ? hex(c) : c);
function rgba(c, a = 1) { const v = toRGB(c); return `rgba(${Math.round(v[0])},${Math.round(v[1])},${Math.round(v[2])},${a})`; }
function mix(a, b, u) { const A = toRGB(a), Bc = toRGB(b); return [lerp(A[0], Bc[0], u), lerp(A[1], Bc[1], u), lerp(A[2], Bc[2], u)]; }

function rng(seed) { // mulberry32
  let a = seed >>> 0;
  return () => { a = (a + 0x6D2B79F5) >>> 0; let t = a; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}

function mk(w, h) { const c = document.createElement('canvas'); c.width = w; c.height = h; return c; }
const fnt = (size, weight = 600, fam = SANS) => `${weight} ${size}px ${fam}`;
function text(g, s, x, y, o = {}) {
  g.font = fnt(o.size || 32, o.weight || 600, o.fam || SANS);
  g.fillStyle = o.color || WHITE; g.textAlign = o.align || 'left'; g.textBaseline = o.base || 'alphabetic';
  g.letterSpacing = (o.ls || 0) + 'px'; g.fillText(s, x, y); g.letterSpacing = '0px';
}
function tw(g, s, size, weight = 600, fam = SANS, ls = 0) {
  g.font = fnt(size, weight, fam); g.letterSpacing = ls + 'px'; const w = g.measureText(s).width; g.letterSpacing = '0px'; return w;
}
// Inter sets a roomy hyphen at display sizes; tighten it by hand
const HY_K = -0.07;
function dispW(g, s, size, ls) {
  const parts = s.split('-'); if (parts.length === 1) return tw(g, s, size, 600, SANS, ls);
  let w = 0; parts.forEach((p, i) => { w += tw(g, p, size, 600, SANS, ls); if (i < parts.length - 1) w += tw(g, '-', size, 600, SANS, ls) + 2 * HY_K * size; });
  return w;
}
function dispText(g, s, x, y, o) {
  const parts = s.split('-'); if (parts.length === 1) { text(g, s, x, y, o); return; }
  parts.forEach((p, i) => { text(g, p, x, y, o); x += tw(g, p, o.size, 600, SANS, o.ls); if (i < parts.length - 1) { x += HY_K * o.size; text(g, '-', x, y, o); x += tw(g, '-', o.size, 600, SANS, o.ls) + HY_K * o.size; } });
}
function rrect(g, x, y, w, h, r) { g.beginPath(); g.roundRect(x, y, w, h, r); }

// dark rounded panel: soft shadow, faint top light, 1px translucent border
function panel(g, x, y, w, h, r, o = {}) {
  g.save();
  if (o.shadow !== false) { g.shadowColor = `rgba(0,0,0,${o.shadowA ?? 0.55})`; g.shadowBlur = o.shadowBlur ?? 48; g.shadowOffsetY = o.shadowY ?? 20; }
  rrect(g, x, y, w, h, r); g.fillStyle = o.fill || PANEL; g.fill();
  g.restore();
  g.save(); rrect(g, x, y, w, h, r); g.clip();
  const lg = g.createLinearGradient(0, y, 0, y + Math.min(h, 160));
  lg.addColorStop(0, 'rgba(255,255,255,0.045)'); lg.addColorStop(1, 'rgba(255,255,255,0)');
  g.fillStyle = lg; g.fillRect(x, y, w, h); g.restore();
  rrect(g, x + 0.5, y + 0.5, w - 1, h - 1, r); g.strokeStyle = o.border || BORDER; g.lineWidth = 1; g.stroke();
}

function glow(g, x, y, r, c, a) {
  const gr = g.createRadialGradient(x, y, 0, x, y, r);
  gr.addColorStop(0, rgba(c, a)); gr.addColorStop(0.4, rgba(c, a * 0.45)); gr.addColorStop(1, rgba(c, 0));
  g.fillStyle = gr; g.fillRect(x - r, y - r, r * 2, r * 2);
}

// ---------------------------------------------------------------- static layers
const bgCanvas = mk(W, H);
(() => {
  const g = bgCanvas.getContext('2d');
  const gr = g.createRadialGradient(W / 2, H * 0.46, 0, W / 2, H * 0.46, W * 0.62);
  gr.addColorStop(0, BG1); gr.addColorStop(0.55, '#121319'); gr.addColorStop(1, BG0);
  g.fillStyle = gr; g.fillRect(0, 0, W, H);
  const v = g.createRadialGradient(W / 2, H / 2, H * 0.35, W / 2, H / 2, W * 0.66);
  v.addColorStop(0, 'rgba(0,0,0,0)'); v.addColorStop(1, 'rgba(0,0,0,0.6)');
  g.fillStyle = v; g.fillRect(0, 0, W, H);
})();
function bg(g, glows = []) {
  g.setTransform(1, 0, 0, 1, 0, 0); g.globalAlpha = 1; g.globalCompositeOperation = 'source-over';
  g.drawImage(bgCanvas, 0, 0);
  for (const q of glows) glow(g, q[0], q[1], q[2], q[3], q[4]);
}
function vignette(g, a = 0.5) {
  const v = g.createRadialGradient(W / 2, H / 2, H * 0.4, W / 2, H / 2, W * 0.7);
  v.addColorStop(0, 'rgba(0,0,0,0)'); v.addColorStop(1, `rgba(0,0,0,${a})`);
  g.fillStyle = v; g.fillRect(0, 0, W, H);
}

// film grain / dither (keeps dark gradients from banding in 8-bit H.264)
const grains = [0, 1, 2, 3].map(i => {
  const c = mk(W / 2, H / 2), g = c.getContext('2d'), id = g.createImageData(W / 2, H / 2), r = rng(1000 + i);
  for (let p = 0; p < id.data.length; p += 4) { const v = r() < 0.5 ? 0 : 255; id.data[p] = id.data[p + 1] = id.data[p + 2] = v; id.data[p + 3] = Math.floor(r() * 9); }
  g.putImageData(id, 0, 0); return c;
});
function grain(g, t) {
  g.setTransform(1, 0, 0, 1, 0, 0); g.globalAlpha = 1; g.globalCompositeOperation = 'source-over';
  g.drawImage(grains[Math.floor(t * FPS + 0.5) % 4], 0, 0, W, H);
}

// ---------------------------------------------------------------- glyphs (the app's layer-kind icons)
function glyph(g, kind, x, y, s, c) { // centred at x,y, box size s
  g.save(); g.translate(x, y); g.strokeStyle = c; g.fillStyle = c; g.lineCap = 'round'; g.lineJoin = 'round';
  const h = s / 2;
  if (kind === 'pt') { g.beginPath(); g.arc(0, 0, s * 0.26, 0, TAU); g.fill(); }
  else if (kind === 'ln') { g.lineWidth = s * 0.14; g.beginPath(); g.moveTo(-h * 0.7, h * 0.7); g.lineTo(h * 0.7, -h * 0.7); g.stroke(); }
  else if (kind === 'ar') { g.lineWidth = s * 0.1; rrect(g, -h * 0.7, -h * 0.7, s * 0.7, s * 0.7, s * 0.12); g.globalAlpha *= 0.35; g.fill(); g.globalAlpha /= 0.35; g.stroke(); }
  else if (kind === 'gr') { const q = s * 0.3, gp = s * 0.08; for (const [i, j, a] of [[0, 0, 1], [1, 0, 0.55], [0, 1, 0.55], [1, 1, 0.85]]) { g.globalAlpha = a; g.fillRect(-q - gp / 2 + i * (q + gp), -q - gp / 2 + j * (q + gp), q, q); } }
  else if (kind === 'id') { g.lineWidth = s * 0.09; g.beginPath(); for (const d of [-0.2, 0.2]) { g.moveTo(-h * 0.7, d * s); g.lineTo(h * 0.7, d * s); g.moveTo(d * s - s * 0.05, -h * 0.75); g.lineTo(d * s + s * 0.05 * -1, h * 0.75); } g.stroke(); }
  g.restore();
}
function checkPath(g, cx, cy, r, prog, lw, c) {
  const pts = [[-0.40, 0.02], [-0.11, 0.31], [0.42, -0.27]].map(p => [cx + p[0] * r, cy + p[1] * r]);
  const l1 = Math.hypot(pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]), l2 = Math.hypot(pts[2][0] - pts[1][0], pts[2][1] - pts[1][1]);
  const L = (l1 + l2) * prog; if (L <= 0) return;
  g.save(); g.strokeStyle = c; g.lineWidth = lw; g.lineCap = 'round'; g.lineJoin = 'round'; g.beginPath(); g.moveTo(...pts[0]);
  if (L <= l1) { const u = L / l1; g.lineTo(lerp(pts[0][0], pts[1][0], u), lerp(pts[0][1], pts[1][1], u)); }
  else { g.lineTo(...pts[1]); const u = (L - l1) / l2; g.lineTo(lerp(pts[1][0], pts[2][0], u), lerp(pts[1][1], pts[2][1], u)); }
  g.stroke(); g.restore();
}

// =================================================================== S1  0 → 4B
// pinpoint → line → 45° → stripes slice a white globe → dive into whiteout
const GR = 300, GC = mk(2 * GR + 40, 2 * GR + 40), gctx = GC.getContext('2d');
const globeDots = (() => { const r = rng(7), a = []; for (let i = 0; i < 90; i++) a.push([Math.asin(r() * 1.7 - 0.85), r() * TAU, 0.6 + r() * 0.8]); return a; })();
function drawGlobe(R, rot) {
  const g = gctx, c = GR + 20; g.setTransform(1, 0, 0, 1, 0, 0); g.clearRect(0, 0, GC.width, GC.height);
  const gr = g.createRadialGradient(c - R * 0.38, c - R * 0.42, R * 0.05, c, c, R);
  gr.addColorStop(0, '#FFFFFF'); gr.addColorStop(0.5, '#F3F5F8'); gr.addColorStop(0.86, '#DCE1E9'); gr.addColorStop(1, '#C3CAD6');
  g.fillStyle = gr; g.beginPath(); g.arc(c, c, R, 0, TAU); g.fill();
  g.save(); g.beginPath(); g.arc(c, c, R, 0, TAU); g.clip();
  const p0 = 0.36, sp = Math.sin(p0), cp = Math.cos(p0);
  const proj = (lat, lon) => { const cl = Math.cos(lat), dl = lon - rot; return [c + R * cl * Math.sin(dl), c - R * (cp * Math.sin(lat) - sp * cl * Math.cos(dl)), sp * Math.sin(lat) + cp * cl * Math.cos(dl)]; };
  g.strokeStyle = 'rgba(38,50,76,0.17)'; g.lineWidth = 1.6;
  const line = (f, n) => { g.beginPath(); let pen = false; for (let i = 0; i <= n; i++) { const [x, y, v] = f(i / n); if (v >= 0) { if (pen) g.lineTo(x, y); else g.moveTo(x, y); pen = true; } else pen = false; } g.stroke(); };
  for (let lo = 0; lo < 360; lo += 20) line(u => proj(lerp(-Math.PI / 2, Math.PI / 2, u), lo * Math.PI / 180), 72);
  for (let la = -60; la <= 60; la += 20) line(u => proj(la * Math.PI / 180, u * TAU), 144);
  for (const [la, lo, s] of globeDots) { const [x, y, v] = proj(la, lo); if (v > 0.08) { g.fillStyle = rgba(ACC, 0.85 * Math.min(1, v * 1.6)); g.beginPath(); g.arc(x, y, 3.4 * s * (0.55 + 0.45 * v), 0, TAU); g.fill(); } }
  g.restore();
}

function S1(g, t) {
  bg(g);
  const cx = W / 2, cy = H / 2;
  const pointU = smooth(seg(t, 0.04, 0.42));
  const stretch = ev(t, at(1), 0.34);
  const rotU = ev(t, at(2), 0.3, quintOut);
  const globeU = ev(t, at(2), 0.42);
  const splitU = ev(t, at(2.5), 0.28);
  const sliceU = ev(t, at(3), 0.34);
  const dv = seg(t, at(3.15), at(4));
  const z = Math.pow(110, Math.pow(dv, 2.3));

  g.save(); g.translate(cx, cy); g.scale(z, z); g.translate(-cx, -cy);
  const a = lerp(0, -Math.PI / 4, rotU), d = [Math.cos(a), Math.sin(a)], n = [-Math.sin(a), Math.cos(a)];
  const sp = lerp(0, 118, splitU), offs = [-1.5, -0.5, 0.5, 1.5].map(k => k * sp);

  // globe
  if (globeU > 0) {
    drawGlobe(GR, 0.9 + t * 0.32);
    const s = lerp(0.55, 1, globeU);
    g.save(); g.globalAlpha = globeU; glow(g, cx, cy, GR * 1.9 * s, '#C9D6F2', 0.16); g.restore();
    const drawBand = (lo, hi, disp, gap) => {
      g.save(); g.beginPath();
      const p = (o, l) => [cx + n[0] * o + d[0] * l, cy + n[1] * o + d[1] * l];
      const A = p(lo, -3000), Bp = p(lo, 3000), Cp = p(hi, 3000), D = p(hi, -3000);
      g.moveTo(...A); g.lineTo(...Bp); g.lineTo(...Cp); g.lineTo(...D); g.closePath(); g.clip();
      g.translate(d[0] * disp + n[0] * gap, d[1] * disp + n[1] * gap);
      g.translate(cx, cy); g.scale(s, s); g.globalAlpha = globeU; g.drawImage(GC, -GR - 20, -GR - 20); g.restore();
    };
    if (sliceU <= 0) { g.save(); g.translate(cx, cy); g.scale(s, s); g.globalAlpha = globeU; g.drawImage(GC, -GR - 20, -GR - 20); g.restore(); }
    else {
      const bands = [[-5000, offs[0]], [offs[0], offs[1]], [offs[1], offs[2]], [offs[2], offs[3]], [offs[3], 5000]];
      const disp = [-1, 1, 0, -1, 1], gapK = [-2, -1, 0, 1, 2];
      bands.forEach(([lo, hi], i) => drawBand(lo, hi, disp[i] * 64 * sliceU, gapK[i] * 7 * sliceU));
    }
  }

  // light: pinpoint → line → stripes
  const lineA = 1 - sliceU;
  if (pointU > 0 && lineA > 0) {
    g.save(); g.globalCompositeOperation = 'lighter';
    const L = lerp(0, 560, stretch) + splitU * 520;
    if (L < 2) {
      const r = 3.5 * pointU;
      glow(g, cx, cy, 90 * pointU, ACC, 0.55 * pointU); glow(g, cx, cy, 26 * pointU, WHITE, 0.9 * pointU);
      g.fillStyle = WHITE; g.beginPath(); g.arc(cx, cy, r, 0, TAU); g.fill();
    } else {
      offs.forEach((o, i) => {
        const fly = (i % 2 ? 1 : -1) * expoIn(sliceU) * 0 + (i % 2 ? 1 : -1) * sliceU * 1500;
        const ox = cx + n[0] * o + d[0] * fly, oy = cy + n[1] * o + d[1] * fly;
        const x0 = ox - d[0] * L, y0 = oy - d[1] * L, x1 = ox + d[0] * L, y1 = oy + d[1] * L;
        const lg = g.createLinearGradient(x0, y0, x1, y1);
        lg.addColorStop(0, 'rgba(255,255,255,0)'); lg.addColorStop(0.5, rgba(WHITE, lineA)); lg.addColorStop(1, 'rgba(255,255,255,0)');
        g.lineCap = 'round';
        g.shadowColor = rgba(ACC, 0.95 * lineA); g.shadowBlur = 28; g.strokeStyle = lg; g.lineWidth = 3.2 + (1 - splitU) * 1.2;
        g.beginPath(); g.moveTo(x0, y0); g.lineTo(x1, y1); g.stroke();
        g.shadowBlur = 0;
      });
      glow(g, cx, cy, 160 * (1 - splitU * 0.6), ACC, 0.32 * lineA);
    }
    g.restore();
  }
  // cut flash on impact
  if (sliceU > 0) { g.save(); g.globalCompositeOperation = 'lighter'; glow(g, cx, cy, 520, '#DDE6FA', 0.32 * (1 - sliceU)); g.restore(); }
  g.restore();
  // whiteout
  const wa = Math.pow(seg(t, at(3.55), at(4) - 0.02), 1.6);
  if (wa > 0) { g.fillStyle = `rgba(255,255,255,${wa})`; g.fillRect(0, 0, W, H); }
}

// =================================================================== S2  4B → 9B
// dark space of drifting item ID tags; centre type snaps in; diagonal blade wipe
const TAG_DEFS = [
  ['2013306N07162', 'id', ACC], ['Haiyan (2013)', 'ln', ACC], ['storm tracks · line', 'ln', ACC], ['earthquakes · point', 'pt', AMBER],
  ['countries · area', 'ar', GREEN], ['heatwaves summer 2023 · grid', 'gr', AMBER], ['walled cities 1866', 'pt', WHITE], ['EPSG:4326', 'id', GRAY],
  ['GCJ-02', 'id', GRAY], ['districts.geojson', 'ar', ACC], ['cases.csv', 'id', GREEN], ['elevation.tif', 'gr', GREEN],
  ['heatwaves summer 2023.nc', 'gr', AMBER], ['D01', 'ar', GRAY], ['D04', 'ar', GRAY], ['D07', 'ar', ACC], ['D09', 'ar', GRAY], ['D12', 'ar', GRAY],
  ['D15', 'ar', GRAY], ['D17', 'ar', GREEN], ['4,547 storms', 'ln', GRAY], ['max_wind_kt 170', 'id', ACC], ['season 2013', 'id', GRAY],
  ['basin West Pacific', 'ln', ACC], ['凌波微图', 'pt', ACC], ['storm tracks.gpkg', 'ln', ACC], ['higher than 99%', 'pt', GREEN], ['4,999 lines', 'ln', GRAY],
];
let TAGS = null;
function buildTags() {
  const S = 2, Hh = 44;
  const sprites = TAG_DEFS.map(([label, kind, color]) => {
    const tmp = mk(10, 10).getContext('2d');
    const cjk = /[一-鿿]/.test(label);
    const twd = tw(tmp, label, 20, cjk ? 500 : 400, cjk ? CJK : MONO);
    const w = Math.ceil(14 + 18 + 10 + twd + 16), pad = 24;
    const c = mk((w + pad * 2) * S, (Hh + pad * 2) * S), g = c.getContext('2d'); g.scale(S, S);
    rrect(g, pad, pad, w, Hh, 11); g.fillStyle = 'rgba(21,22,29,0.96)'; g.fill();
    rrect(g, pad + 0.5, pad + 0.5, w - 1, Hh - 1, 11); g.strokeStyle = 'rgba(255,255,255,0.13)'; g.lineWidth = 1; g.stroke();
    glyph(g, kind, pad + 14 + 9, pad + Hh / 2, 16, color);
    text(g, label, pad + 14 + 18 + 10, pad + Hh / 2 + 1, { size: 20, weight: cjk ? 500 : 400, fam: cjk ? CJK : MONO, color: 'rgba(232,234,240,0.92)', base: 'middle' });
    const b = mk(c.width, c.height), bg2 = b.getContext('2d'); bg2.filter = 'blur(7px)'; bg2.drawImage(c, 0, 0);
    return { c, b, w: w + pad * 2, h: Hh + pad * 2 };
  });
  const r = rng(42), list = [];
  for (let i = 0; i < 96; i++) {
    let x = (r() * 2 - 1) * 1700, y = (r() * 2 - 1) * 1000;
    const z = 200 + r() * 2900;
    if (Math.abs(x) < 260 && Math.abs(y) < 200) x += x < 0 ? -400 : 400;
    list.push({ s: sprites[i % sprites.length], x, y, z, ph: r() * TAU, dr: 0.6 + r() * 0.8 });
  }
  TAGS = list;
}
function drawTags(g, t, dimCenter) {
  if (!TAGS) buildTags();
  const t2 = t - at(4);
  const rush = expoOut(seg(t2, 0, 1.25));
  const cz = -3000 * (1 - rush) + 230 * t2;
  const camx = 40 * Math.sin(t2 * 0.7), camy = -20 * t2;
  const f = 900, k = 2.0;
  const items = [];
  for (const q of TAGS) {
    const zr = q.z - cz; if (zr < 120) continue;
    const s = f / zr;
    const x = W / 2 + (q.x + Math.sin(t2 * 0.5 * q.dr + q.ph) * 30 - camx) * s;
    const y = H / 2 + (q.y + Math.cos(t2 * 0.4 * q.dr + q.ph) * 20 - camy) * s;
    const a = clamp((4800 - zr) / 2000) * clamp((zr - 140) / 260);
    if (a <= 0.01) continue;
    items.push({ q, zr, s, x, y, a });
  }
  items.sort((p, q) => q.zr - p.zr);
  for (const it of items) {
    const sc = it.s * k, w = it.q.s.w * sc, h = it.q.s.h * sc;
    if (it.x + w / 2 < -50 || it.x - w / 2 > W + 50 || it.y + h / 2 < -50 || it.y - h / 2 > H + 50) continue;
    const bl = Math.max(clamp((it.zr - 1500) / 1700), clamp((520 - it.zr) / 300));
    let a = it.a * lerp(1, 0.55, clamp((it.zr - 1500) / 2500));
    if (dimCenter > 0) { const dd = Math.hypot((it.x - W / 2) / 820, (it.y - 560) / 260); a *= lerp(1, lerp(0.1, 1, smooth((dd - 0.75) / 0.6)), dimCenter); }
    g.globalAlpha = a * (1 - bl); if (bl < 1) g.drawImage(it.q.s.c, it.x - w / 2, it.y - h / 2, w, h);
    g.globalAlpha = a * bl; if (bl > 0) g.drawImage(it.q.s.b, it.x - w / 2, it.y - h / 2, w, h);
  }
  g.globalAlpha = 1;
}
const L1 = ['Double-click', 'any', 'map', 'file.'], L2 = ['It', 'lands', 'on', 'the', 'map.'], L2_ACC = 2;
const FORMATS = ['Shapefile', 'GeoJSON', 'CSV', 'GeoTIFF', 'NetCDF'];
function S2(g, t) {
  bg(g, [[W * 0.3, H * 0.35, 700, ACC, 0.06], [W * 0.75, H * 0.7, 600, ACC, 0.04]]);
  const textOn = ev(t, at(5), 0.4, cubicOut);
  drawTags(g, t, textOn);
  // central darkening behind the type
  if (textOn > 0) { g.save(); g.globalAlpha = textOn; const rg = g.createRadialGradient(W / 2, H / 2, 0, W / 2, H / 2, 760); rg.addColorStop(0, 'rgba(11,12,16,0.85)'); rg.addColorStop(1, 'rgba(11,12,16,0)'); g.fillStyle = rg; g.fillRect(0, 0, W, H); g.restore(); }
  const size = 100, ls = -3, sw = tw(g, ' ', size, 600, SANS, ls);
  const drawLine = (words, y, t0, colorFn) => {
    const widths = words.map(w => dispW(g, w, size, ls));
    const total = widths.reduce((a, b) => a + b, 0) + sw * (words.length - 1);
    let x = W / 2 - total / 2;
    words.forEach((w, i) => {
      const u = ev(t, t0 + i * 0.03, 0.26);
      if (u > 0) {
        g.save(); g.beginPath(); g.rect(x - 20, y - size * 1.05, widths[i] + 40, size * 1.35); g.clip();
        g.globalAlpha = clamp(u * 1.4);
        dispText(g, w, x, y + (1 - u) * size * 0.75, { size, ls, color: colorFn(i) });
        g.restore();
      }
      x += widths[i] + sw;
    });
  };
  drawLine(L1, 492, at(5), () => WHITE);
  const shift = ev(t, at(7.5), 0.45, cubicOut);
  drawLine(L2, 612, at(6), i => (i >= L2_ACC ? rgba(mix(ACC, WHITE, shift)) : WHITE));
  // format chips
  const cw = FORMATS.map(f => tw(g, f, 22, 400, MONO) + 36), gap = 14, totalW = cw.reduce((a, b) => a + b, 0) + gap * (FORMATS.length - 1);
  let x = W / 2 - totalW / 2;
  FORMATS.forEach((f, i) => {
    const u = ev(t, at(6.5) + i * B / 4, 0.3);
    if (u > 0) {
      g.save(); g.globalAlpha = u; const cxm = x + cw[i] / 2, cym = 718; g.translate(cxm, cym); g.scale(lerp(0.86, 1, u), lerp(0.86, 1, u)); g.translate(-cxm, -cym);
      rrect(g, x, 694, cw[i], 48, 24); g.fillStyle = 'rgba(255,255,255,0.045)'; g.fill(); g.strokeStyle = 'rgba(255,255,255,0.14)'; g.lineWidth = 1; g.stroke();
      text(g, f, x + cw[i] / 2, 719, { size: 22, weight: 400, fam: MONO, color: 'rgba(225,228,235,0.85)', align: 'center', base: 'middle' });
      g.restore();
    }
    x += cw[i] + gap;
  });
  vignette(g, 0.35);
  // flash out of the whiteout
  const fl = 1 - ev(t, at(4), 0.55);
  if (fl > 0) { g.fillStyle = `rgba(255,255,255,${fl})`; g.fillRect(0, 0, W, H); }
}

// =================================================================== S3  9B → 15B
// one status icon morphs on each beat through five import steps
const STEPS = [
  { label: 'Queued', note: 'districts.geojson', col: GRAY },
  { label: 'Reading', note: '17 areas', col: WHITE },
  { label: 'Locating', note: 'EPSG:4326', col: AMBER },
  { label: 'Styling', note: 'colour by a value', col: GREEN },
  { label: 'On the map', note: 'D01–D17', col: ACC },
];
function S3(g, t) {
  const cx = W / 2, cy = 440, R = 112;
  const T = i => at(9 + i);
  const m = [0, 1, 2, 3, 4].map(i => (i === 0 ? ev(t, at(8.7), 0.32) : ev(t, T(i), 0.22)));
  const st = t >= T(4) ? 4 : t >= T(3) ? 3 : t >= T(2) ? 2 : t >= T(1) ? 1 : 0;
  // colour of the current state, blended through the morph
  let col = toRGB(GRAY);
  for (let i = 1; i <= 4; i++) col = mix(col, STEPS[i].col, m[i]);
  bg(g, [[cx, cy, 760, col, 0.10]]);

  const zu = seg(t, at(14), at(15));
  const z = Math.pow(46, Math.pow(zu, 2.2));
  const fx = cx - 0.2 * R, fy = cy - 0.3 * R;
  g.save(); g.translate(cx, cy); g.scale(z, z); g.translate(-lerp(cx, fx, smooth(zu)), -lerp(cy, fy, smooth(zu)));

  const appear = m[0];
  const sc = lerp(0.6, 1, appear);
  g.save(); g.translate(cx, cy); g.scale(sc, sc); g.translate(-cx, -cy); g.globalAlpha = appear;
  // soft halo
  g.save(); g.globalCompositeOperation = 'lighter'; glow(g, cx, cy, R * 2.6, col, 0.22 + 0.25 * m[4] * (1 - ev(t, T(4) + 0.1, 0.8))); g.restore();

  const ringW = lerp(lerp(6, 10, m[1]), 12, m[2]);
  const gapLen = lerp(13, 0, m[1]);
  const sweep = lerp(lerp(0, 0.5, m[2]), lerp(0.75, 1, m[4]), m[3]) * TAU;
  // pie (amber → green → accent)
  if (sweep > 0.001) {
    g.save(); g.shadowColor = rgba(col, 0.7); g.shadowBlur = 30;
    g.fillStyle = rgba(col); g.beginPath(); g.moveTo(cx, cy);
    const pr = lerp(R - 22, R + ringW / 2, m[4]);
    g.arc(cx, cy, pr, -Math.PI / 2, -Math.PI / 2 + sweep); g.closePath(); g.fill(); g.restore();
  }
  // ring
  g.save(); g.shadowColor = rgba(col, 0.6); g.shadowBlur = 24; g.strokeStyle = rgba(col); g.lineWidth = ringW;
  if (gapLen > 0.2) { const circ = TAU * R, nd = 24; const dl = circ / nd - gapLen; g.setLineDash([dl, gapLen]); g.lineDashOffset = -t * 40; }
  g.beginPath(); g.arc(cx, cy, R, 0, TAU); g.stroke(); g.restore();
  // reading spinner highlight (state 1 only)
  const spinA = m[1] * (1 - m[2]);
  if (spinA > 0) { g.save(); g.strokeStyle = rgba(WHITE, spinA); g.lineWidth = ringW + 2; g.lineCap = 'round'; g.shadowColor = rgba(WHITE, 0.8 * spinA); g.shadowBlur = 20; const a0 = -Math.PI / 2 + (t - T(1)) * 7; g.beginPath(); g.arc(cx, cy, R, a0, a0 + 0.9); g.stroke(); g.restore(); }
  // check
  if (m[4] > 0) checkPath(g, cx, cy, R * 0.98, ev(t, T(4) + 0.03, 0.2, quintOut), 15, WHITE);
  g.restore(); // appear scale

  // burst ticks + ring wave
  const bu = seg(t, T(4), T(4) + 0.55);
  if (bu > 0 && bu < 1) {
    const e = expoOut(bu), fade = 1 - cubicIn(bu);
    g.save(); g.globalCompositeOperation = 'lighter'; g.lineCap = 'round';
    for (let i = 0; i < 16; i++) {
      const a = i / 16 * TAU + Math.PI / 16, r0 = R + 26 + e * 70, len = lerp(30, 6, e);
      g.strokeStyle = rgba(i % 2 ? WHITE : ACC, fade); g.lineWidth = i % 2 ? 4 : 6;
      g.beginPath(); g.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0); g.lineTo(cx + Math.cos(a) * (r0 + len), cy + Math.sin(a) * (r0 + len)); g.stroke();
    }
    g.strokeStyle = rgba(ACC, 0.7 * fade); g.lineWidth = 3; g.beginPath(); g.arc(cx, cy, R + 10 + e * 190, 0, TAU); g.stroke();
    g.restore();
  }

  // labels
  const ly = 680;
  for (let i = 0; i < 5; i++) {
    const inU = m[i], outU = i < 4 ? m[i + 1] : 0;
    if (inU <= 0 || outU >= 1) continue;
    const y = ly + (1 - inU) * 46 - outU * 46;
    g.save(); g.beginPath(); g.rect(cx - 600, ly - 80, 1200, 150); g.clip();
    g.globalAlpha = clamp(inU * 1.5) * (1 - clamp(outU * 2.5));
    text(g, STEPS[i].label, cx, y, { size: 66, ls: -1.5, align: 'center', color: i === 4 ? WHITE : WHITE });
    text(g, STEPS[i].note, cx, y + 54, { size: 24, weight: 400, fam: MONO, align: 'center', color: 'rgba(170,175,185,0.95)' });
    g.restore();
  }
  // step track: 5 segments
  const segW = 74, sg = 12, tot = 5 * segW + 4 * sg; let sx = cx - tot / 2;
  for (let i = 0; i < 5; i++) {
    const on = m[i];
    rrect(g, sx, 820, segW, 6, 3); g.fillStyle = 'rgba(255,255,255,0.10)'; g.fill();
    if (on > 0) { const c = i <= st ? (st === 4 ? ACC : STEPS[Math.max(i, 1)].col) : GRAY; g.save(); g.shadowColor = rgba(c, 0.7); g.shadowBlur = 12; rrect(g, sx, 820, segW * on, 6, 3); g.fillStyle = rgba(c); g.fill(); g.restore(); }
    text(g, String(i + 1).padStart(2, '0'), sx, 860, { size: 17, weight: 400, fam: MONO, color: i <= st ? 'rgba(220,223,230,0.8)' : 'rgba(138,143,152,0.5)' });
    sx += segW + sg;
  }
  g.restore(); // zoom
  vignette(g, 0.3);
}

// =================================================================== S4  15B → 21B
// ⌘F → "Search places or your data" → type "haiy" → Haiyan (2013) → toast
const TRACKS = (() => {
  const r = rng(99), tr = [];
  for (let i = 0; i < 64; i++) {
    let x = 1250 + r() * 1100, y = 470 + r() * 620, vx = -(9 + r() * 7), vy = -(1.5 + r() * 3.5);
    const rec = 30 + r() * 50, pts = [];
    for (let s = 0; s < 120; s++) {
      pts.push([x, y]);
      if (s > rec) { vx += 0.32; vy -= 0.10; }
      vx += (r() - 0.5) * 0.18; vy += (r() - 0.5) * 0.14;
      x += vx; y += vy; if (y < -100 || x > 2400 || x < -300) break;
    }
    tr.push(pts);
  }
  return tr;
})();
const HAIYAN = (() => { const pts = []; for (let s = 0; s <= 80; s++) { const u = s / 80; pts.push([lerp(1640, 470, u), 800 - 230 * u - 46 * Math.sin(u * Math.PI) + 12 * Math.sin(u * 9)]); } return pts; })();
function polyPart(g, pts, prog) {
  const n = pts.length - 1, e = prog * n, k = Math.floor(e);
  g.beginPath(); g.moveTo(...pts[0]);
  for (let i = 1; i <= Math.min(k, n); i++) g.lineTo(...pts[i]);
  if (k < n) { const f = e - k; g.lineTo(lerp(pts[k][0], pts[k + 1][0], f), lerp(pts[k][1], pts[k + 1][1], f)); }
  return k < n ? [lerp(pts[k][0], pts[k + 1][0], e - k), lerp(pts[k][1], pts[k + 1][1], e - k)] : pts[n];
}
function keycap(g, x, y, s, label, press, sub) {
  const top = s * 0.035 + press * s * 0.04;
  g.save();
  g.shadowColor = 'rgba(0,0,0,0.6)'; g.shadowBlur = s * 0.28; g.shadowOffsetY = s * lerp(0.1, 0.04, press);
  rrect(g, x - s / 2, y - s / 2, s, s, s * 0.18); g.fillStyle = '#0C0D11'; g.fill(); g.restore();
  rrect(g, x - s / 2 + 0.5, y - s / 2 + 0.5, s - 1, s - 1, s * 0.18); g.strokeStyle = 'rgba(255,255,255,0.08)'; g.lineWidth = 1; g.stroke();
  const fx = x - s / 2 + s * 0.045, fy = y - s / 2 + top, fw = s * 0.91, fh = s * 0.86;
  const lg = g.createLinearGradient(0, fy, 0, fy + fh); lg.addColorStop(0, '#2A2C37'); lg.addColorStop(1, '#1D1E27');
  rrect(g, fx, fy, fw, fh, s * 0.15); g.fillStyle = lg; g.fill();
  rrect(g, fx + 0.5, fy + 0.5, fw - 1, fh - 1, s * 0.15); g.strokeStyle = rgba(mix([255, 255, 255], ACC, press), 0.14 + press * 0.5); g.lineWidth = Math.max(1, s * 0.008); g.stroke();
  const lc = rgba(mix([240, 242, 246], ACC, press));
  if (press > 0) { g.save(); g.globalCompositeOperation = 'lighter'; glow(g, x, fy + fh / 2, s * 0.6, ACC, 0.3 * press); g.restore(); }
  text(g, label, x, fy + fh * 0.5 + (sub ? -s * 0.04 : 0), { size: s * 0.4, align: 'center', base: 'middle', color: lc });
  if (sub && s > 80) text(g, sub, fx + fw - s * 0.09, fy + fh - s * 0.1, { size: s * 0.1, weight: 400, align: 'right', color: 'rgba(160,165,175,0.9)' });
}
const ITEMS = [
  { name: 'storms', meta: 'storm tracks · line', id: '4,999 lines', kind: 'ln', col: ACC },
  { name: 'Haiyan (2013)', meta: 'storms · line', id: '2013306N07162', kind: 'ln', col: ACC },
  { name: 'Hagupit (2014)', meta: 'storms · line', id: '', kind: 'ln', col: ACC },
  { name: 'Haitang (2005)', meta: 'storms · line', id: '', kind: 'ln', col: ACC },
  { name: 'Hai Phong', meta: 'place · Vietnam', id: '', kind: 'pt', col: GRAY },
  { name: 'heatwaves summer 2023', meta: 'grid', id: '.nc', kind: 'gr', col: AMBER },
  { name: 'earthquakes', meta: 'point', id: '', kind: 'pt', col: AMBER },
  { name: 'countries', meta: 'area', id: '', kind: 'ar', col: GREEN },
];
const STAGES = [['', at(16.5)], ['h', at(17)], ['ha', at(17.5)], ['hai', at(18)], ['haiy', at(18.5)]];
const visibleIn = q => ITEMS.map((it, i) => (q === '' ? [0, 6, 7, 5].includes(i) : it.name.toLowerCase().startsWith(q)) ? 1 : 0);
function S4(g, t) {
  bg(g);
  const cx = W / 2;
  // ---- map backdrop (storm tracks); zooms to Haiyan on Enter
  const zU = ev(t, at(19), 0.9);
  const mz = lerp(1.0, 1.75, zU), hx = 1050, hy = 640;
  const panX = (t - at(15)) * -6;
  g.save();
  g.translate(lerp(W / 2, W / 2, zU), lerp(H / 2, H / 2 + 20, zU)); g.scale(mz, mz); g.translate(-lerp(W / 2 - panX, hx, zU), -lerp(H / 2, hy, zU));
  g.strokeStyle = 'rgba(255,255,255,0.035)'; g.lineWidth = 1 / mz;
  for (let x = -480; x <= 2400; x += 120) { g.beginPath(); g.moveTo(x, -200); g.lineTo(x, 1300); g.stroke(); }
  for (let y = -240; y <= 1320; y += 120) { g.beginPath(); g.moveTo(-500, y); g.lineTo(2400, y); g.stroke(); }
  g.lineWidth = 1.6 / Math.sqrt(mz); g.lineJoin = 'round';
  g.strokeStyle = rgba(mix(GRAY, ACC, 0.35), lerp(0.22, 0.12, zU));
  for (const p of TRACKS) { g.beginPath(); g.moveTo(...p[0]); for (const q of p) g.lineTo(...q); g.stroke(); }
  // Haiyan
  const hp = ev(t, at(19), 0.75, expoOut);
  if (hp > 0) {
    g.save(); g.lineCap = 'round'; g.shadowColor = rgba(ACC, 0.9); g.shadowBlur = 22; g.strokeStyle = ACC; g.lineWidth = 5 / Math.sqrt(mz);
    const head = polyPart(g, HAIYAN, hp); g.stroke(); g.shadowBlur = 0;
    g.fillStyle = WHITE; g.beginPath(); g.arc(head[0], head[1], 5, 0, TAU); g.fill(); g.restore();
  } else { g.strokeStyle = rgba(mix(GRAY, ACC, 0.35), 0.22); g.beginPath(); g.moveTo(...HAIYAN[0]); for (const q of HAIYAN) g.lineTo(...q); g.stroke(); }
  g.restore();
  vignette(g, 0.55);

  // ---- keycaps
  const ku0 = ev(t, at(15) + 0.04, 0.3), ku1 = ev(t, at(15.5), 0.3);
  const press = clamp(seg(t, at(16), at(16) + 0.05)) * (1 - ev(t, at(16) + 0.13, 0.18));
  const open = ev(t, at(16.5), 0.36);
  const close = ev(t, at(19.5), 0.42);
  const contentA = 1 - seg(t, at(19.5), at(19.5) + 0.06);
  const dim = open * (1 - close);
  if (dim > 0) { g.fillStyle = `rgba(6,7,10,${0.45 * dim})`; g.fillRect(0, 0, W, H); }

  // panel geometry
  const pw = 940, px0 = cx - pw / 2, py = 268, fieldH = 100, rowH = 82;
  const stIdx = STAGES.reduce((a, s, i) => (t >= s[1] ? i : a), 0);
  const pres = ITEMS.map((_, i) => {
    let p = visibleIn(STAGES[0][0])[i];
    for (let s = 1; s < STAGES.length; s++) { if (t < STAGES[s][1]) break; const tg = visibleIn(STAGES[s][0])[i]; p = lerp(p, tg, ev(t, STAGES[s][1], 0.22)); }
    return p;
  });
  const listH = pres.reduce((a, p) => a + p * rowH, 0);
  const ph = fieldH + 1 + 14 + listH + 14;
  // collapse into toast
  const toastW = 600, toastH = 92, toastY = 896;
  const rx = lerp(px0, cx - toastW / 2, close), ry = lerp(py, toastY - toastH / 2, close), rw = lerp(pw, toastW, close), rh = lerp(ph, toastH, close);
  const pscale = lerp(0.94, 1, open);
  if (open > 0) {
    g.save(); g.globalAlpha = open;
    const pcx = rx + rw / 2, pcy = ry + 50; g.translate(pcx, pcy); g.scale(pscale, pscale); g.translate(-pcx, -pcy);
    panel(g, rx, ry, rw, rh, lerp(22, 46, close), { fill: 'rgba(23,24,31,0.97)', shadowBlur: 70, shadowY: 30, shadowA: 0.6, border: 'rgba(255,255,255,0.11)' });
    if (contentA > 0) {
      g.save(); rrect(g, rx, ry, rw, rh, 22); g.clip(); g.globalAlpha = open * contentA;
      // search field
      const fy = ry + fieldH / 2;
      g.save(); g.strokeStyle = 'rgba(200,204,212,0.85)'; g.lineWidth = 3; g.beginPath(); g.arc(px0 + 46, fy - 3, 11, 0, TAU); g.moveTo(px0 + 54, fy + 5); g.lineTo(px0 + 63, fy + 14); g.stroke(); g.restore();
      const q = STAGES[stIdx][0];
      const tx = px0 + 84;
      if (q === '') text(g, 'Search places or your data', tx, fy + 1, { size: 33, weight: 400, base: 'middle', color: 'rgba(138,143,152,0.95)' });
      else text(g, q, tx, fy + 1, { size: 33, weight: 400, base: 'middle', color: WHITE });
      const caretX = tx + (q ? tw(g, q, 33, 400) + 3 : 0);
      const blink = (t - at(16.5)) % B < B * 0.55 || (t >= at(17) - 0.05 && t < at(19)) ? 1 : 0;
      g.fillStyle = rgba(ACC, blink); g.fillRect(caretX, fy - 20, 3, 40);
      // divider
      g.fillStyle = 'rgba(255,255,255,0.08)'; g.fillRect(px0, ry + fieldH, pw, 1);
      // rows
      let y = ry + fieldH + 1 + 14;
      const tops = [];
      ITEMS.forEach((it, i) => { tops.push(y); y += pres[i] * rowH; });
      // highlight on first visible row of current stage (animated)
      const vis = visibleIn(q), first = vis.indexOf(1);
      const prevVis = visibleIn(STAGES[Math.max(0, stIdx - 1)][0]), pFirst = prevVis.indexOf(1);
      const hu = stIdx === 0 ? 1 : ev(t, STAGES[stIdx][1], 0.22);
      const hlY = lerp(tops[pFirst], tops[first], hu);
      const enter = clamp(seg(t, at(19), at(19) + 0.04)) * (1 - ev(t, at(19) + 0.08, 0.3));
      rrect(g, px0 + 12, hlY + 3, pw - 24, rowH - 6, 14);
      g.fillStyle = rgba(ACC, 0.2 + 0.55 * enter); g.fill();
      rrect(g, px0 + 12.5, hlY + 3.5, pw - 25, rowH - 7, 14); g.strokeStyle = rgba(ACC, 0.65); g.lineWidth = 1; g.stroke();
      ITEMS.forEach((it, i) => {
        const p = pres[i]; if (p < 0.02) return;
        const top = tops[i], cy = top + rowH / 2;
        g.save(); g.beginPath(); g.rect(px0, top, pw, p * rowH); g.clip(); g.globalAlpha = open * contentA * clamp(p * 1.3);
        rrect(g, px0 + 32, cy - 20, 40, 40, 10); g.fillStyle = rgba(it.col, 0.16); g.fill();
        glyph(g, it.kind, px0 + 52, cy, 18, it.col);
        const nx = px0 + 92;
        const ql = q && it.name.toLowerCase().startsWith(q) ? q.length : 0;
        const pre = it.name.slice(0, ql), rest = it.name.slice(ql);
        text(g, pre, nx, cy - 6, { size: 29, color: WHITE });
        text(g, rest, nx + tw(g, pre, 29), cy - 6, { size: 29, color: ql ? 'rgba(200,204,212,0.72)' : WHITE });
        text(g, it.meta, nx, cy + 24, { size: 19, weight: 400, fam: MONO, color: 'rgba(138,143,152,1)' });
        if (it.id) text(g, it.id, px0 + pw - 40 - (i === first ? 54 : 0), cy + 2, { size: 21, weight: 400, fam: MONO, align: 'right', base: 'middle', color: 'rgba(170,175,185,0.95)' });
        if (i === first) text(g, '↵', px0 + pw - 40, cy + 2, { size: 28, align: 'right', base: 'middle', color: rgba(mix([220, 224, 232], WHITE, enter)) });
        g.restore();
      });
      g.restore();
    }
    // toast content
    const ta = seg(t, at(19.5) + 0.16, at(19.5) + 0.32);
    if (ta > 0) {
      const tcx = cx, tcy = toastY, lab = 'Zoomed to Haiyan', idt = '2013306N07162';
      const w1 = tw(g, lab, 29), w2 = tw(g, idt, 20, 400, MONO), tot = 40 + 18 + w1 + 22 + w2;
      let x = tcx - tot / 2 + (1 - expoOut(ta)) * 12;
      g.save(); g.globalAlpha = ta;
      g.save(); g.shadowColor = rgba(ACC, 0.8); g.shadowBlur = 16; g.fillStyle = ACC; g.beginPath(); g.arc(x + 20, tcy, 20, 0, TAU); g.fill(); g.restore();
      checkPath(g, x + 20, tcy, 19, ev(t, at(19.5) + 0.2, 0.2), 4.5, WHITE);
      text(g, lab, x + 58, tcy + 1, { size: 29, base: 'middle' });
      text(g, idt, x + 58 + w1 + 22, tcy + 2, { size: 20, weight: 400, fam: MONO, base: 'middle', color: 'rgba(160,165,175,1)' });
      g.restore();
    }
    g.restore();
  }
  // keycaps (before the panel opens they are centre stage; then they shrink into the field's ⌘F hint)
  if (contentA > 0 && ku0 > 0) {
    const hintY = py + fieldH / 2, s = lerp(220, 42, open);
    const pos = [[lerp(cx - 130, px0 + pw - 104, open), lerp(540, hintY, open)], [lerp(cx + 130, px0 + pw - 54, open), lerp(540, hintY, open)]];
    [[ku0, '⌘', 'command'], [ku1, 'F', '']].forEach(([u, lab, sub], i) => {
      if (u <= 0) return;
      g.save(); g.globalAlpha = u * contentA; const [kx, ky] = pos[i]; const ss = s * lerp(0.8, 1, u);
      keycap(g, kx, ky + (1 - u) * 30 * (1 - open), ss, lab, press * (1 - open), sub);
      g.restore();
    });
  }
  // opening: brand-blue frame from the zoomed check splits on the diagonal
  const sp = ev(t, at(15), 0.5);
  if (sp < 1) {
    const a = -Math.PI / 4, d = [Math.cos(a), Math.sin(a)], n = [-Math.sin(a), Math.cos(a)], off = sp * 1300;
    for (const sgn of [-1, 1]) {
      g.save(); g.translate(n[0] * off * sgn, n[1] * off * sgn);
      g.beginPath(); const c0 = [W / 2, H / 2];
      const p = (o, l) => [c0[0] + n[0] * o + d[0] * l, c0[1] + n[1] * o + d[1] * l];
      g.moveTo(...p(0, -3000)); g.lineTo(...p(0, 3000)); g.lineTo(...p(3000 * sgn, 3000)); g.lineTo(...p(3000 * sgn, -3000)); g.closePath();
      g.fillStyle = ACC; g.fill();
      g.strokeStyle = rgba(WHITE, 0.9 * (1 - sp)); g.lineWidth = 3; g.beginPath(); g.moveTo(...p(0, -3000)); g.lineTo(...p(0, 3000)); g.stroke();
      g.restore();
    }
  }
}

// =================================================================== S5  21B → 27B
// the main screen as a board in perspective; cards lift and reorder; Open. Click. Share.
const CARDS = {
  L1: { t: 'districts', m: '17 areas · EPSG:4326', c: 'GeoJSON', kind: 'ar', col: ACC },
  L2: { t: 'cases', m: 'cases.csv · 17 rows', c: 'CSV · joined', kind: 'id', col: GREEN },
  L3: { t: 'elevation', m: 'elevation.tif · grid', c: 'GeoTIFF', kind: 'gr', col: GREEN },
  L4: { t: 'heatwaves', m: 'summer 2023.nc · grid', c: 'NetCDF', kind: 'gr', col: AMBER },
  O1: { t: 'Haiyan', m: 'storms · line', pct: 'higher than 99% of 4,547 storms', kind: 'ln', col: ACC, v: 0.99 },
  O2: { t: 'D07', m: 'districts · area', pct: 'higher than 88% of 17 districts', kind: 'ar', col: ACC, v: 0.88 },
};
// layouts: [card] = [column, index]
const LAYOUTS = [
  { L1: [0, 0], L2: [0, 1], L3: [0, 2], L4: [0, 3], O2: [1, 0], O1: [1, 1] },
  { L2: [0, 0], L3: [0, 1], L4: [0, 2], L1: [1, 0], O2: [1, 1], O1: [1, 2] },  // 22B districts → Click
  { L2: [0, 0], L3: [0, 1], L4: [0, 2], O1: [1, 0], L1: [1, 1], O2: [1, 2] },  // 23B Haiyan to top
  { L3: [0, 0], L4: [0, 1], O1: [1, 0], L1: [1, 1], L2: [1, 2], O2: [1, 3] },  // 24B cases joins
  { L3: [0, 0], L4: [0, 1], O1: [1, 0], L2: [1, 1], O2: [1, 2], L1: [2, 0] },  // 25B districts → Share
  { L3: [0, 0], L4: [0, 1], L2: [1, 0], O2: [1, 1], L1: [2, 0], O1: [2, 1] },  // 26B Haiyan → Share
];
const MOVERS = [null, 'L1', 'O1', 'L2', 'L1', 'O1'];
const COLS = ['Open', 'Click', 'Share'];
const CW = 380, CH = 150, COLX = c => -650 + c * 450, SLOTY = j => -330 + j * 166;
const shadowSprite = (() => { const c = mk(CW + 160, CH + 160), g = c.getContext('2d'); g.filter = 'blur(22px)'; rrect(g, 80, 80, CW, CH, 18); g.fillStyle = 'rgba(0,0,0,0.85)'; g.fill(); return c; })();
function boardCam(t) {
  const u = ev(t, at(21), 0.9);
  return { th: lerp(-0.20, -0.15, seg(t, at(21), at(27))) + lerp(0.06, 0, u), ph: lerp(0.78, 0.64, u), D: 2400, s: lerp(0.98, 1.06, u) + 0.04 * seg(t, at(21), at(27)), cx: 1010, cy: lerp(560, 530, u) };
}
function proj3(cam, u, v, h) {
  const ct = Math.cos(cam.th), st = Math.sin(cam.th);
  const x1 = (u * ct - v * st) * cam.s, y1 = (u * st + v * ct) * cam.s, hh = h * cam.s;
  const cp = Math.cos(cam.ph), sp = Math.sin(cam.ph);
  const y2 = y1 * cp - hh * sp, z2 = hh * cp - y1 * sp;
  const k = cam.D / (cam.D - z2);
  return [cam.cx + x1 * k, cam.cy + y2 * k];
}
function setPlane(g, cam, u, v, h, w, hgt) {
  const O = proj3(cam, u, v, h), X = proj3(cam, u + w, v, h), Y = proj3(cam, u, v + hgt, h);
  g.setTransform((X[0] - O[0]) / w, (X[1] - O[1]) / w, (Y[0] - O[0]) / hgt, (Y[1] - O[1]) / hgt, O[0], O[1]);
}
function quad(g, cam, u0, v0, u1, v1, h = 0) {
  g.beginPath(); const p = [proj3(cam, u0, v0, h), proj3(cam, u1, v0, h), proj3(cam, u1, v1, h), proj3(cam, u0, v1, h)];
  g.moveTo(...p[0]); g.lineTo(...p[1]); g.lineTo(...p[2]); g.lineTo(...p[3]); g.closePath();
}
function drawCard(g, id, lifted) {
  const c = CARDS[id];
  rrect(g, 0, 0, CW, CH, 18); g.fillStyle = lifted > 0 ? rgba(mix([24, 25, 33], [30, 32, 42], lifted)) : '#181921'; g.fill();
  const lg = g.createLinearGradient(0, 0, 0, CH); lg.addColorStop(0, 'rgba(255,255,255,0.05)'); lg.addColorStop(1, 'rgba(255,255,255,0)');
  rrect(g, 0, 0, CW, CH, 18); g.fillStyle = lg; g.fill();
  rrect(g, 0.5, 0.5, CW - 1, CH - 1, 18); g.strokeStyle = rgba(mix([255, 255, 255], ACC, lifted * 0.6), 0.1 + lifted * 0.35); g.lineWidth = 1.2; g.stroke();
  rrect(g, 22, 22, 42, 42, 11); g.fillStyle = rgba(c.col, 0.16); g.fill(); glyph(g, c.kind, 43, 43, 20, c.col);
  text(g, c.t, 80, 52, { size: 27 });
  text(g, c.m, 80, 84 - 0, { size: 18, weight: 400, fam: MONO, color: 'rgba(138,143,152,1)' });
  if (c.pct) {
    // percentile strip like the app's object card
    for (let i = 0; i < 26; i++) { g.fillStyle = 'rgba(255,255,255,0.16)'; g.fillRect(22 + i * 5.2, 108, 2, 16); }
    g.fillStyle = 'rgba(255,255,255,0.7)'; g.fillRect(22 + 13 * 5.2, 104, 2.5, 24);
    g.fillStyle = ACC; g.beginPath(); g.arc(22 + 25 * 5.2 * c.v + 1, 116, 7, 0, TAU); g.fill();
    text(g, c.pct.replace('higher than ', ''), 172, 121, { size: 15, weight: 400, fam: MONO, color: 'rgba(200,204,212,0.9)' });
  } else {
    const cw = tw(g, c.c, 16, 400, MONO) + 22;
    rrect(g, 80, 104, cw, 28, 14); g.fillStyle = 'rgba(255,255,255,0.05)'; g.fill(); g.strokeStyle = 'rgba(255,255,255,0.12)'; g.lineWidth = 1; g.stroke();
    text(g, c.c, 80 + cw / 2, 119, { size: 16, weight: 400, fam: MONO, align: 'center', base: 'middle', color: 'rgba(200,204,212,0.9)' });
    // status: accent disc with check ("On the map")
    g.fillStyle = ACC; g.beginPath(); g.arc(CW - 34, 40, 12, 0, TAU); g.fill(); checkPath(g, CW - 34, 40, 12, 1, 3, WHITE);
  }
}
function S5(g, t) {
  bg(g, [[1000, 380, 900, ACC, 0.07]]);
  const cam = boardCam(t);
  // floor grid
  g.save(); g.strokeStyle = 'rgba(255,255,255,0.045)'; g.lineWidth = 1;
  for (let u = -1400; u <= 1400; u += 90) { const a = proj3(cam, u, -1100, 0), b = proj3(cam, u, 1100, 0); g.beginPath(); g.moveTo(...a); g.lineTo(...b); g.stroke(); }
  for (let v = -1100; v <= 1100; v += 90) { const a = proj3(cam, -1400, v, 0), b = proj3(cam, 1400, v, 0); g.beginPath(); g.moveTo(...a); g.lineTo(...b); g.stroke(); }
  g.restore();
  // columns
  COLS.forEach((name, c) => {
    const u0 = COLX(c) - 18, v0 = -440;
    g.setTransform(1, 0, 0, 1, 0, 0);
    quad(g, cam, u0, v0, u0 + CW + 36, 560); g.fillStyle = 'rgba(255,255,255,0.025)'; g.fill(); g.strokeStyle = 'rgba(255,255,255,0.07)'; g.lineWidth = 1; g.stroke();
    setPlane(g, cam, COLX(c), -420, 0, 300, 60);
    text(g, name, 0, 44, { size: 30 });
  });
  // card positions
  const step = clamp(Math.floor((t - at(22)) / B) + 1, 0, 5);
  const pos = {};
  for (const id of Object.keys(CARDS)) {
    let [c, j] = LAYOUTS[0][id]; let u = COLX(c), v = SLOTY(j), h = 0;
    for (let s = 1; s <= 5; s++) {
      const ts = at(21 + s); if (t < ts) break;
      const [c2, j2] = LAYOUTS[s][id], u2 = COLX(c2), v2 = SLOTY(j2);
      if (MOVERS[s] === id) {
        const lift = ev(t, ts, 0.12, quintOut) * (1 - ev(t, ts + 0.32, 0.16, quintOut));
        const mv = ev(t, ts + 0.04, 0.36, expoInOut);
        u = lerp(u, u2, mv); v = lerp(v, v2, mv); h = lift * 90;
      } else { const mv = ev(t, ts + 0.1, 0.34); u = lerp(u, u2, mv); v = lerp(v, v2, mv); }
    }
    pos[id] = { u, v, h };
  }
  // column counts
  g.setTransform(1, 0, 0, 1, 0, 0);
  const order = Object.keys(pos).sort((a, b) => pos[a].h - pos[b].h || pos[a].v - pos[b].v);
  // shadows first (on floor)
  for (const id of order) {
    const p = pos[id], lift = p.h / 90;
    setPlane(g, cam, p.u - 80 + lift * 10, p.v - 80 + 10 + lift * 40, 0, CW + 160, CH + 160);
    g.globalAlpha = lerp(0.55, 0.35, lift); g.drawImage(shadowSprite, 0, 0); g.globalAlpha = 1;
  }
  for (const id of order) {
    const p = pos[id];
    setPlane(g, cam, p.u, p.v, p.h, CW, CH);
    drawCard(g, id, p.h / 90);
  }
  g.setTransform(1, 0, 0, 1, 0, 0);
  // bottom-left verbs
  const sh = g.createRadialGradient(240, 1000, 0, 240, 1000, 760); sh.addColorStop(0, 'rgba(11,12,16,0.85)'); sh.addColorStop(1, 'rgba(11,12,16,0)');
  g.fillStyle = sh; g.fillRect(0, 300, 1100, 780);
  vignette(g, 0.4);
  const verbs = [['Open.', at(22)], ['Click.', at(24)], ['Share.', at(26)]];
  let x = 112; const size = 112, ls = -3.5;
  verbs.forEach(([w, t0], i) => {
    const u = ev(t, t0, 0.32), ww = tw(g, w, size, 600, SANS, ls);
    if (u > 0) {
      g.save(); g.beginPath(); g.rect(x - 10, 870, ww + 30, 160); g.clip();
      text(g, w, x, 990 + (1 - u) * 130, { size, ls, color: i === 2 ? ACC : WHITE });
      g.restore();
    }
    x += ww + 30;
  });
}

// =================================================================== S6  27B → 34B
// four metric cards in isometric perspective
const MW = 560, MH = 360;
const METRIC_POS = [[-585, -385], [25, -385], [-585, 25], [25, 25]];
const CHART = [3, 6, 9, 15, 19, 27, 33, 42, 49, 58, 66, 74, 80, 85];
function S6(g, t) {
  bg(g, [[960, 560, 900, ACC, 0.08]]);
  const tilt = ev(t, at(32), 1.2 * B * 2, quintInOut);
  const push = seg(t, at(32.9), at(34));
  const k = 0.9 * Math.pow(9, Math.pow(push, 2.6));
  const sq = lerp(1, 0.55, tilt);
  const cx = 960, cy = lerp(560, 600, tilt) + push * 120;
  const ex = [0.866 * k, -0.5 * k * sq], ey = [0.866 * k, 0.5 * k * sq], ez = [0, -k * lerp(1, 0.8, tilt)];
  const P = (x, y, z = 0) => [cx + x * ex[0] + y * ey[0] + z * ez[0], cy + x * ex[1] + y * ey[1] + z * ez[1]];
  const fadeOut = 1 - seg(t, at(33.55), at(34));
  // iso floor grid
  g.save(); g.globalAlpha = fadeOut; g.strokeStyle = 'rgba(255,255,255,0.05)'; g.lineWidth = 1;
  for (let i = -1500; i <= 1500; i += 80) { let a = P(i, -1500), b = P(i, 1500); g.beginPath(); g.moveTo(...a); g.lineTo(...b); g.stroke(); a = P(-1500, i); b = P(1500, i); g.beginPath(); g.moveTo(...a); g.lineTo(...b); g.stroke(); }
  g.restore();
  vignette(g, 0.55);
  const T0 = at(28);
  METRIC_POS.forEach(([x0, y0], i) => {
    const inU = ev(t, at(27) + 0.06 + i * 0.07, 0.5);
    if (inU <= 0) return;
    const hz = lerp(-70, 16, inU), th = 16;
    g.save(); g.globalAlpha = clamp(inU * 1.6) * fadeOut;
    // floor shadow
    g.setTransform(ex[0], ex[1], ey[0], ey[1], ...P(x0 - 10, y0 + 30, 0));
    g.save(); g.filter = 'blur(10px)'; rrect(g, 0, 0, MW, MH, 22); g.fillStyle = `rgba(0,0,0,${0.55 * inU})`; g.fill(); g.restore();
    // side faces (thickness)
    g.setTransform(1, 0, 0, 1, 0, 0);
    const c = [P(x0, y0, hz), P(x0, y0 + MH, hz), P(x0 + MW, y0 + MH, hz)];
    const cb = [P(x0, y0, hz - th), P(x0, y0 + MH, hz - th), P(x0 + MW, y0 + MH, hz - th)];
    g.fillStyle = '#0E0F14'; g.beginPath(); g.moveTo(...c[0]); g.lineTo(...c[1]); g.lineTo(...cb[1]); g.lineTo(...cb[0]); g.closePath(); g.fill();
    g.fillStyle = '#121319'; g.beginPath(); g.moveTo(...c[1]); g.lineTo(...c[2]); g.lineTo(...cb[2]); g.lineTo(...cb[1]); g.closePath(); g.fill();
    // top face
    g.setTransform(ex[0], ex[1], ey[0], ey[1], ...P(x0, y0, hz));
    panel(g, 0, 0, MW, MH, 22, { shadow: false, fill: '#191A22', border: 'rgba(255,255,255,0.12)' });
    metric(g, i, t, T0);
    g.restore();
  });
  g.setTransform(1, 0, 0, 1, 0, 0);
}
function metric(g, i, t, T0) {
  const head = (a, b) => { text(g, a, 34, 60, { size: 30 }); text(g, b, 34, 94, { size: 19, weight: 400, fam: MONO, color: 'rgba(138,143,152,1)' }); };
  if (i === 0) {
    head('Coverage', 'heatwaves summer 2023 · grid');
    const p = ev(t, T0, at(30.5) - T0, quintInOut);
    const val = Math.round(CHART[CHART.length - 1] * p);
    text(g, val + '%', MW - 34, 104, { size: 76, ls: -2, align: 'right' });
    const x0 = 34, x1 = MW - 34, y0 = 330, y1 = 150;
    g.strokeStyle = 'rgba(255,255,255,0.07)'; g.lineWidth = 1;
    for (let k = 0; k <= 3; k++) { const y = lerp(y0, y1, k / 3); g.beginPath(); g.moveTo(x0, y); g.lineTo(x1, y); g.stroke(); }
    const pts = CHART.map((v, k) => [lerp(x0, x1, k / (CHART.length - 1)), lerp(y0, y1, v / 100)]);
    if (p > 0) {
      g.save(); const head = polyPart(g, pts, p); const ag = g.createLinearGradient(0, y1, 0, y0); ag.addColorStop(0, rgba(ACC, 0.35)); ag.addColorStop(1, rgba(ACC, 0));
      g.lineTo(head[0], y0); g.lineTo(x0, y0); g.closePath(); g.fillStyle = ag; g.fill();
      polyPart(g, pts, p); g.strokeStyle = ACC; g.lineWidth = 4; g.lineJoin = 'round'; g.shadowColor = rgba(ACC, 0.8); g.shadowBlur = 14; g.stroke(); g.shadowBlur = 0;
      g.fillStyle = WHITE; g.beginPath(); g.arc(head[0], head[1], 6, 0, TAU); g.fill(); g.restore();
    }
  } else if (i === 1) {
    const names = ['districts.geojson', 'cases.csv', 'elevation.tif', 'heatwaves summer 2023.nc'];
    const done = names.map((_, k) => ev(t, T0 + 0.1 + k * 0.2, 1.0 + k * 0.25, quintInOut));
    const n = done.filter(d => Math.round(d * 100) === 100).length;
    head('Layers', n === 4 ? '4 of 4 on the map' : `loading · ${n} of 4`);
    names.forEach((nm, k) => {
      const y = 140 + k * 54, p = done[k];
      text(g, nm, 34, y + 6, { size: 18, weight: 400, fam: MONO, color: 'rgba(225,228,235,0.92)' });
      text(g, Math.round(p * 100) + '%', MW - 34, y + 6, { size: 18, weight: 400, fam: MONO, align: 'right', color: Math.round(p * 100) === 100 ? GREEN : 'rgba(170,175,185,1)' });
      rrect(g, 34, y + 18, MW - 68, 8, 4); g.fillStyle = 'rgba(255,255,255,0.08)'; g.fill();
      if (p > 0) { rrect(g, 34, y + 18, (MW - 68) * p, 8, 4); g.fillStyle = Math.round(p * 100) === 100 ? GREEN : ACC; g.fill(); }
    });
  } else if (i === 2) {
    const ticks = [at(29), at(29.5), at(30), at(30.5)];
    const nDone = ticks.filter(x => t >= x).length;
    const zero = nDone === 4;
    head('Not placed', 'cases.csv → districts');
    // rolling digit
    const cur = 4 - nDone, prev = cur + 1, u = nDone ? ev(t, ticks[nDone - 1], 0.22) : 1;
    g.save(); g.beginPath(); g.rect(20, 120, 260, 210); g.clip();
    const dy = 190;
    if (u < 1) text(g, String(prev), 34, 300 - u * dy, { size: 190, ls: -6, color: WHITE });
    text(g, String(cur), 34, 300 + (1 - u) * dy, { size: 190, ls: -6, color: zero ? rgba(mix(WHITE, GREEN, u)) : WHITE });
    g.restore();
    text(g, 'records not placed', 230, 250, { size: 21, weight: 400, fam: MONO, color: 'rgba(160,165,175,1)' });
    const zu = zero ? ev(t, ticks[3] + 0.05, 0.3) : 0;
    g.save(); g.globalAlpha = zu; g.fillStyle = GREEN; g.beginPath(); g.arc(242, 290, 11, 0, TAU); g.fill(); checkPath(g, 242, 290, 11, zu, 3, '#0B0C10');
    text(g, 'all 17 districts placed', 262, 297, { size: 19, weight: 400, fam: MONO, color: GREEN }); g.restore();
  } else {
    const fl = at(31);
    const u1 = seg(t, fl, fl + 0.12), u2 = seg(t, fl + 0.12, fl + 0.3);
    const pro = t >= fl + 0.12;
    head('Plan', pro ? 'NimbleMap Pro · Ask box' : 'NimbleMap · free');
    const sy = pro ? quintOut(u2) : 1 - quintIn(u1);
    const lab = pro ? 'Pro' : 'Ready', col = pro ? ACC : GREEN;
    g.save(); g.translate(34, 172); g.scale(1, Math.max(0.001, sy));
    const bw = tw(g, lab, 40) + 64;
    rrect(g, 0, -38, bw, 76, 38);
    if (pro) { g.fillStyle = ACC; g.fill(); } else { g.fillStyle = rgba(GREEN, 0.16); g.fill(); g.strokeStyle = rgba(GREEN, 0.7); g.lineWidth = 1.5; g.stroke(); }
    text(g, lab, bw / 2, 2, { size: 40, align: 'center', base: 'middle', color: pro ? WHITE : GREEN });
    g.restore();
    // Ask box
    const q = 'Which district has the most cases?';
    const ty = ev(t, fl + 0.2, 0.62, x => x);
    rrect(g, 34, 250, MW - 68, 70, 16); g.fillStyle = 'rgba(255,255,255,0.04)'; g.fill(); g.strokeStyle = rgba(pro ? ACC : [255, 255, 255], pro ? 0.6 : 0.1); g.lineWidth = 1.2; g.stroke();
    const shown = pro ? q.slice(0, Math.round(q.length * ty)) : '';
    text(g, shown || 'Ask', 58, 286, { size: 21, weight: 400, base: 'middle', color: shown ? 'rgba(235,237,242,0.95)' : 'rgba(138,143,152,0.8)' });
  }
}

// =================================================================== S7  34B → 20 s
// beams; Peek mark; slide to reveal wordmark; tagline; pill; sheen; fade to black
const logoImg = new Image(); let logoReady = false;
logoImg.onload = () => { logoReady = true; }; logoImg.src = '/design/logo-nimble/variants/concept-8-peek-dark.svg';
const WM = mk(1400, 260), wmc = WM.getContext('2d'), BAND = mk(1400, 260), bandc = BAND.getContext('2d');
function S7(g, t) {
  bg(g);
  const t0 = at(34);
  const cx = W / 2, cy = 452;
  // beams
  const bloom = 1 + 1.2 * (1 - ev(t, t0, 0.9));
  const inten = ev(t, t0 - 0.05, 0.5) * bloom;
  g.save(); g.globalCompositeOperation = 'lighter';
  const n = 16;
  for (let i = 0; i < n; i++) {
    const a = i / n * TAU + (t - t0) * 0.07 + 0.1, w = 0.07 + 0.05 * (0.5 + 0.5 * Math.sin(i * 2.3));
    const L = 1500;
    const rg = g.createRadialGradient(cx, cy, 40, cx, cy, L);
    const al = (0.05 + 0.04 * (0.5 + 0.5 * Math.sin(i * 1.7 + 1))) * inten;
    rg.addColorStop(0, rgba(ACC, al)); rg.addColorStop(0.35, rgba(ACC, al * 0.45)); rg.addColorStop(1, rgba(ACC, 0));
    g.fillStyle = rg; g.beginPath(); g.moveTo(cx, cy); g.arc(cx, cy, L, a - w / 2, a + w / 2); g.closePath(); g.fill();
  }
  glow(g, cx, cy, 620, ACC, 0.16 * inten);
  glow(g, cx, cy, 220, '#9DB8F5', 0.10 * inten);
  g.restore();
  vignette(g, 0.5);

  // lockup metrics
  const markImg = 280, body = markImg * 824 / 1024, gap = 46;
  const wsize = 136, wls = -3.5, ww = tw(g, 'NimbleMap', wsize, 600, SANS, wls);
  const csize = 66, cw = tw(g, '凌波微图', csize, 500, CJK, 4), dgap = 40;
  const total = body + gap + ww + dgap + 1 + dgap + cw;
  const left = cx - total / 2;
  const slide = ev(t, at(35), 0.55);
  const mcx = lerp(cx, left + body / 2, slide);
  // mark
  const mu = ev(t, t0, 0.5);
  if (mu > 0 && logoReady) {
    const s = lerp(0.78, 1, mu);
    g.save(); g.globalAlpha = mu;
    g.save(); g.globalCompositeOperation = 'lighter'; glow(g, mcx, cy, 260, ACC, 0.25 * mu * (1 - 0.5 * slide)); g.restore();
    g.shadowColor = 'rgba(0,0,0,0.6)'; g.shadowBlur = 40; g.shadowOffsetY = 18;
    g.drawImage(logoImg, mcx - markImg * s / 2, cy - markImg * s / 2, markImg * s, markImg * s);
    g.restore();
  }
  // wordmark (revealed from behind the mark)
  if (slide > 0) {
    const wx = left + body + gap, base = cy + wsize * 0.36;
    const clipL = mcx + body / 2 + 6;
    wmc.setTransform(1, 0, 0, 1, 0, 0); wmc.clearRect(0, 0, WM.width, WM.height);
    text(wmc, 'NimbleMap', 0, 180, { size: wsize, ls: wls, color: '#E3E6EC' });
    // sheen
    const shU = seg(t, at(38), at(38) + 0.62);
    if (shU > 0 && shU < 1) {
      bandc.setTransform(1, 0, 0, 1, 0, 0); bandc.clearRect(0, 0, BAND.width, BAND.height);
      bandc.globalCompositeOperation = 'source-over'; bandc.drawImage(WM, 0, 0);
      bandc.globalCompositeOperation = 'source-in';
      const bx = lerp(-300, ww + 300, expoInOut(shU));
      const lg = bandc.createLinearGradient(bx - 120, 0, bx + 120, 60);
      lg.addColorStop(0, 'rgba(255,255,255,0)'); lg.addColorStop(0.5, 'rgba(255,255,255,1)'); lg.addColorStop(1, 'rgba(255,255,255,0)');
      bandc.fillStyle = lg; bandc.fillRect(0, 0, BAND.width, BAND.height); bandc.globalCompositeOperation = 'source-over';
    }
    const off = (1 - slide) * -120;
    g.save(); g.beginPath(); g.rect(clipL, cy - 200, W, 400); g.clip();
    g.globalAlpha = clamp(slide * 1.5);
    g.drawImage(WM, wx + off, base - 180);
    if (shU > 0 && shU < 1) {
      g.globalCompositeOperation = 'lighter'; g.drawImage(BAND, wx + off, base - 180);
      g.filter = 'blur(14px)'; g.globalAlpha = 0.8; g.drawImage(BAND, wx + off, base - 180); g.filter = 'none';
      g.globalCompositeOperation = 'source-over';
    }
    // divider + Chinese name
    const dx = wx + ww + dgap + off, cu = ev(t, at(35) + 0.12, 0.5);
    g.globalAlpha = cu; g.fillStyle = 'rgba(255,255,255,0.2)'; g.fillRect(dx, cy - 44, 1, 88);
    text(g, '凌波微图', dx + 1 + dgap + (1 - cu) * -40, cy + csize * 0.36, { size: csize, weight: 500, fam: CJK, ls: 4, color: GRAY });
    g.restore();
  }
  // tagline
  const tu = ev(t, at(36), 0.6, cubicOut);
  if (tu > 0) {
    g.save(); g.globalAlpha = tu;
    text(g, 'Open a map file. See what it says.', cx, 676 + (1 - tu) * 18, { size: 46, weight: 400, ls: -0.5, align: 'center', color: 'rgba(235,237,242,0.86)' });
    g.restore();
  }
  // pill
  const pu = ev(t, at(37), 0.4);
  if (pu > 0) {
    const label = 'kangning-huang.github.io/NimbleMap', fs = 27;
    const lw = tw(g, label, fs, 400, MONO), pw = lw + 70 + 26, ph = 66, py = 770;
    g.save(); g.globalAlpha = pu; g.translate(cx, py); g.scale(lerp(0.9, 1, pu), lerp(0.9, 1, pu)); g.translate(-cx, -py);
    rrect(g, cx - pw / 2, py - ph / 2, pw, ph, ph / 2); g.fillStyle = 'rgba(255,255,255,0.05)'; g.fill();
    rrect(g, cx - pw / 2 + 0.5, py - ph / 2 + 0.5, pw - 1, ph - 1, ph / 2); g.strokeStyle = 'rgba(255,255,255,0.16)'; g.lineWidth = 1; g.stroke();
    const ix = cx - pw / 2 + 40;
    g.save(); g.shadowColor = rgba(ACC, 0.9); g.shadowBlur = 12; g.fillStyle = ACC; g.beginPath(); g.arc(ix, py, 9, 0, TAU); g.fill(); g.restore();
    text(g, label, ix + 30, py + 1, { size: fs, weight: 400, fam: MONO, base: 'middle', color: 'rgba(240,242,246,0.95)' });
    g.restore();
  }
  // fade to black (the one plain fade the brief asks for)
  const fb = Math.pow(seg(t, 19.3, 19.95), 1.6);
  if (fb > 0) { g.fillStyle = `rgba(0,0,0,${fb})`; g.fillRect(0, 0, W, H); }
}

// =================================================================== transitions
const LA = mk(W, H), LB = mk(W, H), la = LA.getContext('2d'), lb = LB.getContext('2d');
function T_blade(g, t, A, Bf, t0, t1) {
  const u = expoInOut(seg(t, t0, t1));
  A(la, t); Bf(lb, t);
  const { d, nn, P } = bladeGeom(t0, t1, t);
  bg(g);
  g.drawImage(LA, d[0] * u * 60, d[1] * u * 60);
  g.save(); g.beginPath();
  const q = (o, l) => [P[0] + nn[0] * o + d[0] * l, P[1] + nn[1] * o + d[1] * l];
  const side = nn[0] > 0 ? -1 : 1; // revealed region is behind the blade (left)
  g.moveTo(...q(0, -2500)); g.lineTo(...q(0, 2500)); g.lineTo(...q(4000 * side, 2500)); g.lineTo(...q(4000 * side, -2500)); g.closePath(); g.clip();
  g.drawImage(bgCanvas, 0, 0);
  g.drawImage(LB, -d[0] * (1 - u) * 80, -d[1] * (1 - u) * 80);
  g.restore();
}
function bladeGeom(t0, t1, t) {
  const ang = -62 * Math.PI / 180, d = [Math.cos(ang), Math.sin(ang)], nn = [-d[1], d[0]];
  const Px = tt => lerp(-700, W + 700, expoInOut(seg(tt, t0, t1)));
  return { d, nn, P: [Px(t), H / 2], v: (Px(t + 0.002) - Px(t - 0.002)) / 0.004 };
}
// blade drawn once per frame after motion-blur accumulation, as a streak
function bladeOverlay(g, t, t0, t1) {
  const { d, nn, P, v } = bladeGeom(t0, t1, t);
  const smear = Math.max(6, Math.abs(v) / (FPS * 2));
  const q = (o, l) => [P[0] + nn[0] * o + d[0] * l, P[1] + nn[1] * o + d[1] * l];
  g.save(); g.globalCompositeOperation = 'lighter';
  const s0 = q(-smear, 0), s1 = q(0, 0);
  const lg = g.createLinearGradient(s0[0], s0[1], s1[0], s1[1]);
  lg.addColorStop(0, rgba(ACC, 0)); lg.addColorStop(1, rgba(ACC, 0.55));
  g.fillStyle = lg; g.beginPath(); g.moveTo(...q(-smear, -2500)); g.lineTo(...q(-smear, 2500)); g.lineTo(...q(0, 2500)); g.lineTo(...q(0, -2500)); g.closePath(); g.fill();
  g.shadowColor = rgba(ACC, 1); g.shadowBlur = 36; g.strokeStyle = WHITE; g.lineWidth = 4;
  g.beginPath(); g.moveTo(...q(0, -2500)); g.lineTo(...q(0, 2500)); g.stroke();
  g.restore();
}
function T_push(g, t, A, Bf, t0, t1) {
  const u = seg(t, t0, t1);
  A(la, t); Bf(lb, t);
  g.setTransform(1, 0, 0, 1, 0, 0); g.fillStyle = BG0; g.fillRect(0, 0, W, H);
  const ua = expoIn(clamp(u * 1.3)), sa = lerp(1, 0.62, ua);
  g.save(); g.globalAlpha = 1 - clamp(u * 1.6); g.translate(W / 2, H / 2); g.scale(sa, sa); g.translate(-W / 2, -H / 2); g.drawImage(LA, 0, 0); g.restore();
  const ub = expoOut(clamp((u - 0.25) / 0.75)), sb = lerp(1.6, 1, ub);
  g.save(); g.globalAlpha = clamp((u - 0.2) * 2.5); g.translate(W / 2, H / 2); g.scale(sb, sb); g.translate(-W / 2, -H / 2); g.drawImage(LB, 0, 0); g.restore();
}
function T_slice(g, t, A, Bf, t0, t1) {
  const u = seg(t, t0, t1);
  A(la, t); Bf(lb, t);
  bg(g);
  const ib = expoOut(clamp((u - 0.15) / 0.85)), sb = lerp(1.12, 1, ib);
  g.save(); g.translate(W / 2, H / 2); g.scale(sb, sb); g.translate(-W / 2, -H / 2); g.drawImage(LB, 0, 0); g.restore();
  const ang = Math.PI / 4, d = [Math.cos(ang), Math.sin(ang)], n = [-Math.sin(ang), Math.cos(ang)], w = 420;
  const edges = [-1e4, -w * 1.5, -w * 0.5, w * 0.5, w * 1.5, 1e4];
  for (let i = 0; i < 5; i++) {
    const e = expoIn(clamp((u - i * 0.06) / 0.7)), sgn = i % 2 ? 1 : -1, disp = sgn * e * 2800;
    g.save(); g.beginPath();
    const p = (o, l) => [W / 2 + n[0] * o + d[0] * l, H / 2 + n[1] * o + d[1] * l];
    g.moveTo(...p(edges[i], -3000)); g.lineTo(...p(edges[i], 3000)); g.lineTo(...p(edges[i + 1], 3000)); g.lineTo(...p(edges[i + 1], -3000)); g.closePath(); g.clip();
    g.translate(d[0] * disp, d[1] * disp); g.drawImage(LA, 0, 0);
    g.restore();
    if (i > 0 && e < 1) { g.save(); g.globalCompositeOperation = 'lighter'; g.translate(d[0] * disp, d[1] * disp); g.shadowColor = ACC; g.shadowBlur = 24; g.strokeStyle = rgba(WHITE, 0.9 * (1 - e)); g.lineWidth = 2.5; g.beginPath(); g.moveTo(...p(edges[i], -3000)); g.lineTo(...p(edges[i], 3000)); g.stroke(); g.restore(); }
  }
}

// =================================================================== timeline
const BL0 = at(8.25), BL1 = at(9);
function drawFrame(g, t) {
  g.setTransform(1, 0, 0, 1, 0, 0); g.globalAlpha = 1; g.globalCompositeOperation = 'source-over'; g.filter = 'none';
  if (t < at(4)) S1(g, t);
  else if (t < BL0) S2(g, t);
  else if (t < BL1) T_blade(g, t, S2, S3, BL0, BL1);
  else if (t < at(15)) S3(g, t);
  else if (t < at(20.75)) S4(g, t);
  else if (t < at(21.3)) T_push(g, t, S4, S5, at(20.75), at(21.3));
  else if (t < at(26.5)) S5(g, t);
  else if (t < at(27.05)) T_slice(g, t, S5, S6, at(26.5), at(27.05));
  else if (t < at(34)) S6(g, t);
  else S7(g, t);
}
// motion blur: average sub-frames across a 180° shutter on fast moves
const MB = [[at(3.1), at(4.35)], [at(8.2), at(9.1)], [at(13.95), at(15.45)], [at(20.7), at(21.4)], [at(26.45), at(27.2)], [at(32.8), at(34.15)], [at(34), at(34.6)]];
function samples(t) {
  for (const [a, b] of MB) if (t >= a && t <= b) return 12;
  for (let k = 22; k <= 26; k++) if (t >= at(k) && t <= at(k) + 0.45) return 8;
  for (const k of [35]) if (t >= at(k) && t <= at(k) + 0.35) return 8;
  return 1;
}
const cv = document.getElementById('c'), ctx = cv.getContext('2d');
const SUB = mk(W, H), sub = SUB.getContext('2d'), ACCUM = mk(W, H), acc = ACCUM.getContext('2d');
window.renderFrame = t => {
  const n = samples(t);
  if (n === 1) drawFrame(ctx, t);
  else {
    const sh = 1 / (FPS * 2);
    for (let i = 0; i < n; i++) {
      const ti = t + (i / (n - 1) - 0.5) * sh;
      drawFrame(sub, ti);
      acc.setTransform(1, 0, 0, 1, 0, 0); acc.globalAlpha = 1 / (i + 1); acc.drawImage(SUB, 0, 0);
    }
    acc.globalAlpha = 1;
    ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.globalAlpha = 1; ctx.globalCompositeOperation = 'source-over'; ctx.drawImage(ACCUM, 0, 0);
  }
  if (t >= BL0 && t < BL1) bladeOverlay(ctx, t, BL0, BL1);
  grain(ctx, t);
  return true;
};
window.DURATION = 20;
Promise.all([
  document.fonts.load(fnt(40, 400)), document.fonts.load(fnt(40, 600)), document.fonts.load(fnt(40, 500, CJK), '凌波微图'),
  document.fonts.load(fnt(40, 400, MONO)), document.fonts.load(fnt(40, 500, MONO)),
  new Promise(r => { if (logoReady) r(); else { logoImg.addEventListener('load', r); logoImg.addEventListener('error', r); } }),
]).then(() => { buildTags(); window.renderFrame(0); window.__ready = true; });
})();
