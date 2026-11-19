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
        if (d[0] === "A" && ack.has(d.slice(1))) { ack.get(d.slice(1)).ok(); ack.delete(d.slice(1)); }
        if (d[0] === "F") {
          for (const pending of ack.values()) pending.bad(new Error("Upload refused: too large or invalid"));
          ack.clear();
        }
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
    if (blob.size > 420 * 1024 * 1024) throw new Error("File too large");
    const done = new Promise((ok, bad) => ack.set(id, { ok, bad }));
    // Install the rejection handler before asynchronous encoding can yield to the port callback.
    done.catch(() => {});
    for (let at = 0; at < blob.size; at += CHUNK) port.postMessage("B" + id + ":" + await b64(blob.slice(at, at + CHUNK)));
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
    tip.textContent = "ساختن فایل با صفحه‌ی خاموش هم ادامه دارد. اگر سرعت مهم‌تر است، برنامه را روی صفحه نگه دارید؛ در پس‌زمینه ممکن است کندتر شود.";
    st.appendChild(tip);
  }
  // «کپی متن صفحه»: through the shell (Server.copySensitive), which marks the text as sensitive and takes it off the clipboard
  // again after a minute and a half
  const copyBtn = $("copy");
  if (copyBtn) copyBtn.onclick = () => {
    const text = S.paras.join("\n\n");
    if (!text) return toast("متنی برای کپی نیست.");
    api("/api/clip", { method: "POST", body: text }).then(() => toast("متن صفحه کپی شد. پس از یک دقیقه و نیم از حافظه‌ی کپی پاک می‌شود."),
      () => toast("کپی نشد؛ متن را انتخاب و کپی کنید."));
  };
  // The screen guard (MainActivity.applyScreenGuard): on unless the reader switches it off here.
  const forget = $("forgetField");
  if (forget) {
    const f = document.createElement("div");
    f.className = "field";
    const lab = document.createElement("label");
    lab.className = "check";
    const box = document.createElement("input");
    box.type = "checkbox"; box.id = "allowShots";
    box.checked = !!(S.settings && S.settings.allowScreenshots);
    box.onchange = () => setSetting("allowScreenshots", box.checked);
    const txt = document.createElement("span");
    txt.textContent = "اجازه‌ی عکس و فیلم گرفتن از صفحه‌ی خوانا";
    lab.append(box, txt);
    const tip = document.createElement("p");
    tip.className = "editnote";
    tip.textContent = "خاموش (پیش‌فرض، امن‌تر): هیچ برنامه‌ی دیگری نمی‌تواند از صفحه‌ی خوانا عکس یا فیلم بگیرد و در فهرست برنامه‌های اخیر هم متن دیده نمی‌شود. خودتان هم نمی‌توانید اسکرین‌شات بگیرید؛ برای آن، این را روشن کنید.";
    f.append(lab, tip);
    forget.parentNode.insertBefore(f, forget);
    // (the settings come after this page is built: the box follows them whenever they are applied)
    const applied = window.applySettings;
    window.applySettings = function () { applied.apply(this, arguments); box.checked = !!(S.settings && S.settings.allowScreenshots); };
  }
  const reveal = $("mp3Reveal");
  if (reveal) reveal.textContent = "فایل‌های من / ذخیره";
})();

/* The public download has no microphone permission (window.KHANA_NO_MIC, set when it is built): recording is hidden, a voice
   sample is chosen as a file. */
if (window.KHANA_NO_MIC) {
  for (const id of ["recBtn", "recTime"]) { const e = document.getElementById(id); if (e) e.hidden = true; }
  const box = document.querySelector(".myvoice");
  if (box) {
    const say = box.querySelector(".say"); if (say) say.hidden = true;
    const hint = box.querySelector(".hint"); if (hint) hint.textContent = "خوانا می‌تواند با صدای خود شما بخواند. یک فایل صوتی کوتاه از صدای خودتان (۱۰ تا ۳۰ ثانیه، بدون سر و صدا) انتخاب کنید.";
  }
}

/* The system-bar margins belong to the Android shell: it is told the page's own bar colours
   (ground and writing) whenever the look changes («رنگ پس‌زمینه», or the device going dark), so they match. A see-through
   ground (the glass looks) is laid over the page's colour first. */
(function () {
  const rgb = s => { const m = /rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?/.exec(s || ""); return m ? [+m[1], +m[2], +m[3], m[4] === undefined ? 1 : +m[4]] : null; };
  const hex = c => "#" + c.slice(0, 3).map(v => Math.round(v).toString(16).padStart(2, "0")).join("");
  let last = "";
  function send() {
    const barEl = document.querySelector(".bar");
    if (!barEl) return;
    const cs = getComputedStyle(barEl), page = rgb(getComputedStyle(document.body).backgroundColor) || [255, 255, 255, 1];
    let g = rgb(cs.backgroundColor) || page;
    if (g[3] < 1) g = [0, 1, 2].map(k => g[k] * g[3] + page[k] * (1 - g[3]));
    const ink = rgb(cs.color) || [0, 0, 0, 1];
    const msg = hex(g) + " " + hex(ink);
    if (msg === last) return;
    last = msg;
    api("/api/shellbar", { method: "POST", body: msg }).catch(() => { last = ""; });
  }
  const later = () => setTimeout(send, 60); // (after the look's colours are applied)
  new MutationObserver(later).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  if (window.matchMedia) matchMedia("(prefers-color-scheme: dark)").addEventListener("change", later);
  addEventListener("load", later);
  setTimeout(send, 1500);
})();

/* Native playback owns the entire reading session, so turning the screen off never changes
   player or depends on background JavaScript timers. Only foreground highlighting is polled. */
(function () {
  const pageStop = stopReading;
  let control = Promise.resolve(), timer = null, polling = false, lastPosition = null, finishing = false, startAt = null;
  const command = (path, body = {}) => (control = control.catch(() => {}).then(() => jpost(path, body)));
  stopReading = function () {
    pageStop(); clearTimeout(timer); lastPosition = null; reader.nativePosition = null;
    command('/api/read_stop').catch(() => {});
  };
  function highlight(pos) {
    clearReadingMarks();
    if (pos.para < 0) return;
    const paragraph = document.querySelector(`.text p[data-i="${pos.para}"]`);
    if (!paragraph) return;
    paragraph.classList.add('reading');
    const words = [...paragraph.querySelectorAll('.w')].filter(w => +w.dataset.s >= pos.start && +w.dataset.s < pos.end);
    words.forEach(w => w.classList.add('sent'));
    const fraction = Math.min(0.999, Math.max(0, pos.elapsed / Math.max(0.01, pos.duration)));
    const current = words[Math.floor(fraction * words.length)];
    if (current) {
      current.classList.add('now');
      if (!reader.paused && !getSelection().toString().trim()) {
        const rect = current.getBoundingClientRect();
        const top = document.querySelector('.bar').getBoundingClientRect().bottom + 12;
        const bottom = $('docTools').getBoundingClientRect().top - 12;
        if (bottom > top && (rect.top < top || rect.bottom > bottom)) {
          window.scrollBy(0, rect.top - (top + Math.max(0, bottom - top - rect.height) * 0.35));
        }
      }
    }
  }
  async function poll(run, waiting = 0) {
    if (reader.run !== run || !reader.active || document.hidden || polling) return;
    polling = true;
    try {
      const pos = await jget('/api/read_status');
      if (reader.run !== run) return;
      if (!pos.active) {
        if (waiting < 15 && !lastPosition && !pos.error) { timer = setTimeout(() => poll(run, waiting + 1), 200); return; }
        const error = pos.error; stopReading(); if (error) toast(error); return;
      }
      if (!S.doc || pos.doc !== S.doc.id) { stopReading(); return; }
      lastPosition = pos;
      reader.nativePosition = pos;
      if (pos.page !== S.page) {
        const paras = await getParas(pos.page);
        if (reader.run !== run) return;
        showPage(pos.page, paras);
      }
      reader.paused = !!pos.paused; setPlayButton(); highlight(pos);
    } catch (e) { if (reader.run === run) { stopReading(); toast(e.message); } }
    finally { polling = false; }
    if (reader.run === run && reader.active) timer = setTimeout(() => poll(run), 250);
  }
  readFrom = async function (para, offset = 0) {
    // (finishing: a new voice or speed: the phone ends the sentence being heard by itself, so it is not stopped here)
    const exact = finishing; finishing = false;
    if (exact) { pageStop(); clearTimeout(timer); lastPosition = null; reader.nativePosition = null; } else stopReading();
    if (!S.doc) return;
    const run = reader.run, doc = S.doc, settings = { ...S.settings }, first = S.page;
    startAt = { page: first, para, start: offset, end: offset };
    reader.active = true; reader.paused = false; setPlayButton();
    try {
      // The sandboxed PDF renderer needs a visible page to prepare scans. OCR them once before
      // native reading starts; no pictures or book files are written to shared storage.
      if (doc.engine === 'pdfjs') {
        const lengths = await jget(`/api/lengths?doc=${doc.id}`);
        const last = settings.autoNext === false ? first : doc.pages;
        for (let n = first; n <= last; n++) {
          if (reader.run !== run || !S.doc || S.doc.id !== doc.id) return;
          if (lengths.letters[n - 1] < 0) {
            $('readingState').textContent = `آماده کردن صفحه‌ی اسکن‌شده… ${fa(n)} از ${fa(last)}`;
            await pageReq(`/api/page?doc=${doc.id}&n=${n}&tr=false`, n, doc.id);
          }
        }
      }
      const announceTitle = false;
      if (reader.run !== run) return;
      const request = { doc: doc.id, pages: doc.pages, page: first, para, start: offset,
        voice: settings.voice, speed: Number(settings.speed) || 1, tr: !!(doc.canTranslate && settings.translate),
        rev: !!settings.reversed, yeh: !!settings.persianYeh, autoNext: settings.autoNext !== false, announceEnd: !doc.isTyped, announceTitle, exact };
      control = control.catch(() => {}).then(() => reader.run === run ? jpost('/api/read_start', request) : null);
      await control;
      if (reader.run === run) poll(run);
    } catch (e) { if (reader.run === run) { stopReading(); toast(e.message); } }
  };
  async function pause(paused) {
    reader.paused = paused; setPlayButton();
    try { await command('/api/read_pause', { paused }); } catch (e) { stopReading(); toast(e.message); }
  }
  $('play').onclick = () => {
    if (!S.doc || !S.paras.length) return toast('متنی برای خواندن نیست.');
    if (S.view !== 'text') { S.view = 'text'; syncChrome(); }
    if (reader.active) pause(!reader.paused); else readFrom(0);
  };
  $('stop').onclick = stopReading;
  restartReading = function () {
    clearTimeout(restartT);
    if (!reader.active) return;
    restartT = setTimeout(async () => {
      if (!reader.active) return;
      // (1.29 deep check: before the phone reports a position, where this reading was started, not the top of the page)
      const pos = lastPosition || (startAt && startAt.page === S.page ? startAt : null), paused = reader.paused;
      if (pos && pos.para < 0) return;  // (the closing «پایان متن.»: nothing is left to read again)
      // A new voice or speed: the sentence being heard is finished in the old one while the next is made in the new one, and the
      // reading goes on right after it (before, it was cut and the same sentence made again from nothing: a long silence).
      const finish = !paused && pos && pos.para >= 0 && pos.page === S.page && pos.end > pos.start;
      finishing = !!finish;
      await readFrom(pos && pos.para >= 0 ? pos.para : 0, pos && pos.para >= 0 ? (finish ? pos.end : pos.start) : 0);
      if (paused && reader.active) await pause(true);
    }, 400);
  };
  document.addEventListener('visibilitychange', () => {
    if (!document.hidden && reader.active) { clearTimeout(timer); poll(reader.run); }
  });
})();

(function () {
 document.documentElement.classList.add('android-reader');
 const button = $('readingToolsBtn'); button.hidden = false;
 button.onclick = () => {
  const open = document.body.classList.toggle('reading-tools-open');
  button.setAttribute('aria-expanded', String(open));
  button.textContent = open ? 'بستن ابزارها' : 'صفحه و سرعت';
 };
})();

// Positive means the next page in the reader's writing direction. Long presses, taps and vertical scrolls stay untouched.
function bookSwipeDelta(dx, dy, elapsed, width, rtl) {
  const threshold = Math.max(56, Math.min(96, width * 0.18));
  if (![dx, dy, elapsed, width].every(Number.isFinite) || elapsed < 0 || elapsed > 700 || Math.abs(dx) < threshold || Math.abs(dx) < Math.abs(dy) * 1.6) return 0;
  return (dx > 0 ? 1 : -1) * (rtl ? 1 : -1);
}
(function () {
  const page = $('quickPage'), speed = $('quickSpeed'), content = $('mainContent');
  page.hidden = speed.hidden = false;
  $('paragraphHelp').textContent += ' برای صفحه‌ی بعد به راست و برای صفحه‌ی قبل به چپ بکشید.';
  function close() {
    document.body.classList.remove('quick-page-open', 'quick-speed-open');
    page.setAttribute('aria-expanded', 'false'); speed.setAttribute('aria-expanded', 'false');
  }
  function toggle(kind, button) {
    const open = !document.body.classList.contains(kind);
    close();
    document.body.classList.remove('reading-tools-open');
    $('readingToolsBtn').setAttribute('aria-expanded', 'false');
    $('readingToolsBtn').textContent = 'صفحه و سرعت';
    $('moreTools').open = false;
    if (open) {
      document.body.classList.add(kind); button.setAttribute('aria-expanded', 'true');
      const first = kind === 'quick-page-open' ? (!$('prev').disabled ? $('prev') : !$('next').disabled ? $('next') : button) : $('slower');
      requestAnimationFrame(() => { if (document.body.classList.contains(kind)) first.focus({ preventScroll: true }); });
    }
  }
  page.onclick = () => toggle('quick-page-open', page);
  speed.onclick = () => toggle('quick-speed-open', speed);
  document.addEventListener('pointerdown', e => {
    if (!e.target.closest('#quickPage,#quickSpeed,#pageControls,#speedControls')) close();
  });
  document.addEventListener('focusin', e => { if (!e.target.closest('#docTools')) close(); });
  document.addEventListener('keydown', e => {
    if (e.key !== 'Escape') return;
    const back = document.body.classList.contains('quick-page-open') ? page : document.body.classList.contains('quick-speed-open') ? speed : null;
    if (back) { close(); back.focus({ preventScroll: true }); e.preventDefault(); e.stopImmediatePropagation(); }
  }, true);
  let gesture = null, suppressClickUntil = 0;
  const selectable = () => !getSelection() || getSelection().isCollapsed;
  const ready = () => S.doc && S.doc.pages > 1 && !S.editing && !document.body.classList.contains('keyboard-open') && !$('moreTools').open && !document.querySelector('.panel:not([hidden])') && selectable();
  content.addEventListener('pointerdown', e => {
    gesture = null;
    if (e.pointerType !== 'touch' || !e.isPrimary || !ready() || !e.target.closest('#textView,#pageView') || e.target.closest('input,textarea,button,a,[contenteditable="true"]')) return;
    gesture = { id: e.pointerId, x: e.clientX, y: e.clientY, time: performance.now(), doc: S.doc.id, page: S.page, seq: S.loadSeq };
  }, { passive: true });
  content.addEventListener('pointermove', e => {
    if (!gesture || e.pointerId !== gesture.id) return;
    const dx = Math.abs(e.clientX - gesture.x), dy = Math.abs(e.clientY - gesture.y);
    if (dy > 12 && dy > dx || !selectable()) gesture = null;
  }, { passive: true });
  content.addEventListener('pointercancel', () => { gesture = null; }, { passive: true });
  content.addEventListener('pointerup', e => {
    const g = gesture; gesture = null;
    if (!g || e.pointerId !== g.id || !ready() || S.doc.id !== g.doc || S.page !== g.page || S.loadSeq !== g.seq) return;
    const delta = bookSwipeDelta(e.clientX - g.x, e.clientY - g.y, performance.now() - g.time, content.clientWidth, getComputedStyle(content).direction === 'rtl');
    const next = S.page + delta;
    if (!delta) return;
    suppressClickUntil = performance.now() + 500;
    e.preventDefault();
    if (next < 1 || next > S.doc.pages) return;
    close();
    loadPage(next).catch(e => toast(e.message));
    scrollTo({ top: 0, behavior: 'instant' });
  });
  content.addEventListener('click', e => {
    if (performance.now() < suppressClickUntil) { e.preventDefault(); e.stopImmediatePropagation(); }
  }, true);
  addEventListener('resize', () => { gesture = null; });
})();
