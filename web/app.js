"use strict";
// Defense-in-depth offline lock: disable outbound real-time/beacon network APIs on all platforms.
for (const k of ["RTCPeerConnection", "webkitRTCPeerConnection", "RTCDataChannel", "RTCSessionDescription", "RTCIceCandidate", "WebSocket", "EventSource"]) {
  try { Object.defineProperty(window, k, { value: undefined, configurable: false, writable: false }); } catch (_) {}
}
try { if (navigator.sendBeacon) Object.defineProperty(navigator, "sendBeacon", { value: () => false, configurable: false, writable: false }); } catch (_) {}
// (the token comes in a meta tag: the page has no inline script, so its policy can forbid them)
const TOKEN = document.querySelector("meta[name=khana-token]").content;
const $ = id => document.getElementById(id);
const fa = n => String(n).replace(/\d/g, d => "۰۱۲۳۴۵۶۷۸۹"[d]);
// (numbers typed in a box are Farsi digits; whatever digits are typed are read: ۱۲, 12 and ١٢ are all twelve)
const latinDigits = t => String(t).replace(/[۰-۹]/g, d => "۰۱۲۳۴۵۶۷۸۹".indexOf(d)).replace(/[٠-٩]/g, d => "٠١٢٣٤٥٦٧٨٩".indexOf(d));
const numOf = v => parseInt(latinDigits(v).replace(/\D/g, ""), 10) || 0;
const esc = s => String(s ?? "").replace(/[&<>"']/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
const reEsc = s => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

async function api(path, opts = {}) {
  try {
    const res = await fetch(path, { ...opts, headers: { "X-Khana-Token": TOKEN, ...(opts.headers || {}) } });
    if (!res.ok) {
      // (the app is locked: the lock screen comes up, web/lock.js)
      if (res.status === 423 && window.khanaLockHit) window.khanaLockHit();
      let msg = "خطا";
      try { msg = (await res.json()).error || msg; } catch (e) {}
      throw new Error(msg);
    }
    return res;
  } catch (err) {
    if (err instanceof TypeError) {
      throw new Error("خطا در ارتباط با بخش داخلی کتاب‌خوان. برنامه را ببندید و دوباره باز کنید.");
    }
    throw err;
  }
}
const jget = async p => (await api(p)).json();
const jpost = async (p, body) => (await api(p, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) })).json();

const S = { settings: {}, voices: [], doc: null, page: 1, view: "text", paras: [], loadSeq: 0 };

/* ---------------- settings ---------------- */
// (the looks to choose from; anything else follows the system: light or dark)
// (modern: the 2026 design, the default for a new install; a saved choice is kept)
const THEMES = ["modern", "auto", "light", "dark", "night", "black", "blackyellow", "glass", "glassdark", "depth", "depthdark"];
function applySettings() {
  const s = S.settings;
  const uiScale = [1, 1.15, 1.3].includes(Number(s.uiScale)) ? Number(s.uiScale) : 1;
  document.documentElement.style.setProperty("--ui-scale", String(uiScale));
  document.documentElement.dataset.lowvision = s.lowVision ? "1" : "0";
  for (const id of ["visionBtn", "lowVisionTop"]) $(id).setAttribute("aria-pressed", String(!!s.lowVision));
  $("lowVisionSwitch").checked = !!s.lowVision;
  document.querySelectorAll("input[name=lvColor]").forEach(r => r.checked = r.value === (s.lowVisionColor === "white" ? "white" : "yellow"));
  document.body.classList.toggle("focus-reading", !!s.focusMode);
  $("focusBtn").textContent = s.focusMode ? "همه‌ی ابزارها" : "نمای خلوت";
  $("focusBtn").setAttribute("aria-pressed", String(!!s.focusMode));
  document.querySelectorAll('[data-ui-scale]').forEach(b => b.setAttribute('aria-pressed', String(Number(b.dataset.uiScale) === uiScale)));
  document.querySelectorAll('[data-vision-theme]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.visionTheme === s.theme)));
  document.documentElement.style.setProperty("--fs", s.fontSize + "px");
  const lineHeight = Math.min(3, Math.max(1.5, Number(s.lineHeight) || 2.2));
  document.documentElement.style.setProperty("--reader-line-height", String(lineHeight));
  // (1.29) the space between the words of the book: wider helps tired eyes and readers who lose their place in a line
  const ws = ["normal", "wide", "wider"].includes(s.wordSpacing) ? s.wordSpacing : "normal";
  document.documentElement.style.setProperty("--word-gap", ws === "wider" ? ".5em" : ws === "wide" ? ".25em" : "normal");
  document.querySelectorAll("input[name=wordSpacing]").forEach(r => r.checked = r.value === ws);
  $("lineHeight").value = lineHeight;
  $("lineHeightVal").textContent = fa(lineHeight.toFixed(1));
  $("lineHeight").setAttribute("aria-valuetext", fa(lineHeight.toFixed(1)) + " برابر اندازه‌ی قلم");
  // (the text's own page: its width in letters' widths, 100 is the whole window; the room round the text, 100 is as it was made)
  const pw = Math.min(100, Math.max(26, Number(s.pageWidth) || 46)), pm = Math.min(200, Math.max(20, Number(s.pageMargin) || 100));
  document.documentElement.style.setProperty("--pw", pw >= 100 ? "100%" : Math.round(pw * 21) + "px");
  document.documentElement.style.setProperty("--pm", String(pm / 100));
  $("pageWidth").value = pw; $("pageWidthVal").textContent = pw >= 100 ? "تمام پهنا" : fa(pw);
  $("pageMargin").value = pm; $("pageMarginVal").textContent = fa(pm) + "٪";
  $("clickSound").checked = !!s.clickSound;
  $("sentenceLines").checked = !!s.sentenceLines;
  $("speed").value = s.speed; $("speedVal").textContent = pct(s.speed);
  $("speedChip").textContent = pct(s.speed);
  $("quickSpeedValue").textContent = pct(s.speed);
  $("quickSpeed").setAttribute("aria-label", "سرعت خواندن " + pct(s.speed) + "؛ تنظیم سرعت");
  $("speedChip").classList.toggle("changed", Number(s.speed) !== 1);
  $("speedChip").setAttribute("aria-label", "سرعت " + pct(s.speed) + "؛ برای سرعت عادی بزنید");
  const chosenTheme = s.theme === undefined ? "modern" : s.theme;
  document.documentElement.dataset.theme = THEMES.includes(chosenTheme) ? chosenTheme : "auto";
  document.querySelectorAll("input[name=comma]").forEach(r => r.checked = r.value === (s.commaPause || "short"));
  document.querySelectorAll("input[name=sentence]").forEach(r => r.checked = r.value === (s.sentencePause || "normal"));
  document.querySelectorAll("input[name=theme]").forEach(r => r.checked = r.value === (THEMES.includes(chosenTheme) ? chosenTheme : "auto"));
  const ACCENTS = ["khana", "blue", "green", "orange", "pink", "purple", "teal", "red"];
  // (1.29: «آبی» had no colour of its own and showed the theme's own colour; a saved «آبی» from before is that colour, «رنگ پوسته»)
  if (s.accent === "blue" && !s.accentFixed) { s.accent = "khana"; s.accentFixed = true; }
  const accent = ACCENTS.includes(s.accent) ? s.accent : "khana";
  document.documentElement.dataset.accent = accent;
  document.querySelectorAll("input[name=accent]").forEach(r => r.checked = r.value === accent);
  const iconStyle = ["flat", "line", "ios", "glossy"].includes(s.iconStyle) ? s.iconStyle : (document.documentElement.classList.contains("android-reader") ? "ios" : "glossy"); // (the phone's own look: iPhone-style tiles)
  document.documentElement.dataset.icons = iconStyle;
  document.querySelectorAll("input[name=iconStyle]").forEach(r => r.checked = r.value === iconStyle);
  const corners = ["soft", "sharp"].includes(s.corners) ? s.corners : "round";
  document.documentElement.dataset.corners = corners;
  document.querySelectorAll("input[name=corners]").forEach(r => r.checked = r.value === corners);
  $("startSound").checked = s.startSound !== false;
  document.documentElement.dataset.text3d = s.text3d ? "1" : "0";
  $("text3d").checked = !!s.text3d;
  const chosenFont = ["vazir", "vazirbold", "vazirlight", "naskh", "system"].includes(s.font) ? s.font : "vazir";
  document.documentElement.dataset.font = chosenFont;
  document.querySelectorAll("input[name=font]").forEach(r => r.checked = r.value === chosenFont);
  document.querySelectorAll("[data-speed]").forEach(b => b.classList.toggle("on", Number(b.dataset.speed) === Number(s.speed) && b.id !== "speedReset"));
  $("fontSize").value = s.fontSize; $("fontVal").textContent = fa(s.fontSize);
  $("fontChip").textContent = fa(s.fontSize);
  $("fontChip").setAttribute("aria-label", "اندازه‌ی متن " + fa(s.fontSize) + "؛ بازگشت به ۲۱");
  $("fontDown").disabled = Number(s.fontSize) <= 14;
  $("fontUp").disabled = Number(s.fontSize) >= 72;
  $("voice").value = s.voice;
  if (typeof syncVoiceUI === "function") syncVoiceUI();
  applyCursor(s.cursor || "large");
  document.querySelectorAll("input[name=cursor]").forEach(r => r.checked = r.value === (s.cursor || "large"));
  $("autoNext").checked = !!s.autoNext;
  $("naturalTone").checked = s.naturalTone !== false;
  if ($("showWaitReason")) $("showWaitReason").checked = s.showWaitReason !== false;
  if ($("smoothExtra")) $("smoothExtra").checked = s.smoothBuffer === "extra";
  const trq = document.querySelector(`input[name=trQuality][value="${s.trQuality === "accurate" ? "accurate" : "fast"}"]`);
  if (trq) trq.checked = true;
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
  if (typeof renderMultiVoiceExport === "function") renderMultiVoiceExport();
  if (saveKind !== "pdf") {
    const mvOn = !!s.exportMultiVoice && typeof exportVoiceIds === "function";
    const mvIds = mvOn ? exportVoiceIds() : [];
    if (mvOn && mvIds.length >= 2) {
      const names = mvIds.map(id => (S.voices.find(x => x.id === id) || {}).name || id).join(" ← ");
      $("mp3Voice").textContent = `خواندن نوبتی با ${fa(mvIds.length)} مدل صدا: ${names} · سرعت ${pct(s.speed)} · ${fa(s.bitrate)} کیلوبیت`;
    } else {
      $("mp3Voice").textContent = "صدا: " + (v ? v.name : "-") + " · سرعت " + pct(s.speed) + " · " + fa(s.bitrate) + " کیلوبیت (از تنظیمات)";
    }
  }
  document.documentElement.classList.remove("boot-dark");
}
let saveT;
function setSetting(k, v, reloadText) {
  S.settings[k] = v; applySettings();
  clearTimeout(saveT); saveT = setTimeout(() => jpost("/api/settings", S.settings).catch(() => {}), 300);
  if (reloadText && S.doc) loadPage(S.page, true);
}
// speech is prepared well ahead, so a new voice or speed restarts reading at the sentence being heard
let restartT;
let restartGen = 0; // (a newer change cancels an older one still waiting for its sentences)
function restartReading(immediate = false, reason = "") {
  clearTimeout(restartT);
  const gen = ++restartGen;
  if (!reader.active) return;
  if (reason && window.setReadingWait) window.setReadingWait(reason);
  const runRestart = async () => {
    if (!reader.active) return;
    dropOldSpeech();
    reader.restarting = gen;
    try { await restartNow(gen, reason); } finally { if (reader.restarting === gen) reader.restarting = 0; }
  };
  if (immediate) runRestart();
  else restartT = setTimeout(runRestart, 350);
}
async function restartNow(gen, reason = "") {
  {
    const now = actx ? actx.currentTime : 0;
    // (1.29 deep check: before the first sentence is heard nothing is playing yet; the next piece of the queue is where reading
    // started, not the top of the page)
    const piece = reader.playing.find(u => now < u.t1) || reader.playing[reader.playing.length - 1] || reader.queue[reader.next] || reader.queue[reader.queue.length - 1];
    const wasPaused = reader.paused;
    if (piece && piece.p < 0) return;  // (the closing «پایان متن.»: nothing is left to read again)
    // A new voice (or speed, or pause): the sentence being said is finished first, while the next two are already made in the
    // new way; then the reading goes on with them at once. (Before: the sound was cut and the first new sentence made from
    // nothing, a silence of several seconds. If the sentences are not ready in time, it goes on as before.)
    if (piece && !wasPaused && actx && piece.p >= 0 && piece.page === S.page && piece.t1 > now) {
      const k = reader.queue.indexOf(piece), next = k >= 0 ? reader.queue[k + 1] : undefined;
      if (next && next.p >= 0 && next.page === S.page) {
        const run = reader.run;
        try {
          const warm = piecesFor(S.page, S.paras, next.p, next.start).slice(0, 2).reverse().map(q => {
            const w = speechBuffer(q.text, run, true);
            if (w.owner) w.owner.keep = true;
            return w.catch(() => {});
          });
          await Promise.race([Promise.all(warm), sleep(Math.max(0, (piece.t1 - now) * 1000))]);
          if (!reader.active || reader.run !== run || gen !== restartGen) return;
          // (1.29 deep check: paused meanwhile: the full restart below, which keeps the pause; before, the change was lost and
          // up to 30 s of old speech stayed queued)
          if (!reader.paused) {
            const wait = piece.t1 - actx.currentTime;
            if (wait > 0.05) await sleep(wait * 1000);
            if (!reader.active || reader.run !== run || gen !== restartGen) return;
          }
          if (reader.paused) { readFrom(next.p, next.start, { reason }); actx.suspend(); reader.paused = true; setPlayButton(); return; }
          readFrom(next.p, next.start, { reason });
          return;
        } catch (e) { if (!reader.active || reader.run !== run || gen !== restartGen) return; }
      }
    }
    if (piece && piece.page !== S.page) showPage(piece.page, reader.pageParas[piece.page], !reader.partial[piece.page]);
    if (piece && piece.p >= 0) readFrom(piece.p, piece.start, { reason }); else readFrom(0, 0, { reason });
    if (wasPaused) { actx.suspend(); reader.paused = true; setPlayButton(); }
  }
}
function syncVoiceUI() {
  const cur = S.settings.voice;
  $("voice").value = cur;
  $("dockVoice").value = cur;
  const v = (S.voices || []).find(x => x.id === cur);
  if ($("dockVoiceName")) $("dockVoiceName").textContent = v ? v.name : "مدل صدا";
  if ($("dockVoiceTag")) $("dockVoiceTag").textContent = "";
  document.querySelectorAll(".voice-card").forEach(c => {
    const on = c.dataset.voice === cur;
    c.classList.toggle("active", on);
    c.setAttribute("aria-checked", String(on));
  });
  const nat = S.settings.naturalTone !== false;
  if ($("vpNaturalTone")) $("vpNaturalTone").checked = nat;
  const cp = S.settings.commaPause || "short", sp = S.settings.sentencePause || "normal";
  const rhythm = (cp === "short" && sp === "short") ? "fast" : (cp === "long" && sp === "long") ? "calm" : "normal";
  document.querySelectorAll("input[name=vpRhythm]").forEach(r => r.checked = r.value === rhythm);
}
function selectVoice(id, restart = true) {
  if (!id) return;
  const changed = S.settings.voice !== id;
  setSetting("voice", id);
  syncVoiceUI();
  if (changed) {
    ttsCache.clear();
    if (restart) restartReading(true, "در حال تعویض صدا…");
  }
}
$("voice").onchange = e => selectVoice(e.target.value);
$("dockVoice").onchange = e => selectVoice(e.target.value);
if ($("dockVoiceBtn")) {
  $("dockVoiceBtn").onclick = () => {
    syncVoiceUI();
    openPanel("voicePanel");
    $("dockVoiceBtn").setAttribute("aria-expanded", "true");
  };
  new MutationObserver(() => {
    if ($("voicePanel").hidden) $("dockVoiceBtn").setAttribute("aria-expanded", "false");
  }).observe($("voicePanel"), { attributes: true, attributeFilter: ["hidden"] });
}
// reading speed: 0.5× to 2×; the slider, the −/+ buttons, the presets and the Speed menu all come here
// (shown in percent: ۱۰۰٪ is normal; the −/+ buttons go one percent at a time, the menu's keys five)
const pct = v => fa(Math.round(Number(v) * 100)) + "٪";
const SPEED_MIN = 0.5, SPEED_MAX = 2, SPEED_STEP = 0.01, SPEED_KEY_STEP = 0.05;
function setSpeed(v, quiet) {
  v = Math.round(Math.min(SPEED_MAX, Math.max(SPEED_MIN, Number(v) || 1)) * 100) / 100;
  if (v === Number(S.settings.speed)) { if (!quiet) toast("سرعت " + pct(v)); return; }
  setSetting("speed", v); ttsCache.clear(); restartReading(); showBookTime();
  if (!quiet) toast(v === 1 ? "سرعت عادی (۱۰۰٪)" : "سرعت " + pct(v));
}
const resetSpeed = () => setSpeed(1);
$("speed").oninput = e => setSpeed(+e.target.value, true);
$("slower").onclick = () => setSpeed(Number(S.settings.speed) - SPEED_STEP);
$("faster").onclick = () => setSpeed(Number(S.settings.speed) + SPEED_STEP);
$("speedChip").onclick = resetSpeed;
document.querySelectorAll("[data-speed]").forEach(b => b.onclick = () => setSpeed(+b.dataset.speed));
function setFontSize(v) { setSetting("fontSize", Math.min(72, Math.max(14, Math.round(v)))); }
$("fontDown").onclick = () => setFontSize(Number(S.settings.fontSize) - 2);
$("fontUp").onclick = () => setFontSize(Number(S.settings.fontSize) + 2);
$("fontChip").onclick = () => setFontSize(21);
// commands from the app's menu bar (src/main.rs)
window.khanaMenu = cmd => {
  if (document.body.classList.contains("locked")) return;
  if (cmd === "stop") stopReading();
  // (the reading notification's pause, Android: the play button pauses and goes on)
  else if (cmd === "playpause") { if (reader.active) $("play").click(); }
  else if (cmd === "speed-reset") resetSpeed();
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
$("lineHeight").oninput = e => setSetting("lineHeight", +e.target.value);
document.querySelectorAll("input[name=cursor]").forEach(r => r.onchange = () => setSetting("cursor", r.value));
$("autoNext").onchange = e => { setSetting("autoNext", e.target.checked); restartReading(); };
$("naturalTone").onchange = e => { setSetting("naturalTone", e.target.checked); syncVoiceUI(); ttsCache.clear(); bufCache.clear(); restartReading(); };
if ($("vpNaturalTone")) $("vpNaturalTone").onchange = e => { setSetting("naturalTone", e.target.checked); $("naturalTone").checked = e.target.checked; ttsCache.clear(); bufCache.clear(); restartReading(); };
document.querySelectorAll("input[name=vpRhythm]").forEach(r => r.onchange = () => {
  const map = { fast: ["short", "short"], normal: ["short", "normal"], calm: ["long", "long"] };
  const [cp, sp] = map[r.value] || ["short", "normal"];
  setSetting("commaPause", cp);
  setSetting("sentencePause", sp);
  document.querySelectorAll("input[name=comma]").forEach(x => x.checked = x.value === cp);
  document.querySelectorAll("input[name=sentence]").forEach(x => x.checked = x.value === sp);
  ttsCache.clear(); restartReading();
});
if ($("showWaitReason")) $("showWaitReason").onchange = e => { setSetting("showWaitReason", e.target.checked); if (!e.target.checked && window.setReadingWait) window.setReadingWait(""); };
if ($("smoothExtra")) $("smoothExtra").onchange = e => setSetting("smoothBuffer", e.target.checked ? "extra" : "normal");
document.querySelectorAll("input[name=trQuality]").forEach(r => r.onchange = () => { setSetting("trQuality", r.value); pageCache.clear(); });
$("autoPlay").onchange = e => setSetting("autoPlay", e.target.checked);
document.querySelectorAll("input[name=hl]").forEach(r => r.onchange = () => setSetting("highlight", r.value));
document.querySelectorAll("input[name=fc]").forEach(r => r.onchange = () => setSetting("frameColor", r.value));
document.querySelectorAll("input[name=comma]").forEach(r => r.onchange = () => { setSetting("commaPause", r.value); syncVoiceUI(); ttsCache.clear(); restartReading(); });
document.querySelectorAll("input[name=sentence]").forEach(r => r.onchange = () => { setSetting("sentencePause", r.value); syncVoiceUI(); restartReading(); });
document.querySelectorAll("input[name=theme]").forEach(r => r.onchange = () => setSetting("theme", r.value));
document.querySelectorAll("input[name=font]").forEach(r => r.onchange = () => setSetting("font", r.value));
document.querySelectorAll("input[name=accent]").forEach(r => r.onchange = () => { S.settings.accentFixed = true; setSetting("accent", r.value); });
document.querySelectorAll("input[name=iconStyle]").forEach(r => r.onchange = () => setSetting("iconStyle", r.value));
document.querySelectorAll("input[name=corners]").forEach(r => r.onchange = () => setSetting("corners", r.value));
$("startSound").onchange = e => setSetting("startSound", e.target.checked);
$("text3d").onchange = e => setSetting("text3d", e.target.checked);
document.querySelectorAll("input[name=wordSpacing]").forEach(r => r.onchange = () => setSetting("wordSpacing", r.value));
$("pageWidth").oninput = e => setSetting("pageWidth", Number(e.target.value));
$("pageMargin").oninput = e => setSetting("pageMargin", Number(e.target.value));
$("sentenceLines").onchange = e => {
  const position = currentPiece();
  const anchor = position && position.page === S.page
    ? $("textView").querySelector(`p[data-i="${position.p}"]`)
    : [...$("textView").querySelectorAll('p[data-i]')].find(p => p.getBoundingClientRect().bottom > document.querySelector('.bar').getBoundingClientRect().bottom);
  const index = anchor && anchor.dataset.i, y = anchor && anchor.getBoundingClientRect().top;
  setSetting("sentenceLines", e.target.checked);
  if (S.doc && !S.editing) {
    renderText(true);
    requestAnimationFrame(() => {
      const next = index == null ? null : $("textView").querySelector(`p[data-i="${index}"]`);
      if (next) window.scrollBy(0, next.getBoundingClientRect().top - y);
    });
  }
};
$("clickSound").onchange = e => { setSetting("clickSound", e.target.checked); if (e.target.checked) clickTone(true); };

/* ---------------- a sound for every tap (Settings; off unless chosen): a short tick, higher when something is switched on, lower when off ---------------- */
// (its own little sound engine: the reader's is paused and resumed with the reading)
let clickCtx = null;
function clickTone(on) {
  try {
    clickCtx = clickCtx || new (window.AudioContext || window.webkitAudioContext)();
    if (clickCtx.state === "suspended") clickCtx.resume();
    const t = clickCtx.currentTime, o = clickCtx.createOscillator(), g = clickCtx.createGain();
    o.type = "sine"; o.frequency.value = on === true ? 1320 : on === false ? 520 : 880;
    g.gain.setValueAtTime(0.0001, t); g.gain.exponentialRampToValueAtTime(0.18, t + 0.004); g.gain.exponentialRampToValueAtTime(0.0001, t + 0.06);
    o.connect(g); g.connect(clickCtx.destination); o.start(t); o.stop(t + 0.07);
  } catch (e) {}
}
addEventListener("click", e => {
  if (!S.settings || !S.settings.clickSound) return;
  const el = e.target.closest && e.target.closest("button, .btn, input[type=checkbox], input[type=radio], summary, select");
  if (!el || el.disabled) return;
  // (a label's click comes again on its box: that one is the one heard)
  if (el.matches("input[type=checkbox]")) return clickTone(el.checked);
  if (el.matches("input[type=radio]")) return clickTone(true);
  if (el.hasAttribute("aria-pressed") || el.classList.contains("on")) return clickTone(el.classList.contains("on") || el.getAttribute("aria-pressed") === "true");
  clickTone();
}, true);

/* ---------------- finding a setting by a word of its name ---------------- */
// Settings in groups (tabs): one group at a time; a search looks in all of them
const SET_TABS = ["voice", "text", "look", "words", "safe"];
let setTab = "voice";
try { const t = localStorage.getItem("khana-set-tab"); if (SET_TABS.includes(t)) setTab = t; } catch (e) {}
function applySetTabs() {
  const searching = $("setFind").value.trim() !== "";
  document.querySelectorAll("#settings .panel-body > .field").forEach(f => {
    f.classList.toggle("set-off", !searching && (f.dataset.set || "safe") !== setTab);
  });
  $("setTabs").hidden = searching;
  document.querySelectorAll("#setTabs button").forEach(b => b.setAttribute("aria-selected", String(b.dataset.tab === setTab)));
}
document.querySelectorAll("#setTabs button").forEach(b => b.onclick = () => {
  setTab = b.dataset.tab;
  try { localStorage.setItem("khana-set-tab", setTab); } catch (e) {}
  applySetTabs();
  document.querySelector("#settings .panel-body").scrollTop = 0;
});
$("setFind").oninput = () => {
  const q = $("setFind").value.trim().replace(/ي/g, "ی").replace(/ك/g, "ک");
  let shown = 0;
  document.querySelectorAll("#settings .panel-body > .field").forEach(f => {
    const hit = !q || f.textContent.replace(/\u200c/g, " ").includes(q) || f.textContent.includes(q);
    f.hidden = !hit; f.classList.toggle("found", !!q && hit);
    if (hit) shown++;
  });
  $("setNone").hidden = shown > 0;
  applySetTabs();
};
applySetTabs();
new MutationObserver(() => { if ($("settings").hidden && $("setFind").value) { $("setFind").value = ""; $("setFind").oninput(); } }).observe($("settings"), { attributes: true, attributeFilter: ["hidden"] });
$("persianYeh").onchange = e => setSetting("persianYeh", e.target.checked, true);
$("reversed").onchange = e => setSetting("reversed", e.target.checked, true);
$("bitrate").onchange = e => setSetting("bitrate", +e.target.value);
async function previewVoice(voiceId, btn) {
  const prev = btn ? btn.textContent : "";
  if (btn) { btn.disabled = true; btn.textContent = "…"; }
  try {
    const url = await ttsUrl("سلام. من کتاب‌خوان هستم و متن فارسی را برای شما می‌خوانم.", voiceId);
    stopReading(); const a = $("audio"); a.src = url; await a.play();
  } catch (e) { toast(e.message); } finally { if (btn) { btn.disabled = false; btn.textContent = prev; } }
}
$("preview").onclick = () => previewVoice(S.settings.voice, $("preview"));

function renderVoiceCards(container, closeOnPick) {
  if (!container) return;
  container.textContent = "";
  const groups = [["mine", "صدای من"], ["natural", "صدای طبیعی"]];
  for (const [g, label] of groups) {
    const list = (S.voices || []).filter(v => v.group === g);
    if (!list.length) continue;
    const head = document.createElement("div");
    head.className = "voice-group-title";
    head.textContent = label;
    container.append(head);
    const grid = document.createElement("div");
    grid.className = "voice-grid";
    for (const v of list) {
      const card = document.createElement("div");
      card.className = "voice-card" + (v.id === S.settings.voice ? " active" : "");
      card.dataset.voice = v.id;
      card.setAttribute("role", "radio");
      card.setAttribute("tabindex", "0");
      card.setAttribute("aria-checked", String(v.id === S.settings.voice));
      const badgeClass = v.group === "mine" ? "vc-ic-mine" : "vc-ic-m";
      const sub = v.group === "mine" ? "صدای ضبط‌شده‌ی شما" : "صدای طبیعی";
      card.innerHTML = `<span class="vc-ic ${badgeClass}" aria-hidden="true"><svg viewBox="0 0 24 24" focusable="false"><path d="M4 9.5v5h3.5L12 18V6L7.5 9.5H4z" fill="#fff"/><path d="M15.5 9a4 4 0 0 1 0 6" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg></span><span class="vc-meta"><span class="vc-name">${esc(v.name)}</span><span class="vc-sub">${esc(sub)}</span></span><button type="button" class="vc-play" aria-label="شنیدن ${esc(v.name)}">▶</button>`;
      card.querySelector(".vc-play").onclick = ev => {
        ev.stopPropagation();
        previewVoice(v.id, ev.currentTarget);
      };
      const pick = () => {
        selectVoice(v.id, true);
        if (closeOnPick) $("voicePanel").hidden = true;
      };
      card.onclick = pick;
      card.onkeydown = ev => { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); pick(); } };
      grid.append(card);
    }
    container.append(grid);
  }
}

function fillVoices(voices) {
  S.voices = voices;
  const groups = [["mine", "صدای من"], ["natural", "صدای طبیعی"]];
  $("voice").innerHTML = groups.map(([g, label]) => {
    const opts = voices.filter(v => v.group === g).map(v => `<option value="${esc(v.id)}">${esc(v.name)}</option>`).join("");
    return opts ? `<optgroup label="${label}">${opts}</optgroup>` : "";
  }).join("");
  if (!voices.some(v => v.id === S.settings.voice) && voices[0]) S.settings.voice = voices[0].id;
  // the same list on the reading bar, so the voice can be changed without opening Settings
  $("dockVoice").innerHTML = $("voice").innerHTML;
  renderVoiceCards($("voiceCards"), false);
  renderVoiceCards($("voicePanelCards"), true);
  if (typeof renderMultiVoiceExport === "function") renderMultiVoiceExport();
  syncVoiceUI();
  $("myDel").hidden = !voices.some(v => v.id === "mine");
}

const DEFAULT_EXPORT_VOICES = ["seda1", "seda2", "seda4", "seda7", "seda10"];
function exportVoiceIds() {
  const avail = new Set((S.voices || []).map(v => v.id));
  const saved = Array.isArray(S.settings.exportVoices) ? S.settings.exportVoices : DEFAULT_EXPORT_VOICES;
  let picked = [...new Set(saved.filter(id => avail.has(id)))];
  if (picked.length < 2) {
    const fallback = DEFAULT_EXPORT_VOICES.filter(id => avail.has(id));
    picked = fallback.length >= 2 ? fallback : (S.voices || []).slice(0, 5).map(v => v.id);
  }
  return picked;
}
function renderMultiVoiceExport() {
  const cb = $("multiVoiceExport"), box = $("multiVoiceBox"), grid = $("multiVoiceGrid");
  if (!cb || !box || !grid) return;
  const on = !!S.settings.exportMultiVoice;
  cb.checked = on;
  box.hidden = !on || saveKind === "pdf";
  const picked = new Set(exportVoiceIds());
  grid.textContent = "";
  for (const v of (S.voices || [])) {
    const btn = document.createElement("button");
    btn.type = "button";
    const active = picked.has(v.id);
    btn.className = "mv-chip" + (active ? " on" : "");
    btn.textContent = (active ? "✓ " : "") + v.name;
    btn.setAttribute("aria-pressed", String(active));
    btn.onclick = () => {
      const cur = exportVoiceIds();
      const idx = cur.indexOf(v.id);
      if (idx >= 0) {
        if (cur.length <= 2) return toast("برای خواندن نوبتی دست‌کم ۲ مدل صدا لازم است.");
        cur.splice(idx, 1);
      } else {
        cur.push(v.id);
      }
      setSetting("exportVoices", cur);
    };
    grid.append(btn);
  }
}
if ($("multiVoiceExport")) $("multiVoiceExport").onchange = e => setSetting("exportMultiVoice", e.target.checked);
if ($("multiVoiceDefault")) $("multiVoiceDefault").onclick = () => {
  const avail = new Set((S.voices || []).map(v => v.id));
  const dflt = DEFAULT_EXPORT_VOICES.filter(id => avail.has(id));
  setSetting("exportVoices", dflt.length >= 2 ? dflt : (S.voices || []).slice(0, 5).map(v => v.id));
  toast("۵ مدل صدای متنوع انتخاب شد.");
};

/* ---------------- my voice ----------------
   Recorded as plain PCM through Web Audio and sent as a WAV file, so it works the same in
   every browser engine; a chosen file (Voice Memos m4a, mp3, wav) is converted on the Mac. */
async function useMyVoice(blob, name) {
  $("myStatus").textContent = "در حال آماده کردن صدای شما…";
  try {
    if (blob.size > 50 * 1024 * 1024) throw new Error("فایل نمونهٔ صدا خیلی بزرگ است.");
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
const MAX_MY_VOICE_SECS = 45;
async function stopRecording(save) {
  if (!rec) return;
  const r = rec; rec = null;
  clearInterval(r.timer);
  try { r.node.disconnect(); } catch (_) {}
  try { if (r.sink) r.sink.disconnect(); } catch (_) {}
  try { r.src.disconnect(); } catch (_) {}
  r.stream.getTracks().forEach(t => t.stop());
  r.ctx.close().catch(() => {});
  $("recBtn").textContent = "● ضبط صدای من"; $("recTime").hidden = true;
  if (!save) return;
  if ((Date.now() - r.t0) < 4000) { $("myStatus").textContent = "ضبط خیلی کوتاه بود. دست‌کم ۵ ثانیه صحبت کنید."; return; }
  useMyVoice(wavBlob(r.chunks, r.ctx.sampleRate), "recording.wav");
}
let recAsking = false;  // (1.29 deep check: a second tap while the microphone is being asked for opened a second recording)
$("recBtn").onclick = async () => {
  if (rec) return stopRecording(true);
  if (recAsking) return;
  stopReading();
  let stream;
  recAsking = true;
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, autoGainControl: true, noiseSuppression: true } });
  } catch (e) {
    recAsking = false;
    $("myStatus").textContent = "کتاب‌خوان به میکروفون دسترسی ندارد. در تنظیمات دستگاه اجازه‌ی میکروفون را بدهید، یا یک فایل صوتی ضبط‌شده را با «انتخاب فایل صدا» وارد کنید.";
    return;
  }
  recAsking = false;
  const ctx = new (window.AudioContext || window.webkitAudioContext)();
  try { await ctx.resume(); } catch (_) {}
  const src = ctx.createMediaStreamSource(stream), node = ctx.createScriptProcessor(4096, 1, 1);
  const sink = ctx.createGain(); sink.gain.value = 0;
  const chunks = [];
  node.onaudioprocess = e => chunks.push(new Float32Array(e.inputBuffer.getChannelData(0)));
  src.connect(node); node.connect(sink); sink.connect(ctx.destination);
  rec = { ctx, src, node, sink, stream, chunks, t0: Date.now() };
  $("recBtn").textContent = "■ پایان ضبط"; $("recTime").hidden = false;
  $("myStatus").textContent = "متن را آرام بخوانید؛ پس از پایان، «■ پایان ضبط» را بزنید (تا ۴۵ ثانیه).";
  const show = () => { const t = Math.floor((Date.now() - rec.t0) / 1000); $("recTime").textContent = "● " + fa(t) + " از " + fa(MAX_MY_VOICE_SECS) + " ثانیه"; if (t >= MAX_MY_VOICE_SECS) stopRecording(true); };
  show(); rec.timer = setInterval(() => rec && show(), 250);
};
$("voiceFile").onchange = e => { const f = e.target.files[0]; e.target.value = ""; if (f) useMyVoice(f, f.name); };
$("forgetAll").onclick = async () => {
  if (!await askConfirm("همه‌ی ویرایش‌ها، صدای شما، تنظیمات و اصلاح‌های تلفظ از این دستگاه پاک شود؟ اگر قفل برنامه روشن باشد، خاموش هم می‌شود. این کار برگشت ندارد.")) return;
  try {
    stopReading();
    await jpost("/api/forget", {});
    try { localStorage.clear(); } catch (e) {}
    location.reload();
  } catch (e) { toast(e.message); }
};
$("myDel").onclick = async () => {
  if (!await askConfirm("صدای ضبط‌شده‌ی شما پاک شود؟")) return;
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
  if ($("lexExport")) $("lexExport").hidden = !n;
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
const stripWordPunct = s => String(s || "").trim().replace(/^[«»()\[\]{}،؛:.!?؟"'`\-–—\s]+|[«»()\[\]{}،؛:.!?؟"'`\-–—\s]+$/g, "");
async function sayWord(text, spelling = "", btn = null) {
  const prevLabel = btn ? btn.textContent : "";
  if (btn) { btn.disabled = true; btn.textContent = "در حال آماده‌سازی…"; }
  try {
    stopReading();
    ensureAudio();
    const payload = { text, voice: S.settings.voice, speed: S.settings.speed, quick: true };
    if (spelling && spelling.trim()) payload.spelling = spelling.trim();
    const r = await api("/api/tts", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload) });
    const buf = await r.arrayBuffer();
    if (buf.byteLength <= 44) return toast("صدایی ساخته نشد.");
    try {
      const ctx = ensureAudio();
      if (ctx.state === "suspended") await ctx.resume();
      const decoded = await ctx.decodeAudioData(buf.slice(0));
      const src = ctx.createBufferSource();
      src.buffer = decoded;
      src.connect(ctx.destination);
      src.start();
    } catch (_) {
      const u = URL.createObjectURL(new Blob([buf], { type: "audio/wav" }));
      audio.src = u;
      await audio.play();
    }
  } catch (e) { toast(e.message || "پخش صدا انجام نشد."); }
  finally { if (btn) { btn.disabled = false; btn.textContent = prevLabel; } }
}
$("lexAdd").onclick = async () => {
  const w = stripWordPunct($("lexWord").value), p = $("lexSay").value.trim();
  if (!w || !p) return toast("کلمه و تلفظ را بنویسید.");
  await saveLexicon({ ...(S.settings.lexicon || {}), [w]: p });
  $("lexWord").value = ""; $("lexSay").value = "";
  toast("تلفظ «" + w + "» ذخیره شد.");
};
$("lexTest").onclick = async () => {
  const w = stripWordPunct($("lexWord").value), p = $("lexSay").value.trim();
  if (!w) return toast("کلمه را بنویسید.");
  await sayWord(w, p, $("lexTest"));
};
$("lexMarks").onclick = () => openPronunciation($("lexWord").value);
$("lexList").addEventListener("click", async e => {
  const say = e.target.closest("[data-say]"), del = e.target.closest("[data-del]");
  if (say) sayWord(say.dataset.say, (S.settings.lexicon || {})[say.dataset.say] || "", say);
  if (del) { const lex = { ...(S.settings.lexicon || {}) }; delete lex[del.dataset.del]; await saveLexicon(lex); }
});
if ($("lexExport")) $("lexExport").onclick = async () => {
  const lex = S.settings.lexicon || {};
  if (!Object.keys(lex).length) return toast("فهرست تلفظ‌ها خالی است.");
  if (isAndroidShell()) {
    try { await jpost("/api/lexicon_export", {}); } catch (e) { toast(e.message); }
    return;
  }
  const blob = new Blob([JSON.stringify({ lexicon: lex }, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url; a.download = "khana-lexicon.json"; a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
};
if ($("lexImport") && $("lexImportFile")) {
  $("lexImport").onclick = () => $("lexImportFile").click();
  $("lexImportFile").onchange = async () => {
    const file = $("lexImportFile").files[0];
    $("lexImportFile").value = "";
    if (!file) return;
    if (file.size > 512 * 1024) return toast("فایل فهرست خیلی بزرگ است.");
    try {
      const parsed = JSON.parse(await file.text());
      const raw = parsed && typeof parsed === "object" && !Array.isArray(parsed)
        ? (parsed.lexicon && typeof parsed.lexicon === "object" && !Array.isArray(parsed.lexicon) ? parsed.lexicon : parsed)
        : null;
      if (!raw) throw new Error("قالب فایل درست نیست.");
      const merged = { ...(S.settings.lexicon || {}) };
      let added = 0;
      for (const [k, v] of Object.entries(raw)) {
        if (k === "__proto__" || k === "constructor" || k === "prototype") continue;
        if (typeof k !== "string" || typeof v !== "string") continue;
        const wk = stripWordPunct(k), pv = v.trim();
        if (!wk || !pv || wk.length > 80 || pv.length > 80) continue;
        if (Object.keys(merged).length >= 5000 && !(wk in merged)) break;
        merged[wk] = pv;
        added++;
      }
      if (!added) return toast("کلمه‌ی معتبری در فایل پیدا نشد.");
      await saveLexicon(merged);
      toast(`${fa(added)} اصلاح تلفظ اضافه شد.`);
    } catch (_) { toast("این فایل خوانده نشد. یک فایل JSON درست انتخاب کنید."); }
  };
}
// select a word in the text -> a button offers to fix how it is read
document.addEventListener("selectionchange", () => {
  const t = stripWordPunct(getSelection().toString());
  const ok = t && t.length <= 35 && !/\s/.test(t) && $("textView").contains(getSelection().anchorNode);
  $("fixWord").hidden = !ok;
  if (ok) {
    // (while the text is being edited the button changes the word in the whole book; otherwise it fixes how the word is said)
    $("fixWord").textContent = S.editing ? `اصلاح «${t}»` : `اصلاح تلفظ «${t}»`;
    $("fixWord").dataset.word = t;
  }
});
$("fixWord").onclick = () => {
  $("fixWord").hidden = true;
  const word = $("fixWord").dataset.word;
  try { const sel = getSelection(); if (sel) sel.removeAllRanges(); } catch (e) {}
  if (S.editing) return openWordEditor(word);
  openPronunciation(word);
};

// Separate letter selection avoids opening the tablet keyboard just to add vowel marks,
// while #pronText also lets the user type Persian marks, simpler Persian, or Finglish (e.g. boresh).
const pronunciation = { word: "", letters: [], selected: -1, generation: 0, customText: "" };
function markedLetters(word) {
  const letters = [];
  for (const c of String(word).replace(/[\u200c\u200d\ufeffـ]/g, "")) {
    if (/\p{M}/u.test(c) && letters.length) letters[letters.length - 1] += c;
    else letters.push(c);
  }
  return letters;
}
function markLetter(letter, mark) {
  if (!mark) return letter.replace(/[\u064b-\u0655\u0670]/g, "");
  if (mark === "ّ") return letter.includes(mark) ? letter.replace(/ّ/g, "") : letter.replace(/ْ/g, "") + mark;
  return letter.replace(mark === "ْ" ? /[\u064b-\u0655\u0670]/g : /[\u064b-\u0650\u0652-\u0655\u0670]/g, "") + mark;
}
function currentPronSpelling() {
  const typed = $("pronText") ? $("pronText").value.trim() : "";
  return typed || pronunciation.letters.join("");
}
function renderPronunciation(syncInput = true) {
  const box = $("pronLetters"); box.replaceChildren();
  pronunciation.letters.forEach((letter, i) => {
    const selectable = /\p{Script=Arabic}/u.test(letter) && /\p{L}/u.test(letter);
    const b = document.createElement(selectable ? "button" : "span");
    b.textContent = letter;
    if (selectable) {
      b.type = "button"; b.dataset.letter = i;
      b.setAttribute("aria-label", "حرف " + letter);
      b.setAttribute("aria-pressed", String(i === pronunciation.selected));
    }
    box.appendChild(b);
  });
  const joined = pronunciation.letters.join("");
  if (syncInput && $("pronText")) $("pronText").value = joined;
  $("pronPreview").textContent = currentPronSpelling();
  $("pronMarks").querySelectorAll("button").forEach(b => b.disabled = pronunciation.selected < 0);
  $("pronSave").disabled = !currentPronSpelling();
}
function openPronunciation(word) {
  word = stripWordPunct(word);
  if (!word || word.length > 80) return toast("یک کلمه را انتخاب کنید.");
  try { const sel = getSelection(); if (sel) sel.removeAllRanges(); } catch (e) {}
  stopReading();
  pronunciation.generation++;
  pronunciation.word = word;
  const saved = (S.settings.lexicon || {})[word] || "";
  const initial = saved || word;
  pronunciation.letters = markedLetters(/\p{Script=Arabic}/u.test(initial) ? initial : word);
  pronunciation.selected = pronunciation.letters.findIndex(c => /\p{Script=Arabic}/u.test(c) && /\p{L}/u.test(c));
  $("pronTitle").textContent = "اصلاح تلفظ «" + word + "»";
  if ($("pronText")) $("pronText").value = initial;
  if ($("pronReset")) $("pronReset").hidden = !saved;
  renderPronunciation(false);
  openPanel("pronPanel");
  const first = $("pronLetters").querySelector('button[aria-pressed="true"]');
  if (first) first.focus({ preventScroll: true });
}
if ($("pronText")) {
  $("pronText").oninput = () => {
    const val = $("pronText").value.trim();
    if (/\p{Script=Arabic}/u.test(val)) {
      pronunciation.letters = markedLetters(val);
      if (pronunciation.selected >= pronunciation.letters.length) {
        pronunciation.selected = pronunciation.letters.findIndex(c => /\p{Script=Arabic}/u.test(c) && /\p{L}/u.test(c));
      }
      renderPronunciation(false);
    } else {
      $("pronPreview").textContent = val || pronunciation.word;
      $("pronSave").disabled = !val;
    }
  };
}
$("pronLetters").onclick = e => {
  const b = e.target.closest("button[data-letter]");
  if (!b) return;
  pronunciation.selected = Number(b.dataset.letter);
  renderPronunciation(false);
  $("pronLetters").querySelector(`button[data-letter="${pronunciation.selected}"]`).focus({ preventScroll: true });
};
$("pronMarks").onclick = e => {
  const b = e.target.closest("button[data-mark]");
  if (!b || pronunciation.selected < 0) return;
  const i = pronunciation.selected;
  pronunciation.letters[i] = markLetter(pronunciation.letters[i], b.dataset.mark);
  renderPronunciation(true);
};
if ($("pronReset")) $("pronReset").onclick = async () => {
  const word = pronunciation.word;
  if (!word) return;
  const lex = { ...(S.settings.lexicon || {}) };
  delete lex[word];
  await saveLexicon(lex);
  $("pronPanel").hidden = true;
  toast("اصلاح تلفظ «" + word + "» حذف شد.");
};
$("pronListen").onclick = () => sayWord(pronunciation.word, currentPronSpelling(), $("pronListen"));
$("pronSave").onclick = async () => {
  const word = pronunciation.word, spelling = currentPronSpelling();
  if (!word || !spelling) return toast("تلفظ درست را وارد کنید.");
  const generation = pronunciation.generation;
  $("pronSave").disabled = true;
  try {
    await saveLexicon({ ...(S.settings.lexicon || {}), [word]: spelling });
    if ($("lexWord") && stripWordPunct($("lexWord").value) === word) { $("lexWord").value = ""; $("lexSay").value = ""; }
    if (generation === pronunciation.generation) $("pronPanel").hidden = true;
    toast("تلفظ «" + word + "» ذخیره شد.");
  } catch (e) { toast(e.message); }
  finally { if (generation === pronunciation.generation) $("pronSave").disabled = !currentPronSpelling(); }
};

/* ---------------- panels ---------------- */
// A window over the page, usable without seeing it: the focus goes into it, Tab stays inside it, and when it closes the
// focus returns to the button that opened it (a screen reader would otherwise be left at the top of the page).
const panelOpener = new Map();
const panelStack = [], modalInert = new Map();
// Release decoded start-page artwork textures & stop all SVG animations whenever reading, typing, in any panel, or backgrounded.
function syncWelcomeMedia() {
  const suspend = !!S.doc || document.hidden || !!document.querySelector(".panel:not([hidden])") || document.body.classList.contains("locked");
  document.body.classList.toggle("art-suspended", suspend);
  const imgs = document.querySelectorAll(".living-village image");
  for (const img of imgs) {
    if (suspend) {
      const cur = img.getAttribute("href");
      if (cur) { img.dataset.href = cur; img.removeAttribute("href"); }
    } else if (!img.getAttribute("href") && img.dataset.href) {
      img.setAttribute("href", img.dataset.href);
    }
  }
}
document.addEventListener("visibilitychange", syncWelcomeMedia);
function syncModalState() {
  if (document.body.classList.contains("locked")) {
    panelStack.length = 0;
    modalInert.clear();
    for (const el of document.body.children) el.inert = el.id !== "lockScreen" && el.tagName !== "SCRIPT";
    syncWelcomeMedia();
    return;
  }
  const visible = [...document.querySelectorAll(".panel:not([hidden])")];
  for (const p of visible) if (!panelStack.includes(p)) panelStack.push(p);
  for (let i = panelStack.length - 1; i >= 0; i--) if (panelStack[i].hidden) panelStack.splice(i, 1);
  const top = panelStack[panelStack.length - 1];
  for (const el of document.body.children) {
    if (top) {
      if (!modalInert.has(el)) modalInert.set(el, el.inert);
      const live = el.id === "srLive" || el.getAttribute("role") === "status" || el.getAttribute("role") === "alert";
      el.inert = el !== top && !live && el.tagName !== "SCRIPT";
    } else if (modalInert.has(el)) {
      el.inert = modalInert.get(el);
    }
  }
  if (!top) modalInert.clear();
  panelStack.forEach((p, i) => p.style.zIndex = String(70 + i));
  syncWelcomeMedia();
}
function openPanel(id) {
  if (document.body.classList.contains("locked")) return;
  if ($(id).hidden) panelOpener.set(id, document.activeElement?.closest?.(".more-tools")?.querySelector("summary") || document.activeElement);
  $(id).hidden = false;
  const at = panelStack.indexOf($(id));
  if (at >= 0) panelStack.splice(at, 1);
  panelStack.push($(id));
  syncModalState();
  $(id).querySelector("button,select,input")?.focus();
}
// said by a screen reader, shown nowhere
function announce(msg) { const n = $("srLive"); n.textContent = ""; setTimeout(() => { n.textContent = msg; }, 50); }
document.querySelectorAll(".panel").forEach(p => {
  new MutationObserver(() => {
    syncModalState();
    if (!p.hidden) return;
    const back = panelOpener.get(p.id);
    panelOpener.delete(p.id);
    if (document.body.classList.contains("locked")) return;
    if (back && back.isConnected && !back.disabled && (document.activeElement === document.body || p.contains(document.activeElement))) back.focus({ preventScroll: true });
  }).observe(p, { attributes: true, attributeFilter: ["hidden"] });
  p.addEventListener("keydown", e => {
    if (e.key !== "Tab") return;
    const f = [...p.querySelectorAll("button, select, input, textarea, summary, [tabindex]")].filter(x => !x.disabled && x.tabIndex >= 0 && x.offsetParent !== null);
    if (!f.length) return;
    const first = f[0], last = f[f.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
  });
});
document.querySelectorAll(".panel").forEach(p => {
  p.addEventListener("click", e => { if (e.target === p || e.target.hasAttribute("data-close")) p.hidden = true; });
});
addEventListener("keydown", e => {
  if (e.key === "Escape" && !e.defaultPrevented) {
    stopRecording(false);
    const top = panelStack[panelStack.length - 1];
    if (top) { e.preventDefault(); top.hidden = true; }
  }
});
$("settingsBtn").onclick = () => openPanel("settings");
$("visionBtn").onclick = () => toggleLowVision();
$("aboutBtn").onclick = () => openPanel("aboutPanel");  // (1.29: Khana's logo opens «درباره‌ی کتاب‌خوان»)
$("lowVisionTop").onclick = () => toggleLowVision();
$("lowVisionSwitch").onchange = e => { if (e.target.checked !== !!S.settings.lowVision) toggleLowVision(); };
// (1.29) «نمایش درشت و پررنگ», the low-vision mode: one touch makes everything large and strong (yellow on black, large text and buttons, a large
// cursor, one sentence per line); a second touch brings back exactly what was set before
const LOW_VISION = { theme: "blackyellow", fontSize: 34, lineHeight: 2.6, uiScale: 1.3, cursor: "large", sentenceLines: true };
// (1.29: yellow on black, or white on black, as the reader chooses)
const lowVisionTheme = () => S.settings.lowVisionColor === "white" ? "black" : "blackyellow";
document.querySelectorAll("input[name=lvColor]").forEach(r => r.onchange = () => {
  S.settings.lowVisionColor = r.value;
  if (S.settings.lowVision) S.settings.theme = lowVisionTheme();
  setSetting("lowVisionColor", r.value);
});
function toggleLowVision() {
  const s = S.settings;
  if (!s.lowVision) {
    const prev = {};
    for (const k of Object.keys(LOW_VISION)) prev[k] = s[k];
    s.lowVisionPrev = prev;
    for (const [k, v] of Object.entries(LOW_VISION)) s[k] = k === "fontSize" ? Math.max(v, Number(s.fontSize) || 0) : v;
    s.theme = lowVisionTheme();
    s.lowVision = true;
    toast("نمایش درشت و پررنگ روشن شد. برای بازگشت به حالت عادی، روی همین پیام یا دکمه‌ی «Aa» در بالای صفحه بزنید.", () => {
      if (S.settings && S.settings.lowVision) toggleLowVision();
    });
  } else {
    const prev = s.lowVisionPrev && typeof s.lowVisionPrev === "object" ? s.lowVisionPrev : {};
    const normal = { theme: "modern", fontSize: 21, lineHeight: 2.2, uiScale: 1, cursor: "large", sentenceLines: true };
    for (const k of Object.keys(LOW_VISION)) s[k] = prev[k] !== undefined && prev[k] !== null ? prev[k] : normal[k];
    s.lowVision = false; s.lowVisionPrev = {};
    toast("نمایش معمولی برگشت.");
  }
  setSetting("lowVision", s.lowVision);
}
$("visionFontDown").onclick = () => setFontSize(Number(S.settings.fontSize) - 2);
$("visionFontUp").onclick = () => setFontSize(Number(S.settings.fontSize) + 2);
$("focusBtn").onclick = () => setSetting("focusMode", !S.settings.focusMode);
document.querySelectorAll('[data-ui-scale]').forEach(b => b.onclick = () => setSetting('uiScale', Number(b.dataset.uiScale)));
document.querySelectorAll('[data-vision-theme]').forEach(b => b.onclick = () => setSetting('theme', b.dataset.visionTheme));
$("moreTools").addEventListener('click', e => { const btn = e.target.closest('button'); if (btn && !btn.closest('.text-size')) $("moreTools").open = false; });
document.addEventListener('pointerdown', e => { if (!$("moreTools").contains(e.target)) $("moreTools").open = false; });
$("moreTools").addEventListener('keydown', e => {
  if (e.key === 'Escape' && $("moreTools").open) { e.preventDefault(); $("moreTools").open = false; $("moreTools").querySelector('summary').focus(); }
});
$("helpBtn").onclick = $("welcomeHelp").onclick = () => openPanel("helpPanel");
if ($("helpSettings")) $("helpSettings").onclick = () => { $("helpPanel").hidden = true; openPanel("settings"); };
$("openFileBtn").onclick = $("welcomeOpen").onclick = () => $("fileInput").click();
// a soft bell when the start page opens: three rising notes, made here (no sound file). Browsers may hold it back until the first tap.
function startChime() {
  try {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    const ctx = new AC();
    [523.25, 659.25, 783.99].forEach((f, i) => {
      const t = ctx.currentTime + i * 0.22;
      const osc = ctx.createOscillator(), gain = ctx.createGain();
      osc.type = "sine";
      osc.frequency.value = f;
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(0.18, t + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 1.1);
      osc.connect(gain).connect(ctx.destination);
      osc.start(t);
      osc.stop(t + 1.15);
    });
    setTimeout(() => ctx.close && ctx.close(), 2000);
  } catch (e) { /* no sound is fine */ }
}
// the welcome picture: a press shows or hides the quotation in the water
{
  const jump = $("jumpArt");
  if (jump) {
    // Use only the device's local clock to tint the still image. No time value leaves the device.
    const setSceneLight = () => {
      const hour = new Date().getHours();
      jump.dataset.light = hour >= 6 && hour < 9 ? "dawn" : hour >= 9 && hour < 17 ? "day" : hour >= 17 && hour < 20 ? "evening" : "night";
    };
    setSceneLight();
    setInterval(setSceneLight, 60_000);
    const toggleQuote = () => {
      const open = jump.classList.toggle("show-quote");
      jump.setAttribute("aria-expanded", String(open));
      if ($("waterQuote")) $("waterQuote").setAttribute("aria-hidden", String(!open));
    };
    jump.addEventListener("click", toggleQuote);
    jump.addEventListener("keydown", e => {
      if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleQuote(); }
    });
  }
}

/* ---------------- opening ---------------- */
// pictures (a photo of a page, a screenshot): read with the app's text recognition, one page per picture
const isImage = f => /\.(png|jpe?g|webp|gif|bmp|heic|tiff?)$/i.test(f.name) || /^image\//.test(f.type || "");
const MAX_IMAGE_PIXELS = 40_000_000;
const MAX_UPLOAD_BYTES = 300 * 1024 * 1024;
// Inspect PNG/GIF/JPEG/WebP header dimensions BEFORE decoding so a tiny decompression bomb cannot OOM the renderer.
async function checkImageHeaderPixels(file) {
  try {
    const head = new Uint8Array(await file.slice(0, 65536).arrayBuffer());
    const dv = new DataView(head.buffer);
    let w = 0, h = 0;
    if (head.length >= 24 && head[0] === 0x89 && head[1] === 0x50 && head[2] === 0x4E && head[3] === 0x47) {
      w = dv.getUint32(16, false); h = dv.getUint32(20, false); // PNG IHDR
    } else if (head.length >= 10 && head[0] === 0x47 && head[1] === 0x49 && head[2] === 0x46) {
      w = dv.getUint16(6, true); h = dv.getUint16(8, true); // GIF
    } else if (head.length >= 30 && head[0] === 0x52 && head[1] === 0x49 && head[2] === 0x46 && head[3] === 0x46 && head[8] === 0x57 && head[9] === 0x45 && head[10] === 0x42 && head[11] === 0x50) {
      const chunk = String.fromCharCode(head[12], head[13], head[14], head[15]);
      if (chunk === "VP8 " && head.length >= 30) { w = dv.getUint16(26, true) & 0x3FFF; h = dv.getUint16(28, true) & 0x3FFF; }
      else if (chunk === "VP8L" && head.length >= 25) { const b = dv.getUint32(21, true); w = (b & 0x3FFF) + 1; h = ((b >> 14) & 0x3FFF) + 1; }
      else if (chunk === "VP8X" && head.length >= 30) { w = 1 + (head[24] | (head[25] << 8) | (head[26] << 16)); h = 1 + (head[27] | (head[28] << 8) | (head[29] << 16)); }
    } else if (head.length >= 4 && head[0] === 0xFF && head[1] === 0xD8) {
      let off = 2;
      while (off + 8 < head.length) {
        if (head[off] !== 0xFF) break;
        const m = head[off + 1];
        if (m === 0xD8 || m === 0xD9 || m === 0x01 || (m >= 0xD0 && m <= 0xD7)) { off += 2; continue; }
        const segLen = dv.getUint16(off + 2, false);
        if (segLen < 2) break;
        if ((m >= 0xC0 && m <= 0xC3) || (m >= 0xC5 && m <= 0xC7) || (m >= 0xC9 && m <= 0xCB) || (m >= 0xCD && m <= 0xCF)) {
          h = dv.getUint16(off + 5, false); w = dv.getUint16(off + 7, false); break;
        }
        off += 2 + segLen;
      }
    }
    if (w > 0 && h > 0 && w * h > MAX_IMAGE_PIXELS) {
      throw new Error("ابعاد این تصویر بیش از حد بزرگ و ناامن است: " + file.name);
    }
  } catch (e) {
    if (e && /ناامن/.test(e.message)) throw e;
  }
}
async function openImages(files) {
  // (limits: the pictures are decoded in the page; a huge one is refused before anything is drawn)
  if (files.length > 50) throw new Error("حداکثر ۵۰ تصویر را می‌توان با هم باز کرد.");
  if (files.some(f => f.size > 80 * 1024 * 1024)) throw new Error("یکی از تصویرها خیلی بزرگ است (بیشتر از ۸۰ مگابایت).");
  const pages = [];
  for (let i = 0; i < files.length; i++) {
    note(`در حال خواندن تصویر ${fa(i + 1)} از ${fa(files.length)}…`);
    await checkImageHeaderPixels(files[i]);
    let bmp;
    try { bmp = await createImageBitmap(files[i]); } catch (e) { throw new Error("این تصویر باز نشد: " + files[i].name); }
    try {
      if (bmp.width * bmp.height > MAX_IMAGE_PIXELS) throw new Error("تصویر خیلی بزرگ است: " + files[i].name);
      // (about the size of a page scanned at 250 dpi: large pictures are made smaller, small ones larger)
      const long = Math.max(bmp.width, bmp.height), k = long > 3200 ? 3200 / long : long < 1600 ? Math.min(2, 2200 / long) : 1;
      const w = Math.max(1, Math.round(bmp.width * k)), h = Math.max(1, Math.round(bmp.height * k));
      const cv = document.createElement("canvas"); cv.width = w; cv.height = h;
      const cx = cv.getContext("2d", { willReadFrequently: true });
      cx.fillStyle = "#fff"; cx.fillRect(0, 0, w, h); cx.drawImage(bmp, 0, 0, w, h);
      const px = cx.getImageData(0, 0, w, h).data, body = new Uint8Array(8 + w * h), dv = new DataView(body.buffer);
      dv.setUint32(0, w, true); dv.setUint32(4, h, true);
      for (let j = 0, o = 8; j < px.length; j += 4, o++) body[o] = (px[j] * 77 + px[j + 1] * 150 + px[j + 2] * 29) >> 8;
      cv.width = 0; cv.height = 0;
      const r = await (await api("/api/ocr_image", { method: "POST", body: body.buffer })).json();
      if (r.error) throw new Error(r.error);
      pages.push(r.paragraphs || []);
    } finally {
      try { bmp.close(); } catch (_) {}
    }
  }
  if (!pages.some(p => p.length)) throw new Error("در این تصویر متنی پیدا نشد.");
  const name = files.length === 1 ? files[0].name : files[0].name.replace(/\.[^.]+$/, "") + " (+" + (files.length - 1) + ").png";
  return await (await api("/api/open_pages?name=" + encodeURIComponent(name), { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ pages }) })).json();
}
function setOpening(busy) {
  S.opening = busy;
  $("mainContent").setAttribute("aria-busy", String(busy));
  for (const id of ["openFileBtn", "welcomeOpen", "fileInput", "typeBtn", "typeBtn2"]) $(id).disabled = busy;
  const openLbl = $("openFileBtn").querySelector(".btn-label");
  if (openLbl) openLbl.textContent = busy ? "در حال باز کردن…" : "باز کردن";
  else $("openFileBtn").textContent = busy ? "در حال باز کردن…" : "باز کردن فایل";
  $("welcomeOpen").textContent = busy ? "در حال آماده کردن متن…" : "انتخاب متن برای خواندن";
}
async function verifyFileMagic(file) {
  if (file.size > MAX_UPLOAD_BYTES) throw new Error("فایل خیلی بزرگ است (بیشتر از ۳۰۰ مگابایت).");
  const head = new Uint8Array(await file.slice(0, 1024).arrayBuffer());
  if (/\.pdf$/i.test(file.name)) {
    const sig = String.fromCharCode(...head);
    if (!sig.includes("%PDF-")) throw new Error("این فایل ساختار معتبر PDF ندارد و به دلایل امنیتی باز نشد.");
  } else if (/\.(docx|epub|odt|pptx)$/i.test(file.name)) {
    if (head.length < 4 || head[0] !== 0x50 || head[1] !== 0x4B) {
      throw new Error("این فایل ساختار معتبر فشرده‌سازی ندارد و باز نشد.");
    }
  }
}
async function openFile(file, images) {
  if (!file) return;
  if (S.opening) return toast("لطفاً صبر کنید تا فایل باز شود.");
  setOpening(true);
  stopReading();
  clearPageImgCache();
  note(`در حال باز کردن «${file.name}»…`);
  try {
    if (!images) await verifyFileMagic(file);
    // (a PDF is read by pdf.js in its sandboxed frame; other files by the app)
    const d = /\.pdf$/i.test(file.name) ? await openWithPdfjs(file)
      : isImage(file) ? await openImages(images || [file])
      : await (await api("/api/open?name=" + encodeURIComponent(file.name), { method: "POST", body: await file.arrayBuffer() })).json();
    S.doc = d; S.page = 1; S.letters = null;
    S.view = "text";
    loadLengths();
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
  } catch (e) { note(e.message, true); }
  finally { setOpening(false); }
}
/* ---------------- pdf.js, in its own sandboxed frame (as GrapheneOS's PDF Viewer) ----------------
   The frame (pdfjs/frame.html) has an origin of its own: it cannot read this page, nor make the
   app's requests (it never has the token); its policy lets it load only pdf.js's own files; it has
   no permissions. It gets a PDF's bytes and answers with text positions and pictures, which are
   treated as untrusted (text is escaped when shown; the app checks what it is sent). */
let pdfFrame = null, pdfReady = null, pdfSeq = 0, pendingPdfOpenId = 0;
const pdfWait = new Map();
function pdfjsFrame() {
  if (pdfReady) return pdfReady;
  pdfFrame = document.createElement("iframe");
  pdfFrame.hidden = true;
  pdfFrame.setAttribute("sandbox", "allow-scripts");
  pdfFrame.setAttribute("allow", "");
  pdfFrame.setAttribute("referrerpolicy", "no-referrer");
  pdfFrame.src = location.host === "khana.invalid" ? "https://khana-pdf.invalid/pdfjs/frame.html" : "pdfjs/frame.html";
  pdfReady = new Promise((ok, no) => {
    const t = setTimeout(() => {
      // (a frame that never started is removed, so a retry starts clean and no stale frame stays in the page)
      pdfReady = null;
      if (pdfFrame) { pdfFrame.remove(); pdfFrame = null; }
      no(new Error("pdf.js شروع نشد."));
    }, 20000);
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
  if (!pdfFrame || e.source !== pdfFrame.contentWindow || e.origin !== "null") return;
  const m = e.data || {};
  if (m.ready) { if (pdfFrame.ready) pdfFrame.ready(); return; }
  if (m.needPassword) {
    if (!pendingPdfOpenId || m.id !== pendingPdfOpenId || document.body.classList.contains("locked")) return;
    askPassword(!!m.again).then(pw => {
      if (pdfFrame && pdfFrame.contentWindow) {
        pdfFrame.contentWindow.postMessage(pw == null ? { op: "password", cancel: true } : { op: "password", value: pw }, "*");
      }
    });
    return;
  }
  const w = pdfWait.get(m.id);
  if (!w) return;
  pdfWait.delete(m.id);
  if (m.id === pendingPdfOpenId) pendingPdfOpenId = 0;
  if (m.ok) w.ok(m.data); else w.no(new Error("pdf.js: " + String(m.error).slice(0, 200)));
});
// On the Mac pdf.js is not in a frame of this page but in a web view of its own (another process): the page asks the app,
// the app asks pdf.js. The answers are the same as the frame's.
async function pdfViewCall(msg) {
  let qs = "op=" + msg.op + (msg.n ? "&n=" + msg.n : "") + (msg.width ? "&width=" + Math.round(msg.width) : "") + (msg.dpi ? "&dpi=" + msg.dpi : "");
  let body = msg.op === "open" ? msg.data : undefined;
  for (;;) {
    const r = await api("/api/pdfview?" + qs, { method: "POST", body });
    const kind = r.headers.get("X-Khana-Pdf");
    if (kind === "password") {
      const pw = await askPassword(r.headers.get("X-Khana-Pdf-Again") === "1");
      qs = "op=password&id=" + r.headers.get("X-Khana-Pdf-Id") + (pw == null ? "&cancel=1" : "");
      body = pw == null ? undefined : pw;
      continue;
    }
    if (kind === "json") return r.json();
    if (kind === "png" || kind === "jpeg") return new Blob([await r.arrayBuffer()], { type: kind === "jpeg" ? "image/jpeg" : "image/png" });
    if (kind === "gray") {
      const buf = await r.arrayBuffer(), dv = new DataView(buf);
      return { w: dv.getUint32(0, true), h: dv.getUint32(4, true), px: new Uint8Array(buf, 8) };
    }
    throw new Error("pdf.js: " + (await r.text()).slice(0, 200));
  }
}
function resetPdfFrame() {
  for (const w of pdfWait.values()) w.no(new Error("پردازش PDF متوقف شد."));
  pdfWait.clear();
  pendingPdfOpenId = 0;
  pdfReady = null;
  if (pdfFrame) { try { pdfFrame.remove(); } catch (_) {} pdfFrame = null; }
}
async function pdfCall(msg, transfer) {
  if (window.KHANA_PDF_VIEW) return pdfViewCall(msg);
  await pdfjsFrame();
  return new Promise((ok, no) => {
    const id = ++pdfSeq;
    if (msg.op === "open") pendingPdfOpenId = id;
    const ms = msg.op === "open" ? 120000 : 45000;
    // If a hostile/corrupted PDF hangs pdf.js, tear down and recreate the isolated sandbox frame cleanly.
    const t = setTimeout(() => {
      if (pdfWait.delete(id)) {
        if (pendingPdfOpenId === id) pendingPdfOpenId = 0;
        resetPdfFrame();
        no(new Error("PDF باز نشد یا بیش از حد طول کشید (سند آسیب‌دیده یا ناامن)."));
      }
    }, ms);
    pdfWait.set(id, { ok: v => { clearTimeout(t); ok(v); }, no: e => { clearTimeout(t); no(e); } });
    pdfFrame.contentWindow.postMessage({ ...msg, id }, "*", transfer || []);
  });
}
async function openWithPdfjs(file) {
  if (file.size > 300 * 1024 * 1024) throw new Error("فایل خیلی بزرگ است (بیشتر از ۳۰۰ مگابایت).");
  if (window.resetPdfZoom) window.resetPdfZoom(); // Reset zoom when opening new PDF
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
    if (n === 1 || n % 10 === 0) note(`در حال خواندن «${file.name}»… صفحه‌ی ${fa(n)} از ${fa(pages)}`);
    const t = await pdfCall({ op: "text", n });
    out.push({ w: t.w, h: t.h, images: !!t.images, items: t.items });
  }
  const res = await api("/api/open_layout?name=" + encodeURIComponent(file.name), { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ pages: out }) });
  loadPdfBookmarks();
  return res.json();
}
// Load and display PDF bookmarks
async function loadPdfBookmarks() {
  if (!$("bookmarkBtn")) return;
  try {
    const { bookmarks } = await pdfCall({ op: "outline" });
    const list = $("bookmarkList");
    list.innerHTML = "";
    if (!bookmarks || bookmarks.length === 0) {
      list.innerHTML = "<p class='hint'>این فایل نشانه‌ای ندارد.</p>";
      return;
    }
    const ul = document.createElement("ul");
    ul.className = "outline-list";
    for (const bm of bookmarks) {
      const li = document.createElement("li");
      li.style.marginRight = (bm.level || 0) * 20 + "px";
      const btn = document.createElement("button");
      btn.type = "button";
      btn.textContent = bm.title + " (صفحه‌ی " + fa(bm.page) + ")";
      // (1.29 deep check: was showPage with a field that does not exist and an undefined closePanel, so no bookmark worked)
      btn.onclick = () => { $("bookmarkPanel").hidden = true; if (S.doc) loadPage(Math.min(Math.max(1, bm.page | 0), S.doc.pages)); };
      li.appendChild(btn);
      ul.appendChild(li);
    }
    list.appendChild(ul);
  } catch (e) {
    const list = $("bookmarkList");
    list.innerHTML = "<p class='hint'>خطا در خواندن نشانه‌ها: " + esc(String(e).slice(0, 100)) + "</p>";
  }
}
$("bookmarkBtn").onclick = async () => {
  if (!S.doc) return;
  openPanel("bookmarkPanel");
  await loadPdfBookmarks();
};
// a scanned page, drawn grey at 250 dpi by pdf.js, for the app's text recognition
const picturesGiven = new Map();
function givePicture(n, docId = S.doc && S.doc.id) {
  const key = docId + "|" + n;
  if (!picturesGiven.has(key)) {
    const p = (async () => {
      if (!S.doc || S.doc.id !== docId) throw new Error("سند تغییر کرده است.");
      const g = await pdfCall({ op: "gray", n, dpi: 250 });
      if (!S.doc || S.doc.id !== docId) throw new Error("سند تغییر کرده است.");
      const body = new Uint8Array(8 + g.px.length), dv = new DataView(body.buffer);
      dv.setUint32(0, g.w, true); dv.setUint32(4, g.h, true); body.set(g.px instanceof Uint8Array ? g.px : new Uint8Array(g.px.buffer || g.px), 8);
      await api(`/api/page_picture?doc=${docId}&n=${n}`, { method: "POST", body: body.buffer });
    })();
    p.catch(() => picturesGiven.delete(key));
    picturesGiven.set(key, p);
  }
  return picturesGiven.get(key);
}

function note(text, warn, action) {
  const n = $("note");
  n.replaceChildren(document.createTextNode(String(text ?? "")));
  if (action && action.label) {
    n.appendChild(document.createTextNode(" "));
    const b = document.createElement("button");
    b.type = "button";
    if (action.id) b.id = action.id;
    b.textContent = action.label;
    if (action.onClick) b.onclick = action.onClick;
    n.appendChild(b);
  }
  n.hidden = false;
  n.classList.toggle("warn", !!warn);
}
$("fileInput").onchange = e => {
  ensureAudio();
  const fs = [...e.target.files];
  // (several pictures together are the pages of one document)
  if (fs.length > 1 && fs.every(isImage)) openFile(fs[0], fs); else openFile(fs[0]);
  e.target.value = "";
};
let dragDepth = 0;
addEventListener("dragenter", e => { if (document.body.classList.contains("locked")) return; if ([...e.dataTransfer.types].includes("Files")) { dragDepth++; $("drop").hidden = false; } });
addEventListener("dragleave", () => { if (--dragDepth <= 0) { dragDepth = 0; $("drop").hidden = true; } });
addEventListener("dragover", e => e.preventDefault());
addEventListener("drop", e => {
  e.preventDefault(); dragDepth = 0; $("drop").hidden = true;
  if (document.body.classList.contains("locked")) return;
  ensureAudio();
  const fs = [...e.dataTransfer.files];
  if (fs.length > 1 && fs.every(isImage)) openFile(fs[0], fs); else openFile(fs[0]);
});

/* ---------------- pages ---------------- */
const pageCache = new Map();  // "page|rev|yeh|tr" -> Promise<paragraphs>
const translating = () => !!(S.doc && S.doc.canTranslate && S.settings.translate && !S.doc.isFinglish);
// A page request; a scanned page of a pdf.js document is first drawn by pdf.js and read with OCR.
async function pageReq(url, n, docId = S.doc && S.doc.id) {
  let r = await jget(url);
  if (r.needsPicture) { await givePicture(n, docId); r = await jget(url); }
  return r;
}
let sourceSeq = 0;
$("sourceBtn").onclick = async () => {
  if (!S.doc) return;
  const docId = S.doc.id, n = S.page, seq = ++sourceSeq;
  openPanel("sourcePanel");
  const box = $("sourceText");
  box.lang = S.doc.lang || "und";
  box.textContent = "در حال آماده کردن متن اصلی…";
  box.setAttribute("aria-busy", "true");
  try {
    const r = await pageReq(`/api/page?doc=${docId}&n=${n}&rev=${!!S.settings.reversed}&yeh=${!!S.settings.persianYeh}&tr=false`, n, docId);
    if (seq !== sourceSeq) return;
    box.textContent = S.doc && S.doc.id === docId && S.page === n
      ? (r.paragraphs.join("\n\n") || "این صفحه متن قابل استخراج ندارد.")
      : "سند یا صفحه تغییر کرده است؛ این پنجره را ببندید و دوباره باز کنید.";
  } catch (e) {
    if (seq === sourceSeq) box.textContent = "متن اصلی باز نشد: " + e.message;
  } finally {
    if (seq === sourceSeq) box.setAttribute("aria-busy", "false");
  }
};
const pageBoxesMap = new Map(); // "docId|page" -> { w, h, paras: [[[x0,top,x1,bot,rtl], ...], ...] }
function getParas(n) {
  const s = S.settings, tr = translating(), key = `${S.doc.id}|${n}|${!!s.reversed}|${!!s.persianYeh}|${tr}`;
  if (!pageCache.has(key)) {
    const docId = S.doc.id;
    const p = pageReq(`/api/page?doc=${docId}&n=${n}&rev=${!!s.reversed}&yeh=${!!s.persianYeh}&tr=${tr}`, n).then(r => {
      if (r && r.boxes) pageBoxesMap.set(`${docId}|${n}`, r.boxes);
      return r.paragraphs;
    });
    p.catch(() => pageCache.delete(key));
    pageCache.set(key, p);
  }
  return pageCache.get(key);
}
// A page being translated comes a few paragraphs at a time: {paragraphs, done}. A finished page
// goes into the cache like any other.
async function pageSoFar(n) {
  const s = S.settings, docId = S.doc.id, key = `${docId}|${n}|${!!s.reversed}|${!!s.persianYeh}|true`;
  if (pageCache.has(key)) return { paragraphs: await pageCache.get(key), done: true };
  const gen = editGen;
  const r = await pageReq(`/api/page?doc=${docId}&n=${n}&rev=${!!s.reversed}&yeh=${!!s.persianYeh}&tr=true&partial=1`, n);
  if (r && r.boxes) pageBoxesMap.set(`${docId}|${n}`, r.boxes);
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
    await sleep(600);
    if (seq !== S.loadSeq) return;
    const r = await pageSoFar(n);
    if (r && seq === S.loadSeq) {
      growPage(n, r.paragraphs, r.done);
      if (r.done) {
        if (!reader.active || !readingWaitText) updatePrepBanner("", -1, "", false);
      } else if (!reader.active) {
        const tot = r.total || 0;
        const pct = tot > 0 ? Math.min(99, Math.round((r.paragraphs.length / tot) * 100)) : -1;
        updatePrepBanner(tot > 0 ? `در حال ترجمه‌ی آفلاین ادامه‌ی صفحه… (${fa(r.paragraphs.length)} از ${fa(tot)} بند)` : "در حال ترجمه‌ی آفلاین ادامه‌ی صفحه…", pct, "بندهای اول آماده‌اند؛ می‌توانید همین الان خواندن را شروع کنید.", true);
      }
    }
  }
}

async function loadPage(n, keepReading) {
  if (!S.doc) return;
  n = Math.max(1, Math.min(S.doc.pages, n | 0));
  if (!keepReading) stopReading();
  // (a screen reader is told where it is; not while the app's own voice reads on into the next page)
  if (!keepReading && n !== S.page) announce(`صفحه‌ی ${fa(n)} از ${fa(S.doc.pages)}`);
  S.page = n;
  const seq = ++S.loadSeq;
  S.paras = [];
  S.pageDone = false;
  $("play").disabled = true;
  $("textView").replaceChildren();
  $("textView").setAttribute("aria-busy", "true");
  syncChrome();
  // the page's picture needs no text: drawn at once, not after a translation that may be waiting its turn
  if (S.view === "page") renderPageImage();
  // a scanned page is read (OCR) and a foreign one translated the first time: say so meanwhile
  const waitT = setTimeout(() => { if (seq === S.loadSeq) $("textView").innerHTML = `<p class="empty">${translating() ? "در حال ترجمه‌ی این صفحه به فارسی…" : "در حال آماده کردن این صفحه…"}</p>`; }, 250);
  try {
  if (translating()) {
    // shown (and readable) from its first translated paragraphs on; the rest follows
    updatePrepBanner("در حال بارگذاری مدل و ترجمه‌ی آفلاین صفحه…", -1, "اولین بار ممکن است چند ثانیه طول بکشد؛ به‌محض ترجمه‌ی بند اول، نمایش و خواندن شروع می‌شود.", true);
    let r;
    try {
      r = await pageSoFar(n);
      while (!r.done && !r.paragraphs.length && seq === S.loadSeq) {
        const tot = r.total || 0;
        const pct = tot > 0 ? Math.min(95, Math.round((r.paragraphs.length / tot) * 100)) : -1;
        updatePrepBanner(tot > 0 ? `در حال ترجمه‌ی آفلاین صفحه… (${fa(r.paragraphs.length)} از ${fa(tot)} بند)` : "در حال بارگذاری مدل و ترجمه‌ی آفلاین صفحه…", pct, "در حال ترجمه‌ی بند اول — به‌محض آماده شدن نمایش داده می‌شود.", true);
        await sleep(400);
        r = await pageSoFar(n);
      }
    } finally { clearTimeout(waitT); }
    if (seq !== S.loadSeq) return;
    S.paras = r.paragraphs; S.pageDone = r.done;
    renderText();
    if (r.done) updatePrepBanner("", -1, "", false);
    else {
      const tot = r.total || 0;
      const pct = tot > 0 ? Math.min(99, Math.round((r.paragraphs.length / tot) * 100)) : -1;
      updatePrepBanner(tot > 0 ? `در حال ترجمه‌ی آفلاین ادامه‌ی صفحه… (${fa(r.paragraphs.length)} از ${fa(tot)} بند)` : "در حال ترجمه‌ی آفلاین ادامه‌ی صفحه…", pct, "بندهای اول آماده‌اند؛ می‌توانید همین الان خواندن را شروع کنید.", true);
    }
    if (!r.done) followPage(n, seq).catch(e => {
      if (seq !== S.loadSeq) return;
      updatePrepBanner("", -1, "", false);
      stopReading();
      note("ترجمه کامل نشد: " + e.message, true, {
        id: "retryTranslation",
        label: "تلاش دوباره",
        onClick: () => { $("note").hidden = true; loadPage(n); },
      });
      announce("ترجمه کامل نشد. دکمه‌ی تلاش دوباره در دسترس است.");
    });
    return;
  }
  const paras = await getParas(n).finally(() => clearTimeout(waitT));
  if (seq !== S.loadSeq) return;
  updatePrepBanner("", -1, "", false);
  S.paras = paras; S.pageDone = true;
  renderText();
  } catch (e) {
    if (seq !== S.loadSeq) return;
    S.paras = []; S.pageDone = false;
    $("textView").textContent = "متن این صفحه آماده نشد: " + e.message;
    const retry = document.createElement("button");
    retry.type = "button"; retry.textContent = "تلاش دوباره";
    retry.onclick = () => loadPage(n);
    $("textView").append(retry);
    announce("متن این صفحه آماده نشد. می‌توانید دوباره تلاش کنید.");
  } finally {
    clearTimeout(waitT);
    if (seq === S.loadSeq) $("textView").setAttribute("aria-busy", "false");
  }
}
function showPage(n, paras, done = true) {  // switch the visible page while reading continues
  if (typeof reader !== "undefined" && reader.pendingTarget && reader.pendingTarget.page !== n && window.clearPendingStart) window.clearPendingStart();
  S.page = n; S.paras = paras; S.pageDone = done; S.loadSeq++;
  syncChrome(); renderText();
  if (S.view === "page") renderPageImage();
}
// every word is its own span (data-s = its offset in the paragraph) so reading can light it up
function highlightTok(tok, re, found) {
  if (!re) return esc(tok);
  re.lastIndex = 0;
  let out = "", last = 0, m;
  while ((m = re.exec(tok)) !== null) {
    if (m[0].length === 0) { re.lastIndex++; continue; }
    if (m.index > last) out += esc(tok.slice(last, m.index));
    found();
    out += `<mark>${esc(m[0])}</mark>`;
    last = m.index + m[0].length;
  }
  if (last < tok.length) out += esc(tok.slice(last));
  return out;
}
function paraHtml(p, i, re, found) {
  // Use the same sentence boundaries as read-aloud, keeping original UTF-16 word offsets.
  const lines = !!(S.settings && S.settings.sentenceLines);
  const segments = lines ? spokenSentences(p) : [{start: 0, text: p}];
  const h = segments.map((segment, index) => {
    const text = lines ? segment.text.replace(/\s+$/, "") : segment.text;
    const html = text.replace(/\S+|\s+/g, (tok, off) => {
      if (/^\s/.test(tok)) return tok;
      const w = highlightTok(tok, re, found);
      return `<span class="w" data-s="${segment.start + off}">${w}</span>`;
    });
    return (index ? "<br>" : "") + html;
  }).join("");
  return `<p dir="auto" data-i="${i}" tabindex="${i === 0 ? 0 : -1}" aria-keyshortcuts="Enter" title="کلیک یا Enter: خواندن از این بند">${h}</p>`;
}
const moreNote = () => S.pageDone === false ? '<p class="empty" id="trMore">در حال ترجمه‌ی ادامه‌ی این صفحه…</p>' : "";
// paragraphs from `from` on were added (a page still being translated): add them, keep the rest
function appendText(from) {
  $("play").disabled = !S.paras.length;
  const box = $("textView"), q = $("q").value.trim();
  if (q || box.querySelectorAll("p[data-i]").length !== from) return renderText();
  const html = S.paras.slice(from).map((p, k) => paraHtml(p, from + k, null, () => {})).join("");
  const more = $("trMore");
  if (more) more.insertAdjacentHTML("beforebegin", html); else box.insertAdjacentHTML("beforeend", html + moreNote());
}
function renderText(preservePosition = false) {
  $("play").disabled = !S.paras.length;
  const box = $("textView");
  box.lang = translating() ? "fa" : (S.doc && S.doc.lang || "fa");
  const focused = box.contains(document.activeElement) && document.activeElement.closest("p[data-i]");
  const focusedIndex = focused ? Number(focused.dataset.i) : null;
  const q = $("q").value.trim();
  let count = 0;
  if (!S.paras.length) {
    box.innerHTML = '<p class="empty">این صفحه متنی ندارد.</p>';
  } else {
    const re = q ? new RegExp(reEsc(q), "g") : null;
    box.innerHTML = S.paras.map((p, i) => paraHtml(p, i, re, () => count++)).join("") + moreNote();
  }
  $("count").textContent = q ? (count ? fa(count) + " مورد" : "پیدا نشد") : "";
  const first = box.querySelector("mark");
  if (first) { first.classList.add("cur"); if (!preservePosition) first.scrollIntoView({ block: "center" }); }
  reader.shownWord = null;
  armParagraphs();
  if (focusedIndex !== null) {
    const next = box.querySelector(`p[data-i="${focusedIndex}"]`);
    if (next) {
      if (!S.editing) box.querySelectorAll("p[data-i]").forEach(p => p.tabIndex = p === next ? 0 : -1);
      next.focus({ preventScroll: preservePosition });
    }
  }
}
const pdfBoxes = new Map();
function normBoxText(s) {
  let t = String(s || "");
  try { t = t.normalize("NFKC"); } catch (_) {}
  return t.toLowerCase()
    .replace(/[\u0640\u064B-\u065F\u0670\u200c\u200d\u200e\u200f]/g, "")
    .replace(/[يی]/g, "ی").replace(/[كک]/g, "ک").replace(/[أإآ]/g, "ا").replace(/[ۀة]/g, "ه")
    .replace(/[^\p{L}\p{N}\s]/gu, " ").replace(/\s+/g, " ").trim();
}
function revStr(s) { return [...s].reverse().join(""); }
function buildPageLines(w, h, items) {
  const clean = (items || []).filter(it => it && typeof it[0] === "string" && it[0].trim() && !/^[─_—\-]+$/.test(it[0].trim()));
  const lines = [];
  for (const it of clean) {
    const [str, x0, top, x1, bot, size, rtl] = it;
    const mid = (top + bot) * 0.5;
    let line = lines.find(l => Math.abs(l.mid - mid) <= Math.max(4, Math.min(l.size, size) * 0.55));
    if (!line) {
      line = { x0, top, x1, bot, mid, size, rtl: !!rtl, items: [] };
      lines.push(line);
    }
    line.x0 = Math.min(line.x0, x0);
    line.x1 = Math.max(line.x1, x1);
    line.top = Math.min(line.top, top);
    line.bot = Math.max(line.bot, bot);
    if (rtl) line.rtl = true;
    line.items.push({ str, x0, top, x1, bot, size });
  }
  lines.sort((a, b) => a.top - b.top);
  for (const l of lines) {
    // Sort PDF spans in natural reading order (right-to-left for Persian/Arabic, left-to-right for Latin)
    l.items.sort((a, b) => l.rtl ? (b.x1 - a.x1) : (a.x0 - b.x0));
    // Reconstruct accurate per-word boxes: merge tiny-gap glyph fragments & proportionally split multi-word spans
    const rawWords = [];
    for (const it of l.items) {
      const s = it.str;
      const parts = [];
      let m;
      const re = /\S+/g;
      while ((m = re.exec(s)) !== null) parts.push({ text: m[0], start: m.index, end: m.index + m[0].length });
      if (!parts.length) continue;
      const spanW = Math.max(1, it.x1 - it.x0);
      const totalLen = Math.max(1, s.length);
      for (const p of parts) {
        let wx0, wx1;
        if (parts.length === 1) {
          wx0 = it.x0; wx1 = it.x1;
        } else if (l.rtl) {
          wx1 = it.x1 - (p.start / totalLen) * spanW;
          wx0 = it.x1 - (p.end / totalLen) * spanW;
        } else {
          wx0 = it.x0 + (p.start / totalLen) * spanW;
          wx1 = it.x0 + (p.end / totalLen) * spanW;
        }
        rawWords.push({
          str: p.text, x0: Math.min(wx0, wx1), x1: Math.max(wx0, wx1),
          top: it.top, bot: it.bot, size: it.size || l.size,
          joinPrev: p.start === 0 && !/^\s/.test(s),
        });
      }
    }
    const words = [];
    for (const rw of rawWords) {
      const prev = words[words.length - 1];
      const gap = prev ? (l.rtl ? (prev.x0 - rw.x1) : (rw.x0 - prev.x1)) : 999;
      if (prev && rw.joinPrev && gap > -rw.size * 0.4 && gap < rw.size * 0.22) {
        prev.str += rw.str;
        prev.x0 = Math.min(prev.x0, rw.x0);
        prev.x1 = Math.max(prev.x1, rw.x1);
        prev.top = Math.min(prev.top, rw.top);
        prev.bot = Math.max(prev.bot, rw.bot);
      } else {
        words.push({ str: rw.str, x0: rw.x0, x1: rw.x1, top: rw.top, bot: rw.bot });
      }
    }
    for (const wd of words) { wd.norm = normBoxText(wd.str); wd.revNorm = wd.norm ? revStr(wd.norm) : ""; }
    l.words = words.filter(wd => wd.norm.length > 0);
    l.text = l.words.map(wd => wd.str).join(" ");
    l.norm = normBoxText(l.text);
  }
  return { w: Math.max(1, w), h: Math.max(1, h), lines: lines.filter(l => l.words.length > 0) };
}
function ensurePdfBoxes(n) {
  if (!S.doc || S.doc.engine !== "pdfjs") return null;
  const key = `${S.doc.id}|${n}`;
  const cached = pdfBoxes.get(key);
  if (cached && !cached.pending) return cached;
  if (!cached) {
    pdfBoxes.set(key, { pending: true });
    pdfCall({ op: "text", n }).then(t => {
      if (S.doc && `${S.doc.id}|${n}` === key && t) pdfBoxes.set(key, buildPageLines(t.w, t.h, t.items));
    }).catch(() => pdfBoxes.delete(key));
  }
  return null;
}
let lastPageHlKey = "";
let lastSentPlan = null;
let lastLineAnchor = { page: -1, para: -1, lineIdx: 0 };
window.clearPageLiveHighlight = function (resetAnchor = false) {
  lastPageHlKey = "";
  lastSentPlan = null;
  if (resetAnchor) lastLineAnchor = { page: -1, para: -1, lineIdx: 0 };
  const layer = $("pageHlLayer"), cap = $("pageLiveCaption");
  if (layer) layer.replaceChildren();
  if (cap) { cap.hidden = true; cap.replaceChildren(); }
};
function wordMatchesPdf(spokenNorm, wd) {
  const pdfNorm = wd && wd.norm, pdfRev = wd && wd.revNorm;
  if (!spokenNorm || !pdfNorm) return false;
  if (spokenNorm === pdfNorm || spokenNorm === pdfRev) return true;
  if (spokenNorm.length >= 3 && pdfNorm.length >= 3) {
    const ratio = Math.min(spokenNorm.length, pdfNorm.length) / Math.max(spokenNorm.length, pdfNorm.length);
    if (ratio >= 0.68 && (pdfNorm.startsWith(spokenNorm) || spokenNorm.startsWith(pdfNorm) || pdfRev.startsWith(spokenNorm) || spokenNorm.startsWith(pdfRev))) {
      return true;
    }
  }
  return false;
}
function pageCharBand(paraIdx, start, end) {
  let before = 0, total = 0;
  for (let i = 0; i < S.paras.length; i++) {
    const len = Math.max(12, (S.paras[i] || "").length);
    if (i < paraIdx) before += len;
    total += len;
  }
  total = Math.max(1, total);
  const pLen = Math.max(12, (S.paras[paraIdx] || "").length);
  const s0 = before / total;
  const s1 = (before + pLen) / total;
  const c0 = (before + Math.min(pLen, Math.max(0, start || 0))) / total;
  const c1 = (before + Math.min(pLen, Math.max(start || 0, end > start ? end : pLen))) / total;
  return { s0, s1, c0: Math.min(0.99, c0), c1: Math.min(1, Math.max(c0 + 0.01, c1)) };
}
function planSentenceOnPdf(box, page, paraIdx, start, end, sentText) {
  const lines = box.lines;
  const band = pageCharBand(paraIdx, start, end);
  const expectedLine = Math.min(lines.length - 1, Math.max(0, Math.floor(band.c0 * lines.length)));
  // Translated books: track strictly by character-weighted line band (never jumps between paragraphs!)
  if (translating()) {
    const startLine = Math.min(lines.length - 1, Math.max(0, Math.floor(band.c0 * lines.length)));
    const endLine = Math.min(lines.length - 1, Math.max(startLine, Math.floor(band.c1 * lines.length)));
    return { translated: true, lines: lines.slice(startLine, endLine + 1), startLine };
  }
  // Original language PDF: search ONLY within a bounded window around expectedLine / lastLineAnchor
  const rawWords = (sentText.match(/\S+/g) || []);
  const spokenNorms = rawWords.map(normBoxText);
  const meaningful = spokenNorms.filter(t => t.length >= 2);
  let bestStart = -1, bestSpan = 1, bestScore = -1, bestRatio = 0;
  const samePage = lastLineAnchor.page === page;
  const advancing = samePage && paraIdx >= lastLineAnchor.para;
  const center = advancing ? Math.round((lastLineAnchor.lineIdx + expectedLine) * 0.5) : expectedLine;
  const winLo = advancing ? Math.max(0, lastLineAnchor.lineIdx - 1) : Math.max(0, center - 8);
  const winHi = Math.min(lines.length - 1, Math.max(center + 10, winLo + 14));
  if (meaningful.length > 0) {
    for (let i = winLo; i <= winHi; i++) {
      for (let span = 1; span <= 3 && i + span <= lines.length; span++) {
        const flat = [];
        for (let k = i; k < i + span; k++) for (const wd of lines[k].words) flat.push(wd);
        let cur = 0, hits = 0;
        for (const t of meaningful) {
          for (let j = cur; j < flat.length; j++) {
            if (wordMatchesPdf(t, flat[j])) { hits++; cur = j + 1; break; }
          }
        }
        const ratio = hits / meaningful.length;
        if (ratio < 0.40) continue;
        const dist = Math.abs(i - expectedLine);
        const prox = Math.max(0, 1 - dist / Math.max(4, lines.length * 0.4));
        const sc = ratio * 1.5 - (span - 1) * 0.08 + 0.25 * prox;
        if (sc > bestScore) { bestScore = sc; bestStart = i; bestSpan = span; bestRatio = ratio; }
      }
    }
  }
  // If Persian PDF glyphs are non-extractable/custom-encoded (weak match), follow smooth monotonic line band
  if (bestStart < 0 || bestRatio < 0.40) {
    const startLine = advancing ? Math.min(lines.length - 1, Math.max(lastLineAnchor.lineIdx, expectedLine)) : expectedLine;
    const endLine = Math.min(lines.length - 1, Math.max(startLine, Math.floor(band.c1 * lines.length)));
    lastLineAnchor = { page, para: paraIdx, lineIdx: startLine };
    return { translated: true, lines: lines.slice(startLine, endLine + 1), startLine };
  }
  lastLineAnchor = { page, para: paraIdx, lineIdx: bestStart };
  const matchedLines = lines.slice(bestStart, bestStart + bestSpan);
  const flatPdf = [];
  for (let li = 0; li < matchedLines.length; li++) {
    for (const wd of matchedLines[li].words) flatPdf.push({ ...wd, line: matchedLines[li], lineIdx: li });
  }
  // Monotonic forward alignment from each spoken word index -> PDF word box
  const wordBoxes = new Array(spokenNorms.length).fill(null);
  let cursor = 0;
  for (let wi = 0; wi < spokenNorms.length; wi++) {
    const wn = spokenNorms[wi];
    if (!wn) continue;
    const lookLimit = Math.min(flatPdf.length, cursor + 6);
    for (let j = cursor; j < lookLimit; j++) {
      if (wordMatchesPdf(wn, flatPdf[j])) {
        wordBoxes[wi] = flatPdf[j];
        cursor = j + 1;
        break;
      }
    }
  }
  let lastBox = null;
  for (let wi = 0; wi < wordBoxes.length; wi++) {
    if (wordBoxes[wi]) { lastBox = wordBoxes[wi]; continue; }
    let nextBox = null;
    for (let k = wi + 1; k < wordBoxes.length; k++) { if (wordBoxes[k]) { nextBox = wordBoxes[k]; break; } }
    if (lastBox) wordBoxes[wi] = lastBox;
    else if (nextBox) wordBoxes[wi] = nextBox;
  }
  return { translated: false, lines: matchedLines, wordBoxes, flatPdf, startLine: bestStart };
}
window.updatePageLiveHighlight = function (page, paraIdx, start, end, wordText, fraction, wordIdx = -1, totalWords = 0) {
  const layer = $("pageHlLayer"), cap = $("pageLiveCaption"), img = $("pageImg");
  if (!layer || !cap) return;
  const srvBox = S.doc ? pageBoxesMap.get(`${S.doc.id}|${page}`) : null;
  const exactParaLines = srvBox && Array.isArray(srvBox.paras) ? srvBox.paras[paraIdx] : null;
  const hasParaText = !!(S.paras && S.paras[paraIdx]);
  if (S.view !== "page" || !S.doc || S.page !== page || paraIdx < 0 || (!hasParaText && !(exactParaLines && exactParaLines.length))) {
    window.clearPageLiveHighlight();
    return;
  }
  const paraText = (S.paras && S.paras[paraIdx]) || "";
  const sentText = (end > start ? paraText.slice(start, end) : paraText).trim();
  const rawWords = sentText.match(/\S+/g) || [];
  const effTotal = totalWords > 0 ? totalWords : rawWords.length;
  const effIdx = wordIdx >= 0 ? Math.min(Math.max(0, wordIdx), Math.max(0, rawWords.length - 1))
    : Math.min(Math.max(0, Math.floor((fraction || 0) * rawWords.length)), Math.max(0, rawWords.length - 1));
  const imgReady = !!(img && img.naturalWidth > 0 && img.dataset.page === String(page));
  const key = `${page}|${paraIdx}|${start}|${end}|${effIdx}|${imgReady ? 1 : 0}|${translating() ? Math.round((fraction || 0) * 40) : 0}`;
  if (key === lastPageHlKey && (layer.childElementCount > 0 || !imgReady)) return;
  lastPageHlKey = key;

  // 1) Sticky live caption pill (exact effIdx word — never shifts the PDF page image below!)
  if (sentText && rawWords.length) {
    cap.hidden = false;
    cap.replaceChildren();
    let wi = 0, lastPos = 0, m;
    const re = /\S+/g;
    while ((m = re.exec(sentText)) !== null) {
      if (m.index > lastPos) cap.append(document.createTextNode(sentText.slice(lastPos, m.index)));
      if (wi === effIdx) {
        const span = document.createElement("span");
        span.className = "page-cap-now";
        span.textContent = m[0];
        cap.append(span);
      } else {
        cap.append(document.createTextNode(m[0]));
      }
      lastPos = m.index + m[0].length;
      wi++;
    }
    if (lastPos < sentText.length) cap.append(document.createTextNode(sentText.slice(lastPos)));
  } else {
    cap.hidden = true;
  }

  // 2) Wait until the PDF page image is rendered so overlay boxes never land on a 0px unloaded stage
  layer.replaceChildren();
  if (!imgReady) return;
  const pct = (v, m) => Math.max(0, Math.min(100, (v / m) * 100)).toFixed(2) + "%";
  const addBox = (cls, x0, top, x1, bot, w, h) => {
    const d = document.createElement("div");
    d.className = cls;
    d.style.left = pct(x0 - 2, w);
    d.style.top = pct(top - 2, h);
    d.style.width = pct(Math.max(8, x1 - x0 + 4), w);
    d.style.height = pct(Math.max(10, bot - top + 4), h);
    layer.append(d);
    return d;
  };
  let focusEl = null;
  // PRIMARY EXACT PATH: backend-supplied exact 1-to-1 paragraph line boxes from src/text.rs (ZERO jumping across paragraphs!)
  if (exactParaLines && exactParaLines.length > 0) {
    const w = Math.max(1, srvBox.w), h = Math.max(1, srvBox.h);
    const paraLen = Math.max(1, paraText.length);
    const s0 = paraText.length ? Math.min(0.99, Math.max(0, (start || 0) / paraLen)) : 0;
    const s1 = paraText.length ? Math.min(1.0, Math.max(s0 + 0.01, (end > start ? end : paraLen) / paraLen)) : 1.0;
    const curFrac = Math.min(0.999, Math.max(0, s0 + (s1 - s0) * Math.min(0.99, Math.max(0, fraction || 0))));
    const widths = exactParaLines.map(ln => Math.max(10, ln[2] - ln[0]));
    const totalW = widths.reduce((a, b) => a + b, 0);
    let acc = 0, activeLine = exactParaLines[exactParaLines.length - 1], inLineFrac = 0;
    for (let i = 0; i < exactParaLines.length; i++) {
      const ln = exactParaLines[i];
      const f0 = acc / totalW, f1 = (acc + widths[i]) / totalW;
      acc += widths[i];
      if (f1 >= s0 - 0.01 && f0 <= s1 + 0.01) {
        addBox("page-hl-sent", ln[0], ln[1], ln[2], ln[3], w, h);
      }
      if (curFrac >= f0 && (curFrac <= f1 || i === exactParaLines.length - 1)) {
        activeLine = ln;
        inLineFrac = Math.min(0.96, Math.max(0, (curFrac - f0) / Math.max(0.001, f1 - f0)));
      }
    }
    const [lx0, ltop, lx1, lbot, lrtl] = activeLine;
    const lw = Math.max(12, lx1 - lx0);
    const segW = Math.min(lw, Math.max(18, lw * 0.14));
    const wx0 = lrtl ? (lx1 - segW - inLineFrac * (lw - segW)) : (lx0 + inLineFrac * (lw - segW));
    focusEl = addBox("page-hl-word", wx0, ltop, wx0 + segW, lbot, w, h);
  } else {
    const band = pageCharBand(paraIdx, start, end);
    const prog = Math.min(0.99, Math.max(0, band.c0 + (band.c1 - band.c0) * Math.min(0.99, Math.max(0, fraction || 0))));
    const box = ensurePdfBoxes(page);
    if (box && box.lines.length > 0) {
      const startLine = Math.min(box.lines.length - 1, Math.max(0, Math.floor(band.c0 * box.lines.length)));
      const endLine = Math.min(box.lines.length - 1, Math.max(startLine, Math.floor(band.c1 * box.lines.length)));
      const subLines = box.lines.slice(startLine, endLine + 1);
      const f = Math.min(0.99, Math.max(0, fraction || 0));
      const li = Math.min(subLines.length - 1, Math.max(0, Math.floor(f * subLines.length)));
      const line = subLines[li];
      const inLineFrac = Math.min(0.95, Math.max(0, f * subLines.length - li));
      addBox("page-hl-sent", line.x0, line.top, line.x1, line.bot, box.w, box.h);
      const segW = Math.max(18, (line.x1 - line.x0) * 0.15);
      const wx0 = line.rtl ? (line.x1 - segW - inLineFrac * (line.x1 - line.x0 - segW)) : (line.x0 + inLineFrac * (line.x1 - line.x0 - segW));
      focusEl = addBox("page-hl-word", wx0, line.top, wx0 + segW, line.bot, box.w, box.h);
    } else {
      const g = document.createElement("div");
      g.className = "page-hl-guide";
      g.style.top = (8 + prog * 82).toFixed(1) + "%";
      layer.append(g);
      focusEl = g;
    }
  }
  if (focusEl && !reader.paused) {
    const r = focusEl.getBoundingClientRect();
    const capBot = cap.hidden ? 0 : cap.getBoundingClientRect().bottom;
    const topBound = Math.max(document.querySelector(".bar").getBoundingClientRect().bottom, capBot) + 14;
    const dockTop = $("docTools").getBoundingClientRect().top - 14;
    if (dockTop > topBound + 40 && (r.top < topBound || r.bottom > dockTop)) {
      window.scrollBy(0, r.top - (topBound + Math.max(0, dockTop - topBound - r.height) * 0.35));
    }
  }
};
const pageImgCache = new Map(); // "docId|page|w" -> objectURL (LRU up to 8 pages)
const pageImgPending = new Map();
function clearPageImgCache() {
  for (const url of pageImgCache.values()) { try { URL.revokeObjectURL(url); } catch (_) {} }
  pageImgCache.clear();
  pageImgPending.clear();
  const img = $("pageImg");
  if (img) { img.removeAttribute("src"); img.dataset.page = ""; img.dataset.blob = ""; }
}
function fetchPageImgUrl(id, n, w) {
  const key = `${id}|${n}|${w}`;
  if (pageImgCache.has(key)) {
    const u = pageImgCache.get(key);
    pageImgCache.delete(key);
    pageImgCache.set(key, u);
    return Promise.resolve(u);
  }
  if (pageImgPending.has(key)) return pageImgPending.get(key);
  const p = pdfCall({ op: "render", n, width: w }).then(blob => {
    pageImgPending.delete(key);
    if (!(blob instanceof Blob)) throw new Error("تصویر صفحه ساخته نشد.");
    const u = URL.createObjectURL(blob);
    if (!S.doc || S.doc.id !== id) { URL.revokeObjectURL(u); return null; }
    pageImgCache.set(key, u);
    while (pageImgCache.size > 8) {
      const [oldK, oldU] = pageImgCache.entries().next().value;
      pageImgCache.delete(oldK);
      try { URL.revokeObjectURL(oldU); } catch (_) {}
    }
    return u;
  }).catch(e => { pageImgPending.delete(key); throw e; });
  pageImgPending.set(key, p);
  return p;
}
function renderPageImage() {
  const sheetW = Math.min(document.querySelector(".sheet").clientWidth || 720, 900);
  const dpr = Math.min(devicePixelRatio || 1, 1.5);
  const w = Math.max(480, Math.min(1350, Math.round(sheetW * dpr)));
  if (S.doc && S.doc.engine === "pdfjs") {
    const n = S.page, id = S.doc.id, img = $("pageImg");
    if (!pageBoxesMap.has(`${id}|${n}`)) ensurePdfBoxes(n);
    const key = `${id}|${n}|${w}`;
    const cached = pageImgCache.get(key);
    if (cached) {
      if (img.src !== cached) img.src = cached;
      img.dataset.page = String(n);
      img.classList.remove("stale");
      if (n < S.doc.pages && !pageImgCache.has(`${id}|${n + 1}|${w}`)) {
        fetchPageImgUrl(id, n + 1, w).catch(() => {});
      }
      return;
    }
    if (img.dataset.page !== String(n)) { img.classList.add("stale"); window.clearPageLiveHighlight(); }
    fetchPageImgUrl(id, n, w).then(u => {
      if (!u || !S.doc || S.doc.id !== id || S.page !== n) return;
      img.src = u;
      img.dataset.page = String(n);
      img.classList.remove("stale");
      // Background prefetch of the next page so page turns & live reading flip instantaneously!
      if (n < S.doc.pages && !pageImgCache.has(`${id}|${n + 1}|${w}`)) {
        fetchPageImgUrl(id, n + 1, w).catch(() => {});
      }
    }).catch(e => toast(e.message));
  }
}
function syncChrome() {
  const has = !!S.doc;
  $("welcome").hidden = has;
  $("skipContent").href = has && S.view === "text" ? "#textView" : "#mainContent";
  // (the book's tools and the player show only with a book open, the search only over its text: app.css)
  document.body.classList.toggle("has-doc", has);
  document.body.classList.toggle("view-text", has && S.view === "text");
  syncWelcomeMedia();
  $("docTools").hidden = !has; $("searchRow").hidden = !has || S.view !== "text";
  if (!has) return;
  const isTypedDoc = !!S.doc.isTyped;
  if ($("closeDocBtn")) {
    const closeLbl = isTypedDoc ? "پاک کردن متن و بازگشت" : "بستن متن و بازگشت به صفحه‌ی آغاز";
    $("closeDocBtn").title = closeLbl;
    $("closeDocBtn").setAttribute("aria-label", closeLbl);
  }
  if ($("closeTextLabel")) $("closeTextLabel").textContent = isTypedDoc ? "پاک کردن متن" : "بستن کتاب";
  $("pageInput").value = fa(S.page);
  $("pageCount").textContent = fa(S.doc.pages);
  $("quickPageNumber").textContent = fa(S.page);
  $("quickPage").setAttribute("aria-label", "صفحه " + fa(S.page) + " از " + fa(S.doc.pages) + "؛ انتخاب صفحه");
  $("fileName").title = S.doc.name || "";
  $("pageInput").setAttribute("aria-label", "شماره‌ی صفحه؛ از ۱ تا " + fa(S.doc.pages));
  showBookTime();
  $("prev").disabled = S.page <= 1; $("next").disabled = S.page >= S.doc.pages;
  $("first").disabled = S.page <= 1; $("last").disabled = S.page >= S.doc.pages;
  $("viewPage").disabled = !S.doc.hasImages;
  $("trBtn").hidden = !S.doc.canTranslate;
  $("sourceBtn").hidden = !S.doc.canTranslate;
  $("pdfBtn").hidden = !S.doc.canTranslate;
  $("trBtn").classList.toggle("on", translating());
  $("viewText").classList.toggle("on", S.view === "text");
  $("viewPage").classList.toggle("on", S.view === "page");
  $("viewText").setAttribute("aria-pressed", String(S.view === "text"));
  $("viewPage").setAttribute("aria-pressed", String(S.view === "page"));
  $("trBtn").setAttribute("aria-pressed", String(translating()));
  $("textView").hidden = S.view !== "text";
  $("pageView").hidden = S.view !== "page";
  $("readingTip").hidden = !S.doc.hasText || S.view !== "text";
  $("bookmarkBtn").hidden = S.view !== "page" || !S.doc.hasImages;
  $("zoomBtn").hidden = S.view !== "page" || !S.doc.hasImages;
}
$("first").onclick = () => { if (S.doc && S.page > 1) { loadPage(1); scrollTo(0, 0); } };
$("last").onclick = () => { if (S.doc && S.page < S.doc.pages) { loadPage(S.doc.pages); scrollTo(0, 0); } };
$("prev").onclick = () => { loadPage(S.page - 1); scrollTo(0, 0); };
$("next").onclick = () => { loadPage(S.page + 1); scrollTo(0, 0); };
// (the number is checked on Enter, on leaving the box, and on change: the digits are rewritten as Farsi while typing, and then the browser sends no "change" by itself)
const goTyped = () => { const n = numOf($("pageInput").value); if (n >= 1 && S.doc) loadPage(Math.min(n, S.doc.pages)); };
$("pageInput").onchange = goTyped;
$("pageInput").onblur = () => { if (numOf($("pageInput").value) !== S.page) goTyped(); };
$("pageInput").addEventListener("keydown", e => { if (e.key === "Enter") { e.preventDefault(); goTyped(); } });
$("viewText").onclick = () => { S.view = "text"; if ($("moreTools")) $("moreTools").open = false; if ($("zoomMenu")) $("zoomMenu").hidden = true; syncChrome(); };
$("trBtn").onclick = () => { setSetting("translate", !S.settings.translate); loadPage(S.page); };
$("viewPage").onclick = () => { S.view = "page"; if ($("moreTools")) $("moreTools").open = false; window.clearPageLiveHighlight(true); syncChrome(); renderPageImage(); };
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
  if (e.defaultPrevented || document.body.classList.contains("locked") || document.querySelector(".panel:not([hidden])")) return;
  const command = e.metaKey || e.ctrlKey;
  if (command && !e.altKey && !e.shiftKey && e.code === "KeyO") {
    e.preventDefault(); if (!S.opening) $("fileInput").click(); return;
  }
  if (command && !e.altKey && !e.shiftKey && e.code === "KeyF" && S.doc && S.view === "text") {
    e.preventDefault(); if (document.body.classList.contains("is-reading") && !document.body.classList.contains("reading-tools-open")) $("readingToolsBtn").click(); $("moreTools").open = true; $("q").focus(); $("q").select(); return;
  }
  if (e.key === "F1") { e.preventDefault(); openPanel("helpPanel"); return; }
  // A focused control keeps its own Space/arrow behavior; dialogs and editable text never start playback.
  if (!S.doc || command || e.altKey || e.target.closest("input,select,textarea,button,a,summary,[role=button]") || e.target.isContentEditable) return;
  if (e.key === "ArrowLeft") { e.preventDefault(); $("prev").click(); }
  if (e.key === "ArrowRight") { e.preventDefault(); $("next").click(); }
  if (e.key === " ") { e.preventDefault(); $("play").click(); }
});
let clipClearTimer = null, clipActive = false;
function clearBrowserClip() {
  if (clipClearTimer) { clearTimeout(clipClearTimer); clipClearTimer = null; }
  if (!clipActive) return;
  clipActive = false;
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText("").catch(() => {});
  }
}
$("copy").onclick = async () => {
  const text = S.paras.join("\n\n");
  if (!text) return toast("متنی برای کپی نیست.");
  try {
    await api("/api/clip", { method: "POST", body: text });
    toast("متن صفحه کپی شد. پس از یک دقیقه و نیم از حافظه‌ی کپی پاک می‌شود.");
    return;
  } catch (e) {}
  try {
    await navigator.clipboard.writeText(text);
    clipActive = true;
    if (clipClearTimer) clearTimeout(clipClearTimer);
    clipClearTimer = setTimeout(clearBrowserClip, 90000);
    toast("متن صفحه کپی شد. پس از یک دقیقه و نیم از حافظه‌ی کپی پاک می‌شود.");
  } catch (e) {
    toast("کپی نشد؛ متن را انتخاب و کپی کنید.");
  }
};

/* ---------------- reading aloud ----------------
   Speech is made sentence by sentence and played on the Web Audio clock: each sentence is
   scheduled to start exactly when the previous one ends (plus a natural pause), and the next
   sentences and the next page are prepared ahead, so there is no wait between paragraphs. */
const ttsCache = new Map();  // text -> Promise<blob URL> (voice test / dictionary)
async function ttsUrl(text, voiceId = S.settings.voice) {
  const key = voiceId + "|" + S.settings.speed + "|" + text;
  if (!ttsCache.has(key)) {
    const p = api("/api/tts", { method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text, voice: voiceId, speed: S.settings.speed }) })
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
// Android can prepare several sentences faster than real time when the requests overlap.
// Start them in reading order and bound the work; playback still awaits each piece in order.
const speechConcurrency = /Android/.test(navigator.userAgent) ? Math.max(1, Math.min(3, Math.floor((navigator.hardwareConcurrency || 2) / 2))) : 1;
const speechTasks = [];
let speechRunning = 0;
// (urgent: the sentences after a change of voice or speed go before everything already waiting)
function queueSpeech(make, urgent, key) {
  return new Promise((resolve, reject) => {
    speechTasks[urgent ? "unshift" : "push"]({ make, resolve, reject, key });
    pumpSpeech();
  });
}
// What makes a reading sentence sound different: a change of any of these makes every sentence prepared before useless.
const speechPrefix = () => S.settings.voice + "|" + S.settings.speed + "|" + (S.settings.naturalTone !== false) + "|" + (S.settings.commaPause || "short") + "|";
// A new voice or speed: sentences still waiting in the old one are dropped here, and the phone stops making the ones it has begun
// (each was a few seconds of work; a reading keeps up to 30 seconds ahead, so the new voice used to wait about half a minute).
// (counted from the clock, so a page opened again never sends a count lower than one the phone has seen)
let speechGen = Date.now(), speechGenPrefix = "";
const isDropped = e => /^(skipped|cancelled)$/.test(String(e && e.message));
function dropOldSpeech() {
  const now = speechPrefix();
  if (!speechGenPrefix) { speechGenPrefix = now; return; }
  if (now === speechGenPrefix) return;
  speechGenPrefix = now; speechGen = Math.max(speechGen + 1, Date.now());
  for (let k = speechTasks.length - 1; k >= 0; k--) {
    const t = speechTasks[k];
    if (t.key && !t.key.startsWith(now)) { speechTasks.splice(k, 1); bufCache.delete(t.key); t.reject(new Error("skipped")); }
  }
  jpost("/api/tts_gen", { gen: speechGen }).catch(() => {});
}
function pumpSpeech() {
  while (speechRunning < speechConcurrency && speechTasks.length) {
    const task = speechTasks.shift();
    speechRunning++;
    Promise.resolve().then(task.make).then(task.resolve, task.reject).finally(() => {
      speechRunning--;
      pumpSpeech();
    });
  }
}
function speechBuffer(text, run, urgent) {
  if (!speechGenPrefix) speechGenPrefix = speechPrefix();
  const key = speechPrefix() + text;
  if (!bufCache.has(key)) {
    // (gen: the phone drops this sentence when the voice or speed changes while it is being made)
    const body = JSON.stringify({ text, voice: S.settings.voice, speed: S.settings.speed, gen: speechGen });
    // a piece queued for reading that was stopped or jumped away from is skipped, so the new
    // position does not wait behind sentences nobody will hear
    const owner = { run };
    const p = queueSpeech(() => {
      if (owner.run !== undefined && reader.run !== owner.run && !owner.keep) throw new Error("skipped");
      return api("/api/tts", { method: "POST", headers: { "Content-Type": "application/json" }, body });
    }, urgent, key)
      // (a text with nothing to say comes back as a sound with no samples, which WebKit refuses with
      // «Decoding failed»: a moment's silence, and the reading goes on)
      .then(r => r.arrayBuffer()).then(b => b.byteLength <= 44 ? ensureAudio().createBuffer(1, 1, 22050) : ensureAudio().decodeAudioData(b));
    p.owner = owner;
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
function spokenSentences(text) {
  const out = [];
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (!/[.!?؟…]/.test(text[i])) continue;
    if (text[i] === ".") {
      if (/\p{N}/u.test(text[i - 1] || "") && /\p{N}/u.test(text[i + 1] || "")) continue;
      if (/[a-z]/i.test(text[i - 1] || "") && /[a-z]/i.test(text[i + 1] || "")) continue;
      const head = text.slice(start, i + 1);
      if (/(?:^|\s)(?:ص|ج|ش|ق|هـ)\.$/.test(head) || /^\s*[\p{N}]{1,2}\.$/u.test(head)) continue;
      if (/(?:^|\s)(?:Mr|Mrs|Ms|Dr|Prof|Sr|Jr|St|vs|e\.g|i\.e)\.$/i.test(head) || /(?:^|\s)(?:[A-Z]\.)+$/.test(head)) continue;
    }
    let end = i + 1;
    while (end < text.length && /[.!?؟…"'’”»)\]]/.test(text[end])) end++;
    while (end < text.length && /\s/.test(text[end])) end++;
    out.push({ start, text: text.slice(start, end) });
    start = end; i = end - 1;
  }
  if (start < text.length) out.push({ start, text: text.slice(start) });
  return out;
}
function piecesFor(page, paras, fromPara, fromOff = 0) {
  const out = [];
  let afterTitle = false;
  paras.forEach((p, i) => {
    if (i < fromPara) return;
    const title = isTitleLine(p);
    let first = true;
    for (const sentence of spokenSentences(p)) {
      const start = sentence.start, sent = sentence.text;
      const parts = sent.length > 180 ? sent.split(/(?<=[،؛,:;—–])\s+/) : [sent];
      let off = start;
      parts.forEach((part, k) => {
        const at = p.indexOf(part, off);
        if (/[\p{L}\p{N}]/u.test(part) && !(i === fromPara && at + part.length <= fromOff)) {
          const before = out.length ? out[out.length - 1].text : "";
          const asked = /[؟?!][»”"'’)\]]*$/.test(before);
          const trailed = /(?:\.\.\.|…)[»”"'’)\]]*$/.test(before);
          const pause = title ? 0.58 : afterTitle ? 0.48 : first ? 0.52 : k > 0 ? 0.14 : trailed ? 0.42 : asked ? 0.38 : 0.26;
          out.push({ page, p: i, start: at, end: at + part.length, text: part.trim(), pause: Math.max(0.05, pause * (({ short: 0.45, long: 1.45 })[S.settings.sentencePause] || 1)) });
          afterTitle = title;
        }
        first = false; off = at + part.length;
      });
    }
  });
  return out;
}

const reader = { active: false, paused: false, run: 0, queue: [], next: 0, playing: [], lastEnd: 0, lastPage: 0, sources: [], pendingTarget: null };
let readingWaitText = "";
let readingPauseReason = "";
window.setPauseReason = function (reason) {
  readingPauseReason = reason ? String(reason) : "";
  const el = $("readingState");
  if (el && reader.paused) {
    el.textContent = readingPauseReason || "خواندن مکث شده";
  }
};
window.clearPendingStart = function () {
  reader.pendingTarget = null;
  document.querySelectorAll(".text .w.pending-start").forEach(el => {
    el.classList.remove("pending-start", "pending-pulse");
    el.removeAttribute("aria-current");
  });
  document.querySelectorAll(".text p.pending-read").forEach(el => el.classList.remove("pending-read"));
};
window.markPendingStart = function (paraIdx, offset, wordEl, reason) {
  window.clearPendingStart();
  const p = document.querySelector(`.text p[data-i="${paraIdx}"]`);
  const w = (wordEl && wordEl.isConnected) ? wordEl
    : (p ? ([...p.querySelectorAll(".w")].find(el => +el.dataset.s >= offset) || p.querySelector(".w")) : null);
  if (p) p.classList.add("pending-read");
  if (w) {
    w.classList.add("pending-start");
    w.setAttribute("aria-current", "location");
  }
  reader.pendingTarget = { page: S.page, para: paraIdx, start: offset, started: false, at: Date.now() };
  if (window.setReadingWait) window.setReadingWait(reason || "در حال شروع خواندن از کلمه‌ی انتخاب‌شده…");
};
let prepStartAt = 0, prepTick = null;
function updatePrepBanner(reason, progress = -1, sub = "", forceShow = false) {
  const banner = $("prepBanner");
  if (!banner) return;
  const allow = !S.settings || S.settings.showWaitReason !== false;
  const visible = allow && !!reason && (forceShow || (reader.active && !reader.paused) || !!reader.pendingTarget);
  if (!visible) {
    banner.hidden = true;
    prepStartAt = 0;
    clearInterval(prepTick); prepTick = null;
    return;
  }
  if (!prepStartAt) {
    prepStartAt = Date.now();
    clearInterval(prepTick);
    prepTick = setInterval(() => {
      const t = $("prepTimer");
      if (!t || banner.hidden || !prepStartAt) return;
      const secs = Math.max(1, Math.floor((Date.now() - prepStartAt) / 1000));
      t.textContent = fa(secs) + " ثانیه";
    }, 1000);
  }
  banner.hidden = false;
  if ($("prepTitle") && $("prepTitle").textContent !== reason) $("prepTitle").textContent = reason;
  const fill = $("prepFill");
  if (fill) {
    const det = typeof progress === "number" && progress >= 0 && progress <= 100;
    fill.classList.toggle("indet", !det);
    fill.style.width = det ? Math.max(4, Math.round(progress)) + "%" : "38%";
  }
  const subEl = $("prepSub");
  if (subEl) {
    subEl.textContent = sub || (typeof progress === "number" && progress >= 0
      ? fa(Math.round(progress)) + "٪ آماده شد — خواندن به‌محض آماده شدن شروع می‌شود."
      : "همه‌چیز آفلاین روی دستگاه انجام می‌شود؛ به‌محض آماده شدن، خواندن شروع می‌شود.");
  }
  const secs = Math.floor((Date.now() - prepStartAt) / 1000);
  if ($("prepTimer")) $("prepTimer").textContent = secs >= 1 ? fa(secs) + " ثانیه" : "";
}
window.setPrepProgress = updatePrepBanner;
window.setReadingWait = function (reason, progress = -1, sub = "") {
  const show = !S.settings || S.settings.showWaitReason !== false;
  readingWaitText = show && reason ? String(reason) : "";
  const el = $("readingState");
  const waiting = !!readingWaitText && ((reader.active && !reader.paused) || !!reader.pendingTarget);
  document.body.classList.toggle("is-waiting", waiting);
  if (el) {
    el.setAttribute("aria-busy", String(waiting));
    const state = waiting ? readingWaitText : (reader.paused ? (readingPauseReason || "خواندن مکث شده") : reader.active ? "در حال خواندن" : "آماده‌ی خواندن");
    if (el.textContent !== state) el.textContent = state;
  }
  updatePrepBanner(waiting ? readingWaitText : "", progress, sub, false);
  if (window.syncTypeControls && $("typePanel") && !$("typePanel").hidden) {
    window.syncTypeControls(waiting ? readingWaitText : (reader.active ? (reader.paused ? "خواندن مکث شد." : "در حال خواندن متن شما…") : ""));
  }
};
function setPlayButton() {
  const wasReading = document.body.classList.contains("is-reading");
  const wasPaused = document.body.classList.contains("is-paused");
  if (reader.active && !reader.paused && (!wasReading || wasPaused)) $("moreTools").open = false;
  document.body.classList.toggle("is-paused", !!reader.active && !!reader.paused);
  document.body.classList.toggle("is-reading", !!reader.active);
  if (!reader.active) {
    if (!reader.pendingTarget) {
      readingWaitText = "";
      if (S.pageDone !== false) updatePrepBanner("", -1, "", false);
    }
    document.body.classList.remove("reading-tools-open");
    $("readingToolsBtn").setAttribute("aria-expanded", "false");
    $("readingToolsBtn").textContent = "صفحه و سرعت";
  }
  const isPlaying = !!(reader.active && !reader.paused);
  $("play").setAttribute("aria-pressed", String(isPlaying));
  $("play").setAttribute("aria-label", isPlaying ? "مکث خواندن" : reader.paused ? "ادامه‌ی خواندن" : "شروع خواندن");
  const playIc = $("play").querySelector(".play-ic"), pauseIc = $("play").querySelector(".pause-ic"), playLbl = $("playLabel");
  if (playIc && pauseIc && playLbl) {
    playIc.hidden = isPlaying;
    pauseIc.hidden = !isPlaying;
    playLbl.textContent = isPlaying ? "مکث" : (reader.paused ? "ادامه" : "خواندن");
  } else {
    $("play").textContent = isPlaying ? "❚❚ مکث" : (reader.paused ? "▶ ادامه" : "▶ خواندن");
  }
  $("stop").disabled = !reader.active;
  const waiting = !!readingWaitText && ((reader.active && !reader.paused) || !!reader.pendingTarget);
  document.body.classList.toggle("is-waiting", waiting);
  const state = waiting ? readingWaitText : (reader.paused ? (readingPauseReason || "خواندن مکث شده") : reader.active ? "در حال خواندن" : "آماده‌ی خواندن");
  if ($("readingState").textContent !== state) $("readingState").textContent = state;
  $("readingState").setAttribute("aria-busy", String(waiting));
  if (window.syncTypeControls) window.syncTypeControls(reader.active ? state : "");
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
async function readFrom(i, fromOff = 0, opts = {}) {
  stopReading(!!opts.keepPending);
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
  setReadingWait(opts.reason || "در حال ساختن صدای جمله‌ی اول…");
  requestAnimationFrame(() => tick(run));
  try {
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
      if (starved) { setReadingWait("در حال ترجمه‌ی آفلاین ادامه‌ی صفحه…"); await sleep(600); continue; }
      if (reader.next >= reader.queue.length) {
        // the last page has been read: say so, so the listener knows the book is finished
        if (!ended && reader.lastPage >= S.doc.pages && !S.doc.isTyped) {
          ended = true;
          reader.queue.push({ page: reader.lastPage, p: -1, start: 0, end: 0, text: "پایان متن.", pause: 1.2 });
          continue;
        }
        setReadingWait("");
        while (reader.run === run && actx.currentTime < reader.lastEnd) await sleep(200);
        break;
      }
      // stay well ahead of what is being heard, so a sentence the voice has to make again
      // (a retry takes a few seconds) never leaves a gap; prepare the following pieces too
      // (and nothing new is asked for while a change of voice waits for its first sentences: they go first)
      if (reader.lastEnd - actx.currentTime > 45 || reader.restarting) { await sleep(200); continue; }
      const piece = reader.queue[reader.next];
      if (reader.next === 0) setReadingWait(opts.reason || "در حال ساختن صدای جمله‌ی اول…");
      else if (reader.lastEnd <= actx.currentTime + 0.08) setReadingWait("در حال آماده‌سازی جمله‌ی بعدی برای ادامه‌ی روان… (پردازش آفلاین روی دستگاه)");
      const lowBuffer = reader.next < 3 || (reader.lastEnd - actx.currentTime) < 10;
      const bufP = speechBuffer(piece.text, run, lowBuffer);
      const lookahead = S.settings.smoothBuffer === "extra" ? 8 : 6;
      for (let k = 1; k <= lookahead; k++) if (reader.queue[reader.next + k]) speechBuffer(reader.queue[reader.next + k].text, run, reader.next + k < 4).catch(() => {});
      // On older laptops/PCs with "smoothBuffer: extra", pre-buffer the first 3 clauses before starting audio
      if (reader.next === 0 && S.settings.smoothBuffer === "extra" && !(S.doc && S.doc.isTyped)) {
        const target = Math.min(3, reader.queue.length);
        let ready = 0;
        const warms = [];
        for (let k = 0; k < target; k++) {
          warms.push(speechBuffer(reader.queue[k].text, run, true).then(() => {
            ready++;
            if (reader.run === run && ready < target) setReadingWait(`در حال آماده‌سازی برای خواندنِ بی‌وقفه… (${fa(ready)} از ${fa(target)} جمله)`, Math.round((ready / target) * 100));
          }).catch(() => {}));
        }
        await Promise.race([Promise.all(warms), sleep(12000)]);
        if (reader.run !== run) return;
      }
      let buf;
      // (a sentence dropped because the voice changed is asked for again, in the new voice)
      try { buf = await bufP; } catch (e) { if (reader.run === run && isDropped(e)) { await sleep(100); continue; } throw e; }
      if (reader.run !== run) return;
      setReadingWait("");
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
  if (reader.pendingTarget) window.clearPendingStart();
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
  let pos = (now - piece.t0) / (piece.t1 - piece.t0) * piece.total, curIdx = words.length - 1, cur = words[curIdx];
  for (let k = 0; k < words.length; k++) { pos -= piece.weights[k]; if (pos < 0) { curIdx = k; cur = words[k]; break; } }
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
    if (S.view === "text") {
      const r = cur.getBoundingClientRect(), top = $("textView").closest("main").getBoundingClientRect().top;
      if (r.top < innerHeight * 0.25 || r.bottom > innerHeight * 0.8 || r.top < top)
        cur.scrollIntoView({ block: "center", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
    }
  }
  if (S.view === "page" && window.updatePageLiveHighlight) {
    const frac = Math.min(0.999, Math.max(0, (now - piece.t0) / Math.max(0.01, piece.t1 - piece.t0)));
    window.updatePageLiveHighlight(piece.page, piece.p, piece.start, piece.end, cur ? cur.textContent : "", frac, curIdx, words.length);
  }
}
function clearReadingMarks() {
  document.querySelectorAll(".text .sent, .text .now").forEach(w => w.classList.remove("sent", "now"));
  document.querySelectorAll(".text p.reading").forEach(p => p.classList.remove("reading"));
  reader.shownWord = null; reader.shownPiece = null;
  if (window.clearPageLiveHighlight) window.clearPageLiveHighlight(true);
}
function stopReading(keepPending = false) {
  reader.run++; reader.active = false; reader.paused = false;
  reader.sources.forEach(s => { try { s.stop(); } catch (e) {} });
  reader.sources = []; reader.playing = [];
  if (actx && actx.state === "suspended") actx.resume();
  audio.pause();
  clearReadingMarks();
  if (!keepPending && window.clearPendingStart) window.clearPendingStart();
  setPlayButton();
}
$("play").onclick = () => {
  if (!S.doc || !S.paras.length) return toast("متنی برای خواندن نیست.");
  ensureAudio();
  if (reader.active && !reader.paused) { actx.suspend(); reader.paused = true; return setPlayButton(); }
  if (reader.paused) { reader.paused = false; actx.resume(); return setPlayButton(); }
  readFrom(0);
};
$("stop").onclick = () => stopReading(false);
let pendingClickT = null;
$("textView").addEventListener("click", e => {
  const p = e.target.closest("p[data-i]");
  // (editing: a tap on a paragraph opens its window; a selected word is left for the «change the word» button)
  if (S.editing) return;
  if (!p) return;
  if (e.detail > 1) {
    clearTimeout(pendingClickT);
    if (reader.pendingTarget && !reader.pendingTarget.started) window.clearPendingStart();
    return;
  }
  ensureAudio();
  // from the sentence clicked (the word's offset; between words, the paragraph's start)
  const w = e.target.closest(".w"), paraIdx = +p.dataset.i, off = w ? +w.dataset.s : 0;
  // If user taps the exact same word again while we are already preparing to read from it,
  // confirm visually instead of cancelling & restarting synthesis!
  const pt = reader.pendingTarget;
  if (pt && pt.page === S.page && pt.para === paraIdx && pt.start === off && pt.started && reader.active) {
    if (w) { w.classList.remove("pending-pulse"); void w.offsetWidth; w.classList.add("pending-pulse"); }
    if (window.setReadingWait) window.setReadingWait("انتخاب شد؛ در حال آماده‌سازی صدا از همین‌جا…");
    return;
  }
  const id = S.doc && S.doc.id, page = S.page, seq = S.loadSeq;
  const startReason = "در حال شروع خواندن از کلمه‌ی انتخاب‌شده…";
  window.markPendingStart(paraIdx, off, w, startReason);
  clearTimeout(pendingClickT);
  // Brief 220ms guard: a double-click selects a word (to fix its pronunciation) instead of reading
  pendingClickT = setTimeout(() => {
    if (S.doc && S.doc.id === id && S.page === page && S.loadSeq === seq && p.isConnected && !getSelection().toString()) {
      if (reader.pendingTarget) reader.pendingTarget.started = true;
      readFrom(paraIdx, off, { reason: startReason, keepPending: true });
    } else if (reader.pendingTarget && !reader.pendingTarget.started) {
      window.clearPendingStart();
    }
  }, 220);
});
$("textView").addEventListener("keydown", e => {
  const p = e.target.closest("p[data-i]");
  if (!p || S.editing || e.target.isContentEditable || e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return;
  const paragraphs = [...$("textView").querySelectorAll("p[data-i]")];
  const i = paragraphs.indexOf(p);
  const next = e.key === "ArrowDown" ? Math.min(i + 1, paragraphs.length - 1)
    : e.key === "ArrowUp" ? Math.max(i - 1, 0)
    : e.key === "Home" ? 0 : e.key === "End" ? paragraphs.length - 1 : -1;
  if (next >= 0) {
    e.preventDefault();
    paragraphs.forEach((el, at) => el.tabIndex = at === next ? 0 : -1);
    paragraphs[next].focus();
  } else if (e.key === "Enter") {
    e.preventDefault(); ensureAudio(); readFrom(+p.dataset.i);
  }
});

/* ---------------- MP3, and PDF of the translation (the same dialog) ---------------- */
/* ---------------- how long the book takes to listen to ----------------
   From the letters on each page (the core counts them, /api/lengths) and the voice's pace: about 9.6 letters a second at 100 %
   with the pauses between paragraphs (measured: a book page of 1,362 letters made 143 s of sound). A translated book is read in Farsi, about 0.8 times as many letters as the
   source (measured on a Dutch novel). Pages not read yet (scans) count as the average of the others. */
const LETTERS_PER_SECOND = 9.6, TRANSLATED_LETTERS = 0.8;
function durText(sec, exact) {
  if (!(sec > 0)) return "";
  if (sec < 50 && !exact) return "کمتر از یک دقیقه";
  const h = Math.floor(sec / 3600), m = Math.floor((sec % 3600) / 60), s = Math.round(sec % 60);
  if (exact) {
    if (h > 0) return fa(h) + " ساعت" + (m > 0 ? " و " + fa(m) + " دقیقه" : "");
    if (m > 0) return fa(m) + " دقیقه" + (s > 0 ? " و " + fa(s) + " ثانیه" : "");
    return fa(Math.max(1, s)) + " ثانیه";
  }
  const mr = Math.round((sec % 3600) / 60);
  return "حدود " + (h ? fa(h) + " ساعت" + (mr ? " و " + fa(mr) + " دقیقه" : "") : fa(Math.max(1, mr)) + " دقیقه");
}
async function loadLengths() {
  const d = S.doc;
  S.letters = null;
  try {
    const r = await jget(`/api/lengths?doc=${d.id}&rev=${!!S.settings.reversed}&yeh=${!!S.settings.persianYeh}`);
    if (S.doc === d) { S.letters = r.letters; showBookTime(); }
  } catch (e) { /* (no time shown) */ }
}
/** Seconds of listening for pages first..last at the reading speed set, or 0 when not known. */
function listenSeconds(first, last) {
  const L = S.letters;
  if (!L || !L.length) return 0;
  const known = L.filter(x => x >= 0), avg = known.length ? known.reduce((a, b) => a + b, 0) / known.length : 0;
  let n = 0;
  for (let p = Math.max(1, first); p <= Math.min(last, L.length); p++) n += L[p - 1] >= 0 ? L[p - 1] : avg;
  const letters = n * (translating() ? TRANSLATED_LETTERS : 1);
  return letters / (LETTERS_PER_SECOND * (Number(S.settings.speed) || 1));
}
function showBookTime() {
  const el = $("bookTime");
  if (!el) return;
  const left = S.doc ? listenSeconds(S.page, S.doc.pages) : 0, all = S.doc ? listenSeconds(1, S.doc.pages) : 0;
  el.hidden = !(left > 0);
  if (left > 0) {
    el.textContent = (S.page > 1 ? durText(left) + " شنیدن تا پایان کتاب" : "کل کتاب: " + durText(all) + " شنیدن") + " (با سرعت " + pct(S.settings.speed) + ")";
    el.title = "کل کتاب: " + durText(all) + " شنیدن";
  }
}
/** The save window's line: how long the sound file will be, for the pages chosen. */
function showSaveLength() {
  if (!S.doc || saveKind === "pdf") { $("mp3Len").textContent = ""; return; }
  const scope = document.querySelector("input[name=scope]:checked").value;
  let first = S.page, last = S.page;
  if (scope === "all") { first = 1; last = S.doc.pages; }
  if (scope === "range") { first = numOf($("from").value) || 1; last = numOf($("to").value) || first; }
  const sec = listenSeconds(first, last);
  $("mp3Len").textContent = sec > 0 ? "طول فایل صدا: " + durText(sec) : "";
}

let job = null, pollT;
var saveKind = "mp3";  // (var: applySettings may run before this line)
function openSave(kind) {
  if (!S.doc) { toast("اول یک فایل را باز کنید."); return; }
  if (!(job && job.status === "running")) {
    saveKind = kind;
    $("mp3Status").hidden = true;
    $("from").value = fa(S.page); $("to").value = fa(S.doc.pages);
    document.querySelector(`input[name=scope][value=${kind === "pdf" ? "all" : "page"}]`).checked = true;
  }
  const pdf = saveKind === "pdf";
  $("mp3Cancel").textContent = pdf ? "لغو" : "توقف و نگه‌داشتن بخش آماده";
  $("mp3Cancel").disabled = false;
  $("mp3Title").textContent = pdf ? "ذخیره‌ی ترجمه‌ی فارسی به صورت PDF" : "ذخیره به صورت فایل صدا";
  $("fmtField").hidden = pdf; $("fastField").hidden = pdf; $("endWordField").hidden = pdf;
  if ($("multiVoiceField")) $("multiVoiceField").hidden = pdf;
  syncBitrate();
  $("mp3Start").textContent = pdf ? "ساختن PDF" : "ساختن فایل صدا";
  applySettings();
  showSaveLength();
  if (pdf) $("mp3Voice").textContent = "هر صفحه روی همین دستگاه به فارسی ترجمه می‌شود (برای هر صفحه چند ثانیه تا یک دقیقه) و فایل PDF در پوشه‌ی Downloads ذخیره می‌شود.";
  openPanel("mp3");
}
// (the quality is asked for next to the format, and only when the format is MP3)
function syncBitrate() { $("bitrateField").hidden = saveKind === "pdf" || document.querySelector("input[name=afmt]:checked").value !== "mp3"; }
document.querySelectorAll("input[name=afmt]").forEach(r => r.addEventListener("change", syncBitrate));
$("mp3Btn").onclick = () => openSave("mp3");
$("mp3TopBtn").onclick = () => $("mp3Btn").click();
$("pdfBtn").onclick = () => openSave("pdf");
for (const id of ["pageInput", "from", "to"]) $(id).addEventListener("input", e => { const t = fa(latinDigits(e.target.value).replace(/\D/g, "")); if (t !== e.target.value) e.target.value = t; });
$("from").oninput = $("to").oninput = () => { document.querySelector('input[name=scope][value=range]').checked = true; showSaveLength(); };
document.querySelectorAll("input[name=scope]").forEach(r => r.addEventListener("change", showSaveLength));
$("mp3Start").onclick = async () => {
  if (reader.active) stopReading(false);
  const scope = document.querySelector("input[name=scope]:checked").value;
  let first = S.page, last = S.page;
  if (scope === "all") { first = 1; last = S.doc.pages; }
  if (scope === "range") { first = numOf($("from").value); last = numOf($("to").value); }
  // (1.29 deep check: an empty or reversed range is put right instead of being sent as it is)
  if (!(first >= 1)) first = 1;
  if (!(last >= 1)) last = first;
  if (last < first) [first, last] = [last, first];
  first = Math.min(first, S.doc.pages); last = Math.min(last, S.doc.pages);
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
      : await jpost("/api/mp3", { doc: S.doc.id, first, last, voice: S.settings.voice,
      voices: ($("multiVoiceExport") && $("multiVoiceExport").checked) ? exportVoiceIds() : [],
      speed: S.settings.speed,
      format: document.querySelector("input[name=afmt]:checked").value, fast: $("fastExport").checked, endWord: $("endWordExport").checked,
      bitrate: S.settings.bitrate, reversed: S.settings.reversed, persianYeh: S.settings.persianYeh, translate: translating() });
    $("mp3Status").hidden = false; $("mp3Reveal").hidden = true;
    $("mp3Pct").textContent = fa(0) + "%"; $("mp3Eta").textContent = "";
    $("mp3Start").disabled = true; $("mp3Cancel").hidden = false;
    poll();
  } catch (e) { toast(e.message); }
};
$("mp3Reveal").onclick = () => {
  // (where sound files stay inside the app: its own list is shown)
  if (S.library && saveKind !== "pdf") { $("mp3").hidden = true; openPanel("libPanel"); return libShow(); }
  if (job) jpost("/api/reveal?id=" + job.id, {}).catch(e => toast(e.message));
};
$("mp3Cancel").onclick =() => { if (job) jpost("/api/cancel?id=" + job.id, {}).catch(() => {}); };
// how long is left, from how fast the work has gone so far: «حدود ۵ دقیقه مانده · حدود ساعت ۲۱:۴۵ آماده است»
let jobClock = { id: null, t0: 0 };
function timeLeft(progress) {
  const spent = (Date.now() - jobClock.t0) / 1000;
  if (!(progress >= 0.02) || spent < 8) return "در حال برآورد زمان…";
  const left = spent * (1 - progress) / progress;
  const ready = new Date(Date.now() + left * 1000);
  const clock = fa(ready.getHours()) + ":" + fa(String(ready.getMinutes()).padStart(2, "0"));
  const h = Math.floor(left / 3600), m = Math.round((left % 3600) / 60);
  const span = left < 50 ? "کمتر از یک دقیقه" : h ? `حدود ${fa(h)} ساعت${m ? " و " + fa(m) + " دقیقه" : ""}` : `حدود ${fa(Math.max(1, m))} دقیقه`;
  return `${span} مانده · حدود ساعت ${clock} آماده است`;
}
async function poll() {
  clearTimeout(pollT);
  try { job = await jget("/api/job?id=" + job.id); } catch (e) { toast(e.message); return; }
  if (jobClock.id !== job.id) jobClock = { id: job.id, t0: Date.now() };
  const pct = Math.round(job.progress * 100);
  $("mp3Bar").style.width = pct + "%";
  $("mp3Prog").setAttribute("aria-valuenow", pct);
  const len = durText(job.seconds, true);
  const pdf = saveKind === "pdf";
  if (job.status === "running") {
    // (the sign at the number's right, close to it: «۳۵%», whatever the direction of the line around it)
    $("mp3Pct").textContent = fa(pct) + "%";
    $("mp3Eta").textContent = job.lowBattery ? "" : timeLeft(job.progress);
    $("mp3Text").classList.toggle("hot", job.heat >= 2 || !!job.lowBattery);
    $("mp3Text").textContent = job.lowBattery ? "باتری کم است (۲۰٪ یا کمتر). کار متوقف شده است و با وصل کردن شارژر خودش ادامه پیدا می‌کند." : job.heat >= 2 ? (job.heat >= 3 ? "دستگاه خیلی داغ شده است. کار تا خنک شدن آن متوقف شده است و بعد خودش ادامه پیدا می‌کند." : "دستگاه داغ شده است. برای سلامت آن، کار آهسته‌تر پیش می‌رود.") : pdf ? "در حال ترجمه…" : (pct === 0 ? "در حال آماده کردن نخستین بخش… (ممکن است چند ثانیه تا چند دقیقه طول بکشد)" : `در حال ساختن… تا اینجا ${len} صدا`);
    pollT = setTimeout(poll, 700); return;
  }
  $("mp3Start").disabled = false; $("mp3Cancel").hidden = true;
  $("mp3Eta").textContent = "";
  $("mp3Text").classList.remove("hot");
  $("mp3Pct").textContent = job.status === "done" ? fa(100) + "%" : "";
  if (job.status === "done") {
    $("mp3Text").textContent = (S.library && !pdf ? `فایل ساخته شد و در «فایل‌های من» است: «${job.saved}»` : `فایل در پوشه‌ی دانلودها ذخیره شد: «${job.saved}»`) + (pdf ? "" : ` · ${len}`);
    $("mp3Reveal").hidden = false;
  } else if (job.status === "cancelled") {
    $("mp3Text").textContent = job.saved ? `متوقف شد. صدای آماده‌شده به‌صورت فایل ناقص نگه داشته شد: «${job.saved}» · ${len}` : "متوقف شد؛ هنوز بخشی از صدا آماده نشده بود.";
    $("mp3Reveal").hidden = !job.saved;
  }
  else $("mp3Text").textContent = "ساخته نشد: " + job.error;
}

/* ---------------- «فایل‌های من»: the sound files kept inside the app (a phone), played here ---------------- */
let libUrl = null, libT = 0, libSeekBusy = false, libDurationMs = 0, libPaused = false;
const isAndroidShell = () => document.documentElement.classList.contains("android-reader");
function libClock(ms) {
  const s = Math.max(0, Math.floor((ms || 0) / 1000));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60;
  const mm = String(m).padStart(2, "0"), ss = String(sec).padStart(2, "0");
  return (h > 0 ? String(h).padStart(2, "0") + ":" : "") + mm + ":" + ss;
}
function libStop(keepNative = false) {
  clearTimeout(libT); libT = 0;
  const au = $("libAudio");
  au.pause(); au.removeAttribute("src"); au.load();
  if (libUrl) { URL.revokeObjectURL(libUrl); libUrl = null; }
  if (!keepNative && isAndroidShell()) {
    jpost("/api/lib_play_stop", {}).catch(() => {});
  }
  if (!keepNative) $("libPlayer").hidden = true;
}
async function libPoll() {
  clearTimeout(libT);
  if ($("libPanel").hidden || !isAndroidShell()) return;
  try {
    const st = await jget("/api/lib_play_status");
    if (!st.active) {
      $("libPlayer").hidden = true;
      if (st.error) toast(st.error);
      return;
    }
    libPaused = !!st.paused;
    libDurationMs = st.durationMs || 0;
    $("libNow").textContent = st.name || "";
    $("libAudio").hidden = true;
    $("libNative").hidden = false;
    $("libPlayer").hidden = false;
    $("libPlayPause").textContent = libPaused ? "ادامه" : "مکث";
    if (!libSeekBusy) {
      $("libSeek").value = libDurationMs > 0 ? Math.round((st.positionMs * 1000) / libDurationMs) : 0;
    }
    $("libTime").textContent = `${libClock(st.positionMs)} / ${libClock(libDurationMs)}`;
    libT = setTimeout(libPoll, 500);
  } catch (e) { }
}
if ($("libPlayPause")) $("libPlayPause").onclick = () => {
  jpost("/api/lib_play_pause", { paused: !libPaused }).then(libPoll).catch(e => toast(e.message));
};
if ($("libStopBtn")) $("libStopBtn").onclick = () => libStop(false);
if ($("libSeek")) {
  $("libSeek").oninput = () => {
    libSeekBusy = true;
    const ms = libDurationMs > 0 ? Math.round((Number($("libSeek").value) / 1000) * libDurationMs) : 0;
    $("libTime").textContent = `${libClock(ms)} / ${libClock(libDurationMs)}`;
  };
  $("libSeek").onchange = () => {
    const ms = libDurationMs > 0 ? Math.round((Number($("libSeek").value) / 1000) * libDurationMs) : 0;
    libSeekBusy = false;
    jpost("/api/lib_play_seek", { positionMs: ms }).then(libPoll).catch(e => toast(e.message));
  };
}
async function libShow() {
  let r;
  try { r = await jget("/api/library"); } catch (e) { return toast(e.message); }
  if (isAndroidShell()) libPoll();
  const list = $("libList");
  list.textContent = "";
  $("libEmpty").hidden = r.files.length > 0;
  // (the only thing Khana keeps that grows with use: the saved sound files. Their total is shown, and above 1 GB a reminder to
  // delete the ones already heard; nothing is deleted on its own, they are the reader's files)
  const total = r.files.reduce((n, f) => n + (f.bytes || 0), 0), mb = total / 1048576;
  $("libTotal").hidden = !r.files.length;
  $("libTotal").textContent = fa(r.files.length) + " فایل، روی هم " + (mb >= 1024 ? fa((mb / 1024).toFixed(1)) + " گیگابایت" : fa(Math.max(1, Math.round(mb))) + " مگابایت") + " از حافظه‌ی دستگاه."
    + (mb >= 1024 ? " فایل‌هایی را که دیگر لازم ندارید حذف کنید تا گوشی جا داشته باشد." : "");
  for (const f of r.files) {
    const row = document.createElement("div"); row.className = "librow";
    const info = document.createElement("div"); info.className = "lib-info";
    const name = document.createElement("bdi"); name.className = "name"; name.dir = "auto"; name.textContent = f.name;
    const acts = document.createElement("div"); acts.className = "acts";
    const size = document.createElement("span"); size.className = "size";
    size.textContent = (f.seconds > 0 ? durText(f.seconds, true) + " · " : "") + (f.bytes >= 1048576 ? fa((f.bytes / 1048576).toFixed(1)) + " مگابایت" : fa(Math.max(1, Math.round(f.bytes / 1024))) + " کیلوبایت");
    info.append(name, size);
    const q = "name=" + encodeURIComponent(f.name);
    const play = document.createElement("button"); play.type = "button"; play.className = "primary"; play.textContent = "▶ پخش";
    play.onclick = async () => {
      try {
        stopReading(); libStop(true);
        play.disabled = true;
        if (isAndroidShell()) {
          await jpost("/api/lib_play_start", { name: f.name });
          $("libNow").textContent = f.name;
          $("libAudio").hidden = true;
          $("libNative").hidden = false;
          $("libPlayer").hidden = false;
          libPoll();
        } else {
          $("libNative").hidden = true;
          $("libAudio").hidden = false;
          const blob = await (await api("/api/library_file?" + q)).blob();
          libUrl = URL.createObjectURL(blob);
          $("libNow").textContent = f.name;
          $("libPlayer").hidden = false;
          $("libAudio").src = libUrl;
          await $("libAudio").play();
        }
      } catch (e) { toast(e.message); } finally { play.disabled = false; }
    };
    const out = document.createElement("button"); out.type = "button"; out.textContent = "ذخیره در دستگاه";
    out.onclick = () => jpost("/api/library_export?" + q, {}).catch(e => toast(e.message));
    const del = document.createElement("button"); del.type = "button"; del.textContent = "حذف";
    let sure = false;
    del.onclick = async () => {
      // (asked by a second tap, not by a question window)
      if (!sure) { sure = true; del.textContent = "تأیید حذف"; setTimeout(() => { sure = false; del.textContent = "حذف"; }, 4000); return; }
      try { if ($("libNow").textContent === f.name) libStop(false); await jpost("/api/library_delete?" + q, {}); toast("حذف شد."); libShow(); } catch (e) { toast(e.message); }
    };
    acts.append(play, out, del);
    row.append(info, acts);
    list.append(row);
  }
}
$("libBtn").onclick = () => { openPanel("libPanel"); libShow(); };
new MutationObserver(() => {
  if ($("libPanel").hidden) {
    if (isAndroidShell()) clearTimeout(libT);
    else libStop(false);
  }
}).observe($("libPanel"), { attributes: true, attributeFilter: ["hidden"] });

/* ---------------- misc ---------------- */
let toastT;
// (1.29) a message stays long enough to be read slowly or through a magnifier: at least 6 seconds and longer for a longer message,
// twice as long with «نمایش درشت و پررنگ»; a touch closes it sooner (it used to vanish after 3.5 seconds, however long it was)
function toast(msg, onTap = null) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.toggle("clickable", typeof onTap === "function");
  t.hidden = false;
  clearTimeout(toastT);
  const ms = Math.max(6000, 2500 + String(msg).length * 70) * (S.settings && S.settings.lowVision ? 2 : 1);
  t.onclick = () => {
    clearTimeout(toastT);
    t.hidden = true;
    t.classList.remove("clickable");
    if (typeof onTap === "function") onTap();
  };
  toastT = setTimeout(() => { t.hidden = true; t.classList.remove("clickable"); }, ms);
}
let resizeT; addEventListener("resize", () => { clearTimeout(resizeT); resizeT = setTimeout(() => S.doc && S.view === "page" && renderPageImage(), 250); });

async function init() {
  try {
    // (the app lock, web/lock.js: if the app is locked the start waits here for the password)
    if (window.khanaLockGate) await window.khanaLockGate();
    const r = await jget("/api/init");
    S.settings = r.settings; S.voices = r.voices;
    S.library = !!r.library; $("libBtn").hidden = !S.library;
    // (editing by voice needs the recognizer, which the Mac and Android apps have, and the microphone)
    $("voiceBtn").hidden = !(r.canListen && navigator.mediaDevices && !window.KHANA_NO_MIC);
    // (speech to text: the Mac app carries the model; a live recording also needs the microphone)
    $("sttRow").hidden = !r.canWrite;
    $("sttLive").hidden = !(navigator.mediaDevices && !window.KHANA_NO_MIC);
    fillVoices(r.voices);
    applySettings(); renderLexicon(); syncChrome(); setPlayButton();
    if (S.settings.startSound !== false) startChime();
  } catch (e) { note("برنامه درست اجرا نشد: " + e.message, true); }
}
// (after every script has run: web/lock.js comes after this one)
if (document.readyState === "loading") addEventListener("DOMContentLoaded", init); else init();


/* ---------------- editing the text: a paragraph changed or taken out, a word changed everywhere ---------------- */
// The corrections are kept on this computer, per book, and reach the reading, the MP3 and the PDF.
const editQuery = () => `doc=${S.doc.id}&rev=${!!S.settings.reversed}&yeh=${!!S.settings.persianYeh}&tr=${translating()}`;
let editGen = 0, lastFound = null;
async function sendEdit(body, keep) {
  if (!S.doc) return false;
  const doc = S.doc.id, page = S.page, query = editQuery();
  try {
    const r = await jpost(`/api/edit?${query}`, { n: page, ...body });
    if (r.error) { toast(r.error); return false; }
    if (!S.doc || S.doc.id !== doc) return false;
    editGen++;
    if (body.kind === "word" && typeof r.found === "number") {
      lastFound = r.found;
      toast(r.found ? `در ${fa(r.found)} جای کتاب عوض شد.` : "کلمه پیدا نشد. املای آن را بررسی کنید.");
    }
    // (1.29 deep check: the page shown now, which may be a later one if «بعدی» was pressed while this edit was saved)
    const y = window.scrollY, here = S.page;
    pageCache.clear(); stopReading();
    await loadPage(here);
    if (!S.doc || S.doc.id !== doc) return false;
    if (here === page) window.scrollTo(0, y);
    $("edInfo").textContent = r.edits ? `${fa(r.edits)} ویرایش در این کتاب` : "";
    $("edUndo").disabled = !r.canUndo;
    return true;
  } catch (e) { toast(e.message); return false; }
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
// editing in the page itself: in edit mode every paragraph is a box to type in (select, type, Delete, paste work as in any editor);
// leaving a paragraph saves it (all text removed: the paragraph is taken out)
function armParagraphs() {
  document.querySelectorAll("#textView p[data-i]").forEach(p => {
    if (!S.editing) { p.removeAttribute("contenteditable"); return; }
    const shown = S.paras[+p.dataset.i];
    if (shown === undefined) return;
    p.textContent = shown;
    p.dataset.shown = shown;
    p.dataset.doc = S.doc.id;
    p.contentEditable = "plaintext-only";
    p.tabIndex = 0;
    if (p.contentEditable !== "plaintext-only") p.contentEditable = "true";
    p.spellcheck = false;
    p.setAttribute("autocorrect", "off");
    p.setAttribute("autocomplete", "off");
    p.setAttribute("role", "textbox"); p.setAttribute("aria-multiline", "true");
    p.setAttribute("aria-label", `بند ${fa(+p.dataset.i + 1)}، قابل ویرایش`);
  });
}
$("textView").addEventListener("focusout", async e => {
  const p = e.target.closest && e.target.closest("p[data-i]");
  if (!S.editing || !p || p.dataset.shown === undefined) return;
  if (!S.doc || p.dataset.doc !== S.doc.id) return;
  const shown = p.dataset.shown;
  const text = p.innerText.replace(/\u00a0/g, " ").trim();
  p.dataset.shown = text;
  if (text === shown.trim()) return;
  // (all its text erased: the paragraph is taken out at once. No question is asked here: a question put while the focus
  // leaves the paragraph was answered «no» by the window itself, and the text came back)
  if (await sendEdit({ kind: "paragraph", shown, text })) toast(text ? "بند ذخیره شد." : "بند حذف شد. با «برگرداندن آخرین تغییر» آن را برگردانید.");
  else renderText();   // (not saved: the page shows what is kept, not what was typed)
});
$("textView").addEventListener("keydown", e => {
  const p = e.target.closest && e.target.closest("p[data-i]");
  if (S.editing && p && e.key === "Escape") { p.textContent = p.dataset.shown; p.blur(); }
});
$("paraSave").onclick = async () => {
  if (editing.shown == null) return;
  if (await sendEdit({ kind: "paragraph", shown: editing.shown, text: $("paraText").value })) $("paraPanel").hidden = true;
};
$("paraDel").onclick = async () => {
  if (editing.shown == null || !await askConfirm("این بند حذف شود؟")) return;
  if (await sendEdit({ kind: "paragraph", shown: editing.shown, text: "" })) $("paraPanel").hidden = true;
};
function openWordEditor(from) {
  const position = currentPiece();
  const selection = getSelection(), node = selection && selection.anchorNode;
  const element = node && (node.nodeType === 1 ? node : node.parentElement);
  const selectedParagraph = element && element.closest('p[data-i]');
  const paragraph = selectedParagraph ? S.paras[+selectedParagraph.dataset.i] : position && position.page === S.page ? S.paras[position.p] : "";
  const word = typeof from === "string" ? from : document.querySelector('.w.now')?.textContent || "";
  $("edFrom").value = word; $("edTo").value = "";
  const at = paragraph ? Math.max(0, paragraph.indexOf(word)) : 0;
  $("wordContext").textContent = paragraph ? (at > 90 ? "…" : "") + paragraph.slice(Math.max(0, at - 90), at + 170) + (paragraph.length > at + 170 ? "…" : "") : "";
  $("wordContext").hidden = !paragraph;
  stopReading();
  openPanel("wordPanel");
  ($("edFrom").value ? $("edTo") : $("edFrom")).focus({ preventScroll: true });
}
$("editBtn").onclick = () => {
  const position = currentPiece();
  const paragraphs = [...$("textView").querySelectorAll('p[data-i]')];
  const anchor = position && position.page === S.page ? paragraphs.find(p => +p.dataset.i === position.p)
    : paragraphs.find(p => p.getBoundingClientRect().bottom > document.querySelector('.bar').getBoundingClientRect().bottom);
  const index = anchor && anchor.dataset.i, y = anchor && anchor.getBoundingClientRect().top;
  stopReading();
  S.editing = !S.editing;
  document.body.classList.toggle("editing-text", S.editing);
  $("editBar").hidden = !S.editing;
  $("textView").classList.toggle("editing", S.editing);
  $("editBtn").classList.toggle("primary", S.editing);
  renderText(true);
  requestAnimationFrame(() => {
    const next = index == null ? null : $("textView").querySelector(`p[data-i="${index}"]`);
    if (next) window.scrollBy(0, next.getBoundingClientRect().top - y);
  });
  if (S.editing) {
    stopReading();
    if (S.view !== "text") { S.view = "text"; syncChrome(); }
    if (S.doc) jget(`/api/edits?doc=${S.doc.id}`).then(r => { $("edInfo").textContent = r.edits ? `${fa(r.edits)} ویرایش در این کتاب` : ""; $("edUndo").disabled = !r.canUndo; }).catch(() => {});
  }
};
$("edWordBtn").onclick = () => openWordEditor(getSelection().toString().trim().split(/\s+/)[0] || undefined);
$("edMarks").onclick = () => openPronunciation($("edFrom").value);
$("edDone").onclick = () => $("editBtn").click();
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
  if (!$("edFrom").value.trim()) return toast("کلمه‌ی فعلی را انتخاب کنید یا بنویسید.");
  if (await sendEdit({ kind: "word", from: $("edFrom").value, to: $("edTo").value })) { $("edFrom").value = ""; $("edTo").value = ""; $("wordPanel").hidden = true; toast("اصلاح کلمه ذخیره شد."); }
};
$("edReset").onclick = async () => {
  if (await askConfirm("همه‌ی ویرایش‌های این کتاب و همه‌ی کلمه‌های اصلاح‌شده‌ی کتاب‌ها پاک شود؟") && await sendEdit({ kind: "reset" })) toast("ویرایش‌ها پاک شد.");
};
$("edUndo").onclick = async () => { if (await sendEdit({ kind: "undo" })) toast("آخرین تغییر برگردانده شد."); };


/* ---------------- a text typed or pasted: read aloud and saved as sound ---------------- */
// (changed while a job runs: its remaining pieces are made the new way at once)
$("fastExport").onchange = () => {
  if (!(job && job.status === "running")) return;
  const fast = $("fastExport").checked;
  jpost("/api/mp3fast", { fast }).then(() => toast(fast ? "از این لحظه سریع‌تر ساخته می‌شود." : "از این لحظه با بررسی کامل ساخته می‌شود.")).catch(e => toast(e.message));
};
$("fastExport").checked = false;   // Full quality checks by default; the faster unchecked mode is an explicit choice.
$("m4aLabel").hidden = /Android|iPhone|iPad/i.test(navigator.userAgent);   // (the phones have no M4A maker)
// (Windows: no MP3 and no M4A maker yet: WAV)
if (/Windows/i.test(navigator.userAgent)) {
  $("m4aLabel").hidden = true; $("mp3Label").hidden = true;
  document.querySelector("input[name=afmt][value=wav]").checked = true;
  { const n = [...$("mp3Btn").childNodes].reverse().find(x => x.nodeType === 3); if (n) n.textContent = "ذخیره‌ی صدا"; } // (only the words: the icon stays)
}
let lastTypedKey = "";
function syncTypeControls(statusMsg) {
  const btn = $("typeRead"), stop = $("typeStop"), st = $("typeReadStatus");
  if (!btn) return;
  const inType = $("typePanel") && !$("typePanel").hidden && S.doc && S.doc.isTyped;
  if (inType && reader.active) {
    btn.textContent = reader.paused ? "▶ ادامه" : "❚❚ مکث";
    if (stop) stop.hidden = false;
  } else {
    btn.textContent = "▶ خواندن";
    if (stop) stop.hidden = true;
  }
  if (st) {
    if (statusMsg !== undefined) {
      st.textContent = statusMsg;
      st.hidden = !statusMsg;
    } else if (!inType || !reader.active) {
      st.textContent = "";
      st.hidden = true;
    }
  }
}
window.syncTypeControls = syncTypeControls;
function closeCurrentDoc(clearTypedBox = false, toastMsg = "") {
  const wasTyped = !!(S.doc && S.doc.isTyped);
  stopReading(false);
  if (clearTypedBox || wasTyped) {
    if (clearTypedBox && $("typeText")) $("typeText").value = "";
    lastTypedKey = "";
  }
  S.doc = null;
  S.page = 1;
  S.paras = [];
  S.pageDone = true;
  S.letters = null;
  S.loadSeq++;
  if ($("textView")) { $("textView").replaceChildren(); $("textView").hidden = true; }
  if ($("pageView")) $("pageView").hidden = true;
  if ($("fileName")) { $("fileName").textContent = ""; $("fileName").title = ""; }
  if ($("note")) { $("note").replaceChildren(); $("note").hidden = true; }
  updatePrepBanner("", -1, "", false);
  clearPageImgCache();
  pageBoxesMap.clear();
  pdfBoxes.clear();
  pageCache.clear();
  $("welcome").hidden = false;
  syncChrome();
  setPlayButton();
  syncTypeControls("");
  jpost("/api/close", {}).catch(() => {});
  if (toastMsg) toast(toastMsg);
}
if ($("closeDocBtn")) $("closeDocBtn").onclick = () => {
  const isTyped = !!(S.doc && S.doc.isTyped);
  closeCurrentDoc(isTyped, isTyped ? "متن پاک شد." : "");
};
if ($("closeTextBtn")) $("closeTextBtn").onclick = () => {
  const isTyped = !!(S.doc && S.doc.isTyped);
  if ($("moreTools")) $("moreTools").open = false;
  closeCurrentDoc(isTyped, isTyped ? "متن پاک شد." : "");
};
async function openTyped(stayInPanel = false) {
  const t = $("typeText").value.trim();
  if (!t) { toast("اول متنی بنویسید."); return false; }
  const fg = $("typeFinglish") && $("typeFinglish").checked ? "1" : "0";
  if (stayInPanel) syncTypeControls("در حال آماده‌سازی صدا…");
  else stopReading();
  const d = await (await api("/api/open?source=typed&finglish=" + fg + "&name=" + encodeURIComponent("متن من.txt"), { method: "POST", body: new TextEncoder().encode(t).buffer })).json();
  if (d.error) { syncTypeControls(""); toast(d.error); return false; }
  lastTypedKey = t + "|" + fg;
  S.doc = d; S.page = 1; S.view = "text"; S.letters = null;
  pageCache.clear();
  $("fileName").textContent = d.name;
  $("from").value = fa(1); $("to").value = fa(d.pages);
  $("welcome").hidden = true;
  note("متن شما آماده است. با «خواندن» بشنوید، با «نوشتن متن» ویرایش کنید، یا با دکمه‌ی روبه‌رو آن را پاک کنید.", false, {
    id: "clearTypedBannerBtn",
    label: "پاک کردن متن",
    onClick: () => closeCurrentDoc(true, "متن پاک شد."),
  });
  if (stayInPanel) {
    const r = await jget(`/api/page?doc=${d.id}&n=1&rev=${!!S.settings.reversed}&yeh=${!!S.settings.persianYeh}&tr=false`);
    S.paras = r.paragraphs || [];
    S.pageDone = true;
    renderText();
    syncChrome();
    return true;
  }
  loadLengths();
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
  if (f.size > 400 * 1024 * 1024) { sttSay("فایل صوتی خیلی بزرگ است."); return; }
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
  sttRec = { stop: false, cancel: false };
  $("sttLive").textContent = "■ پایان ضبط"; $("sttFile").disabled = true;
  const t0 = Date.now();
  while (!sttRec.stop && Date.now() - t0 < 2 * 3600 * 1000) { sttSay("در حال ضبط… " + sttMMSS((Date.now() - t0) / 1000) + " (برای پایان، «پایان ضبط» را بزنید)"); await sleep(250); }
  const cancelled = !!sttRec.cancel;
  node.disconnect(); src.disconnect(); stream.getTracks().forEach(t => t.stop()); ctx.close();
  sttRec = null; $("sttLive").textContent = "🎤 ضبط و نوشتن"; $("sttFile").disabled = false;
  if (cancelled) { chunks.forEach(c => c.fill(0)); cur.length = 0; sttSay(""); return; }
  if (cur.length) chunks.push(Int16Array.from(cur));
  const all = new Int16Array(chunks.reduce((n, c) => n + c.length, 0)); let at = 0;
  for (const c of chunks) { all.set(c, at); at += c.length; }
  if (all.length < 16000) { sttSay("ضبط خیلی کوتاه بود."); return; }
  sttSay("ضبط تمام شد؛ نوشتن شروع می‌شود…");
  await sttRun("/api/stt_start?lang=fa&pcm=1", all.buffer);
};
const showType = () => { openPanel("typePanel"); syncTypeControls(); $("typeText").focus(); };
$("typeBtn").onclick = showType;
$("typeBtn2").onclick = showType;
let typeBusy = false;
$("typeRead").onclick = async () => {
  if (typeBusy) return;
  ensureAudio();
  const curText = $("typeText").value.trim();
  const fg = $("typeFinglish") && $("typeFinglish").checked ? "1" : "0";
  if (!curText) return toast("اول متنی بنویسید.");
  // If already reading this exact typed text inside "نوشتن متن", toggle pause/resume directly (works even while #docTools is inert!)
  if (reader.active && S.doc && S.doc.isTyped && (curText + "|" + fg) === lastTypedKey) {
    if (window.togglePause) window.togglePause();
    else if (actx) {
      reader.paused = !reader.paused;
      if (reader.paused) actx.suspend(); else actx.resume();
      setPlayButton();
    }
    syncTypeControls(reader.paused ? "خواندن مکث شد." : "در حال خواندن متن شما…");
    return;
  }
  typeBusy = true;
  $("typeRead").disabled = true;
  try {
    if (await openTyped(true) && S.paras.length) {
      syncTypeControls("در حال ساختن صدای متن شما…");
      await readFrom(0, 0, { reason: "در حال ساختن صدای متن شما…" });
    }
  } catch (e) {
    syncTypeControls("");
    toast(e.message || "خواندن متن شروع نشد.");
  } finally {
    typeBusy = false;
    $("typeRead").disabled = false;
  }
};
if ($("typeStop")) $("typeStop").onclick = () => { stopReading(false); syncTypeControls(""); };
if ($("typeClear")) $("typeClear").onclick = () => {
  $("typeText").value = "";
  lastTypedKey = "";
  if (S.doc && S.doc.isTyped) closeCurrentDoc(true, "متن پاک شد.");
  else { stopReading(false); syncTypeControls(""); }
  $("typeText").focus();
};
if ($("typePanel")) {
  new MutationObserver(() => {
    if ($("typePanel").hidden && !$("typeText").value.trim() && S.doc && S.doc.isTyped) {
      closeCurrentDoc(true);
    }
  }).observe($("typePanel"), { attributes: true, attributeFilter: ["hidden"] });
}
$("typeSave").onclick = async () => { if (await openTyped(false)) openSave("mp3"); };

/* ---------------- editing the text by voice: for people who cannot see the screen well ---------------- */
// The user says a command; the app repeats what it understood, in a spoken sentence, and asks «درست است؟»; only «بله» changes the text.
let voiceBusy = false, voiceHelped = false;
const buzz = ms => { try { navigator.vibrate && navigator.vibrate(ms); } catch (e) {} };
const vstatus = t => { const b = $("voiceStatus"); b.textContent = t; b.hidden = !t; };
async function speakWait(text) {
  const ctx = ensureAudio();
  try {
    const r = await api("/api/tts", { method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text, voice: S.settings.voice, speed: S.settings.speed, quick: true }) });
    const buf = await r.arrayBuffer();
    if (buf.byteLength > 44) {
      if (ctx.state === "suspended") await ctx.resume();
      const decoded = await ctx.decodeAudioData(buf.slice(0));
      await new Promise(done => {
        const src = ctx.createBufferSource();
        src.buffer = decoded;
        src.connect(ctx.destination);
        const t = setTimeout(done, Math.min(25000, Math.ceil(decoded.duration * 1000) + 1200));
        src.onended = () => { clearTimeout(t); done(); };
        src.start();
      });
      await sleep(300); // let speaker reverberation settle before opening the microphone
      return;
    }
  } catch (_) {}
  try {
    const url = await ttsUrl(text);
    audio.src = url;
    await new Promise(done => { audio.onended = audio.onerror = () => done(); audio.play().catch(() => done()); setTimeout(done, 25000); });
    await sleep(300);
  } catch (_) {}
}
// one recording from the microphone, until the user stops speaking (or the time is up); the recognizer's answer: {text, cmd, yes}
async function listenOnce(maxMs, mode, endQuiet = 1.35) {
  // echoCancellation: false avoids Android WebView / Bluetooth AEC muting the user right after prompt speech!
  const stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: true, autoGainControl: true } });
  let ctx, src, node, sink, rate = 16000, hidden = document.hidden || document.body.classList.contains("locked");
  const chunks = [], t0 = Date.now();
  let heard = false, quiet = 0, floor = 0.008, floorInit = false, seen = 0, maxRms = 0;
  const left = () => { if (document.hidden || document.body.classList.contains("locked")) { hidden = true; stream.getTracks().forEach(track => track.stop()); } };
  document.addEventListener('visibilitychange', left);
  try {
    if (hidden) throw new Error("برای ویرایش با صدا، برنامه را روی صفحه نگه دارید.");
    ctx = new (window.AudioContext || window.webkitAudioContext)();
    try { await ctx.resume(); } catch (e) {}
    rate = ctx.sampleRate;
    src = ctx.createMediaStreamSource(stream); node = ctx.createScriptProcessor(4096, 1, 1);
    sink = ctx.createGain(); sink.gain.value = 0;
    node.onaudioprocess = e => {
      const d = e.inputBuffer.getChannelData(0); chunks.push(new Float32Array(d));
      let sum = 0; for (let i = 0; i < d.length; i++) sum += d[i] * d[i];
      const rms = Math.sqrt(sum / d.length), secs = d.length / ctx.sampleRate;
      seen += secs;
      // Ignore the initial 100ms mic-open pop so it never poisons the noise floor
      if (seen < 0.10) return;
      if (rms > maxRms) maxRms = rms;
      if (!floorInit) { floor = Math.min(0.02, Math.max(0.002, rms)); floorInit = true; }
      else if (!heard) { floor = floor * 0.85 + Math.min(rms, floor * 1.15) * 0.15; }
      const thr = Math.max(0.006, Math.min(0.032, floor * 2.1));
      if (rms > thr) {
        if (!heard) { heard = true; vstatus("🎤 صدایتان را می‌شنوم…"); }
        quiet = 0;
      } else if (heard) {
        quiet += secs;
      }
    };
    src.connect(node); node.connect(sink); sink.connect(ctx.destination);
    while (!hidden && !document.body.classList.contains("locked") && Date.now() - t0 < maxMs && !(heard && quiet > endQuiet) && !(!heard && Date.now() - t0 > 7500)) await sleep(100);
    if (document.body.classList.contains("locked")) hidden = true;
    if (hidden) throw new Error("ضبط با خروج از برنامه متوقف شد. دوباره تلاش کنید.");
  } finally {
    document.removeEventListener('visibilitychange', left);
    stream.getTracks().forEach(track => track.stop());
    try { if (node) node.disconnect(); } catch (_) {}
    try { if (sink) sink.disconnect(); } catch (_) {}
    try { if (src) src.disconnect(); } catch (_) {}
    if (ctx) ctx.close().catch(() => {});
    if (hidden) chunks.forEach(chunk => chunk.fill(0));
  }
  const all = new Float32Array(chunks.reduce((n, c) => n + c.length, 0)); let at = 0;
  for (const c of chunks) { all.set(c, at); at += c.length; }
  // Drop first 100ms mic-open click before computing peak & sending to Vosk
  const trim = Math.min(Math.floor(rate * 0.10), Math.max(0, all.length - Math.floor(rate * 0.4)));
  const sig = trim > 0 ? all.subarray(trim) : all;
  let peak = 0;
  { const bins = new Uint32Array(1001);
    for (let i = 0; i < sig.length; i++) bins[Math.floor(Math.min(1, Math.abs(sig[i])) * 1000)]++;
    let acc = 0; const want = sig.length * 0.995;
    for (let b = 0; b <= 1000; b++) { acc += bins[b]; if (acc >= want) { peak = (b + 1) / 1000; break; } } }
  // Only skip Vosk if the microphone captured pure silence (never throw away quiet speech!)
  if (!heard && peak < 0.0025 && maxRms < 0.002) {
    vstatus("صدایی از میکروفون دریافت نشد.");
    return { text: "", cmd: null, yes: false, go: null, silent: true };
  }
  vstatus("در حال تشخیص گفتار…");
  const gain = peak > 0.001 ? Math.min(35, 0.72 / peak) : 1;
  // 16 kHz, 16-bit: each output sample is the mean of the input samples it covers
  const ratio = rate / 16000, out = new Int16Array(Math.floor(sig.length / ratio));
  for (let i = 0; i < out.length; i++) {
    const a = Math.floor(i * ratio), b = Math.max(a + 1, Math.floor((i + 1) * ratio));
    let m = 0; for (let k = a; k < b && k < sig.length; k++) m += sig[k];
    out[i] = Math.max(-1, Math.min(1, gain * m / (b - a))) * 32767;
  }
  const r = await (await api("/api/listen?mode=" + mode + "&doc=" + encodeURIComponent(S.doc.id), { method: "POST", body: out.buffer })).json();
  // (what was heard is shown, so a person who can see a little, or a helper, sees what went wrong)
  if (r.text !== undefined) vstatus("شنیدم: «" + (clean(r.text) || "—") + "»");
  return r;
}
function currentPiece() {
  const native = reader.nativePosition;
  if (native && native.para >= 0) return { page: native.page, p: native.para, start: native.start, end: native.end };
  const now = actx ? actx.currentTime : 0;
  const p = reader.playing.find(u => now >= u.t0 && now < u.t1) || reader.playing[reader.playing.length - 1];
  return p && p.p >= 0 ? { page: p.page, p: p.p } : null;
}
const clean = t => (t || "").replace(/\[unk\]/g, " ").replace(/\s+/g, " ").trim();
async function voiceEdit() {
  if (voiceBusy || !S.doc) return;
  voiceBusy = true;
  // «این کلمه» is the word being read now (marked on the page)
  const curEl = document.querySelector(".w.now");
  const curWord = curEl ? clean(curEl.textContent.replace(/[^\p{L}\u200c]/gu, "")) : "";
  const cur = currentPiece();
  stopReading();
  const ask = async (say, show, mode, ms, endQuiet) => { vstatus(show); if (say) await speakWait(say); vstatus("گوش می‌دهم…\n" + show); return await listenOnce(ms, mode, endQuiet); };
  const stop = async m => { vstatus(m); await speakWait(m); };
  // (a yes or no: if it was neither heard as a «no» nor as a «yes» — noise, a word run together — it is asked once more)
  const askYes = async (say, show, ms) => {
    let r = await ask(say, show, "yes", ms);
    if (!r.yes && !/(?:^|\s)(?:نه|خیر|نادرست|لغو|غلط)(?:\s|$)/.test(r.text || "")) r = await ask("متوجه نشدم. بله یا خیر؟", show, "yes", ms);
    return r;
  };
  const theWord = async () => curWord || clean((await ask("کدام کلمه؟", "کدام کلمه؟ آن را بگویید", "book", 9000)).text);
  try {
    vstatus("در حال آماده شدن…");
    let r1 = await ask("بفرمایید.", "بگویید، مثلاً: «تلفظ این کلمه را درست کن» · «کسره‌ی اضافه» · «این بند را عوض کن» · «این کلمه را جدا کن» · «این کلمه را عوض کن» · «این بند را حذف کن»", "cmd", 10000);
    // (not understood, or nothing heard — show what was heard or if mic was silent before asking once more)
    if (!r1.cmd) {
      const heardTxt = clean(r1.text);
      const retryMsg = r1.silent
        ? "صدایی نشنیدم. دوباره نزدیک‌تر به میکروفون بگویید."
        : (heardTxt ? `شنیدم «${heardTxt}» اما متوجه دستور نشدم. دوباره بگویید.` : "متوجه نشدم. دوباره بگویید.");
      r1 = await ask(r1.silent ? "صدایی نشنیدم. دوباره بگویید." : "متوجه نشدم. دوباره بگویید.", retryMsg + "\nمثلاً: «تلفظ این کلمه» · «کسره‌ی اضافه» · «این بند را عوض کن»", "cmd", 10000);
    }
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
      const r6 = await askYes("این متن جای بند فعلی بیاید؟ بگویید بله یا خیر.", "شنیدم:\n«" + newText + "»\nجای این بند بیاید؟ بله یا خیر", 7000);
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
        const r4 = await askYes(q2, q2, 7000);
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
        const r3 = await askYes("درست شد؟", "درست شد؟ بله یا خیر", 7000);
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
    const r2 = await askYes(q + " بله یا خیر؟", q, 7000);
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
  if (document.body.classList.contains("locked")) return false;
  const open = [...document.querySelectorAll(".panel")].filter(p => !p.hidden);
  if (open.length) { open.forEach(p => { p.hidden = true; }); return true; }
  if (S.doc && S.page > 1) { $("prev").click(); return true; }
  return false;
};

// Keep the last lines of a document and transient messages above the entire player on every screen size.
if (typeof ResizeObserver !== "undefined") {
  const dockObserver = new ResizeObserver(() => {
    document.documentElement.style.setProperty("--dock-height", Math.ceil($("docTools").getBoundingClientRect().height) + "px");
  });
  dockObserver.observe($("docTools"));
}

// Track the visual viewport rather than hiding editing controls behind the soft keyboard.
(function () {
  let normalHeight = window.innerHeight;
  function fit() {
    const viewport = window.visualViewport, height = viewport ? viewport.height : window.innerHeight;
    if (!document.activeElement || !document.activeElement.matches('input,textarea,[contenteditable]')) normalHeight = window.innerHeight;
    document.documentElement.style.setProperty('--visual-height', height + 'px');
    document.documentElement.style.setProperty('--visual-top', (viewport ? viewport.offsetTop : 0) + 'px');
    document.body.classList.toggle('keyboard-open', height < normalHeight * 0.8);
  }
  window.addEventListener('resize', fit);
  if (window.visualViewport) { visualViewport.addEventListener('resize', fit); visualViewport.addEventListener('scroll', fit); }
  document.addEventListener('focusin', fit); document.addEventListener('focusout', fit);
  fit();
})();

// Swipe gestures for page navigation: left swipe = next page, right swipe = previous page
(function () {
  let touchStartX = 0, touchStartY = 0;
  document.addEventListener("touchstart", e => {
    if (document.body.classList.contains("locked") || document.querySelector(".panel:not([hidden])") || e.touches.length !== 1 || !S.doc || document.documentElement.classList.contains("android-reader")) { touchStartX = 0; return; }
    touchStartX = e.touches[0].clientX;
    touchStartY = e.touches[0].clientY;
  }, { passive: true });
  document.addEventListener("touchend", e => {
    if (document.body.classList.contains("locked") || document.querySelector(".panel:not([hidden])") || !S.doc || !touchStartX || document.documentElement.classList.contains("android-reader")) return;
    const touchEndX = e.changedTouches[0].clientX;
    const touchEndY = e.changedTouches[0].clientY;
    const dx = touchEndX - touchStartX, dy = touchEndY - touchStartY;
    if (Math.abs(dx) < 50 || Math.abs(dy) > Math.abs(dx)) return; // need significant horizontal swipe
    if (dx > 0 && S.page > 1) $("prev").click(); // right swipe = previous page
    else if (dx < 0 && S.page < S.doc.pages) $("next").click(); // left swipe = next page
  }, { passive: true });
})();

// Zoom and rotation controls for PDF viewing
(function () {
  let pdfZoom = 1, rot = 0;
  const updateZoom = z => { if (z !== undefined) pdfZoom = z; applyView(); };
  // (a turned picture keeps its old box in the page: the margins make room for the turned one, and it is shrunk to fit the width)
  const applyView = () => {
    const img = $("pageImg");
    img.style.transform = ""; img.style.marginBlock = "";
    if (pdfZoom === 1 && rot === 0) return;
    const w = img.offsetWidth, h = img.offsetHeight, odd = rot % 180 !== 0;
    const fit = odd && h ? Math.min(1, ($("pageView").clientWidth || w) / h) : 1;
    if (odd) img.style.marginBlock = ((w * fit - h) / 2) + "px";
    img.style.transform = `rotate(${rot}deg) scale(${pdfZoom * fit})`;
    img.style.transformOrigin = odd ? "center center" : "top center";
  };
  const turn = d => { rot = (rot + d + 360) % 360; applyView(); $("zoomMenu").hidden = true; };
  $("pageImg").addEventListener("load", applyView);
  window.resetPdfZoom = () => { rot = 0; updateZoom(1); }; // Reset zoom when opening new document
  $("zoomBtn").onclick = e => { const menu = $("zoomMenu"); menu.hidden = !menu.hidden; };
  document.addEventListener("pointerdown", e => { const m = $("zoomMenu"); if (m && !m.hidden && !m.contains(e.target) && !e.target.closest("#zoomBtn")) m.hidden = true; });
  $("zoomOut").onclick = () => { updateZoom(Math.max(0.5, pdfZoom - 0.1)); $("zoomMenu").hidden = true; };
  $("zoomReset").onclick = () => { updateZoom(1); $("zoomMenu").hidden = true; };
  $("rotLeft").onclick = () => turn(-90);
  $("rotRight").onclick = () => turn(90);
  $("zoomIn").onclick = () => { updateZoom(Math.min(3, pdfZoom + 0.1)); $("zoomMenu").hidden = true; };
  // Keyboard shortcuts for zoom: + and - keys (never intercept while locked, in modals, or typing in inputs)
  document.addEventListener("keydown", e => {
    if (!S.doc || S.view !== "page" || document.body.classList.contains("locked")) return;
    if (document.querySelector(".panel:not([hidden])")) return;
    if (e.target && e.target.closest && e.target.closest("input,textarea,select,[contenteditable]")) return;
    if ((e.key === "+" || e.key === "=") && !e.ctrlKey && !e.metaKey) { e.preventDefault(); updateZoom(Math.min(3, pdfZoom + 0.1)); }
    else if (e.key === "-" && !e.ctrlKey && !e.metaKey) { e.preventDefault(); updateZoom(Math.max(0.5, pdfZoom - 0.1)); }
  });
  // Pinch-to-zoom for touch devices
  let lastDistance = 0;
  document.addEventListener("touchmove", e => {
    if (document.body.classList.contains("locked") || document.querySelector(".panel:not([hidden])") || !S.doc || S.view !== "page" || e.touches.length !== 2) { lastDistance = 0; return; }
    const dx = e.touches[0].clientX - e.touches[1].clientX;
    const dy = e.touches[0].clientY - e.touches[1].clientY;
    const distance = Math.hypot(dx, dy);
    if (lastDistance > 0) {
      const scale = distance / lastDistance;
      if (scale > 1.02) updateZoom(Math.min(3, pdfZoom * 1.05));
      else if (scale < 0.98) updateZoom(Math.max(0.5, pdfZoom / 1.05));
    }
    lastDistance = distance;
  }, { passive: true });
  $("zoomBtn").hidden = true;
})();

// a question with two Persian buttons, drawn on the page (the phone's own confirm box is in English): resolves true for «تأیید»
function askConfirm(message) {
  return new Promise(resolve => {
    const back = document.createElement("div");
    back.className = "confirm-back";
    back.innerHTML = '<div class="confirm-box" role="alertdialog" aria-modal="true" aria-labelledby="confirmMsg"><p id="confirmMsg"></p><div class="confirm-actions"><button type="button" class="confirm-no">انصراف</button><button type="button" class="confirm-yes primary">تأیید</button></div></div>';
    back.querySelector("#confirmMsg").textContent = message;
    const onKey = e => { if (e.key === "Escape") { e.preventDefault(); done(false); } };
    const done = ok => { back.remove(); document.removeEventListener("keydown", onKey, true); resolve(ok); };
    back.querySelector(".confirm-no").onclick = () => done(false);
    back.querySelector(".confirm-yes").onclick = () => done(true);
    document.addEventListener("keydown", onKey, true);
    document.body.appendChild(back);
    back.querySelector(".confirm-no").focus();
  });
}

// Keep --dock-height synced to the live bottom player height so text never hides under it on any screen/orientation.
(function () {
  const dock = $("docTools");
  if (!dock) return;
  const syncDockHeight = () => {
    const h = dock.hidden ? 0 : Math.ceil(dock.getBoundingClientRect().height);
    document.documentElement.style.setProperty("--dock-height", h + "px");
  };
  if (window.ResizeObserver) new ResizeObserver(syncDockHeight).observe(dock);
  addEventListener("resize", syncDockHeight);
  syncDockHeight();
})();

// Privacy hooks called by lock.js when locking/unlocking: stop active mic capture, scrub open book DOM & caches,
// and cleanly re-fetch the current page after unlock.
window.khanaOnLock = () => {
  try { stopReading(); } catch (e) {}
  try { stopRecording(false); } catch (e) {}
  try { clearBrowserClip(); } catch (e) {}
  if (typeof sttRec !== "undefined" && sttRec) { sttRec.cancel = true; sttRec.stop = true; }
  document.querySelectorAll(".confirm-back .confirm-no").forEach(b => b.click());
  if ($("zoomMenu")) $("zoomMenu").hidden = true;
  if ($("moreTools")) $("moreTools").open = false;
  if ($("textView")) $("textView").innerHTML = "";
  if ($("fileName")) $("fileName").textContent = "";
  if ($("note")) { $("note").textContent = ""; $("note").hidden = true; }
  if ($("sourceText")) $("sourceText").textContent = "";
  if ($("bookmarkList")) $("bookmarkList").innerHTML = "";
  if ($("libList")) $("libList").innerHTML = "";
  if ($("lexList")) $("lexList").innerHTML = "";
  clearPageImgCache();
  pageBoxesMap.clear();
  pdfBoxes.clear();
  S.paras = [];
  pageCache.clear();
  for (const p of ttsCache.values()) {
    Promise.resolve(p).then(u => { if (typeof u === "string" && u.startsWith("blob:")) try { URL.revokeObjectURL(u); } catch (e) {} }).catch(() => {});
  }
  ttsCache.clear();
  bufCache.clear();
};
window.khanaOnUnlock = () => {
  if (S.doc) {
    if ($("fileName")) $("fileName").textContent = S.doc.name;
    loadPage(S.page).catch(() => {});
  }
};
