// pdf.js in its sandboxed frame (frame.html). The reader page sends a PDF's bytes and asks for a
// page's text with its positions, a page picture, or a grey picture for text recognition. Settings
// as GrapheneOS's PDF Viewer: no eval (isEvalSupported false), no XFA forms, no scripting, no
// WebAssembly (the JavaScript JPEG 2000 decoder instead), no system fonts (only pdf.js's own) and no font
// loading by the browser at all (disableFontFace: a PDF's fonts are read by pdf.js itself and drawn as shapes; the frame's
// policy has no font-src, so default-src 'none' refuses every font from anywhere),
// images capped; the document comes as bytes, never fetched.
for (const k of ["RTCPeerConnection", "webkitRTCPeerConnection", "RTCDataChannel", "WebSocket", "EventSource"]) {
  try { Object.defineProperty(globalThis, k, { value: undefined, configurable: false, writable: false }); } catch (_) {}
}
try { if (navigator.sendBeacon) Object.defineProperty(navigator, "sendBeacon", { value: () => false, configurable: false, writable: false }); } catch (_) {}

import * as pdfjsLib from "./pdf.mjs";

const BASE = new URL("./", import.meta.url).href;
pdfjsLib.GlobalWorkerOptions.workerSrc = BASE + "pdf.worker.mjs";
let doc = null;

const IMAGE_OPS = new Set([pdfjsLib.OPS.paintImageXObject, pdfjsLib.OPS.paintInlineImageXObject,
  pdfjsLib.OPS.paintImageMaskXObject, pdfjsLib.OPS.paintImageXObjectRepeat]);

// the password the reader page's dialog gives back (null: the dialog was closed)
let passwordWaiter = null;

const MAX_PDF_PAGES = 5000;
const MAX_ITEMS_PER_PAGE = 40000;
const MAX_OPS_FOR_RULES = 200000;

async function open(bytes, requestId) {
  // (pdf.js 6: a document is closed through its loading task; the document itself has no destroy)
  if (doc) { const old = doc; doc = null; await old.loadingTask.destroy().catch(() => {}); }
  const task = pdfjsLib.getDocument({
    data: new Uint8Array(bytes),
    isEvalSupported: false, enableXfa: false, enableScripting: false, enablePermissions: false,
    useWasm: false, useSystemFonts: false, disableFontFace: true,
    disableRange: true, disableStream: true, disableAutoFetch: true, isOffscreenCanvasSupported: false,
    wasmUrl: BASE + "wasm/", standardFontDataUrl: BASE + "standard_fonts/", cMapUrl: BASE + "cmaps/", cMapPacked: true,
    maxImageSize: 64 * 1024 * 1024, stopAtErrors: false,
    verbosity: 0, // (errors only: pdf.js's warnings, which can name a PDF's fonts or contents, are not written to the console)
  });
  let cancelled = false;
  // An encrypted PDF asks for its password (again, when it was wrong): the reader page shows the dialog and answers with
  // {op: "password", value} or {op: "password", cancel: true}.
  task.onPassword = (update, reason) => {
    send({ needPassword: true, id: requestId, again: reason === pdfjsLib.PasswordResponses.INCORRECT_PASSWORD });
    new Promise(r => { passwordWaiter = r; }).then(value => {
      passwordWaiter = null;
      if (value == null) { cancelled = true; task.destroy().catch(() => {}); } else update(value);
    });
  };
  try {
    doc = await task.promise;
  } catch (err) {
    throw new Error(cancelled ? "password cancelled" : String((err && err.message) || err));
  }
  if (doc.numPages > MAX_PDF_PAGES) {
    const tooMany = doc.numPages;
    const old = doc; doc = null;
    await old.loadingTask.destroy().catch(() => {});
    throw new Error(`این PDF خیلی بزرگ است (${tooMany} صفحه؛ حداکثر ${MAX_PDF_PAGES} صفحه).`);
  }
  return { pages: doc.numPages };
}

// a page's text: [text, x0, top, x1, bottom, size, rtl, end of line] per piece, in the page's
// coordinates (points, top left at 0,0); images: whether a page without text has pictures
async function text(n) {
  const page = await doc.getPage(n);
  // (the page's memory is let go even when reading it fails: a damaged page must not stay in memory)
  try { return await textOf(page); } finally { page.cleanup(); }
}
// Detects an isolated short horizontal footnote separator rule drawn in the lower half of a PDF page.
// Emitting a synthetic "─────" marker at its exact y-position lets the Rust extractor reliably skip
// Persian footnotes (پانویس/پاورقی) even when they use the same font size as body text.
function findFootnoteRules(ops, vp) {
  const OPS = pdfjsLib.OPS;
  if (!ops || !Array.isArray(ops.fnArray)) return [];
  const stack = [];
  let ctm = vp.transform.slice();
  const pt = (x, y) => [ctm[0] * x + ctm[2] * y + ctm[4], ctm[1] * x + ctm[3] * y + ctm[5]];
  const raw = [];
  const addSeg = (x1, y1, x2, y2) => {
    const [ax, ay] = pt(x1, y1), [bx, by] = pt(x2, y2);
    if (Math.abs(ay - by) > 2.5) return;
    const x0 = Math.min(ax, bx), xEnd = Math.max(ax, bx), y = (ay + by) * 0.5;
    const w = xEnd - x0;
    if (w >= vp.width * 0.08 && w <= vp.width * 0.68 && y >= vp.height * 0.42 && y <= vp.height * 0.94) {
      raw.push({ x0, x1: xEnd, y });
    }
  };
  const addRect = (x, y, w, h) => {
    const [ax, ay] = pt(x, y), [bx, by] = pt(x + w, y + h);
    const rx0 = Math.min(ax, bx), rx1 = Math.max(ax, bx);
    const ry0 = Math.min(ay, by), ry1 = Math.max(ay, by);
    const rw = rx1 - rx0, rh = ry1 - ry0;
    if (rh <= 3.5 && rw >= vp.width * 0.08 && rw <= vp.width * 0.68 && ry0 >= vp.height * 0.42 && ry1 <= vp.height * 0.94) {
      raw.push({ x0: rx0, x1: rx1, y: (ry0 + ry1) * 0.5 });
    }
  };
  for (let i = 0; i < ops.fnArray.length; i++) {
    const fn = ops.fnArray[i], a = ops.argsArray[i];
    if (fn === OPS.save) {
      stack.push(ctm.slice());
    } else if (fn === OPS.restore) {
      if (stack.length) ctm = stack.pop();
    } else if (fn === OPS.transform && a && a.length === 6) {
      ctm = pdfjsLib.Util.transform(ctm, a);
    } else if (fn === OPS.rectangle && a && a.length === 4) {
      addRect(a[0], a[1], a[2], a[3]);
    } else if (fn === OPS.constructPath && a && Array.isArray(a[0]) && Array.isArray(a[1])) {
      const sub = a[0], coords = a[1];
      let cx = 0, cy = 0, ci = 0;
      for (let j = 0; j < sub.length; j++) {
        const op = sub[j];
        if (op === OPS.moveTo) { cx = coords[ci++]; cy = coords[ci++]; }
        else if (op === OPS.lineTo) { const nx = coords[ci++], ny = coords[ci++]; addSeg(cx, cy, nx, ny); cx = nx; cy = ny; }
        else if (op === OPS.rectangle) { const rx = coords[ci++], ry = coords[ci++], rw = coords[ci++], rh = coords[ci++]; addRect(rx, ry, rw, rh); }
        else if (op === OPS.curveTo) { cx = coords[ci + 4]; cy = coords[ci + 5]; ci += 6; }
        else if (op === OPS.curveTo2 || op === OPS.curveTo3) { cx = coords[ci + 2]; cy = coords[ci + 3]; ci += 4; }
      }
    }
  }
  // Deduplicate rules at nearly the same vertical y; skip pages with >=3 rules (tables/charts/underlined lists)
  const rules = [];
  for (const r of raw) {
    if (!rules.some(u => Math.abs(u.y - r.y) < 4)) rules.push(r);
  }
  return rules.length >= 1 && rules.length <= 2 ? rules : [];
}

async function textOf(page) {
  const vp = page.getViewport({ scale: 1 });
  // (unnormalized: a «لا» ligature stays one character; pdf.js's own expansion came out reversed
  // after its right-to-left reordering, «پلاسچی» → «پالسچی»; the app's repair expands it)
  const reader = page.streamTextContent({ disableNormalization: true }).getReader();
  const items = [];
  let remaining = 2_000_000;
  let complete = false;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) {
        complete = true;
        break;
      }
      for (const it of value.items) {
        if (typeof it.str === "string") {
          remaining -= it.str.length;
          if (remaining < 0) throw new Error("PDF page text exceeds the extraction limit");
        }
        if (typeof it.str === "string" && items.length >= MAX_ITEMS_PER_PAGE) throw new Error("PDF page has too many text items");
        if (typeof it.str !== "string") continue;
        const [a, b, c, d, e, f] = pdfjsLib.Util.transform(vp.transform, it.transform);
        const size = Math.hypot(c, d) || 1;
        if (!it.str.trim() && !it.hasEOL) continue;
        // Skip vertical/sideways margin or photo-credit runs so they never merge into horizontal body lines
        if (Math.abs(b) > Math.abs(a) * 1.5) continue;
        const x0 = Math.min(e, e + it.width), x1 = Math.max(e, e + it.width);
        const top = f - size * 0.8, bot = f + size * 0.2;
        // Skip off-page items outside the visible CropBox/viewport (e.g. facing page of a cropped spread)
        if (x1 <= 0.5 || x0 >= vp.width - 0.5 || bot <= 0.5 || top >= vp.height - 0.5) continue;
        items.push([it.str, x0, top, x1, bot, size, it.dir === "rtl" ? 1 : 0, it.hasEOL ? 1 : 0]);
      }
    }
  } finally {
    if (!complete) await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
  let images = false;
  try {
    const ops = await page.getOperatorList();
    images = ops.fnArray.some(f => IMAGE_OPS.has(f));
    if (items.some(i => i[0].trim()) && ops.fnArray.length <= MAX_OPS_FOR_RULES) {
      for (const r of findFootnoteRules(ops, vp)) {
        const above = items.filter(i => i[0].trim() && i[4] < r.y - 1).length;
        const below = items.filter(i => i[0].trim() && i[2] > r.y + 1).length;
        if (above >= 4 && below >= 1) {
          items.push(["─────", r.x0, r.y, r.x1, r.y + 1, 8, 1, 1]);
        }
      }
    }
  } catch {}
  return { w: vp.width, h: vp.height, items, images };
}

// A page is drawn no larger than this many pixels (GrapheneOS PDF Viewer's MAX_RENDER_PIXELS: 8,388,608): a page that claims
// to be metres wide would otherwise ask for a canvas of billions of pixels.
const MAX_RENDER_PIXELS = 8_388_608;
const MAX_VIEW_PIXELS = 4_000_000;

async function draw(n, scale, maxPixels = MAX_RENDER_PIXELS) {
  const page = await doc.getPage(n);
  try { return await drawPage(page, scale, maxPixels); } finally { page.cleanup(); }
}
async function drawPage(page, scale, maxPixels = MAX_RENDER_PIXELS) {
  let vp = page.getViewport({ scale });
  const pixels = vp.width * vp.height;
  if (!(pixels > 0) || !Number.isFinite(pixels)) throw new Error("this page has no usable size");
  if (pixels > maxPixels) vp = page.getViewport({ scale: scale * Math.sqrt(maxPixels / pixels) });
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.floor(vp.width)); canvas.height = Math.max(1, Math.floor(vp.height));
  const ctx = canvas.getContext("2d", { alpha: false });
  ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, canvas.width, canvas.height);
  // (intent "print": drawn at once, not frame by frame with requestAnimationFrame, which WebKit holds
  // back while the window is not in front: the picture never came)
  await page.render({ canvasContext: ctx, viewport: vp, intent: "print" }).promise;
  return canvas;
}

// a page picture `width` pixels wide, as fast JPEG (5x-10x faster & smaller than PNG on mobile)
async function render(n, width) {
  const page = await doc.getPage(n);
  let canvas;
  try {
    const baseW = Math.max(1, page.getViewport({ scale: 1 }).width);
    const scale = Math.min(6, Math.max(0.1, width / baseW));
    canvas = await drawPage(page, scale, MAX_VIEW_PIXELS);
  } finally {
    page.cleanup();
  }
  try {
    const blob = await new Promise(r => canvas.toBlob(b => r(b), "image/jpeg", 0.86));
    if (blob) return blob;
    return await new Promise(r => canvas.toBlob(r, "image/png"));
  } finally {
    canvas.width = 0; canvas.height = 0;
  }
}

// grey pixels at `dpi`, for text recognition
async function gray(n, dpi) {
  const canvas = await draw(n, dpi / 72, MAX_RENDER_PIXELS);
  try {
    const { width: w, height: h } = canvas;
    const rgba = canvas.getContext("2d").getImageData(0, 0, w, h).data;
    const px = new Uint8Array(w * h);
    for (let i = 0, j = 0; i < px.length; i++, j += 4) px[i] = (rgba[j] * 299 + rgba[j + 1] * 587 + rgba[j + 2] * 114) / 1000;
    return { w, h, px };
  } finally {
    canvas.width = 0; canvas.height = 0;
  }
}

// PDF outline/bookmarks for navigation (bounded depth & count to prevent untrusted PDF DoS)
const MAX_OUTLINE_DEPTH = 16;
const MAX_OUTLINE_ITEMS = 500;
async function outline() {
  if (!doc) throw new Error("no document");
  try {
    const dest = await doc.getOutline();
    if (!Array.isArray(dest)) return { bookmarks: [] };
    const bookmarks = [];
    const resolvePage = async rawDest => {
      try {
        let d = rawDest;
        if (typeof d === "string") d = await doc.getDestination(d);
        if (Array.isArray(d) && d[0] != null) {
          const idx = typeof d[0] === "number" ? d[0] : await doc.getPageIndex(d[0]);
          if (Number.isInteger(idx) && idx >= 0) return idx + 1;
        }
      } catch (_) {}
      return 1;
    };
    const flattenOutline = async (items, level) => {
      if (!Array.isArray(items) || level > MAX_OUTLINE_DEPTH) return;
      for (const item of items) {
        if (bookmarks.length >= MAX_OUTLINE_ITEMS) return;
        const page = item && item.dest ? await resolvePage(item.dest) : 1;
        bookmarks.push({
          title: String((item && item.title) || "بدون عنوان").slice(0, 200),
          page,
          level,
        });
        if (item && Array.isArray(item.items) && item.items.length > 0 && level < MAX_OUTLINE_DEPTH) {
          await flattenOutline(item.items, level + 1);
        }
      }
    };
    await flattenOutline(dest, 0);
    return { bookmarks };
  } catch (err) {
    return { bookmarks: [], error: String(err) };
  }
}

// Where pdf.js lives: in a sandboxed frame of the reader page (messages to and from the page), or, on the Mac, in a web view
// of its own (BRIDGE: it asks the app for its next job and posts its answers to the app; it never meets the reader page).
const BRIDGE = parent === window;
const VIEW = new URL("../view/", import.meta.url).href;
async function send(m, transfer) {
  if (!BRIDGE) return parent.postMessage(m, "*", transfer || []);
  let kind, body = "", extra = "";
  if (m.needPassword) { kind = "password"; extra = m.again ? "&again=1" : ""; }
  else if (!m.ok) { kind = "error"; body = String(m.error); }
  else if (m.data instanceof Blob) { kind = m.data.type === "image/jpeg" ? "jpeg" : "png"; body = await m.data.arrayBuffer(); } // (bytes: WebKit gives the app no body for a Blob)
  else if (m.data && m.data.px) {
    kind = "gray";
    body = new Uint8Array(8 + m.data.px.length);
    const dv = new DataView(body.buffer);
    dv.setUint32(0, m.data.w, true); dv.setUint32(4, m.data.h, true); body.set(m.data.px, 8);
  } else { kind = "json"; body = JSON.stringify(m.data); }
  fetch(`${VIEW}reply?id=${encodeURIComponent(String(m.id ?? ""))}&kind=${encodeURIComponent(kind)}${extra}`, { method: "POST", body }).catch(() => {});
}
let work = Promise.resolve();
function handle(m) {
  if (m.op === "password") { if (passwordWaiter) passwordWaiter(m.cancel ? null : String(m.value ?? "")); return; }
  work = work.catch(() => {}).then(async () => {
    const reply = (data, transfer) => send({ id: m.id, ok: true, data }, transfer);
    try {
      if (m.op === "open") reply(await open(m.data, m.id));
      else if (!doc) throw new Error("no document");
      else if (m.op === "text") reply(await text(m.n));
      else if (m.op === "render") reply(await render(m.n, m.width));
      else if (m.op === "gray") { const g = await gray(m.n, m.dpi); reply(g, [g.px.buffer]); }
      else if (m.op === "outline") reply(await outline());
      else throw new Error("unknown request");
    } catch (err) {
      send({ id: m.id, ok: false, error: String((err && err.message) || err) });
    }
  });
}
if (BRIDGE) {
  const session = Array.from(crypto.getRandomValues(new Uint8Array(16)), b => b.toString(16).padStart(2, "0")).join("");
  (async () => {
    for (;;) {
      try {
        const m = await (await fetch(`${VIEW}next?s=${session}`)).json();
        if (!m.op) continue;
        if (m.op === "open") m.data = await (await fetch(`${VIEW}bytes`)).arrayBuffer();
        handle(m); // (not awaited: the password for a PDF being opened comes as the next job)
      } catch (e) {
        await new Promise(r => setTimeout(r, 500));
      }
    }
  })();
} else {
  addEventListener("message", e => { if (e.source === parent) handle(e.data || {}); });
  send({ ready: true });
}
