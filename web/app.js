"use strict";
// (the token comes in a meta tag: the page has no inline script, so its policy can forbid them)
const TOKEN = document.querySelector("meta[name=khana-token]").content;
const $ = id => document.getElementById(id);
const fa = n => String(n).replace(/\d/g, d => "۰۱۲۳۴۵۶۷۸۹"[d]);
// (numbers typed in a box are Farsi digits; whatever digits are typed are read: ۱۲, 12 and ١٢ are all twelve)
const latinDigits = t => String(t).replace(/[۰-۹]/g, d => "۰۱۲۳۴۵۶۷۸۹".indexOf(d)).replace(/[٠-٩]/g, d => "٠١٢٣٤٥٦٧٨٩".indexOf(d));
const numOf = v => parseInt(latinDigits(v).replace(/\D/g, ""), 10) || 0;
const esc = s => s.replace(/[&<>"]/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;"}[c]));
const reEsc = s => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

async function api(path, opts = {}) {
  const res = await fetch(path, { ...opts, headers: { "X-Khana-Token": TOKEN, ...(opts.headers || {}) } });
  if (!res.ok) {
    let msg = "خطا";
    try { msg = (await res.json()).error || msg; } catch (e) {}
    throw new Error(msg);
  }
  return res;
}
const jget = async p => (await api(p)).json();
const jpost = async (p, body) => (await api(p, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) })).json();

const S = { settings: {}, voices: [], doc: null, page: 1, view: "text", paras: [], loadSeq: 0 };

/* ---------------- settings ---------------- */
function applySettings() {
  const s = S.settings;
  document.documentElement.style.setProperty("--fs", s.fontSize + "px");
  $("speed").value = s.speed; $("speedVal").textContent = pct(s.speed);
  $("speedChip").textContent = pct(s.speed);
  $("speedChip").classList.toggle("changed", Number(s.speed) !== 1);
  $("speedChip").setAttribute("aria-label", "سرعت " + pct(s.speed) + "؛ برای سرعت عادی بزنید");
  document.documentElement.dataset.theme = ["light", "dark", "black", "blackyellow"].includes(s.theme) ? s.theme : "auto";
  document.querySelectorAll("input[name=comma]").forEach(r => r.checked = r.value === (s.commaPause || "normal"));
  document.querySelectorAll("input[name=theme]").forEach(r => r.checked = r.value === (["light", "dark", "black", "blackyellow"].includes(s.theme) ? s.theme : "auto"));
  document.querySelectorAll("[data-speed]").forEach(b => b.classList.toggle("on", Number(b.dataset.speed) === Number(s.speed) && b.id !== "speedReset"));
  $("fontSize").value = s.fontSize; $("fontVal").textContent = fa(s.fontSize);
  $("voice").value = s.voice;
  applyCursor(s.cursor || "large");
  document.querySelectorAll("input[name=cursor]").forEach(r => r.checked = r.value === (s.cursor || "large"));
  $("autoNext").checked = !!s.autoNext;
  $("naturalTone").checked = s.naturalTone !== false;
  $("autoPlay").checked = !!s.autoPlay;
  document.documentElement.dataset.hl = s.highlight || "yellow";
  document.querySelectorAll("input[name=hl]").forEach(r => r.checked = r.value === (s.highlight || "yellow"));
  document.documentElement.dataset.fc = s.frameColor || "text";
  document.querySelectorAll("input[name=fc]").forEach(r => r.checked = r.value === (s.frameColor || "text"));
  $("frameField").hidden = s.highlight !== "frame";
  $("persianYeh").checked = !!s.persianYeh;
  $("reversed").checked = !!s.reversed;
  $("bitrate").value = String(s.bitrate);
  const v = S.voices.find(x => x.id === s.voice);
  if (saveKind !== "pdf") $("mp3Voice").textContent = "صدا: " + (v ? v.name : "-") + " · سرعت " + pct(s.speed) + " · " + fa(s.bitrate) + " کیلوبیت (از تنظیمات)";
}
let saveT;
function setSetting(k, v, reloadText) {
  S.settings[k] = v; applySettings();
  clearTimeout(saveT); saveT = setTimeout(() => jpost("/api/settings", S.settings).catch(() => {}), 300);
  if (reloadText && S.doc) loadPage(S.page, true);
}
// speech is prepared well ahead, so a new voice or speed restarts reading at the sentence being heard
let restartT;
function restartReading() {
  clearTimeout(restartT);
  if (!reader.active) return;
  restartT = setTimeout(() => {
    if (!reader.active) return;
    const now = actx ? actx.currentTime : 0;
    const piece = reader.playing.find(u => now < u.t1) || reader.playing[reader.playing.length - 1];
    const wasPaused = reader.paused;
    if (piece && piece.page !== S.page) showPage(piece.page, reader.pageParas[piece.page], !reader.partial[piece.page]);
    if (piece && piece.p >= 0) readFrom(piece.p, piece.start); else readFrom(0);
    if (wasPaused) { actx.suspend(); reader.paused = true; setPlayButton(); }
  }, 400);
}
$("voice").onchange = e => { setSetting("voice", e.target.value); ttsCache.clear(); restartReading(); };
// reading speed: 0.5× to 2×; the slider, the −/+ buttons, the presets and the Speed menu all come here
// (shown in percent: ۱۰۰٪ is normal; the −/+ buttons go one percent at a time, the menu's keys five)
const pct = v => fa(Math.round(Number(v) * 100)) + "٪";
const SPEED_MIN = 0.5, SPEED_MAX = 2, SPEED_STEP = 0.01, SPEED_KEY_STEP = 0.05;
function setSpeed(v, quiet) {
  v = Math.round(Math.min(SPEED_MAX, Math.max(SPEED_MIN, Number(v) || 1)) * 100) / 100;
  if (v === Number(S.settings.speed)) { if (!quiet) toast("سرعت " + pct(v)); return; }
  setSetting("speed", v); ttsCache.clear(); restartReading();
  if (!quiet) toast(v === 1 ? "سرعت عادی (۱۰۰٪)" : "سرعت " + pct(v));
}
const resetSpeed = () => setSpeed(1);
$("speed").oninput = e => setSpeed(+e.target.value, true);
$("slower").onclick = () => setSpeed(Number(S.settings.speed) - SPEED_STEP);
$("faster").onclick = () => setSpeed(Number(S.settings.speed) + SPEED_STEP);
$("speedChip").onclick = resetSpeed;
document.querySelectorAll("[data-speed]").forEach(b => b.onclick = () => setSpeed(+b.dataset.speed));
function setFontSize(v) { setSetting("fontSize", Math.min(52, Math.max(14, Math.round(v)))); }
// commands from the app's menu bar (src/main.rs)
window.khanaMenu = cmd => {
  if (cmd === "speed-reset") resetSpeed();
  else if (cmd === "speed-up") setSpeed(Number(S.settings.speed) + SPEED_KEY_STEP);
  else if (cmd === "speed-down") setSpeed(Number(S.settings.speed) - SPEED_KEY_STEP);
  else if (cmd === "font-up") setFontSize(Number(S.settings.fontSize) + 2);
  else if (cmd === "font-down") setFontSize(Number(S.settings.fontSize) - 2);
  else if (cmd === "settings") openPanel("settings");
  else if (cmd === "mp3") { if (S.doc) $("mp3Btn").click(); else toast("اول یک فایل را باز کنید."); }
  else if (cmd === "pdf") {
    if (!S.doc) toast("اول یک فایل را باز کنید.");
    else if (!S.doc.canTranslate) toast("این سند فارسی است و ترجمه لازم ندارد.");
    else $("pdfBtn").click();
  }
};
$("fontSize").oninput = e => setSetting("fontSize", +e.target.value);
document.querySelectorAll("input[name=cursor]").forEach(r => r.onchange = () => setSetting("cursor", r.value));
$("autoNext").onchange = e => setSetting("autoNext", e.target.checked);
$("naturalTone").onchange = e => setSetting("naturalTone", e.target.checked);
$("autoPlay").onchange = e => setSetting("autoPlay", e.target.checked);
document.querySelectorAll("input[name=hl]").forEach(r => r.onchange = () => setSetting("highlight", r.value));
document.querySelectorAll("input[name=fc]").forEach(r => r.onchange = () => setSetting("frameColor", r.value));
document.querySelectorAll("input[name=comma]").forEach(r => r.onchange = () => { setSetting("commaPause", r.value); ttsCache.clear(); restartReading(); });
document.querySelectorAll("input[name=theme]").forEach(r => r.onchange = () => setSetting("theme", r.value));
$("persianYeh").onchange = e => setSetting("persianYeh", e.target.checked, true);
$("reversed").onchange = e => setSetting("reversed", e.target.checked, true);
$("bitrate").onchange = e => setSetting("bitrate", +e.target.value);
$("preview").onclick = async () => {
  try {
    const url = await ttsUrl("سلام. من خوانا هستم و متن فارسی را برای شما می‌خوانم.");
    stopReading(); const a = $("audio"); a.src = url; a.play();
  } catch (e) { toast(e.message); }
};

function fillVoices(voices) {
  S.voices = voices;
  const groups = [["mine", "صدای من"], ["natural", "صدای طبیعی"]];
  $("voice").innerHTML = groups.map(([g, label]) => {
    const opts = voices.filter(v => v.group === g).map(v => `<option value="${v.id}">${esc(v.name)}</option>`).join("");
    return opts ? `<optgroup label="${label}">${opts}</optgroup>` : "";
  }).join("");
  if (!voices.some(v => v.id === S.settings.voice) && voices[0]) S.settings.voice = voices[0].id;
  $("myDel").hidden = !voices.some(v => v.id === "mine");
}

/* ---------------- my voice ----------------
   Recorded as plain PCM through Web Audio and sent as a WAV file, so it works the same in
   every browser engine; a chosen file (Voice Memos m4a, mp3, wav) is converted on the Mac. */
async function useMyVoice(blob, name) {
  $("myStatus").textContent = "در حال آماده کردن صدای شما…";
  try {
    const r = await (await api("/api/myvoice?name=" + encodeURIComponent(name), { method: "POST", body: await blob.arrayBuffer() })).json();
    const init = await jget("/api/init");
    fillVoices(init.voices);
    bufCache.clear(); ttsCache.clear();
    setSetting("voice", "mine"); restartReading();
    $("myStatus").textContent = `صدای شما ذخیره شد (${fa(String(r.seconds).replace(".", "٫"))} ثانیه). حالا «▶ امتحان» را بزنید.`;
    $("preview").click();
  } catch (e) { $("myStatus").textContent = e.message; }
}
function wavBlob(chunks, rate) {
  const n = chunks.reduce((a, c) => a + c.length, 0), buf = new DataView(new ArrayBuffer(44 + n * 2));
  const str = (o, t) => [...t].forEach((c, i) => buf.setUint8(o + i, c.charCodeAt(0)));
  str(0, "RIFF"); buf.setUint32(4, 36 + n * 2, true); str(8, "WAVEfmt "); buf.setUint32(16, 16, true);
  buf.setUint16(20, 1, true); buf.setUint16(22, 1, true); buf.setUint32(24, rate, true);
  buf.setUint32(28, rate * 2, true); buf.setUint16(32, 2, true); buf.setUint16(34, 16, true);
  str(36, "data"); buf.setUint32(40, n * 2, true);
  let o = 44;
  for (const c of chunks) for (const x of c) { buf.setInt16(o, Math.max(-1, Math.min(1, x)) * 32767, true); o += 2; }
  return new Blob([buf], { type: "audio/wav" });
}
let rec = null;
async function stopRecording(save) {
  if (!rec) return;
  const r = rec; rec = null;
  clearInterval(r.timer); r.node.disconnect(); r.src.disconnect();
  r.stream.getTracks().forEach(t => t.stop());
  r.ctx.close();
  $("recBtn").textContent = "● ضبط صدای من"; $("recTime").hidden = true;
  if (!save) return;
  if ((Date.now() - r.t0) < 4000) { $("myStatus").textContent = "ضبط خیلی کوتاه بود. دست‌کم ۵ ثانیه صحبت کنید."; return; }
  useMyVoice(wavBlob(r.chunks, r.ctx.sampleRate), "recording.wav");
}
$("recBtn").onclick = async () => {
  if (rec) return stopRecording(true);
  stopReading();
  let stream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, autoGainControl: false, noiseSuppression: true } });
  } catch (e) {
    $("myStatus").textContent = "خوانا به میکروفون دسترسی ندارد. در System Settings › Privacy & Security › Microphone اجازه بدهید، یا با برنامه‌ی Voice Memos ضبط کنید و «انتخاب فایل صدا» را بزنید.";
    return;
  }
  const ctx = new (window.AudioContext || window.webkitAudioContext)();
  const src = ctx.createMediaStreamSource(stream), node = ctx.createScriptProcessor(4096, 1, 1);
  const chunks = [];
  node.onaudioprocess = e => chunks.push(new Float32Array(e.inputBuffer.getChannelData(0)));
  src.connect(node); node.connect(ctx.destination);
  rec = { ctx, src, node, stream, chunks, t0: Date.now() };
  $("recBtn").textContent = "■ پایان ضبط"; $("recTime").hidden = false;
  $("myStatus").textContent = "جمله را بخوانید…";
  const show = () => { const t = Math.floor((Date.now() - rec.t0) / 1000); $("recTime").textContent = "● " + fa(t) + " ثانیه"; if (t >= 20) stopRecording(true); };
  show(); rec.timer = setInterval(() => rec && show(), 250);
};
$("voiceFile").onchange = e => { const f = e.target.files[0]; e.target.value = ""; if (f) useMyVoice(f, f.name); };
$("myDel").onclick = async () => {
  if (!confirm("صدای ضبط‌شده‌ی شما پاک شود؟")) return;
  try {
    await jpost("/api/myvoice_delete", {});
    const init = await jget("/api/init");
    S.settings.voice = init.settings.voice; fillVoices(init.voices); applySettings();
    bufCache.clear(); ttsCache.clear(); restartReading();
    $("myStatus").textContent = "صدای شما پاک شد.";
  } catch (e) { toast(e.message); }
};

/* ---------------- pronunciation dictionary ---------------- */
function renderLexicon() {
  const lex = S.settings.lexicon || {};
  // (the list is shut until opened, and not there when empty: a long list made the settings busy to look at)
  const n = Object.keys(lex).length;
  $("lexDetails").hidden = !n;
  $("lexSummary").textContent = "فهرست اصلاح‌ها (" + fa(n) + ")";
  $("lexList").innerHTML = Object.entries(lex).map(([w, p]) =>
    `<div class="lexrow"><span class="w">${esc(w)} ← <b>${esc(p)}</b></span>` +
    `<button type="button" data-say="${esc(w)}">▶</button><button type="button" data-del="${esc(w)}">حذف</button></div>`).join("");
}
async function saveLexicon(lex) {
  S.settings.lexicon = lex; ttsCache.clear(); bufCache.clear(); renderLexicon();
  clearTimeout(saveT);
  S.settings = await jpost("/api/settings", S.settings);
  renderLexicon();
}
async function sayWord(text) {
  try { stopReading(); audio.src = await ttsUrl(text); audio.play(); } catch (e) { toast(e.message); }
}
$("lexAdd").onclick = async () => {
  const w = $("lexWord").value.trim(), p = $("lexSay").value.trim();
  if (!w || !p) return toast("کلمه و تلفظ را بنویسید.");
  await saveLexicon({ ...(S.settings.lexicon || {}), [w]: p });
  $("lexWord").value = ""; $("lexSay").value = "";
  // without vowel marks the word is read like normal text, so the marks are what give control
  toast(/[\u064b-\u0652]/.test(p) ? "تلفظ «" + w + "» ذخیره شد."
    : "ذخیره شد. برای تلفظ دقیق، حرکت‌ها را هم بگذارید؛ مثلاً هَرجَهَت.");
};
$("lexTest").onclick = async () => {
  const w = $("lexWord").value.trim(), p = $("lexSay").value.trim();
  if (!w) return toast("کلمه را بنویسید.");
  if (p) await saveLexicon({ ...(S.settings.lexicon || {}), [w]: p });
  sayWord(w);
};
$("lexList").addEventListener("click", async e => {
  const say = e.target.closest("[data-say]"), del = e.target.closest("[data-del]");
  if (say) sayWord(say.dataset.say);
  if (del) { const lex = { ...(S.settings.lexicon || {}) }; delete lex[del.dataset.del]; await saveLexicon(lex); }
});
// select a word in the text -> a button offers to fix how it is read
document.addEventListener("selectionchange", () => {
  const t = getSelection().toString().trim();
  const ok = t && t.length <= 30 && !/\s/.test(t) && $("textView").contains(getSelection().anchorNode);
  $("fixWord").hidden = !ok;
  if (ok) {
    // (while the text is being edited the button changes the word in the whole book; otherwise it fixes how the word is said)
    $("fixWord").textContent = S.editing ? `عوض کردن «${t}» در همه‌ی کتاب‌ها` : `اصلاح تلفظ «${t}»`;
    $("fixWord").dataset.word = t;
  }
});
$("fixWord").onclick = () => {
  $("fixWord").hidden = true;
  if (S.editing) return openWordEditor($("fixWord").dataset.word);
  $("lexWord").value = $("fixWord").dataset.word; $("lexSay").value = $("fixWord").dataset.word;
  openPanel("settings");
  $("lexField").scrollIntoView({ block: "start" }); $("lexSay").focus();
};

/* ---------------- panels ---------------- */
function openPanel(id) { $(id).hidden = false; $(id).querySelector("button,select,input").focus(); }
document.querySelectorAll(".panel").forEach(p => {
  p.addEventListener("click", e => { if (e.target === p || e.target.hasAttribute("data-close")) p.hidden = true; });
});
addEventListener("keydown", e => { if (e.key === "Escape") { stopRecording(false); document.querySelectorAll(".panel").forEach(p => p.hidden = true); } });
$("settingsBtn").onclick = () => openPanel("settings");

/* ---------------- opening ---------------- */
// pictures (a photo of a page, a screenshot): read with the app's text recognition, one page per picture
const isImage = f => /\.(png|jpe?g|webp|gif|bmp|heic|tiff?)$/i.test(f.name) || /^image\//.test(f.type || "");
async function openImages(files) {
  // (limits: the pictures are decoded in the page; a huge one is refused before anything is drawn)
  if (files.length > 50) throw new Error("حداکثر ۵۰ تصویر را می‌توان با هم باز کرد.");
  if (files.some(f => f.size > 80 * 1024 * 1024)) throw new Error("یکی از تصویرها خیلی بزرگ است (بیشتر از ۸۰ مگابایت).");
  const pages = [];
  for (let i = 0; i < files.length; i++) {
    note(`در حال خواندن تصویر ${fa(i + 1)} از ${fa(files.length)}…`);
    let bmp;
    try { bmp = await createImageBitmap(files[i]); } catch (e) { throw new Error("این تصویر باز نشد: " + files[i].name); }
    if (bmp.width * bmp.height > 60e6) { bmp.close(); throw new Error("تصویر خیلی بزرگ است: " + files[i].name); }
    // (about the size of a page scanned at 250 dpi: large pictures are made smaller, small ones larger)
    const long = Math.max(bmp.width, bmp.height), k = long > 3200 ? 3200 / long : long < 1600 ? Math.min(2, 2200 / long) : 1;
    const w = Math.max(1, Math.round(bmp.width * k)), h = Math.max(1, Math.round(bmp.height * k));
    const cv = document.createElement("canvas"); cv.width = w; cv.height = h;
    const cx = cv.getContext("2d", { willReadFrequently: true });
    cx.fillStyle = "#fff"; cx.fillRect(0, 0, w, h); cx.drawImage(bmp, 0, 0, w, h);
    const px = cx.getImageData(0, 0, w, h).data, body = new Uint8Array(8 + w * h), dv = new DataView(body.buffer);
    dv.setUint32(0, w, true); dv.setUint32(4, h, true);
    for (let j = 0, o = 8; j < px.length; j += 4, o++) body[o] = (px[j] * 77 + px[j + 1] * 150 + px[j + 2] * 29) >> 8;
    const r = await (await api("/api/ocr_image", { method: "POST", body: body.buffer })).json();
    if (r.error) throw new Error(r.error);
    pages.push(r.paragraphs || []);
  }
  if (!pages.some(p => p.length)) throw new Error("در این تصویر متنی پیدا نشد.");
  const name = files.length === 1 ? files[0].name : files[0].name.replace(/\.[^.]+$/, "") + " (+" + (files.length - 1) + ").png";
  return await (await api("/api/open_pages?name=" + encodeURIComponent(name), { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ pages }) })).json();
}
async function openFile(file, images) {
  if (!file) return;
  stopReading();
  note(`در حال بازکردن «${esc(file.name)}»…`);
  try {
    // (a PDF is read by pdf.js in its sandboxed frame; other files by the app)
    const d = /\.pdf$/i.test(file.name) ? await openWithPdfjs(file)
      : isImage(file) ? await openImages(images || [file])
      : await (await api("/api/open?name=" + encodeURIComponent(file.name), { method: "POST", body: await file.arrayBuffer() })).json();
    S.doc = d; S.page = 1;
    S.view = "text";
    $("fileName").textContent = d.name;
    $("welcome").hidden = true;
    $("from").value = fa(1); $("to").value = fa(d.pages);
    const langName = { en: "انگلیسی", nl: "هلندی", de: "آلمانی", fr: "فرانسوی", es: "اسپانیایی", it: "ایتالیایی", pt: "پرتغالی", tr: "ترکی", pl: "لهستانی" }[d.lang] || "خارجی";
    // (the light download has no translator: a book in another language is said so, not read with the Farsi voice)
    if (d.hasText && d.lang !== "fa" && !d.canTranslate && d.lang !== "unknown") note(`این سند به زبان ${langName} است. نسخه‌ی سبک ترجمه ندارد و فقط متن فارسی را درست می‌خواند. برای خواندن کتاب‌های خارجی، نسخه‌ی کامل را نصب کنید.`, true);
    else if (!d.hasText) note("این سند متنی ندارد؛ احتمالاً اسکن یا تصویر است. می‌توانید صفحه‌ها را در نمای «صفحه» ببینید، اما خواندن با صدا ممکن نیست.", true);
    else if (d.scanned) note(`${fa(d.pages)} صفحه. این سند اسکن است؛ متن هر صفحه با «تشخیص متن» روی همین دستگاه خوانده می‌شود و ممکن است چند غلط داشته باشد.` + (d.canTranslate ? ` متن ${langName} است و به فارسی ترجمه می‌شود.` : ""));
    else if (d.canTranslate) note(`${fa(d.pages)} صفحه، به زبان ${langName}. متن به فارسی ترجمه می‌شود و همان خوانده می‌شود (ترجمه‌ی ماشینی، روی همین دستگاه). متن اصلی را با دکمه‌ی «ترجمه به فارسی» یا نمای «صفحه» ببینید. برای خواندن از هر جای متن، روی همان جمله کلیک کنید.`);
    else note(`${fa(d.pages)} صفحه. برای خواندن از هر جای متن، روی همان جمله کلیک کنید.`);
    if (!d.hasText && d.hasImages) S.view = "page";
    await loadPage(1);
    if (d.hasText && S.settings.autoPlay) {
      // start at the first page that has text (covers are often images)
      let n = 1;
      while (!S.paras.length && n < d.pages) await loadPage(++n);
      if (S.paras.length) readFrom(0);
    }
  } catch (e) { note(esc(e.message), true); }
}
/* ---------------- pdf.js, in its own sandboxed frame (as GrapheneOS's PDF Viewer) ----------------
   The frame (pdfjs/frame.html) has an origin of its own: it cannot read this page, nor make the
   app's requests (it never has the token); its policy lets it load only pdf.js's own files; it has
   no permissions. It gets a PDF's bytes and answers with text positions and pictures, which are
   treated as untrusted (text is escaped when shown; the app checks what it is sent). */
let pdfFrame = null, pdfReady = null, pdfSeq = 0;
const pdfWait = new Map();
function pdfjsFrame() {
  if (pdfReady) return pdfReady;
  pdfFrame = document.createElement("iframe");
  pdfFrame.hidden = true;
  pdfFrame.setAttribute("sandbox", "allow-scripts");
  pdfFrame.setAttribute("allow", "");
  pdfFrame.setAttribute("referrerpolicy", "no-referrer");
  pdfFrame.src = "pdfjs/frame.html";
  pdfReady = new Promise((ok, no) => {
    const t = setTimeout(() => { pdfReady = null; no(new Error("pdf.js شروع نشد.")); }, 20000);
    pdfFrame.ready = () => { clearTimeout(t); ok(); };
  });
  document.body.appendChild(pdfFrame);
  return pdfReady;
}
// The password dialog: resolves with the password, or null when it is closed (Esc, outside, «انصراف»).
function askPassword(again) {
  return new Promise(done => {
    const panel = $("pwPanel"), input = $("pwInput");
    let over = false;
    const finish = value => {
      if (over) return;
      over = true;
      obs.disconnect();
      input.value = "";
      panel.hidden = true;
      $("pwOk").onclick = null; input.onkeydown = null;
      done(value);
    };
    const obs = new MutationObserver(() => { if (panel.hidden) finish(null); });
    $("pwHint").textContent = again ? "رمز درست نیست. دوباره امتحان کنید." : "این PDF با رمز محافظت شده است. رمز را وارد کنید.";
    input.value = "";
    panel.dataset.prompts = String((+panel.dataset.prompts || 0) + 1); // (counts the prompts: the self-test answers each once)
    panel.hidden = false;
    obs.observe(panel, { attributes: true, attributeFilter: ["hidden"] });
    $("pwOk").onclick = () => finish(input.value);
    input.onkeydown = e => { if (e.key === "Enter") { e.preventDefault(); finish(input.value); } };
    input.focus();
  });
}
addEventListener("message", e => {
  if (!pdfFrame || e.source !== pdfFrame.contentWindow) return;
  const m = e.data || {};
  if (m.ready) { if (pdfFrame.ready) pdfFrame.ready(); return; }
  if (m.needPassword) {
    askPassword(!!m.again).then(pw => pdfFrame.contentWindow.postMessage(pw == null ? { op: "password", cancel: true } : { op: "password", value: pw }, "*"));
    return;
  }
  const w = pdfWait.get(m.id);
  if (!w) return;
  pdfWait.delete(m.id);
  if (m.ok) w.ok(m.data); else w.no(new Error("pdf.js: " + String(m.error).slice(0, 200)));
});
async function pdfCall(msg, transfer) {
  await pdfjsFrame();
  return new Promise((ok, no) => {
    const id = ++pdfSeq;
    pdfWait.set(id, { ok, no });
    pdfFrame.contentWindow.postMessage({ ...msg, id }, "*", transfer || []);
  });
}
async function openWithPdfjs(file) {
  const buf = await file.arrayBuffer();
  let pages;
  try {
    ({ pages } = await pdfCall({ op: "open", data: buf }, [buf]));
  } catch (e) {
    if (/password cancelled/.test(String(e.message))) throw new Error("این PDF با رمز محافظت شده است و باز نشد.");
    throw e;
  }
  const out = [];
  for (let n = 1; n <= pages; n++) {
    if (n === 1 || n % 10 === 0) note(`در حال خواندن «${esc(file.name)}»… صفحه‌ی ${fa(n)} از ${fa(pages)}`);
    const t = await pdfCall({ op: "text", n });
    out.push({ w: t.w, h: t.h, images: !!t.images, items: t.items });
  }
  const res = await api("/api/open_layout?name=" + encodeURIComponent(file.name), { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ pages: out }) });
  return res.json();
}
// a scanned page, drawn grey at 250 dpi by pdf.js, for the app's text recognition
const picturesGiven = new Map();
function givePicture(n) {
  const key = S.doc.id + "|" + n;
  if (!picturesGiven.has(key)) {
    const p = (async () => {
      const g = await pdfCall({ op: "gray", n, dpi: 250 });
      const body = new Uint8Array(8 + g.px.length), dv = new DataView(body.buffer);
      dv.setUint32(0, g.w, true); dv.setUint32(4, g.h, true); body.set(new Uint8Array(g.px.buffer || g.px), 8);
      await api(`/api/page_picture?doc=${S.doc.id}&n=${n}`, { method: "POST", body: body.buffer });
    })();
    p.catch(() => picturesGiven.delete(key));
    picturesGiven.set(key, p);
  }
  return picturesGiven.get(key);
}

function note(html, warn) { const n = $("note"); n.innerHTML = html; n.hidden = false; n.classList.toggle("warn", !!warn); }
$("fileInput").onchange = e => {
  ensureAudio();
  const fs = [...e.target.files];
  // (several pictures together are the pages of one document)
  if (fs.length > 1 && fs.every(isImage)) openFile(fs[0], fs); else openFile(fs[0]);
  e.target.value = "";
};
let dragDepth = 0;
addEventListener("dragenter", e => { if ([...e.dataTransfer.types].includes("Files")) { dragDepth++; $("drop").hidden = false; } });
addEventListener("dragleave", () => { if (--dragDepth <= 0) { dragDepth = 0; $("drop").hidden = true; } });
addEventListener("dragover", e => e.preventDefault());
addEventListener("drop", e => {
  e.preventDefault(); dragDepth = 0; $("drop").hidden = true; ensureAudio();
  const fs = [...e.dataTransfer.files];
  if (fs.length > 1 && fs.every(isImage)) openFile(fs[0], fs); else openFile(fs[0]);
});

/* ---------------- pages ---------------- */
const pageCache = new Map();  // "page|rev|yeh|tr" -> Promise<paragraphs>
const translating = () => !!(S.doc && S.doc.canTranslate && S.settings.translate);
// A page request; a scanned page of a pdf.js document is first drawn by pdf.js and read with OCR.
async function pageReq(url, n) {
  let r = await jget(url);
  if (r.needsPicture) { await givePicture(n); r = await jget(url); }
  return r;
}
function getParas(n) {
  const s = S.settings, tr = translating(), key = `${S.doc.id}|${n}|${!!s.reversed}|${!!s.persianYeh}|${tr}`;
  if (!pageCache.has(key)) {
    const p = pageReq(`/api/page?doc=${S.doc.id}&n=${n}&rev=${!!s.reversed}&yeh=${!!s.persianYeh}&tr=${tr}`, n).then(r => r.paragraphs);
    p.catch(() => pageCache.delete(key));
    pageCache.set(key, p);
  }
  return pageCache.get(key);
}
// A page being translated comes a few paragraphs at a time: {paragraphs, done}. A finished page
// goes into the cache like any other.
async function pageSoFar(n) {
  const s = S.settings, key = `${S.doc.id}|${n}|${!!s.reversed}|${!!s.persianYeh}|true`;
  if (pageCache.has(key)) return { paragraphs: await pageCache.get(key), done: true };
  const gen = editGen;
  const r = await pageReq(`/api/page?doc=${S.doc.id}&n=${n}&rev=${!!s.reversed}&yeh=${!!s.persianYeh}&tr=true&partial=1`, n);
  // (a page asked before a correction was made is not kept: it has the old words)
  if (r.done && gen === editGen) pageCache.set(key, Promise.resolve(r.paragraphs));
  return r;
}
// the visible page gets the paragraphs translated since it was shown
function growPage(n, paras, done) {
  if (S.page !== n) return;
  const from = S.paras.length;
  if (paras.length > from) { S.paras = paras; appendText(from); }
  if (done && !S.pageDone) { S.pageDone = true; const m = $("trMore"); if (m) m.remove(); }
}
async function followPage(n, seq) {
  while (seq === S.loadSeq && !S.pageDone) {
    await sleep(700);
    if (seq !== S.loadSeq) return;
    const r = await pageSoFar(n).catch(() => null);
    if (r && seq === S.loadSeq) growPage(n, r.paragraphs, r.done);
  }
}

async function loadPage(n, keepReading) {
  if (!S.doc) return;
  n = Math.max(1, Math.min(S.doc.pages, n | 0));
  if (!keepReading) stopReading();
  S.page = n;
  const seq = ++S.loadSeq;
  syncChrome();
  // the page's picture needs no text: drawn at once, not after a translation that may be waiting its turn
  if (S.view === "page") renderPageImage();
  // a scanned page is read (OCR) and a foreign one translated the first time: say so meanwhile
  const waitT = setTimeout(() => { if (seq === S.loadSeq) $("textView").innerHTML = `<p class="empty">${translating() ? "در حال ترجمه‌ی این صفحه به فارسی…" : "در حال آماده کردن این صفحه…"}</p>`; }, 250);
  if (translating()) {
    // shown (and readable) from its first translated paragraphs on; the rest follows
    let r;
    try {
      r = await pageSoFar(n);
      while (!r.done && !r.paragraphs.length && seq === S.loadSeq) { await sleep(400); r = await pageSoFar(n); }
    } finally { clearTimeout(waitT); }
    if (seq !== S.loadSeq) return;
    S.paras = r.paragraphs; S.pageDone = r.done;
    renderText();
    if (!r.done) followPage(n, seq);
    return;
  }
  const paras = await getParas(n).finally(() => clearTimeout(waitT));
  if (seq !== S.loadSeq) return;
  S.paras = paras; S.pageDone = true;
  renderText();
}
function showPage(n, paras, done = true) {  // switch the visible page while reading continues
  S.page = n; S.paras = paras; S.pageDone = done; S.loadSeq++;
  syncChrome(); renderText();
  if (S.view === "page") renderPageImage();
}
// every word is its own span (data-s = its offset in the paragraph) so reading can light it up
function paraHtml(p, i, re, found) {
  const h = p.replace(/\S+|\s+/g, (tok, off) => {
    if (/^\s/.test(tok)) return tok;
    let w = esc(tok);
    if (re) w = w.replace(re, m => (found(), `<mark>${m}</mark>`));
    return `<span class="w" data-s="${off}">${w}</span>`;
  });
  return `<p dir="auto" data-i="${i}" title="کلیک: خواندن از همین جمله">${h}</p>`;
}
const moreNote = () => S.pageDone === false ? '<p class="empty" id="trMore">در حال ترجمه‌ی ادامه‌ی این صفحه…</p>' : "";
// paragraphs from `from` on were added (a page still being translated): add them, keep the rest
function appendText(from) {
  const box = $("textView"), q = $("q").value.trim();
  if (q || box.querySelectorAll("p[data-i]").length !== from) return renderText();
  const html = S.paras.slice(from).map((p, k) => paraHtml(p, from + k, null, () => {})).join("");
  const more = $("trMore");
  if (more) more.insertAdjacentHTML("beforebegin", html); else box.insertAdjacentHTML("beforeend", html + moreNote());
}
function renderText() {
  const box = $("textView");
  const q = $("q").value.trim();
  let count = 0;
  if (!S.paras.length) {
    box.innerHTML = '<p class="empty">این صفحه متنی ندارد.</p>';
  } else {
    const re = q ? new RegExp(reEsc(esc(q)), "g") : null;
    box.innerHTML = S.paras.map((p, i) => paraHtml(p, i, re, () => count++)).join("") + moreNote();
  }
  $("count").textContent = q ? (count ? fa(count) + " مورد" : "پیدا نشد") : "";
  const first = box.querySelector("mark");
  if (first) { first.classList.add("cur"); first.scrollIntoView({ block: "center" }); }
  reader.shownWord = null;
}
function renderPageImage() {
  const w = Math.round(Math.min(document.querySelector(".sheet").clientWidth, 900) * (devicePixelRatio || 1));
  if (S.doc.engine === "pdfjs") {
    // (only a PDF has pictures of its pages: drawn by pdf.js in its frame, back as a PNG)
    const n = S.page, id = S.doc.id, img = $("pageImg");
    // (the last page's picture, dimmed until this one comes: never taken for the page in the box)
    if (img.dataset.page !== String(n)) img.classList.add("stale");
    pdfCall({ op: "render", n, width: w }).then(blob => {
      if (!S.doc || S.doc.id !== id || S.page !== n || !(blob instanceof Blob)) return;
      const old = img.dataset.blob;
      img.dataset.blob = img.src = URL.createObjectURL(new Blob([blob], { type: "image/png" }));
      img.dataset.page = String(n);
      img.classList.remove("stale");
      if (old) URL.revokeObjectURL(old);
    }).catch(e => toast(e.message));
  }
}
function syncChrome() {
  const has = !!S.doc;
  $("docTools").hidden = !has; $("searchRow").hidden = !has || S.view !== "text";
  if (!has) return;
  $("pageInput").value = fa(S.page);
  $("pageCount").textContent = fa(S.doc.pages);
  $("prev").disabled = S.page <= 1; $("next").disabled = S.page >= S.doc.pages;
  $("viewPage").disabled = !S.doc.hasImages;
  $("trBtn").hidden = !S.doc.canTranslate;
  $("pdfBtn").hidden = !S.doc.canTranslate;
  $("trBtn").classList.toggle("on", translating());
  $("viewText").classList.toggle("on", S.view === "text");
  $("viewPage").classList.toggle("on", S.view === "page");
  $("textView").hidden = S.view !== "text";
  $("pageView").hidden = S.view !== "page";
}
$("prev").onclick = () => { loadPage(S.page - 1); scrollTo(0, 0); };
$("next").onclick = () => { loadPage(S.page + 1); scrollTo(0, 0); };
// (the number is checked on Enter, on leaving the box, and on change: the digits are rewritten as Farsi while typing, and then the browser sends no "change" by itself)
const goTyped = () => { const n = numOf($("pageInput").value); if (n >= 1 && S.doc) loadPage(Math.min(n, S.doc.pages)); };
$("pageInput").onchange = goTyped;
$("pageInput").onblur = () => { if (numOf($("pageInput").value) !== S.page) goTyped(); };
$("pageInput").addEventListener("keydown", e => { if (e.key === "Enter") { e.preventDefault(); goTyped(); } });
$("viewText").onclick = () => { S.view = "text"; syncChrome(); };
$("trBtn").onclick = () => { setSetting("translate", !S.settings.translate); loadPage(S.page); };
$("viewPage").onclick = () => { S.view = "page"; syncChrome(); renderPageImage(); };
$("q").oninput = renderText;
$("q").onkeydown = e => {
  if (e.key !== "Enter") return;
  const ms = [...$("textView").querySelectorAll("mark")]; if (!ms.length) return;
  const i = ms.findIndex(m => m.classList.contains("cur"));
  if (i >= 0) ms[i].classList.remove("cur");
  const nx = ms[(i + (e.shiftKey ? ms.length - 1 : 1)) % ms.length];
  nx.classList.add("cur"); nx.scrollIntoView({ block: "center", behavior: "smooth" });
};
addEventListener("keydown", e => {
  if (!S.doc || e.target.closest("input,select,textarea") || !$("settings").hidden || !$("mp3").hidden) return;
  if (e.key === "ArrowLeft") $("prev").click();
  if (e.key === "ArrowRight") $("next").click();
  if (e.key === " ") { e.preventDefault(); $("play").click(); }
});
$("copy").onclick = () => {
  const text = S.paras.join("\n\n");
  if (!text) return toast("متنی برای کپی نیست.");
  navigator.clipboard.writeText(text).then(() => toast("متن صفحه کپی شد."), () => toast("کپی نشد؛ متن را انتخاب و کپی کنید."));
};

/* ---------------- reading aloud ----------------
   Speech is made sentence by sentence and played on the Web Audio clock: each sentence is
   scheduled to start exactly when the previous one ends (plus a natural pause), and the next
   sentences and the next page are prepared ahead, so there is no wait between paragraphs. */
const ttsCache = new Map();  // text -> Promise<blob URL> (voice test / dictionary)
async function ttsUrl(text) {
  const key = S.settings.voice + "|" + S.settings.speed + "|" + text;
  if (!ttsCache.has(key)) {
    const p = api("/api/tts", { method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text, voice: S.settings.voice, speed: S.settings.speed }) })
      .then(r => r.blob()).then(b => URL.createObjectURL(b));
    p.catch(() => ttsCache.delete(key));
    ttsCache.set(key, p);
    if (ttsCache.size > 40) { const [k, old] = ttsCache.entries().next().value; ttsCache.delete(k); old.then(URL.revokeObjectURL, () => {}); }
  }
  return ttsCache.get(key);
}
const audio = $("audio");
let actx = null;
function ensureAudio() {  // call inside a click/drop so the Mac lets sound start later on its own
  // "playback" asks for a larger sound buffer than the default low-latency one, so a busy moment
  // (speech being made in the background) cannot starve the sound and crackle
  if (!actx) {
    const AC = window.AudioContext || window.webkitAudioContext;
    try { actx = new AC({ latencyHint: "playback" }); } catch (e) { actx = new AC(); }
  }
  if (actx.state === "suspended" && !reader.paused) actx.resume();
  return actx;
}
const bufCache = new Map();  // voice|speed|text -> Promise<AudioBuffer>
let ttsChain = Promise.resolve();  // one request at a time, in order: the next sentence is always made first
function speechBuffer(text, run) {
  const key = S.settings.voice + "|" + S.settings.speed + "|" + (S.settings.naturalTone !== false) + "|" + (S.settings.commaPause || "normal") + "|" + text;
  if (!bufCache.has(key)) {
    const body = JSON.stringify({ text, voice: S.settings.voice, speed: S.settings.speed });
    // a piece queued for reading that was stopped or jumped away from is skipped, so the new
    // position does not wait behind sentences nobody will hear
    const owner = { run };
    const p = ttsChain.then(() => {
      if (owner.run !== undefined && reader.run !== owner.run) throw new Error("skipped");
      return api("/api/tts", { method: "POST", headers: { "Content-Type": "application/json" }, body });
    })
      // (a text with nothing to say comes back as a sound with no samples, which WebKit refuses with
      // «Decoding failed»: a moment's silence, and the reading goes on)
      .then(r => r.arrayBuffer()).then(b => b.byteLength <= 44 ? ensureAudio().createBuffer(1, 1, 22050) : ensureAudio().decodeAudioData(b));
    p.owner = owner;
    ttsChain = p.catch(() => {});
    p.catch(() => bufCache.delete(key));
    bufCache.set(key, p);
    if (bufCache.size > 80) bufCache.delete(bufCache.keys().next().value);
  }
  const p = bufCache.get(key);
  if (p.owner && p.owner.run !== undefined) p.owner.run = run;  // still wanted by the new reading position
  return p;
}

// A page's paragraphs -> speakable pieces: sentences, and long sentences cut at commas.
// (fromOff: in paragraph fromPara, start at the sentence holding this offset)
// How long to wait before a piece, by what it and the one before do: a title has room around it, a new
// paragraph is a new thought, a question is left to sink in, and more is coming after a comma.
function isTitleLine(p) {
  const t = p.trim();
  return t.length <= 70 && t.split(/\s+/).length <= 10 && /[\p{L}\p{N}»”)\]]$/u.test(t);
}
function piecesFor(page, paras, fromPara, fromOff = 0) {
  const out = [];
  let afterTitle = false;
  paras.forEach((p, i) => {
    if (i < fromPara) return;
    const title = isTitleLine(p);
    const re = /[^.!?؟…]+(?:[.!?؟…]+["'’”»)\]]*[.!?؟…]*|$)\s*/g;
    let m, first = true;
    while ((m = re.exec(p)) && m[0]) {
      let start = m.index, sent = m[0];
      const parts = sent.length > 240 ? sent.split(/(?<=[،؛,;])\s+/) : [sent];
      let off = start;
      parts.forEach((part, k) => {
        const at = p.indexOf(part, off);
        if (/[\p{L}\p{N}]/u.test(part) && !(i === fromPara && at + part.length <= fromOff)) {
          const before = out.length ? out[out.length - 1].text : "";
          const asked = /[؟?!][»”"'’)\]]*$/.test(before);
          const pause = title ? 1.1 : afterTitle ? 0.8 : first ? 0.9 : k > 0 ? 0.2 : asked ? 0.6 : 0.45;
          out.push({ page, p: i, start: at, end: at + part.length, text: part.trim(), pause });
          afterTitle = title;
        }
        first = false; off = at + part.length;
      });
    }
  });
  return out;
}

const reader = { active: false, paused: false, run: 0, queue: [], next: 0, playing: [], lastEnd: 0, lastPage: 0, sources: [] };
function setPlayButton() {
  $("play").textContent = reader.active && !reader.paused ? "❚❚ مکث" : (reader.paused ? "▶ ادامه" : "▶ خواندن");
  $("stop").disabled = !reader.active;
}
// a big mouse pointer for people who see badly: drawn here as pictures (black arrow with a white edge; yellow over things that can be pressed)
let cursorUrls = [];
function applyCursor(size) {
  // (a phone has no mouse: no pointer to make bigger, and no setting for it)
  if (/Android|iPhone|iPad/i.test(navigator.userAgent)) { $("cursorField").hidden = true; size = "normal"; }
  cursorUrls.forEach(u => URL.revokeObjectURL(u)); cursorUrls = [];
  const px = size === "huge" ? 56 : size === "large" ? 40 : 0, root = document.documentElement;
  if (!px) { root.style.removeProperty("--cur"); root.style.removeProperty("--cur-hand"); return; }
  const mk = (fill) => {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${px}" height="${px}" viewBox="0 0 24 24"><path d="M3 2 L3 20 L8 15.5 L11.5 22.5 L14.5 21 L11 14.2 L18 14 Z" fill="${fill}" stroke="${fill === "#000" ? "#fff" : "#000"}" stroke-width="1.6" stroke-linejoin="round"/></svg>`;
    const u = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" })); cursorUrls.push(u); return u;
  };
  const hot = Math.round(px * 3 / 24), hot2 = Math.round(px * 2 / 24);
  root.style.setProperty("--cur", `url(${mk("#000")}) ${hot} ${hot2}, auto`);
  root.style.setProperty("--cur-hand", `url(${mk("#FFE600")}) ${hot} ${hot2}, pointer`);
}
const sleep = ms => new Promise(r => setTimeout(r, ms));

// Page n is where the book's text begins: no page before it has text (covers are often pictures).
async function atBookStart(n) {
  if (n > 3) return false;
  for (let k = 1; k < n; k++) if ((await getParas(k)).length) return false;
  return true;
}

// fromOff: start at the sentence holding this offset of paragraph i (a click on it); the book's
// name is said only when reading starts at the very beginning
async function readFrom(i, fromOff = 0) {
  stopReading();
  if (!S.doc) return;
  ensureAudio();
  const run = ++reader.run;
  Object.assign(reader, { active: true, paused: false, queue: piecesFor(S.page, S.paras, i, fromOff), next: 0,
    playing: [], lastEnd: 0, lastPage: S.page, sources: [] });
  if (reader.queue[0]) reader.queue[0].pause = 0;
  reader.pageParas = { [S.page]: S.paras };
  // pages still being translated: their new paragraphs are read before going on
  reader.partial = { [S.page]: S.pageDone === false };
  setPlayButton();
  requestAnimationFrame(() => tick(run));
  try {
    // from the start of the book: its name first (p -1: a piece with no text on screen)
    // Farsi voice spelled the English letters out and began with a strange sound)
    const nameable = S.doc.lang !== "fa" || /[؀-ۿ]/.test(S.doc.title || "");
    if (i === 0 && fromOff === 0 && S.doc.title && nameable && await atBookStart(S.page)) {
      // (a translated book's name is said in Farsi)
      const title = translating() ? (await jget(`/api/title?doc=${S.doc.id}&rev=${!!S.settings.reversed}&yeh=${!!S.settings.persianYeh}&tr=true`)).title : S.doc.title;
      if (reader.run !== run) return;
      if (reader.queue[0]) reader.queue[0].pause = 0.8;
      reader.queue.unshift({ page: S.page, p: -1, start: 0, end: 0, text: title + ".", pause: 0 });
    }
    let ended = false;
    while (reader.run === run) {
      // read on into the next page, prepared before this one ends
      // (a translated page is asked for earlier: it may still be being translated)
      const ahead = reader.queue.length - reader.next < (translating() ? 12 : 4);
      let starved = false;  // nothing left to read until more is translated
      // (asked at most about once a second)
      const mayAsk = Date.now() - (reader.asked || 0) > 800;
      if (ahead && reader.partial[reader.lastPage] && (mayAsk || reader.next >= reader.queue.length)) {
        reader.asked = Date.now();
        // the page is still being translated: take the paragraphs done since
        const n = reader.lastPage, r = await pageSoFar(n);
        if (reader.run !== run) return;
        const old = reader.pageParas[n] || [];
        if (r.paragraphs.length > old.length) {
          reader.pageParas[n] = r.paragraphs;
          reader.queue.push(...piecesFor(n, r.paragraphs, old.length));
        }
        if (r.done) reader.partial[n] = false;
        growPage(n, r.paragraphs, r.done);
        if (r.paragraphs.length > old.length || r.done) continue;
        starved = reader.next >= reader.queue.length;
      } else if (ahead && !reader.partial[reader.lastPage] && S.settings.autoNext && reader.lastPage < S.doc.pages && (mayAsk || reader.next >= reader.queue.length)) {
        reader.asked = Date.now();
        const n = reader.lastPage + 1;
        let paras, done = true;
        if (translating()) { const r = await pageSoFar(n); paras = r.paragraphs; done = r.done; }
        else paras = await getParas(n);
        if (reader.run !== run) return;
        if (paras.length || done) {
          reader.pageParas[n] = paras; reader.lastPage = n; reader.partial[n] = !done;
          reader.queue.push(...piecesFor(n, paras, 0));
          continue;
        }
        starved = reader.next >= reader.queue.length;
      }
      if (starved) { await sleep(600); continue; }
      if (reader.next >= reader.queue.length) {
        // the last page has been read: say so, so the listener knows the book is finished
        if (!ended && reader.lastPage >= S.doc.pages && S.doc.name !== "متن من.txt") {
          ended = true;
          reader.queue.push({ page: reader.lastPage, p: -1, start: 0, end: 0, text: "پایان متن.", pause: 1.2 });
          continue;
        }
        while (reader.run === run && actx.currentTime < reader.lastEnd) await sleep(200);
        break;
      }
      // stay well ahead of what is being heard, so a sentence the voice has to make again
      // (a retry takes a few seconds) never leaves a gap; prepare the following pieces too
      if (reader.lastEnd - actx.currentTime > 30) { await sleep(250); continue; }
      const piece = reader.queue[reader.next];
      const bufP = speechBuffer(piece.text, run);
      for (let k = 1; k <= 3; k++) if (reader.queue[reader.next + k]) speechBuffer(reader.queue[reader.next + k].text, run).catch(() => {});
      const buf = await bufP;
      if (reader.run !== run) return;
      reader.playing = reader.playing.filter(u => u.t1 > actx.currentTime);
      const src = actx.createBufferSource();
      src.buffer = buf; src.connect(actx.destination);
      const t0 = Math.max(actx.currentTime + 0.05, reader.lastEnd + piece.pause);
      src.start(t0);
      Object.assign(piece, { t0, t1: t0 + buf.duration });
      reader.sources.push(src); reader.playing.push(piece);
      // a played sentence lets go of its sound (about 1 MB) and its words on screen: kept, they
      // added up over a long book until WebKit stopped the page (after about 50 pages)
      src.onended = () => {
        const k = reader.sources.indexOf(src);
        if (k >= 0) reader.sources.splice(k, 1);
        try { src.buffer = null; } catch (e) {}
        piece.words = null; piece.para = null; piece.weights = null;
      };
      reader.lastEnd = piece.t1; reader.next++;
    }
  } catch (e) { if (reader.run === run) toast(e.message); }
  if (reader.run === run) stopReading();
}

// Follow the voice on screen: switch pages, light up the sentence and the word being read.
function tick(run) {
  if (reader.run !== run || !reader.active) return;
  requestAnimationFrame(() => tick(run));
  if (!actx || reader.paused) return;
  const now = actx.currentTime;
  const piece = reader.playing.find(u => now >= u.t0 && now < u.t1) || null;
  if (!piece) return;
  if (piece.page !== S.page) showPage(piece.page, reader.pageParas[piece.page], !reader.partial[piece.page]);
  // the sentence's word spans and their weights are looked up once, not on every frame
  if (!piece.words || !piece.words[0] || !piece.words[0].isConnected) {
    const para = document.querySelector(`.text p[data-i="${piece.p}"]`);
    if (!para) return;
    piece.para = para;
    piece.words = [...para.querySelectorAll(".w")].filter(w => +w.dataset.s >= piece.start && +w.dataset.s < piece.end);
    // time inside the sentence is shared out by word length; punctuation adds a little pause
    piece.weights = piece.words.map(w => w.textContent.length + 1.5 + (/[.!?؟،؛:,]$/.test(w.textContent) ? 3 : 0));
    piece.total = piece.weights.reduce((a, b) => a + b, 0);
  }
  const para = piece.para, words = piece.words;
  if (!words.length) return;
  let pos = (now - piece.t0) / (piece.t1 - piece.t0) * piece.total, cur = words[words.length - 1];
  for (let k = 0; k < words.length; k++) { pos -= piece.weights[k]; if (pos < 0) { cur = words[k]; break; } }
  if (reader.shownPiece !== piece || !para.classList.contains("reading")) {
    document.querySelectorAll(".text .sent").forEach(w => w.classList.remove("sent"));
    document.querySelectorAll(".text p.reading").forEach(p => p.classList.remove("reading"));
    para.classList.add("reading");
    words.forEach(w => w.classList.add("sent"));
    reader.shownPiece = piece;
  }
  if (reader.shownWord !== cur) {
    if (reader.shownWord) reader.shownWord.classList.remove("now");
    cur.classList.add("now");
    reader.shownWord = cur;
    const r = cur.getBoundingClientRect(), top = $("textView").closest("main").getBoundingClientRect().top;
    if (r.top < innerHeight * 0.25 || r.bottom > innerHeight * 0.8 || r.top < top)
      cur.scrollIntoView({ block: "center", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
  }
}
function clearReadingMarks() {
  document.querySelectorAll(".text .sent, .text .now").forEach(w => w.classList.remove("sent", "now"));
  document.querySelectorAll(".text p.reading").forEach(p => p.classList.remove("reading"));
  reader.shownWord = null; reader.shownPiece = null;
}
function stopReading() {
  reader.run++; reader.active = false; reader.paused = false;
  reader.sources.forEach(s => { try { s.stop(); } catch (e) {} });
  reader.sources = []; reader.playing = [];
  if (actx && actx.state === "suspended") actx.resume();
  audio.pause();
  clearReadingMarks();
  setPlayButton();
}
$("play").onclick = () => {
  if (!S.doc || !S.paras.length) return toast("متنی برای خواندن نیست.");
  ensureAudio();
  if (S.view !== "text") { S.view = "text"; syncChrome(); }
  if (reader.active && !reader.paused) { actx.suspend(); reader.paused = true; return setPlayButton(); }
  if (reader.paused) { reader.paused = false; actx.resume(); return setPlayButton(); }
  readFrom(0);
};
$("stop").onclick = stopReading;
$("textView").addEventListener("click", e => {
  const p = e.target.closest("p[data-i]");
  // (editing: a tap on a paragraph opens its window; a selected word is left for the «change the word» button)
  if (S.editing) { if (p && !getSelection().toString()) openParaEditor(p); return; }
  if (!p || e.detail > 1) return;
  ensureAudio();
  // from the sentence clicked (the word's offset; between words, the paragraph's start)
  const w = e.target.closest(".w"), off = w ? +w.dataset.s : 0;
  // wait a moment: a double-click selects a word (to fix its pronunciation) instead of reading
  setTimeout(() => { if (!getSelection().toString()) readFrom(+p.dataset.i, off); }, 300);
});

/* ---------------- MP3, and PDF of the translation (the same dialog) ---------------- */
let job = null, pollT;
var saveKind = "mp3";  // (var: applySettings may run before this line)
function openSave(kind) {
  if (!S.doc) return;
  if (!(job && job.status === "running")) {
    saveKind = kind;
    $("mp3Status").hidden = true;
    $("from").value = fa(S.page); $("to").value = fa(S.doc.pages);
    document.querySelector(`input[name=scope][value=${kind === "pdf" ? "all" : "page"}]`).checked = true;
  }
  const pdf = saveKind === "pdf";
  $("mp3Title").textContent = pdf ? "ذخیره‌ی ترجمه‌ی فارسی به صورت PDF" : "ذخیره به صورت فایل صدا";
  $("fmtField").hidden = pdf; $("fastField").hidden = pdf;
  syncBitrate();
  $("mp3Start").textContent = pdf ? "ساختن PDF" : "ساختن فایل صدا";
  applySettings();
  if (pdf) $("mp3Voice").textContent = "هر صفحه روی همین دستگاه به فارسی ترجمه می‌شود (برای هر صفحه چند ثانیه تا یک دقیقه) و فایل PDF در پوشه Downloads ذخیره می‌شود.";
  openPanel("mp3");
}
// (the quality is asked for next to the format, and only when the format is MP3)
function syncBitrate() { $("bitrateField").hidden = saveKind === "pdf" || document.querySelector("input[name=afmt]:checked").value !== "mp3"; }
document.querySelectorAll("input[name=afmt]").forEach(r => r.addEventListener("change", syncBitrate));
$("mp3Btn").onclick = () => openSave("mp3");
$("pdfBtn").onclick = () => openSave("pdf");
for (const id of ["pageInput", "from", "to"]) $(id).addEventListener("input", e => { const t = fa(latinDigits(e.target.value).replace(/\D/g, "")); if (t !== e.target.value) e.target.value = t; });
$("from").oninput = $("to").oninput = () => document.querySelector('input[name=scope][value=range]').checked = true;
$("mp3Start").onclick = async () => {
  const scope = document.querySelector("input[name=scope]:checked").value;
  let first = S.page, last = S.page;
  if (scope === "all") { first = 1; last = S.doc.pages; }
  if (scope === "range") { first = numOf($("from").value); last = numOf($("to").value); }
  try {
    if (S.doc.engine === "pdfjs") {
      // (the app cannot draw a pdf.js document's scanned pages itself: they are drawn here first)
      $("mp3Status").hidden = false;
      for (let n = first; n <= last; n++) {
        $("mp3Text").textContent = `آماده کردن صفحه‌ها… ${fa(n)} از ${fa(last)}`;
        await pageReq(`/api/page?doc=${S.doc.id}&n=${n}&rev=${!!S.settings.reversed}&yeh=${!!S.settings.persianYeh}&tr=false`, n);
      }
    }
    job = saveKind === "pdf" ? await jpost("/api/pdf", { doc: S.doc.id, first, last })
      : await jpost("/api/mp3", { doc: S.doc.id, first, last, voice: S.settings.voice, speed: S.settings.speed,
      format: document.querySelector("input[name=afmt]:checked").value, fast: $("fastExport").checked,
      bitrate: S.settings.bitrate, reversed: S.settings.reversed, persianYeh: S.settings.persianYeh, translate: translating() });
    $("mp3Status").hidden = false; $("mp3Reveal").hidden = true;
    $("mp3Start").disabled = true; $("mp3Cancel").hidden = false;
    poll();
  } catch (e) { toast(e.message); }
};
$("mp3Reveal").onclick = () => { if (job) jpost("/api/reveal?id=" + job.id, {}).catch(e => toast(e.message)); };
$("mp3Cancel").onclick =() => { if (job) jpost("/api/cancel?id=" + job.id, {}).catch(() => {}); };
async function poll() {
  clearTimeout(pollT);
  try { job = await jget("/api/job?id=" + job.id); } catch (e) { toast(e.message); return; }
  const pct = Math.round(job.progress * 100);
  $("mp3Bar").style.width = pct + "%";
  const mins = Math.floor(job.seconds / 60), secs = job.seconds % 60;
  const len = fa(mins) + " دقیقه و " + fa(secs) + " ثانیه";
  const pdf = saveKind === "pdf";
  if (job.status === "running") {
    $("mp3Text").textContent = pdf ? `در حال ترجمه… ${fa(pct)}٪` : (pct === 0 ? "در حال آماده کردن نخستین بخش… (ممکن است چند ثانیه تا چند دقیقه طول بکشد)" : `در حال ساختن… ${fa(pct)}٪ · ${len} صدا`);
    pollT = setTimeout(poll, 700); return;
  }
  $("mp3Start").disabled = false; $("mp3Cancel").hidden = true;
  if (job.status === "done") {
    $("mp3Text").textContent = `فایل در پوشه‌ی دانلودها ذخیره شد: «${job.saved}»` + (pdf ? "" : ` · ${len}`);
    $("mp3Reveal").hidden = false;
  } else if (job.status === "cancelled") $("mp3Text").textContent = "لغو شد.";
  else $("mp3Text").textContent = "ساخته نشد: " + job.error;
}

/* ---------------- misc ---------------- */
let toastT;
function toast(msg) { const t = $("toast"); t.textContent = msg; t.hidden = false; clearTimeout(toastT); toastT = setTimeout(() => t.hidden = true, 3500); }
let resizeT; addEventListener("resize", () => { clearTimeout(resizeT); resizeT = setTimeout(() => S.doc && S.view === "page" && renderPageImage(), 250); });

(async function init() {
  try {
    const r = await jget("/api/init");
    S.settings = r.settings; S.voices = r.voices;
    // (editing by voice needs the recognizer, which the Mac and Android apps have, and the microphone)
    $("voiceBtn").hidden = !(r.canListen && navigator.mediaDevices && !window.KHANA_NO_MIC);
    // (speech to text: the Mac app carries the model; a live recording also needs the microphone)
    $("sttRow").hidden = !r.canWrite;
    $("sttLive").hidden = !(navigator.mediaDevices && !window.KHANA_NO_MIC);
    fillVoices(r.voices);
    applySettings(); renderLexicon(); syncChrome(); setPlayButton();
  } catch (e) { note("برنامه درست اجرا نشد: " + esc(e.message), true); }
})();


/* ---------------- editing the text: a paragraph changed or taken out, a word changed everywhere ---------------- */
// The corrections are kept on this computer, per book, and reach the reading, the MP3 and the PDF.
const editQuery = () => `doc=${S.doc.id}&rev=${!!S.settings.reversed}&yeh=${!!S.settings.persianYeh}&tr=${translating()}`;
let editGen = 0, lastFound = null;
async function sendEdit(body, keep) {
  const r = await jpost(`/api/edit?${editQuery()}`, { n: S.page, ...body });
  if (r.error) { toast(r.error); return false; }
  editGen++;
  // (a word changed: the reader is told in how many places of the whole book it stood)
  if (body.kind === "word" && typeof r.found === "number") {
    lastFound = r.found;
    toast(r.found ? `در ${fa(r.found)} جای کتاب عوض شد.` : "این کلمه جایی در کتاب پیدا نشد؛ شاید املای آن در کتاب فرق دارد.");
  }
  // (the page is shown again with the correction, at the same place: not at the top)
  const y = window.scrollY;
  pageCache.clear();
  stopReading();
  await loadPage(S.page);
  window.scrollTo(0, y);
  $("edInfo").textContent = r.edits ? `${fa(r.edits)} ویرایش در این کتاب` : "";
  return true;
}
// editing a paragraph: a window with a large box, over the page
let editing = { shown: null };
function openParaEditor(p) {
  const shown = S.paras[+p.dataset.i];
  if (shown === undefined) return;
  editing = { shown };
  $("paraText").value = shown;
  openPanel("paraPanel");
  $("paraText").focus();
}
$("paraSave").onclick = async () => {
  if (editing.shown == null) return;
  if (await sendEdit({ kind: "paragraph", shown: editing.shown, text: $("paraText").value })) $("paraPanel").hidden = true;
};
$("paraDel").onclick = async () => {
  if (editing.shown == null || !confirm("این بند حذف شود؟")) return;
  if (await sendEdit({ kind: "paragraph", shown: editing.shown, text: "" })) $("paraPanel").hidden = true;
};
function openWordEditor(from) {
  if (typeof from === "string") $("edFrom").value = from;
  openPanel("wordPanel");
  ($("edFrom").value ? $("edTo") : $("edFrom")).focus();
}
$("editBtn").onclick = () => {
  S.editing = !S.editing;
  $("editBar").hidden = !S.editing;
  $("textView").classList.toggle("editing", S.editing);
  $("editBtn").classList.toggle("primary", S.editing);
  if (S.editing) {
    stopReading();
    if (S.view !== "text") { S.view = "text"; syncChrome(); }
    if (S.doc) jget(`/api/edits?doc=${S.doc.id}`).then(r => { $("edInfo").textContent = r.edits ? `${fa(r.edits)} ویرایش در این کتاب` : ""; }).catch(() => {});
  }
};
$("edWordBtn").onclick = () => openWordEditor();
// the word is always read with the linking «ـِ» (ezafe): «کتاب» becomes «کتابِ» in this book's text (a word correction, so it also reaches the MP3 and the PDF)
async function addEzafe(w) {
  w = (w || "").trim().replace(/\u0650$/, "");
  if (!w) return false;
  return await sendEdit({ kind: "word", from: w, to: w + "\u0650" });
}
$("edEzafe").onclick = async () => {
  const w = $("edFrom").value.trim();
  if (!w) return toast("اول کلمه را بنویسید.");
  if (!await addEzafe(w)) return;
  $("wordPanel").hidden = true;
  toast("«" + w + "» از این پس در همه‌ی کتاب‌ها با کسره‌ی اضافه خوانده می‌شود.");
  sayWord(w + "\u0650 من");
};
$("edWord").onclick = async () => {
  if (!$("edFrom").value.trim()) return toast("کلمه‌ی غلط را بنویسید.");
  if (await sendEdit({ kind: "word", from: $("edFrom").value, to: $("edTo").value })) { $("edFrom").value = ""; $("edTo").value = ""; $("wordPanel").hidden = true; toast("در همه‌ی کتاب‌ها عوض شد."); }
};
$("edReset").onclick = async () => {
  if (confirm("همه‌ی ویرایش‌های این کتاب و همه‌ی کلمه‌های اصلاح‌شده‌ی کتاب‌ها پاک شود؟") && await sendEdit({ kind: "reset" })) toast("ویرایش‌ها پاک شد.");
};


/* ---------------- a text typed or pasted: read aloud and saved as sound ---------------- */
// (changed while a job runs: its remaining pieces are made the new way at once)
$("fastExport").onchange = () => {
  if (!(job && job.status === "running")) return;
  const fast = $("fastExport").checked;
  jpost("/api/mp3fast", { fast }).then(() => toast(fast ? "از این لحظه سریع‌تر ساخته می‌شود." : "از این لحظه با بررسی کامل ساخته می‌شود.")).catch(e => toast(e.message));
};
$("fastExport").checked = /Android|iPhone|iPad/i.test(navigator.userAgent);   // (on by default on the phones, which are slower)
$("m4aLabel").hidden = /Android|iPhone|iPad/i.test(navigator.userAgent);   // (the phones have no M4A maker)
// (Windows: no MP3 and no M4A maker yet: WAV)
if (/Windows/i.test(navigator.userAgent)) {
  $("m4aLabel").hidden = true; $("mp3Label").hidden = true;
  document.querySelector("input[name=afmt][value=wav]").checked = true;
  $("mp3Btn").textContent = "ذخیره‌ی صدا";
}
async function openTyped() {
  const t = $("typeText").value.trim();
  if (!t) { toast("اول متنی بنویسید."); return false; }
  stopReading();
  const d = await (await api("/api/open?name=" + encodeURIComponent("متن من.txt"), { method: "POST", body: new TextEncoder().encode(t).buffer })).json();
  if (d.error) { toast(d.error); return false; }
  S.doc = d; S.page = 1; S.view = "text";
  $("fileName").textContent = d.name;
  $("welcome").hidden = true;
  $("from").value = fa(1); $("to").value = fa(d.pages);
  note(`متن شما آماده است. با دکمه‌ی «خواندن» آن را بشنوید، با «ذخیره‌ی صدا» فایل صدا بسازید و با «ویرایش متن» آن را عوض کنید.`);
  $("typePanel").hidden = true;
  await loadPage(1);
  return true;
}

// ---- speech to text: a sound file or a live recording written down in Farsi
let sttBusy = false, sttRec = null;
const sttSay = m => { $("sttStatus").textContent = m; };
const sttMMSS = s => fa(String(Math.floor(s / 60)).padStart(2, "0")) + ":" + fa(String(Math.floor(s % 60)).padStart(2, "0"));
async function sttRun(path, body) {
  sttBusy = true; $("sttFile").disabled = $("sttLive").disabled = true; $("sttCancel").hidden = false;
  try {
    await api(path, { method: "POST", body });
    let r;
    for (;;) {
      r = await (await api("/api/stt_status")).json();
      if (r.state === "loading") sttSay("مدل آماده می‌شود…");
      else if (r.state === "working") sttSay(`در حال نوشتن… بخش ${fa(r.done)} از ${fa(r.total)}`);
      if (r.state === "done" || r.state === "error") break;
      await sleep(700);
    }
    if (r.state === "error") throw new Error(r.error || "نوشتن ناموفق بود.");
    if (!r.text) { sttSay("گفتاری شنیده نشد."); return; }
    const box = $("typeText");
    box.value = (box.value.trim() ? box.value.replace(/\s+$/, "") + "\n\n" : "") + r.text;
    sttSay("نوشتن تمام شد. متن را بخوانید و در صورت لزوم ویرایش کنید.");
  } catch (e) { sttSay("خطا: " + e.message); }
  finally { sttBusy = false; $("sttFile").disabled = $("sttLive").disabled = false; $("sttCancel").hidden = true; }
}
$("sttFile").onclick = () => { if (!sttBusy) $("sttInput").click(); };
$("sttInput").onchange = async () => {
  const f = $("sttInput").files[0]; $("sttInput").value = "";
  if (!f || sttBusy) return;
  sttSay("فایل خوانده می‌شود…");
  await sttRun("/api/stt_start?lang=fa&name=" + encodeURIComponent(f.name), await f.arrayBuffer());
};
$("sttCancel").onclick = () => { api("/api/stt_cancel", { method: "POST", body: "" }).catch(() => {}); sttSay("انصراف داده شد؛ آنچه نوشته شده بود می‌آید…"); };
$("sttLive").onclick = async () => {
  if (sttBusy) return;
  if (sttRec) { sttRec.stop = true; return; }
  let stream;
  try { stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: true, autoGainControl: true } }); }
  catch (e) { sttSay("به میکروفون دسترسی نیست."); return; }
  const ctx = new (window.AudioContext || window.webkitAudioContext)();
  try { await ctx.resume(); } catch (e) {}
  const src = ctx.createMediaStreamSource(stream), node = ctx.createScriptProcessor(4096, 1, 1);
  const ratio = ctx.sampleRate / 16000, chunks = [];
  let idx = 0, acc = 0, cnt = 0, outN = 0, cur = [];
  // 16 kHz, 16-bit, made as the recording goes (an hour of speech stays about 115 MB)
  node.onaudioprocess = e => {
    const d = e.inputBuffer.getChannelData(0);
    for (let i = 0; i < d.length; i++) {
      acc += d[i]; cnt++; idx++;
      if (idx >= (outN + 1) * ratio) { cur.push(Math.max(-1, Math.min(1, acc / cnt)) * 32767); acc = 0; cnt = 0; outN++; }
    }
    if (cur.length >= 16000) { chunks.push(Int16Array.from(cur)); cur = []; }
  };
  src.connect(node); node.connect(ctx.destination);
  sttRec = { stop: false };
  $("sttLive").textContent = "■ پایان ضبط"; $("sttFile").disabled = true;
  const t0 = Date.now();
  while (!sttRec.stop && Date.now() - t0 < 2 * 3600 * 1000) { sttSay("در حال ضبط… " + sttMMSS((Date.now() - t0) / 1000) + " (برای پایان، «پایان ضبط» را بزنید)"); await sleep(250); }
  node.disconnect(); src.disconnect(); stream.getTracks().forEach(t => t.stop()); ctx.close();
  sttRec = null; $("sttLive").textContent = "🎤 ضبط و نوشتن"; $("sttFile").disabled = false;
  if (cur.length) chunks.push(Int16Array.from(cur));
  const all = new Int16Array(chunks.reduce((n, c) => n + c.length, 0)); let at = 0;
  for (const c of chunks) { all.set(c, at); at += c.length; }
  if (all.length < 16000) { sttSay("ضبط خیلی کوتاه بود."); return; }
  sttSay("ضبط تمام شد؛ نوشتن شروع می‌شود…");
  await sttRun("/api/stt_start?lang=fa&pcm=1", all.buffer);
};
const showType = () => { $("typePanel").hidden = false; $("typeText").focus(); };
$("typeBtn").onclick = showType;
$("typeBtn2").onclick = showType;
$("typeRead").onclick = async () => { ensureAudio(); if (await openTyped() && S.paras.length) readFrom(0); };
$("typeSave").onclick = async () => { if (await openTyped()) openSave("mp3"); };

/* ---------------- editing the text by voice: for people who cannot see the screen well ---------------- */
// The user says a command; the app repeats what it understood, in a spoken sentence, and asks «درست است؟»; only «بله» changes the text.
let voiceBusy = false, voiceHelped = false;
const buzz = ms => { try { navigator.vibrate && navigator.vibrate(ms); } catch (e) {} };
const vstatus = t => { const b = $("voiceStatus"); b.textContent = t; b.hidden = !t; };
async function speakWait(text) {
  ensureAudio();
  const url = await ttsUrl(text);
  audio.src = url;
  await new Promise(done => { audio.onended = audio.onerror = () => done(); audio.play().catch(() => done()); setTimeout(done, 25000); });
}
// one recording from the microphone, until the user stops speaking (or the time is up); the recognizer's answer: {text, cmd, yes}
async function listenOnce(maxMs, mode, endQuiet = 1.2) {
  const stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
  const ctx = new (window.AudioContext || window.webkitAudioContext)();
  try { await ctx.resume(); } catch (e) {}
  const src = ctx.createMediaStreamSource(stream), node = ctx.createScriptProcessor(4096, 1, 1);
  const chunks = [], t0 = Date.now();
  // the level of the room is measured in the first third of a second; speech is anything clearly above it
  let heard = false, quiet = 0, floor = 0, nFloor = 0, seen = 0;
  node.onaudioprocess = e => {
    const d = e.inputBuffer.getChannelData(0);
    chunks.push(new Float32Array(d));
    let sum = 0; for (let i = 0; i < d.length; i++) sum += d[i] * d[i];
    const rms = Math.sqrt(sum / d.length), secs = d.length / ctx.sampleRate;
    seen += secs;
    if (seen < 0.35) { floor = Math.max(floor, rms); nFloor++; return; }
    if (rms > Math.max(0.008, floor * 2.5)) { heard = true; quiet = 0; } else if (heard) quiet += secs;
  };
  src.connect(node); node.connect(ctx.destination);
  while (Date.now() - t0 < maxMs && !(heard && quiet > endQuiet) && !(!heard && Date.now() - t0 > 7000)) await sleep(100);
  node.disconnect(); src.disconnect(); stream.getTracks().forEach(t => t.stop());
  const rate = ctx.sampleRate; ctx.close();
  const all = new Float32Array(chunks.reduce((n, c) => n + c.length, 0)); let at = 0;
  for (const c of chunks) { all.set(c, at); at += c.length; }
  if (!heard) return { text: "", cmd: null, yes: false, go: null, silent: true };
  // a quiet voice is made louder (the loudest part to about 70% of the range, at most 30 times)
  let peak = 0; for (let i = 0; i < all.length; i++) { const v = Math.abs(all[i]); if (v > peak) peak = v; }
  const gain = peak > 0.001 ? Math.min(30, 0.7 / peak) : 1;
  // 16 kHz, 16-bit: each output sample is the mean of the input samples it covers
  const ratio = rate / 16000, out = new Int16Array(Math.floor(all.length / ratio));
  for (let i = 0; i < out.length; i++) {
    const a = Math.floor(i * ratio), b = Math.max(a + 1, Math.floor((i + 1) * ratio));
    let m = 0; for (let k = a; k < b && k < all.length; k++) m += all[k];
    out[i] = Math.max(-1, Math.min(1, gain * m / (b - a))) * 32767;
  }
  const r = await (await api("/api/listen?mode=" + mode + "&doc=" + encodeURIComponent(S.doc.id), { method: "POST", body: out.buffer })).json();
  // (what was heard is shown, so a person who can see a little, or a helper, sees what went wrong)
  if (r.text !== undefined) vstatus("شنیدم: «" + (clean(r.text) || "—") + "»");
  return r;
}
function currentPiece() {
  const now = actx ? actx.currentTime : 0;
  const p = reader.playing.find(u => now >= u.t0 && now < u.t1) || reader.playing[reader.playing.length - 1];
  return p && p.p >= 0 ? { page: p.page, p: p.p } : null;
}
const clean = t => (t || "").replace(/\[unk\]/g, " ").replace(/\s+/g, " ").trim();
async function voiceEdit() {
  if (voiceBusy || !S.doc) return;
  voiceBusy = true;
  // «این کلمه» is the word being read now (marked on the page)
  const curEl = document.querySelector(".w.cur");
  const curWord = curEl ? clean(curEl.textContent.replace(/[^\p{L}\u200c]/gu, "")) : "";
  const cur = currentPiece();
  stopReading();
  const ask = async (say, show, mode, ms, endQuiet) => { vstatus(show); if (say) await speakWait(say); vstatus("گوش می‌دهم…\n" + show); return await listenOnce(ms, mode, endQuiet); };
  const stop = async m => { vstatus(m); await speakWait(m); };
  const theWord = async () => curWord || clean((await ask("کدام کلمه؟", "کدام کلمه؟ آن را بگویید", "book", 9000)).text);
  try {
    vstatus("در حال آماده شدن…");
    const r1 = await ask("بفرمایید.", "بگویید، مثلاً: «تلفظ این کلمه را درست کن» · «کسره‌ی اضافه» · «این بند را عوض کن» · «این کلمه را جدا کن» · «این کلمه را عوض کن» · «این بند را حذف کن»", "cmd", 10000);
    const kind = r1.cmd;
    if (!kind) { await stop("متوجه نشدم."); return; }
    let q, apply;
    if (kind === "rewrite") {
      // the whole paragraph being read is said again by the user, in their own words
      const shown = cur && cur.page === S.page ? S.paras[cur.p] : undefined;
      if (shown === undefined) { await stop("بندی در حال خواندن نبود."); return; }
      const r5 = await ask("متن تازه‌ی این بند را بگویید. وقتی تمام شد، چند ثانیه ساکت بمانید.", "متن تازه‌ی این بند را بگویید؛ در پایان چند ثانیه ساکت بمانید", "free", 55000, 2.4);
      const newText = clean(r5.text);
      if (!newText) { await stop("متنی نشنیدم."); return; }
      vstatus("شنیدم:\n«" + newText + "»");
      await speakWait(newText);
      const r6 = await ask("این متن جای بند فعلی بیاید؟ بگویید بله یا خیر.", "شنیدم:\n«" + newText + "»\nجای این بند بیاید؟ بله یا خیر", "yes", 7000);
      if (r6.yes && await sendEdit({ kind: "paragraph", shown, text: newText })) {
        buzz(80); await stop("انجام شد.");
        if (cur && cur.page === S.page && S.paras.length) { ensureAudio(); readFrom(Math.min(cur.p, S.paras.length - 1)); }
      } else { await stop("انجام نشد."); }
      return;
    }
    if (kind === "paragraph") {
      const shown = cur && cur.page === S.page ? S.paras[cur.p] : undefined;
      if (shown === undefined) { await stop("بندی در حال خواندن نبود."); return; }
      q = "این بند حذف شود؟";
      apply = () => sendEdit({ kind: "paragraph", shown, text: "" });
    } else {
      const w = await theWord();
      if (!w) { await stop("کلمه را نشنیدم."); return; }
      if (kind === "ezafe") {
        const q2 = `«${w}» با کسره‌ی اضافه خوانده شود؟ بله یا خیر؟`;
        const r4 = await ask(q2, q2, "yes", 7000);
        if (r4.yes && await addEzafe(w)) { buzz(80); vstatus("انجام شد."); await speakWait(w + "\u0650 من"); await stop("ذخیره شد."); } else { await stop("انجام نشد."); }
        return;
      }
      if (kind === "say") {
        const p = clean((await ask("چطور؟", `«${w}» چطور خوانده شود؟ بگویید`, "free", 9000)).text);
        if (!p) { await stop("نشنیدم."); return; }
        const before = S.settings.lexicon || {};
        await saveLexicon({ ...before, [w]: p });
        vstatus(`«${w}» ← «${p}»`);
        await speakWait(w);
        const r3 = await ask("درست شد؟", "درست شد؟ بله یا خیر", "yes", 7000);
        if (r3.yes) { buzz(80); await stop("ذخیره شد."); } else { await saveLexicon(before); await stop("برگردانده شد."); }
        return;
      }
      let to = "";
      if (kind === "split") {
        to = clean((await ask("دو کلمه را جدا بگویید.", `«${w}» به چه دو کلمه‌ای جدا شود؟ بگویید`, "free", 9000)).text);
        if (!to) { await stop("نشنیدم."); return; }
        q = `«${w}» به «${to}» تبدیل شود؟`;
      } else if (kind === "replace") {
        to = clean((await ask("با چه کلمه‌ای؟", `«${w}» با چه کلمه‌ای عوض شود؟ بگویید`, "free", 9000)).text);
        if (!to) { await stop("نشنیدم."); return; }
        q = `«${w}» به «${to}» تبدیل شود؟`;
      } else {
        q = `«${w}» حذف شود؟`;
      }
      apply = () => sendEdit({ kind: "word", from: w, to });
    }
    const r2 = await ask(q + " بله یا خیر؟", q, "yes", 7000);
    if (r2.yes && await apply()) {
      buzz(80);
      await stop(typeof lastFound === "number" && (kind === "replace" || kind === "delete" || kind === "split") ? (lastFound ? `انجام شد. در ${lastFound} جای کتاب عوض شد.` : "انجام شد، اما این کلمه جای دیگری در کتاب پیدا نشد.") : "انجام شد.");
      if (cur && cur.page === S.page && S.paras.length) { ensureAudio(); readFrom(Math.min(cur.p, S.paras.length - 1)); }
    } else {
      await stop("انجام نشد.");
    }
  } catch (e) {
    const denied = e && (e.name === "NotAllowedError" || e.name === "SecurityError");
    vstatus(denied ? "برنامه به میکروفون دسترسی ندارد. در تنظیمات دستگاه اجازه‌ی میکروفون را بدهید." : "خطا: " + (e && e.message || e));
  } finally {
    voiceBusy = false;
    setTimeout(() => { if (!voiceBusy) vstatus(""); }, 5000);
  }
}
$("voiceBtn").onclick = voiceEdit;

/* ---------------- figures: go to the next or the previous one and read its caption ---------------- */
let figList = null, figDoc = null;
async function figures() {
  if (figDoc !== S.doc.id) { figList = (await (await api(`/api/figures?doc=${S.doc.id}`)).json()).figures || []; figDoc = S.doc.id; }
  return figList;
}
// where the listener is now: the paragraph being read, else the top of the page
async function jumpFigure(dir) {
  if (!S.doc) return false;
  const list = await figures();
  if (!list.length) { toast("در این کتاب شکل یا تصویری پیدا نشد."); vstatus("شکلی پیدا نشد."); return false; }
  const cur = currentPiece();
  const at = cur && cur.page === S.page ? cur.p : -1;
  const before = (f) => f[0] < S.page || (f[0] === S.page && f[1] <= at);
  const after = (f) => f[0] > S.page || (f[0] === S.page && f[1] > at);
  const pick = dir > 0 ? list.find(after) : [...list].reverse().find(f => f[0] < S.page || (f[0] === S.page && f[1] < Math.max(at, 0)));
  if (!pick) { toast(dir > 0 ? "شکل دیگری بعد از این نیست." : "شکل دیگری قبل از این نیست."); return false; }
  const idx = list.indexOf(pick) + 1;
  stopReading();
  await loadPage(pick[0]);
  const i = translating() ? 0 : Math.min(pick[1], Math.max(0, S.paras.length - 1));
  buzz(50);
  toast(`شکل ${fa(idx)} از ${fa(list.length)}`);
  ensureAudio();
  readFrom(i);
  return true;
}

// ---- the writing window can be made wider or narrower (the width is remembered)
(function () {
  const grip = $("typeGrip"), body = document.querySelector("#typePanel .panel-body");
  const lim = w => Math.max(340, Math.min(window.innerWidth - 40, w));
  const set = (w, save) => {
    if (w == null) { body.style.width = ""; try { localStorage.removeItem("khana-type-width"); } catch (e) {} return; }
    body.style.width = lim(w) + "px";
    if (save) { try { localStorage.setItem("khana-type-width", String(Math.round(lim(w)))); } catch (e) {} }
  };
  try { const w = +localStorage.getItem("khana-type-width"); if (w > 0) set(w, false); } catch (e) {}
  const rtl = () => getComputedStyle(document.documentElement).direction === "rtl";
  grip.addEventListener("pointerdown", e => {
    grip.setPointerCapture(e.pointerId); grip.classList.add("drag"); e.preventDefault();
  });
  grip.addEventListener("pointermove", e => {
    if (!grip.classList.contains("drag")) return;
    set(rtl() ? window.innerWidth - e.clientX : e.clientX, false);
  });
  const end = e => { if (!grip.classList.contains("drag")) return; grip.classList.remove("drag"); set(body.getBoundingClientRect().width, true); };
  grip.addEventListener("pointerup", end); grip.addEventListener("pointercancel", end);
  grip.addEventListener("dblclick", () => set(null));
  grip.addEventListener("keydown", e => {
    const step = e.shiftKey ? 120 : 40, w = body.getBoundingClientRect().width;
    // (toward the page's middle is wider: left in Farsi, where the window stands at the right)
    const wider = rtl() ? "ArrowLeft" : "ArrowRight", narrower = rtl() ? "ArrowRight" : "ArrowLeft";
    if (e.key === wider) { set(w + step, true); e.preventDefault(); }
    else if (e.key === narrower) { set(w - step, true); e.preventDefault(); }
    else if (e.key === "Home") { set(null); }
  });
})();

// Back (the Android back gesture or button): close the window that is open, or go to the page before; false: nothing to go back to
window.khanaBack = function () {
  const open = [...document.querySelectorAll(".panel")].filter(p => !p.hidden);
  if (open.length) { open.forEach(p => { p.hidden = true; }); return true; }
  if (S.doc && S.page > 1) { $("prev").click(); return true; }
  return false;
};
