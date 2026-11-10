// pdf.js in its sandboxed frame (frame.html). The reader page sends a PDF's bytes and asks for a
// page's text with its positions, a page picture, or a grey picture for text recognition. Settings
// as GrapheneOS's PDF Viewer: no eval (isEvalSupported false), no XFA forms, no scripting, no
// WebAssembly (the JavaScript JPEG 2000 decoder instead), no system fonts (only pdf.js's own) and no font
// loading by the browser at all (disableFontFace: a PDF's fonts are read by pdf.js itself and drawn as shapes; the frame's
// policy has no font-src, so default-src 'none' refuses every font from anywhere),
// images capped; the document comes as bytes, never fetched.
import * as pdfjsLib from "./pdf.mjs";

const BASE = new URL("./", import.meta.url).href;
pdfjsLib.GlobalWorkerOptions.workerSrc = BASE + "pdf.worker.mjs";
let doc = null;

const IMAGE_OPS = new Set([pdfjsLib.OPS.paintImageXObject, pdfjsLib.OPS.paintInlineImageXObject,
  pdfjsLib.OPS.paintImageMaskXObject, pdfjsLib.OPS.paintImageXObjectRepeat]);

// the password the reader page's dialog gives back (null: the dialog was closed)
let passwordWaiter = null;

async function open(bytes, requestId) {
  // (pdf.js 6: a document is closed through its loading task; the document itself has no destroy)
  if (doc) { const old = doc; doc = null; await old.loadingTask.destroy().catch(() => {}); }
  const task = pdfjsLib.getDocument({
    data: new Uint8Array(bytes),
    isEvalSupported: false, enableXfa: false, useWasm: false, useSystemFonts: false, disableFontFace: true,
    wasmUrl: BASE + "wasm/", standardFontDataUrl: BASE + "standard_fonts/", cMapUrl: BASE + "cmaps/", cMapPacked: true,
    maxImageSize: 64 * 1024 * 1024, stopAtErrors: false,
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
  return { pages: doc.numPages };
}

// a page's text: [text, x0, top, x1, bottom, size, rtl, end of line] per piece, in the page's
// coordinates (points, top left at 0,0); images: whether a page without text has pictures
async function text(n) {
  const page = await doc.getPage(n);
  const vp = page.getViewport({ scale: 1 });
  // (unnormalized: a «لا» ligature stays one character; pdf.js's own expansion came out reversed
  // after its right-to-left reordering, «پلاسچی» → «پالسچی»; the app's repair expands it)
  const tc = await page.getTextContent({ disableNormalization: true });
  const items = [];
  for (const it of tc.items) {
    if (typeof it.str !== "string") continue;
    const [, , c, d, e, f] = pdfjsLib.Util.transform(vp.transform, it.transform);
    const size = Math.hypot(c, d) || 1;
    if (!it.str.trim() && !it.hasEOL) continue;
    items.push([it.str, e, f - size * 0.8, e + it.width, f + size * 0.2, size, it.dir === "rtl" ? 1 : 0, it.hasEOL ? 1 : 0]);
  }
  let images = false;
  if (!items.some(i => i[0].trim())) {
    const ops = await page.getOperatorList();
    images = ops.fnArray.some(f => IMAGE_OPS.has(f));
  }
  page.cleanup();
  return { w: vp.width, h: vp.height, items, images };
}

// A page is drawn no larger than this many pixels (GrapheneOS PDF Viewer's MAX_RENDER_PIXELS: 8,388,608): a page that claims
// to be metres wide would otherwise ask for a canvas of billions of pixels.
const MAX_RENDER_PIXELS = 8_388_608;

async function draw(n, scale) {
  const page = await doc.getPage(n);
  let vp = page.getViewport({ scale });
  const pixels = vp.width * vp.height;
  if (!(pixels > 0) || !Number.isFinite(pixels)) throw new Error("this page has no usable size");
  if (pixels > MAX_RENDER_PIXELS) vp = page.getViewport({ scale: scale * Math.sqrt(MAX_RENDER_PIXELS / pixels) });
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.floor(vp.width)); canvas.height = Math.max(1, Math.floor(vp.height));
  const ctx = canvas.getContext("2d", { alpha: false });
  ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, canvas.width, canvas.height);
  // (intent "print": drawn at once, not frame by frame with requestAnimationFrame, which WebKit holds
  // back while the window is not in front: the picture never came)
  await page.render({ canvasContext: ctx, viewport: vp, intent: "print" }).promise;
  page.cleanup();
  return canvas;
}

// a page picture `width` pixels wide, as PNG
async function render(n, width) {
  const page = await doc.getPage(n);
  const scale = Math.min(8, Math.max(0.1, width / page.getViewport({ scale: 1 }).width));
  const canvas = await draw(n, scale);
  return await new Promise(r => canvas.toBlob(r, "image/png"));
}

// grey pixels at `dpi`, for text recognition
async function gray(n, dpi) {
  const canvas = await draw(n, dpi / 72);
  const { width: w, height: h } = canvas;
  const rgba = canvas.getContext("2d").getImageData(0, 0, w, h).data;
  const px = new Uint8Array(w * h);
  for (let i = 0, j = 0; i < px.length; i++, j += 4) px[i] = (rgba[j] * 299 + rgba[j + 1] * 587 + rgba[j + 2] * 114) / 1000;
  return { w, h, px };
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
  else if (m.data instanceof Blob) { kind = "png"; body = await m.data.arrayBuffer(); } // (bytes: WebKit gives the app no body for a Blob)
  else if (m.data && m.data.px) {
    kind = "gray";
    body = new Uint8Array(8 + m.data.px.length);
    const dv = new DataView(body.buffer);
    dv.setUint32(0, m.data.w, true); dv.setUint32(4, m.data.h, true); body.set(m.data.px, 8);
  } else { kind = "json"; body = JSON.stringify(m.data); }
  fetch(`${VIEW}reply?id=${m.id}&kind=${kind}${extra}`, { method: "POST", body }).catch(() => {});
}
async function handle(m) {
  if (m.op === "password") { if (passwordWaiter) passwordWaiter(m.cancel ? null : String(m.value ?? "")); return; }
  const reply = (data, transfer) => send({ id: m.id, ok: true, data }, transfer);
  try {
    if (m.op === "open") reply(await open(m.data, m.id));
    else if (!doc) throw new Error("no document");
    else if (m.op === "text") reply(await text(m.n));
    else if (m.op === "render") reply(await render(m.n, m.width));
    else if (m.op === "gray") { const g = await gray(m.n, m.dpi); reply(g, [g.px.buffer]); }
    else throw new Error("unknown request");
  } catch (err) {
    send({ id: m.id, ok: false, error: String((err && err.message) || err) });
  }
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
