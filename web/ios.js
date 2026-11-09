"use strict";
/* Khana Reader on the iPhone: the Mac app's reader page (app.js) with the few differences of the phone. Loaded after app.js
   only on the iPhone (src/app.rs adds it to the page there).

   A request's body goes first, in pieces, to the shell through a script message handler (window.webkit.messageHandlers.khanaBody:
   only this page has it, the pdf.js frame never does), and the request then names it (X-Khana-Body): WKWebView does not pass
   the body of a fetch on to an app's own scheme. */
(function () {
  const handler = window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.khanaBody;
  if (!handler) return;
  const CHUNK = 3 * 1024 * 1024; // (a multiple of 3: each piece is its own base64 text)
  let seq = 0;
  const b64 = blob => new Promise((ok, bad) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result).slice(String(r.result).indexOf(",") + 1));
    r.onerror = () => bad(r.error);
    r.readAsDataURL(blob);
  });
  async function sendBody(body) {
    const blob = body instanceof Blob ? body : new Blob([body]);
    const id = (++seq) + "-" + Math.random().toString(36).slice(2);
    for (let at = 0; at < blob.size; at += CHUNK) await handler.postMessage({ id, data: await b64(blob.slice(at, at + CHUNK)) });
    return id;
  }
  const plain = api;
  api = async function (path, opts = {}) {
    if (opts.body == null || opts.body === "") return plain(path, opts);
    const id = await sendBody(opts.body);
    const { body, ...rest } = opts;
    return plain(path, { ...rest, headers: { ...(opts.headers || {}), "X-Khana-Body": id } });
  };

  // ---- words of the Mac
  const say = (el, from, to) => {
    if (!el) return;
    const w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let n; (n = w.nextNode());) if (n.nodeValue.includes(from)) n.nodeValue = n.nodeValue.split(from).join(to);
    if (el.title) el.title = el.title.split(from).join(to);
  };
  document.querySelectorAll(".offline, .hint, .privacy").forEach(el => {
    say(el, "این دستگاه", "این گوشی");
    say(el, "همین دستگاه", "همین گوشی");
  });
  document.querySelectorAll(".hint").forEach(el => { if (el.querySelector("kbd")) el.hidden = true; });
  const reveal = $("mp3Reveal");
  if (reveal) reveal.textContent = "ذخیره / اشتراک‌گذاری";
})();
