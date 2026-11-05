# Khana Reader (خوانا)

Reads books aloud in Farsi and translates foreign books into Farsi, entirely on your own device. Free, with no account, no ads
and no tracking.

<div dir="rtl">

خوانا کتاب‌ها را به فارسی با صدای بلند می‌خواند و کتاب‌های خارجی را به فارسی ترجمه می‌کند؛ همه‌چیز فقط روی دستگاه خود شما انجام می‌شود. رایگان است و حساب کاربری، تبلیغ یا ردیابی ندارد.

</div>

## Download

Get the files from the **[Releases page](../../releases/latest)**.

### Mac

Needs an Apple Silicon Mac (M1 or newer) with macOS 13 or newer and about 4 GB of disk space. 16 GB of memory is recommended,
because translating uses about 7 GB.

1. Download **both** `KhanaReader-Mac.dmg` and `KhanaReader-Mac.002.dmgpart` into the same folder.
2. Double-click the `.dmg` file and drag the app to Applications.
3. The first time, macOS warns you, because this free app is not signed with a paid Apple Developer ID. Try to open the app,
   then go to **System Settings → Privacy & Security** and choose **Open Anyway**.

### Android

Needs an arm64 phone with Android 8 or newer and a recent Android System WebView (version 133 or newer).

1. Download `KhanaReader-Android.apk` (about 1.3 GB) on your phone.
2. Open it and allow installing from this source when Android asks.

> **Please note:** the Android app has not yet been tried on a real phone. If something goes wrong, please open an
> [issue](../../issues). Please don't attach books you would not like to make public.

## What it does

- **Reads aloud** in Farsi with a natural-sounding voice, and saves the reading as an **MP3**.
- **Opens** PDF, EPUB, FB2, PowerPoint (`.pptx`), Word (`.docx`) and plain text (`.txt`, `.md`) files. On the Mac it also opens older
  Word files, RTF, ODT and HTML, through macOS's own converter.
- **Translates** from English, Dutch, German, French, Spanish, Italian, Portuguese, Turkish and Polish into Farsi, reads the
  translation, and can save it as a **Farsi PDF**.
- **Reads scanned pages** with OCR (text recognition).
- **Leaves out what is not the book** as far as it can tell: page numbers, running heads, footnotes and notes sections, indexes,
  imprint pages, and the like.
- **Easy to use while reading:** click any sentence to start reading from there; change the speed and the text size; search; see the page as it is printed.
- **Opens password-protected PDFs:** it asks for the password in a small dialog. The password goes only to the PDF reader inside the
  app, never to the rest of the app, to a file, or anywhere else.
- **Can read in your own voice** («صدای من»), from a recording or an audio file of you. Please use only your own voice: copying
  someone's voice without their consent is forbidden by the speech model's terms.

## How it works

Everything runs on your device; nothing is sent anywhere.

- **The engine is written in Rust.** It works out a page's lines and paragraphs from the PDF reader's pieces of text, decides what is
  body text and what is not, repairs words the PDF broke, translates, and makes the speech.
- **Translation:** Meta's NLLB-200 model, run with CTranslate2 (the 3.3-billion-parameter model on the Mac, the 600-million one on Android).
- **Speech:** a Farsi text-to-speech model (pocket-tts-farsi-v2), run with ONNX Runtime, with rules for numbers, names and pauses so it
  reads like a person.
- **Scanned pages:** a PaddleOCR text recogniser, and on the Mac also Apple's Vision.
- **PDFs** are read by Mozilla's pdf.js, inside a sandboxed frame of the reading screen. **MP3s** are made with LAME.
- **The reading screen is a web page** (HTML, CSS and JavaScript) shown in the system's web view: WebKit on the Mac, Android's
  WebView on the phone. A small native shell around it opens the window and the file pickers.

## What it is written in

| Language | About | What for |
|---|---|---|
| **Rust** | 10,100 lines (75%) | The whole engine: text layout, translation, speech, OCR, MP3 and PDF export |
| **Java** | 1,200 lines (9%) | The Android shell: the web view, file pickers, background jobs |
| **JavaScript** | 1,150 lines (9%) | The reading screen and the PDF reader's frame |
| **HTML and CSS** | 350 lines (2.5%) | The reading screen's look |
| **C, C++, Objective-C** | 270 lines (2%) | Small bridges to native libraries: translation, Apple's Vision and PDF typesetting |
| **Shell** | 200 lines (1.5%) | Building the apps |
| **Swift** | 70 lines | Build helpers for the Android app (not in the apps) |

The counts are for this repository's own code at version 1.0. Third-party code is not counted: pdf.js (JavaScript), LAME (C) and
CTranslate2's headers (C++).

All of the Rust is safe Rust except one folder, `src/ffi`, where each call into a native library is wrapped once, behind a safe
interface; the compiler refuses `unsafe` anywhere else.

## Privacy

- Your books stay on your device, and there is no account, analytics or server.
- The Android app has no internet permission, no storage permission and no microphone permission. Files are opened and saved
  through Android's own screens.
- The text of the book you have open is kept in memory while it is open, because reading aloud, searching and translating need
  it. The app overwrites its own copies when it lets the book go (another book opened, the window closed). It keeps no copy of
  the book on storage: an MP3 or PDF you ask for is made in the app's private storage until you save it where you choose, and on
  the Mac a Word or RTF file is copied for a moment to a temporary file for Apple's converter and deleted at once. Copies inside
  the web view's page and Android's Java layer are out of the app's reach, and Android keeps other apps out of this app's memory.
- On the Mac, the app runs in Apple's App Sandbox. It holds Apple's "outgoing network" permission only because Apple's WebKit
  needs it to start its own helper processes (the app does not open without it); the program has no network code and its page
  refuses every outside address.
- The safety design follows [GrapheneOS's PDF Viewer](https://github.com/GrapheneOS/PdfViewer): a strict content policy, no
  JavaScript evaluation, no fonts loaded by the browser, a cap on the size of a drawn page, a minimum web view version, and
  reproducible Android builds. See the [credits](THIRD_PARTY_NOTICES.md).

## Limits

**Translation and speech**
- Translation is done by a machine and makes mistakes, especially with names, old spelling, poetry and technical terms. Don't rely on it where accuracy matters.
- The voice speaks Farsi only. Foreign text is translated first. Names and foreign words may be pronounced wrongly.
- The reader uses general rules; it has no corrections made for particular books, so some books will read better than others.

**Documents**
- How well a book reads depends on the file. PDFs with broken fonts, bad scans, unusual layouts, or notes that look like body text can come out wrong: words run together or split, notes read aloud, or parts left out.
- Scanned pages depend on OCR, which makes mistakes, and Android's OCR is weaker than the Mac's.
- One document is open at a time, up to 300 MB.
- Older Word, RTF, ODT and HTML files open on the Mac only.

**Mac**
- Apple Silicon only (no Intel Macs), macOS 13 or newer, about 4 GB of disk and about 7 GB of memory while translating. Translating is slower on a Mac with 8 GB.
- So far tested on one Mac only (M2 Pro, macOS 27).

**Android**
- arm64 phones only, Android 8 or newer, with Android System WebView 133 or newer.
- **Not yet tried on a real phone.**
- It uses the smaller translation model, so its translations have more errors.
- It cannot open a PDF from another app's "Open with"; open files from inside the app.
- You can choose an audio file for your own voice, but there is no recording button.

**Everything**
- The app never updates itself; download a new release to update.
- The translation and speech models are for **non-commercial use only**, so the apps may not be sold.

<div dir="rtl">

## درباره‌ی برنامه و محدودیت‌ها

- ترجمه ماشینی است و اشتباه دارد، به‌ویژه در نام‌ها، املای قدیمی، شعر و واژه‌های تخصصی. جایی که دقت مهم است به آن تکیه نکنید.
- صدای برنامه فقط فارسی است؛ متن خارجی ابتدا ترجمه می‌شود. نام‌ها و واژه‌های خارجی ممکن است درست خوانده نشوند.
- کیفیت خواندن به خود فایل بستگی دارد. PDFهای با فونت خراب، اسکن‌های بد، صفحه‌آرایی‌های غیرمعمول، یا پانوشت‌هایی که شبیه متن اصلی‌اند ممکن است درست خوانده نشوند.
- هر بار فقط یک سند باز است و اندازه‌اش تا ۳۰۰ مگابایت می‌تواند باشد.
- مک: فقط مک‌های اپل‌سیلیکون (نه اینتل)، macOS 13 یا جدیدتر، و برای ترجمه حدود ۷ گیگابایت حافظه.
- اندروید: هنوز روی گوشی واقعی آزمایش نشده است، مدل ترجمه‌ی کوچک‌تری دارد، و نمی‌تواند PDF را از «باز کردن با» برنامه‌های دیگر بگیرد.
- مدل‌های ترجمه و گفتار فقط برای استفاده‌ی غیرتجاری‌اند؛ پس برنامه را نمی‌توان فروخت.
- برنامه به اینترنت وصل نمی‌شود و کتاب‌های شما فقط روی دستگاه خودتان می‌مانند.

</div>

## Licence

Our own code is under the [MIT licence](LICENSE). The translation and speech models are for **non-commercial use only**, so the
complete apps may not be sold. See [credits and licences](THIRD_PARTY_NOTICES.md), including our thanks to GrapheneOS.

## Building from source

The models and prebuilt libraries are too large for this repository; the credits list where each comes from.
`tools/make_app.sh` builds the Mac app and `android/build.sh` builds the Android app. A release of your own needs your own
Android signing key. The code has unit tests (`cargo test --lib`) and a self-test that runs the real window
(`KHANA_SELFTEST=<file>`).
