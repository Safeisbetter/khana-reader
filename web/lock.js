"use strict";
/* The app lock («قفل برنامه»), the page's side of src/lock.rs: the lock screen, the card in Settings, the form for the password, and
   the touches that tell the app the reader is there.

   The page only shows. The app itself refuses every request while it is locked (423), so hiding this screen, or never loading this file,
   gives nothing. The password is sent to the app and not kept here. Loaded after app.js (and the phone's android.js / ios.js). */
(function () {
  const $ = id => document.getElementById(id);
  let st = { enabled: false, locked: false, wait: 0, timeout: -1 };
  const waiting = [];     // the start of the page, waiting for the unlock
  let lastTouch = 0;
  let mode = "";          // the form: "on", "change" or "off"

  async function readStatus() {
    try { st = await jget("/api/lock_status"); } catch (e) { return null; }
    return st;
  }

  /* ---------------- the lock screen ---------------- */
  const isShown = () => document.body.classList.contains("locked");
  function show(message) {
    if (isShown()) { if (message) $("lockErr").textContent = message; return; }
    try { stopReading(); } catch (e) { /* (nothing is being read yet) */ }
    // whatever was open is shut, and nothing under the screen can be reached by keyboard or by a screen reader
    document.querySelectorAll(".panel").forEach(p => { p.hidden = true; });
    document.body.classList.add("locked");
    for (const el of document.body.children) if (el.id !== "lockScreen" && el.tagName !== "SCRIPT") el.inert = true;
    $("lockScreen").hidden = false;
    $("lockPw").value = "";
    $("lockErr").textContent = message || "";
    $("lockForgot").open = false;
    setTimeout(() => $("lockPw").focus(), 30);
  }
  function hide() {
    document.body.classList.remove("locked");
    for (const el of document.body.children) el.inert = false;
    $("lockScreen").hidden = true;
    $("lockPw").value = "";
    renderCard();
    waiting.splice(0).forEach(f => f());
  }

  async function tryUnlock() {
    const pw = $("lockPw").value;
    if (!pw) return;
    $("lockGo").disabled = true;
    try {
      st = await jpost("/api/unlock", { password: pw });
      hide();
    } catch (e) {
      $("lockErr").textContent = e.message;
      $("lockPw").select();
    } finally {
      $("lockGo").disabled = false;
    }
  }
  $("lockGo").onclick = tryUnlock;
  $("lockPw").addEventListener("keydown", e => { if (e.key === "Enter") { e.preventDefault(); tryUnlock(); } });
  $("lockReset").onclick = async () => {
    if (!confirm("همه‌ی اطلاعات خوانا (ویرایش‌ها، صدای ضبط‌شده، تنظیمات و خودِ قفل) از این دستگاه پاک شود؟ این کار برگشت ندارد.")) return;
    if (!confirm("مطمئن هستید؟ بعد از این نمی‌شود آن‌ها را پس گرفت.")) return;
    try { await jpost("/api/forget", {}); location.reload(); } catch (e) { $("lockErr").textContent = e.message; }
  };

  // app.js asks for this before it starts (the lock screen comes up and the start waits for the password), and calls khanaLockHit
  // whenever a request is refused because the app is locked
  window.khanaLockGate = async () => {
    const s = await readStatus();
    if (s && s.locked) { show(); await new Promise(res => waiting.push(res)); }
  };
  window.khanaLockHit = () => { st.locked = true; show(); };

  /* ---------------- the card in Settings and its form ---------------- */
  function renderCard() {
    const on = !!st.enabled;
    $("lockState").textContent = on ? "قفل روشن است." : "قفل خاموش است.";
    $("lockOn").hidden = on;
    $("lockChange").hidden = !on; $("lockOff").hidden = !on; $("lockNow").hidden = !on; $("lockTimeoutRow").hidden = !on;
    if (on) $("lockTimeout").value = String(st.timeout);
  }
  function openForm(m) {
    mode = m;
    $("lockFormTitle").textContent = m === "on" ? "روشن کردن قفل" : m === "change" ? "عوض کردن رمز" : "خاموش کردن قفل";
    $("lockCurRow").hidden = m === "on";
    $("lockNewRow").hidden = m === "off";
    $("lockRepRow").hidden = m === "off";
    for (const id of ["lockCur", "lockNew", "lockRep"]) $(id).value = "";
    $("lockFormErr").textContent = "";
    openPanel("lockForm");
    $(m === "on" ? "lockNew" : "lockCur").focus();
  }
  async function submitForm() {
    const cur = $("lockCur").value, nw = $("lockNew").value, rep = $("lockRep").value;
    const fail = m => { $("lockFormErr").textContent = m; };
    fail("");
    if (mode !== "off" && nw !== rep) return fail("دو رمز یکی نیست.");
    if (mode === "off" && !cur) return fail("رمز فعلی را وارد کنید.");
    $("lockFormOk").disabled = true;
    try {
      if (mode === "off") st = await jpost("/api/lock_off", { password: cur });
      else st = await jpost("/api/lock_set", mode === "change" ? { password: nw, current: cur } : { password: nw });
      for (const id of ["lockCur", "lockNew", "lockRep"]) $(id).value = "";
      $("lockForm").hidden = true;
      renderCard();
      toast(mode === "off" ? "قفل خاموش شد." : mode === "change" ? "رمز عوض شد." : "قفل روشن شد.");
    } catch (e) {
      fail(e.message);
    } finally {
      $("lockFormOk").disabled = false;
    }
  }
  $("lockOn").onclick = () => openForm("on");
  $("lockChange").onclick = () => openForm("change");
  $("lockOff").onclick = () => openForm("off");
  $("lockFormOk").onclick = submitForm;
  for (const id of ["lockCur", "lockNew", "lockRep"]) $(id).addEventListener("keydown", e => { if (e.key === "Enter") { e.preventDefault(); submitForm(); } });
  $("lockNow").onclick = async () => { try { st = await jpost("/api/lock_now", {}); show(); } catch (e) { toast(e.message); } };
  $("lockTimeout").onchange = async () => {
    try { st = await jpost("/api/lock_timeout", { timeout: +$("lockTimeout").value }); toast("ذخیره شد."); } catch (e) { toast(e.message); renderCard(); }
  };
  $("settingsBtn").addEventListener("click", () => { readStatus().then(renderCard); });

  /* ---------------- the reader is there ---------------- */
  // A touch, a key or the wheel tells the app (at most every 10 seconds) that the reader is there, so that it does not lock under
  // them. While the book is being read aloud the reader is there too, whether or not they touch anything.
  function touch(force) {
    const t = Date.now();
    if (!st.enabled || !(st.timeout > 0) || isShown() || (!force && t - lastTouch < 10000)) return;
    lastTouch = t;
    jpost("/api/activity", {}).catch(() => {});
  }
  for (const ev of ["pointerdown", "keydown", "touchstart", "wheel"]) addEventListener(ev, () => touch(false), { capture: true, passive: true });
  setInterval(() => { try { if (reader.active && !reader.paused) touch(true); } catch (e) { /* (not started yet) */ } }, 20000);

  // the app may have locked itself while this page waited in the background: asked at once on return, and every few seconds
  const check = () => { if (isShown()) return; readStatus().then(s => { if (s && s.locked) show(); else if (s) renderCard(); }); };
  setInterval(check, 3000);
  document.addEventListener("visibilitychange", () => { if (!document.hidden) check(); });
  addEventListener("focus", check);
  addEventListener("pageshow", check);
  readStatus().then(renderCard);
})();
