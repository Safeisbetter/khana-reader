"use strict";
/* Khana Reader on Android: the Mac app's reader page (app.js) unchanged, with the few differences of
   the phone. Loaded after app.js only on Android (src/app.rs adds it to the page there).

   Requests: Android's WebView answers the page's requests inside the app (the Java shell hands them
   to the same Rust code as the Mac window; there is no network), but it does not pass a request's
   body on. So a body goes first through a private message channel the shell gives this page (and
   only this page: the pdf.js frame never has it), in pieces, and the request then names it. */
(function () {
  let port = null, portReady = null, ack = new Map(), seq = 0;
  portReady = new Promise(ok => {
    addEventListener("message", e => {
      // (the shell's message carries the page's own token, which the pdf.js frame cannot know)
      if (port || e.data !== "khana-port:" + TOKEN || !e.ports || e.ports.length !== 1) return;
      e.stopImmediatePropagation();
      port = e.ports[0];
      port.onmessage = m => {
        const d = String(m.data);
        if (d[0] === "A" && ack.has(d.slice(1))) { ack.get(d.slice(1))(); ack.delete(d.slice(1)); }
      };
      ok();
    });
  });

  const CHUNK = 3 * 1024 * 1024; // (a multiple of 3: each piece is its own base64 text)
  const b64 = blob => new Promise((ok, bad) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result).slice(String(r.result).indexOf(",") + 1));
    r.onerror = () => bad(r.error);
    r.readAsDataURL(blob);
  });

  async function sendBody(body) {
    await portReady;
    const blob = body instanceof Blob ? body : new Blob([body]);
    const id = (++seq) + "-" + Math.random().toString(36).slice(2);
    for (let at = 0; at < blob.size; at += CHUNK) port.postMessage("B" + id + ":" + await b64(blob.slice(at, at + CHUNK)));
    const done = new Promise(ok => ack.set(id, ok));
    port.postMessage("E" + id);
    await done;
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
  say(document.querySelector(".privacy"), "با بستن برنامه پاک می‌شوند.",
    "با بستن برنامه پاک می‌شوند. هر فایل MP3 و PDF را خودتان جایی که می‌خواهید ذخیره می‌کنید (صفحه‌ی «ذخیره» گوشی).");
  // (why a saved file can take hours: with the app behind another one, or the screen off, Android leaves it only the slow cores)
  const st = $("mp3Status");
  if (st) {
    const tip = document.createElement("p");
    tip.className = "hint";
    tip.textContent = "تا پایان کار، خوانا را روی صفحه نگه دارید (صفحه خودش روشن می‌ماند). اگر برنامه‌ی دیگری را باز کنید یا صفحه را خاموش کنید، کار ادامه دارد اما چند برابر کندتر.";
    st.appendChild(tip);
  }
  const reveal = $("mp3Reveal");
  if (reveal) reveal.textContent = "ذخیره / باز کردن";
})();

/* The public download has no microphone permission (window.KHANA_NO_MIC, set when it is built): recording is hidden, a voice
   sample is chosen as a file. */
if (window.KHANA_NO_MIC) {
  for (const id of ["recBtn", "recTime"]) { const e = document.getElementById(id); if (e) e.hidden = true; }
  const box = document.querySelector(".myvoice");
  if (box) {
    const say = box.querySelector(".say"); if (say) say.hidden = true;
    const hint = box.querySelector(".hint"); if (hint) hint.textContent = "خوانا می‌تواند با صدای خود شما بخواند. یک فایل صوتی کوتاه از صدای خودتان (۱۰ تا ۳۰ ثانیه، بی‌نویز) انتخاب کنید.";
  }
}
