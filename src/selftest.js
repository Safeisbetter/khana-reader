// Self-test of the real window (KHANA_SELFTEST=<file in Downloads>): runs the page's own steps.
// (anything the page's Content-Security-Policy refuses is counted and reported at the end)
const cspViolations = [];
document.addEventListener("securitypolicyviolation", e => cspViolations.push(e.violatedDirective + " " + (e.blockedURI || "inline") + " at " + (e.sourceFile || "?").split("/").pop() + ":" + e.lineNumber + " " + (e.sample || "") + " on " + (e.target && e.target.nodeName)));
addEventListener("load", () => setTimeout(async () => {
  const say = m => fetch("/api/selftest_report", { method: "POST", headers: { "X-Khana-Token": TOKEN }, body: m });
  // (a password prompt is answered from KHANA_SELFTEST_PASSWORDS, one password per prompt; none left: the dialog is closed)
  try {
    const list = (await (await api("/api/selftest_passwords")).json()).list || [];
    let tried = 0, answered = 0;
    setInterval(() => {
      const panel = $("pwPanel");
      if (!panel || panel.hidden || (+panel.dataset.prompts || 0) === answered) return;
      answered = +panel.dataset.prompts;
      const hint = $("pwHint").textContent;
      if (tried < list.length) {
        say("password prompt " + answered + ": «" + hint + "», typing the next one"); $("pwInput").value = list[tried++]; $("pwOk").click();
      } else {
        say("password prompt " + answered + ": «" + hint + "», none left: closing the dialog"); panel.hidden = true;
      }
    }, 50);
  } catch (e) { /* (no passwords) */ }
  try {
    await say("loaded: voices " + (await jget("/api/init")).voices.length + ", mediaDevices " + !!navigator.mediaDevices);
    { const t = document.querySelector(".thanks"); await say("thanks line: «" + (t ? t.textContent : "MISSING") + "», shown " + !!(t && t.offsetParent) + ", width " + (t ? Math.round(t.getBoundingClientRect().width) : 0) + ", nastaliq face there " + document.fonts.check('20px "Noto Nastaliq Urdu"', "تشکر") + ", naskh " + document.fonts.check('20px "DecoType Naskh"', "تشکر")); }
    const b = await (await api("/api/selftest_file")).blob();
    if ((await (await api("/api/selftest_layout", { method: "POST" })).json()).only) {
      const buf = await b.arrayBuffer(), { pages } = await pdfCall({ op: "open", data: buf }, [buf]), out = [];
      for (let n = 1; n <= pages; n++) { const t = await pdfCall({ op: "text", n }); out.push({ w: t.w, h: t.h, images: !!t.images, items: t.items }); }
      await api("/api/selftest_layout", { method: "POST", body: JSON.stringify({ pages: out }) });
      await say("layout saved: " + pages + " pages");
      await say("DONE");
      return;
    }
    const tOpen = Date.now();
    await openFile(new File([b], window.KHANA_SELFTEST_NAME || "selftest.pdf"));
    // a long translated page is shown from its first paragraphs on, and grows while it is read
    if (S.doc.canTranslate) {
      while (!S.paras.length && Date.now() - tOpen < 180000) await new Promise(r => setTimeout(r, 200));
      const shown = (Date.now() - tOpen) / 1000, first = S.paras.length, wasDone = S.pageDone;
      await new Promise(r => setTimeout(r, 15000));
      await say(`translated page: first ${first} paragraph(s) on screen after ${shown.toFixed(1)} s, page done then ${wasDone}; `
        + `15 s later ${S.paras.length} paragraphs (done ${S.pageDone}), reading ${reader.active}, ${reader.queue.length} sentences queued, `
        + `${reader.pageParas[S.page] ? reader.pageParas[S.page].length : 0} paragraphs taken by the reader, more-note shown ${!!document.getElementById("trMore")}`);
      const tAll = Date.now();
      while (!S.pageDone && Date.now() - tAll < 900000) await new Promise(r => setTimeout(r, 1000));
      await say(`translated page complete after ${((Date.now() - tOpen) / 1000).toFixed(0)} s: ${S.paras.length} paragraphs on screen, ${document.querySelectorAll("#textView p[data-i]").length} rendered, reader has ${reader.pageParas[S.page] ? reader.pageParas[S.page].length : 0}, still reading ${reader.active}`);
      stopReading();
    }
    await new Promise(r => setTimeout(r, 3000));
    await say("opened: pages " + document.getElementById("pageCount").textContent + ", paragraphs on screen " + document.querySelectorAll("#text p, .para, p").length);
    try {
      // the looks: each can be chosen, colours the page, and the glass ones frost the bar
      const seen = [];
      for (const th of THEMES) {
        await setSetting("theme", th);
        const cs = getComputedStyle(document.querySelector(".bar"));
        seen.push(th + ": page " + getComputedStyle(document.body).backgroundColor + ", ink " + getComputedStyle(document.body).color + ", frost " + ((cs.backdropFilter || cs.webkitBackdropFilter || "none").slice(0, 10)) + ", radio " + !!document.querySelector(`input[name=theme][value=${th}]:checked`));
      }
      await setSetting("theme", "auto");
      await say("looks: " + seen.join(" | "));
      // the page's width and margin, the search in the settings, the click sound, the heat message
      const tv = $("textView");
      const w0 = Math.round(tv.getBoundingClientRect().width), p0 = parseFloat(getComputedStyle(tv).paddingLeft);
      await setSetting("pageWidth", 30); const w1 = Math.round(tv.getBoundingClientRect().width);
      await setSetting("pageWidth", 100); const w2 = Math.round(tv.getBoundingClientRect().width);
      await setSetting("pageMargin", 20); const p1 = parseFloat(getComputedStyle(tv).paddingLeft);
      await setSetting("pageWidth", 46); await setSetting("pageMargin", 100);
      $("setFind").value = "قلم"; $("setFind").oninput();
      const fields = [...document.querySelectorAll("#settings .panel-body > .field")], shown = fields.filter(f => !f.hidden).length;
      $("setFind").value = "zzzz"; $("setFind").oninput(); const none = !$("setNone").hidden;
      $("setFind").value = ""; $("setFind").oninput();
      const brs = () => document.querySelectorAll("#textView p[data-i] br").length;
      const b0 = brs(); await setSetting("sentenceLines", true); renderText(); const b1 = brs();
      const offsetsOk = [...document.querySelectorAll("#textView .w")].slice(0, 200).every(w => S.paras[+w.closest("p").dataset.i].startsWith(w.textContent, +w.dataset.s));
      const sample = paraHtml("جمله‌ی اول. جمله‌ی دوم است؟ عدد ۳.۵ و ص. ۱۲ و پایان.", 0, null, () => {});
      const sampleBr = (sample.match(/<br>/g) || []).length, plain = sample.replace(/<br>/g, " ").replace(/<[^>]+>/g, "");
      await setSetting("sentenceLines", false); renderText();
      await say(`sentence lines: line breaks ${b0} -> ${b1} with the setting, words still at their places ${offsetsOk}, off again ${brs()}; a sample of four sentences got ${sampleBr} breaks, text unchanged ${plain === "جمله‌ی اول. جمله‌ی دوم است؟ عدد ۳.۵ و ص. ۱۲ و پایان."}`);
      let tone = "no";
      try { await setSetting("clickSound", true); clickTone(true); tone = clickCtx ? clickCtx.state : "no engine"; await setSetting("clickSound", false); } catch (e) { tone = "ERROR " + e; }
      const fileFs = getComputedStyle($("fileName")).fontSize;
      const heat = await jpost("/api/heat?level=2", {}); const back = await jpost("/api/heat?level=0", {});
      await say(`page: width ${w0} -> ${w1} (narrow) -> ${w2} (whole window, window ${innerWidth}); side margin ${p0} -> ${p1} (small); settings search «قلم»: ${shown} of ${fields.length} shown, a wrong word says «not found» ${none}; click sound engine ${tone}; book name ${fileFs}; heat told ${heat.heat} then ${back.heat}`);
    } catch (e) { await say("looks: ERROR " + e); }
    try {
      // the layout: the player at the foot of the window, inside it, not over the top bar; nothing wider than the window
      const box = e => { const r = e.getBoundingClientRect(); return Math.round(r.left) + "," + Math.round(r.top) + " " + Math.round(r.width) + "x" + Math.round(r.height); };
      const dock = $("docTools"), bar = document.querySelector(".bar");
      const wide = [...document.querySelectorAll("body *")].filter(e => e.offsetParent && e.getBoundingClientRect().right > innerWidth + 1 || e.offsetParent && e.getBoundingClientRect().left < -1).map(e => e.id || e.className).slice(0, 5);
      const seen = id => { const e = $(id); return !!(e && e.offsetParent); };
      await say(`layout: window ${innerWidth}x${innerHeight}; top bar ${box(bar)}; player ${box(dock)}, position ${getComputedStyle(dock).position}, body has-doc ${document.body.classList.contains("has-doc")}; in sight: play ${seen("play")}, pages ${seen("pageInput")}, speed ${seen("speedChip")}, search ${seen("q")}, edit ${seen("editBtn")}, save ${seen("mp3Btn")}, settings ${seen("settingsBtn")}, view ${seen("viewText")}; wider than the window: ${JSON.stringify(wide)}`);
    } catch (e) { await say("layout: ERROR " + e); }
    // KHANA_SELFTEST_SCREEN: reading aloud and saving an MP3 while the screen is switched off (the tester puts the display to
    // sleep): every 5 s, whether the page is shown, how far the voice is, how far the file is.
    if ((await jget("/api/init")).screenTest === true) {
      if (!reader.active) $("play").click();
      await new Promise(r => setTimeout(r, 4000));
      for (let n = 8; n <= 30; n++) await pageReq(`/api/page?doc=${S.doc.id}&n=${n}&rev=false&yeh=true&tr=false`, n);
      const mjob = await jpost("/api/mp3", { doc: S.doc.id, first: 8, last: 30, voice: "khabari", speed: 1.0, bitrate: 64 });
      for (let k = 0; k < 22; k++) {
        const j = await jget("/api/job?id=" + mjob.id);
        await say(`screen: page ${document.visibilityState}, sound ${actx.state} at ${actx.currentTime.toFixed(0)} s, sentence ${reader.next} of ${reader.queue.length}, page ${S.page}, reading ${reader.active}; mp3 ${j.status} ${j.error || ""} ${Math.round(j.progress * 100)}%, ${j.seconds} s of sound`);
        if (k === 21) await jpost("/api/cancel?id=" + mjob.id, {}).catch(() => {});
        await new Promise(r => setTimeout(r, 5000));
      }
      stopReading();
      await say("DONE");
      return;
    }
    // Editing the text, as a person does it: edit mode, a paragraph typed over, one word changed, a word taken out, a paragraph taken out, all undone.
    try {
      window.confirm = () => true;
      if ((await jget("/api/init")).testData !== true) throw "skipped: never on the app's real data (it ends with a reset)";
      const wait = ms => new Promise(r => setTimeout(r, ms));
      const first = () => ((document.querySelector("#textView p[data-i='0']") || {}).textContent || "").trim();
      const count = () => document.querySelectorAll("#textView p[data-i]").length;
      const until = async f => { for (let k = 0; k < 60 && !f(); k++) await wait(250); return f(); };
      await until(() => S.pageDone !== false && count() > 0);
      const before = { n: count(), first: first() };
      $("editBtn").click();
      await say("edit: button «" + $("editBtn").textContent.trim() + "», edit mode on " + S.editing + ", bar shown " + !$("editBar").hidden + ", " + before.n + " paragraphs");
      const w0 = (before.first.split(/\s+/).find(x => /^[\u0600-\u06ff]{3,}$/.test(x)) || "");
      $("edFrom").value = w0; $("edTo").value = "ZZتست"; $("edWord").click();
      const changed = await until(() => first().includes("ZZتست"));
      await say("edit: one word «" + w0 + "» changed: " + changed + ", toast «" + ($("toast") ? $("toast").textContent : "") + "»");
      $("edFrom").value = "ZZتست"; $("edTo").value = ""; $("edWord").click();
      const gone = await until(() => !first().includes("ZZتست") && first() !== "");
      await say("edit: that word taken out: " + gone + ", the paragraph now «" + first().slice(0, 60) + "»");
      const typeOver = async text => {
        const el = document.querySelector("#textView p[data-i='0']");
        await say("edit: paragraph is typeable without a tap " + el.isContentEditable);
        el.focus(); el.textContent = text; el.blur();
      };
      await typeOver("این متن برای آزمایش نوشته شد.");
      const typed = await until(() => first() === "این متن برای آزمایش نوشته شد.");
      await say("edit: typed in the page: " + typed + ", count shown «" + $("edInfo").textContent + "»");
      // (taking a paragraph out by erasing its text must not depend on a question: the window may answer «no» by itself)
      window.confirm = () => false;
      await wait(800); await typeOver("");
      const removed = await until(() => count() === before.n - 1);
      await say("edit: all text removed, paragraph taken out: " + removed + " (" + before.n + " -> " + count() + ")");
      window.confirm = () => true;
      $("edReset").click();
      const back = await until(() => count() === before.n && first() === before.first);
      await say("edit: all undone, the page is as it was: " + back + " (" + count() + " paragraphs, first «" + first().slice(0, 40) + "» was «" + before.first.slice(0, 40) + "»), count shown «" + $("edInfo").textContent + "»");
      $("editBtn").click();
    } catch (e) { await say("edit: ERROR " + e); }
    const r = await api("/api/tts", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ text: "سلام، این یک آزمایش است.", voice: "khabari", speed: 1.0 }) });
    const wavBytes = await r.clone().arrayBuffer();
    const buf = await ensureAudio().decodeAudioData(await r.arrayBuffer());
    await say("speech: " + buf.duration.toFixed(2) + " s decoded, audio context " + actx.state);
    if (!$("sttRow").hidden) {
      // the speech just made is written down again (Whisper, inside the sandbox)
      try {
        const t0 = Date.now();
        await api("/api/stt_start?lang=fa&name=x.wav", { method: "POST", body: wavBytes });
        let st;
        do { await new Promise(r => setTimeout(r, 500)); st = await (await api("/api/stt_status")).json(); } while (st.state !== "done" && st.state !== "error" && Date.now() - t0 < 180000);
        await say("speech to text: " + st.state + " in " + ((Date.now() - t0) / 1000).toFixed(1) + " s: " + (st.text || st.error));
      } catch (e) { await say("speech to text FAILED " + e.message); }
    }
    S.view = "page"; syncChrome(); renderPageImage();
    for (let k = 0; k < 30 && !$("pageImg").naturalWidth; k++) await new Promise(r => setTimeout(r, 500));
    let peek = "blocked";
    if (window.KHANA_PDF_VIEW) {
      // (pdf.js in a view of its own: the page has no frame at all, and pdf.js's address is not one the page can ask)
      let reach = "refused";
      try { const r = await fetch("khanapdf://pdf/view/bytes"); reach = "ANSWERED " + r.status; } catch (e) { reach = "refused"; }
      await say(`page picture (pdf.js, own view): ${$("pageImg").naturalWidth}×${$("pageImg").naturalHeight} from ${($("pageImg").src || "").slice(0, 5)}; engine ${S.doc.engine}; frames in the page ${document.querySelectorAll("iframe").length}; the page asking pdf.js's address directly: ${reach}`);
    } else {
      try { peek = pdfFrame.contentWindow.document ? "OPEN" : "blocked"; } catch (e) { peek = "blocked"; }
      await say(`page picture (pdf.js): ${$("pageImg").naturalWidth}×${$("pageImg").naturalHeight} from ${($("pageImg").src || "").slice(0, 5)}; engine ${S.doc.engine}; the page sees into pdf.js's frame: ${peek}, contentDocument ${pdfFrame.contentDocument === null ? "null" : "VISIBLE"}`);
    }
    try {
      // (a rough look at the picture: the share of dark pixels in a 4x4 grid, to compare two settings of pdf.js)
      const im = $("pageImg"), cv = document.createElement("canvas");
      cv.width = 400; cv.height = Math.max(1, Math.round(400 * im.naturalHeight / im.naturalWidth));
      const cx = cv.getContext("2d"); cx.drawImage(im, 0, 0, cv.width, cv.height);
      const px = cx.getImageData(0, 0, cv.width, cv.height).data, grid = new Array(16).fill(0), cnt = new Array(16).fill(0);
      for (let y = 0; y < cv.height; y++) for (let x = 0; x < cv.width; x++) {
        const g = Math.min(3, Math.floor(y * 4 / cv.height)) * 4 + Math.min(3, Math.floor(x * 4 / cv.width)), k = (y * cv.width + x) * 4;
        cnt[g]++; if (px[k] < 140) grid[g]++;
      }
      await say("page ink % in a 4x4 grid: " + grid.map((v, k) => (100 * v / cnt[k]).toFixed(1)).join(" "));
    } catch (e) { await say("page ink: " + e); }
    S.view = "text"; syncChrome();
    // the translated PDF, through the dialog as a click would (only English/Dutch documents)
    await say("pdf button shown " + !$("pdfBtn").hidden + ", canTranslate " + S.doc.canTranslate);
    if (S.doc.canTranslate) {
      $("pdfBtn").click();
      document.querySelector("input[name=scope][value=page]").checked = true;
      await say("pdf dialog: " + $("mp3Title").textContent + " / " + $("mp3Start").textContent);
      await $("mp3Start").onclick();
      const t3 = Date.now();
      while (job && job.status === "running" && Date.now() - t3 < 300000) await new Promise(r => setTimeout(r, 1000));
      await say("pdf: " + job.status + " " + (job.error || "") + " saved as " + job.saved + "; dialog says " + $("mp3Text").textContent);
      document.querySelector("#mp3 [data-close]").click();
    }
    // a click on a word reads from its sentence (not the paragraph's start, and without the name)
    {
      await loadPage(1);
      const para = [...document.querySelectorAll("#textView p[data-i]")].find(p => (S.paras[+p.dataset.i].match(/[.!?؟]\s/g) || []).length >= 1);
      if (para) {
        const text = S.paras[+para.dataset.i], cut = text.search(/[.!?؟]\s/) + 2;
        const word = [...para.querySelectorAll(".w")].find(w => +w.dataset.s >= cut);
        word.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 1 }));
        await new Promise(r => setTimeout(r, 1200));
        const q0 = reader.queue[0];
        await say("click: reading starts at " + JSON.stringify(q0 && q0.text.slice(0, 40)) + ", the clicked word " + JSON.stringify(word.textContent)
          + ", second sentence " + JSON.stringify(text.slice(cut, cut + 40)) + ", paragraph " + para.dataset.i + ", cursor " + getComputedStyle(para).cursor);
        stopReading();
      }
    }
    await say("language of the document: " + S.doc.lang);
    // the page's picture follows the page number at once, without waiting for its text
    {
      const wasView = S.view, img = $("pageImg"), before = img.dataset.page;
      S.view = "page"; syncChrome();
      const target = Math.min(S.doc.pages, 7);
      const t0 = Date.now();
      loadPage(target);
      while (img.dataset.page !== String(target) && Date.now() - t0 < 8000) await new Promise(r => setTimeout(r, 50));
      await say("picture follows the page: asked for " + target + ", showing " + img.dataset.page + " after " + (Date.now() - t0) + " ms; dimmed " + img.classList.contains("stale") + "; text there yet: " + (S.page === target && S.paras.length > 0));
      S.view = wasView; syncChrome();
    }
    // pauses follow what the text does
    {
      const ps = piecesFor(1, ["پیش‌گفتار ویراستار", "جمله‌ی اول. آیا این پرسش است؟ بله است.", "بند تازه."], 0);
      await say("pauses: " + ps.map(x => x.text.slice(0, 12) + " " + x.pause).join(" | "));
    }
    // marks with nothing to say: no piece of their own, and no «Decoding failed» if asked for
    {
      const ps = piecesFor(1, ["او رفته است!'\". علامت تعجب."], 0);
      const silent = await speechBuffer("'\".").then(x => x.length + " sample(s)", e => "FAILED " + e.message);
      await say("marks only: pieces " + JSON.stringify(ps.map(x => x.text)) + ", their sound " + silent);
    }
    // reading from the start says the book's name; reading the last page ends with «پایان»
    await loadPage(1); readFrom(0);
    await new Promise(r => setTimeout(r, 1500));
    await say("start of book: first piece " + JSON.stringify(reader.queue[0] && reader.queue[0].text) + ", title " + JSON.stringify(S.doc.title));
    await loadPage(S.doc.pages); readFrom(0);
    const t2 = Date.now();
    while (reader.active && !reader.queue.some(p => p.text === "پایان متن.") && Date.now() - t2 < 900000) await new Promise(r => setTimeout(r, 1000));
    const ended = reader.queue.some(p => p.text === "پایان متن."), n = reader.queue.length;
    // a played sentence is let go: at the end only the last one or two are still held
    while (reader.active && actx.currentTime < reader.lastEnd - 0.5 && Date.now() - t2 < 300000) await new Promise(r => setTimeout(r, 500));
    await say("end of book: «پایان متن» queued " + ended + ", sentences still held at the end " + reader.sources.length + " of " + n + (ended ? "" : "; message on screen: " + JSON.stringify($("toast").textContent) + ", reached piece " + reader.next));
    stopReading();
    let mjob = await jpost("/api/mp3", { doc: S.doc.id, first: Math.min(2, S.doc.pages), last: Math.min(2, S.doc.pages), voice: "khabari", speed: 1.0, bitrate: 64 });
    const t0 = Date.now();
    while (mjob.status === "running" && Date.now() - t0 < 300000) { await new Promise(r => setTimeout(r, 2000)); mjob = await jget("/api/job?id=" + mjob.id); }
    await say("mp3: " + mjob.status + " " + (mjob.error || "") + " saved as " + mjob.saved + " (" + mjob.seconds + " s) in " + Math.round((Date.now() - t0) / 1000) + " s");
    try {
      // a low battery holds a saving job, the charger lets it go on
      await jpost("/api/power?percent=10&charging=0", {});
      const lj = await jpost("/api/mp3", { doc: S.doc.id, first: Math.min(2, S.doc.pages), last: Math.min(2, S.doc.pages), voice: "khabari", speed: 1.0, bitrate: 64 });
      await new Promise(r => setTimeout(r, 5000));
      const held = await jget("/api/job?id=" + lj.id);
      job = lj; saveKind = "mp3"; await poll(); clearTimeout(pollT);
      const said = $("mp3Text").textContent;
      await jpost("/api/power?percent=10&charging=1", {});
      let after = held;
      for (let k = 0; k < 120 && after.status === "running"; k++) { await new Promise(r => setTimeout(r, 500)); after = await jget("/api/job?id=" + lj.id); }
      await say("low battery: job held " + held.lowBattery + " at " + Math.round(held.progress * 100) + "% (" + held.status + "), window says «" + said.slice(0, 40) + "…»; with the charger: " + after.status + ", low " + after.lowBattery);
    } catch (e) { await say("low battery: ERROR " + e); }
    try {
      // the saving window's numbers: the percent with its sign at the right, and the time left from a made-up half-done job
      job = mjob; saveKind = "mp3"; await poll();
      jobClock = { id: "x", t0: Date.now() - 600000 };
      await say("saving window: percent «" + $("mp3Pct").textContent + "» (direction " + getComputedStyle($("mp3Pct")).direction + "), after 10 minutes at 40%: «" + timeLeft(0.4) + "», at 1%: «" + timeLeft(0.01) + "», bar " + $("mp3Prog").getAttribute("aria-valuenow"));
    } catch (e) { await say("saving window: ERROR " + e); }
    const sc = await (await api("/api/selftest_file?name=scan")).blob();
    const t1 = Date.now();
    await openFile(new File([sc], "scan.pdf"));
    await loadPage(1);
    stopReading();
    await say(`scanned book (pdf.js's picture, the app's OCR): name ${S.doc.name}, note «${$("note").hidden ? "" : $("note").textContent.slice(0, 120)}», ${S.doc.pages} pages, scanned ${S.doc.scanned}, page 1 ${S.paras.length} paragraphs, ${S.paras.join(" ").length} letters in ${Math.round((Date.now() - t1) / 1000)} s`);
  } catch (e) { await say("FAILED " + e); }
  const sw = document.querySelector(".sw-yellow");
  try {
      // (the page's own font comes from the program itself, not a file: it must load)
      const ff = await document.fonts.load("16px Vazirmatn", "فارسی");
      await say("page font Vazirmatn: " + ff.length + " face(s), status " + (ff[0] ? ff[0].status : "none") + ", check " + document.fonts.check("16px Vazirmatn", "فارسی"));
    } catch (e) { await say("page font: " + e); }
    await say("policy: " + cspViolations.length + " refused " + JSON.stringify(cspViolations.slice(0, 5))
    + "; styles from app.css: swatch " + (sw && getComputedStyle(sw).backgroundColor) + ", font " + getComputedStyle(document.body).fontFamily.slice(0, 30));
  // an inline script and an inline style must both be refused now
  const before = cspViolations.length;
  const sc = document.createElement("script"); sc.textContent = "window.__inlineRan = true"; document.body.appendChild(sc);
  const d = document.createElement("div"); d.setAttribute("style", "color: red"); document.body.appendChild(d);
  await new Promise(r => setTimeout(r, 300));
  await say("inline script ran " + !!window.__inlineRan + ", inline style applied " + (getComputedStyle(d).color === "rgb(255, 0, 0)") + ", refusals " + (cspViolations.length - before));
  // «فایل‌های من» (only where the app has such a folder; on the Mac with KHANA_LIBRARY_DIR for this test): listed, played, a copy asked for, deleted
  try {
    if (S.library) {
      $("libBtn").click();
      await new Promise(r => setTimeout(r, 800));
      const rows = () => document.querySelectorAll("#libList .librow").length;
      const n0 = rows();
      const first = document.querySelector("#libList .librow");
      first.querySelector("button.primary").click();
      await new Promise(r => setTimeout(r, 2500));
      const au = $("libAudio");
      const playing = !au.paused, dur = au.duration, from = (au.src || "").slice(0, 5);
      const dels = [...first.querySelectorAll("button")].pop();
      dels.click(); const asked = dels.textContent; dels.click();
      await new Promise(r => setTimeout(r, 1200));
      await say("my files: button shown " + !$("libBtn").hidden + ", listed " + n0 + ", first «" + first.querySelector(".name").textContent + "», playing " + playing + " (" + (dur || 0).toFixed(1) + " s, from " + from + "), delete asks «" + asked + "», after it " + rows() + " left, player hidden " + $("libPlayer").hidden);
      $("libPanel").hidden = true;
    } else await say("my files: this build has no such folder (button hidden " + $("libBtn").hidden + ")");
  } catch (e) { await say("my files: ERROR " + e); }
  // «پاک کردن همه‌ی ردها»: only where the test's own data folder is used (the program run from its build folder, not the app)
  try {
    if ((await jget("/api/init")).testData === true) {
      const libBefore = S.library ? (await jget("/api/library")).files.length : -1;
      await jpost("/api/settings", { speed: 1.25 });
      const before = (await jget("/api/init")).settings.speed;
      const r = await jpost("/api/forget", {});
      const after = (await jget("/api/init")).settings.speed;
      await say("forget: button there " + !!$("forgetAll") + ", answered " + JSON.stringify(r) + ", speed setting " + before + " -> " + after + (S.library ? ", my files " + libBefore + " -> " + (await jget("/api/library")).files.length : ""));
    } else await say("forget: not run on the app's real data");
  } catch (e) { await say("forget: ERROR " + e); }
  await say("DONE");
}, 1500));
