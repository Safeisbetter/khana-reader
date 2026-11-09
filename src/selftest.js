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
    const r = await api("/api/tts", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ text: "سلام، این یک آزمایش است.", voice: "khabari", speed: 1.0 }) });
    const buf = await ensureAudio().decodeAudioData(await r.arrayBuffer());
    await say("speech: " + buf.duration.toFixed(2) + " s decoded, audio context " + actx.state);
    S.view = "page"; syncChrome(); renderPageImage();
    for (let k = 0; k < 30 && !$("pageImg").naturalWidth; k++) await new Promise(r => setTimeout(r, 500));
    let peek = "blocked";
    try { peek = pdfFrame.contentWindow.document ? "OPEN" : "blocked"; } catch (e) { peek = "blocked"; }
    await say(`page picture (pdf.js): ${$("pageImg").naturalWidth}×${$("pageImg").naturalHeight} from ${($("pageImg").src || "").slice(0, 5)}; engine ${S.doc.engine}; the page sees into pdf.js's frame: ${peek}, contentDocument ${pdfFrame.contentDocument === null ? "null" : "VISIBLE"}`);
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
    const sc = await (await api("/api/selftest_file?name=scan")).blob();
    const t1 = Date.now();
    await openFile(new File([sc], "scan.pdf"));
    await loadPage(1);
    stopReading();
    await say(`scanned book (pdf.js's picture, the app's OCR): ${S.doc.pages} pages, scanned ${S.doc.scanned}, page 1 ${S.paras.length} paragraphs, ${S.paras.join(" ").length} letters in ${Math.round((Date.now() - t1) / 1000)} s`);
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
  await say("DONE");
}, 1500));
