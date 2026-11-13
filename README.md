# Khana Reader (خوانا)

Reads books aloud in Farsi and translates foreign books into Farsi, entirely on your own device. Free, with no account, no ads,
no tracking and no internet connection. For **Mac, Android and Windows** (Windows is experimental).

<div dir="rtl">

خوانا کتاب‌ها را به فارسی با صدای بلند می‌خواند و کتاب‌های خارجی را به فارسی ترجمه می‌کند؛ همه‌چیز فقط روی دستگاه خود شما انجام می‌شود. رایگان است و حساب کاربری، تبلیغ، ردیابی و اتصال به اینترنت ندارد. برای **مک، اندروید و ویندوز** (نسخه‌ی ویندوز آزمایشی است).

**[راهنمای فارسی پایین همین صفحه است.](#راهنمای-فارسی)**

</div>

## Download

Get the files from the **[Releases page](../../releases/latest)**. Every platform has a small and a large download, because for
many readers every megabyte is expensive. **If your connection is slow or costly, take the light one.**

| Device | File | Size | What it has |
|---|---|---|---|
| Android, light | `KhanaReader-Android-lite.apk` | about 0.26 GB | Reads Farsi books, pictures and typed text aloud; saves sound. **No translation.** |
| Android, full | `KhanaReader-Android.apk` | about 0.86 GB | The same, and translates foreign books into Farsi. |
| Mac, light | `KhanaReader-Mac-lite.dmg` | about 0.45 GB, one file | Reads Farsi aloud, saves sound, reads scanned pages. **No translation, no dictation.** |
| Mac, medium | `KhanaReader-Mac-medium.dmg` | about 1 GB, one file | The same, and translates with the smaller model (the one the phone uses). |
| Mac, full | `KhanaReader-Mac.dmg` **and** every `KhanaReader-Mac.00x.dmgpart` | about 5 GB, three files | The best translation model and dictation (speech to text). |
| Windows, light (experimental) | `KhanaReader-Windows-light-experimental.zip` | about 0.2 GB | No translation. |
| Windows, full (experimental) | `KhanaReader-Windows-full-experimental.zip` | about 0.75 GB | With translation (the smaller model). |

**Mac:** an Apple Silicon Mac (M1 or newer) with macOS 13 or newer. For the full download put all three files in one folder, then
open the `.dmg`. Drag the app to Applications. The first time, macOS warns you, because this free app is not signed with a paid
Apple Developer ID: try to open the app, then go to **System Settings → Privacy & Security** and choose **Open Anyway**.
The full version needs about 7 GB of disk and 16 GB of memory is recommended; the light one runs on any Apple Silicon Mac.

**Android, for speed:** keep Khana on the screen while it reads or saves a file (the screen stays on by itself). Behind another app, or with
the screen off, Android leaves it only the slow processor cores and the work takes several times longer.

**Android:** an arm64 phone with Android 8 or newer and Android System WebView 133 or newer. Open the file and allow installing
from this source when Android asks. Both versions are signed with the same key, so one can be installed over the other.

**Windows:** Windows 10 or 11 (64-bit) with Microsoft's Edge WebView2 (Windows 11 has it). Unzip the whole folder and double-click
`KhanaReader.exe`. It saves WAV instead of MP3 and has no translated PDF yet.

> **Please note, honestly:** the Mac app is the only one used every day by a person. The Android app has not yet been tried on a
> real phone (it has been run and timed on one tablet), and the Windows app has only been run by GitHub's own build computers. If something goes wrong, please open an
> [issue](../../issues). Don't attach books you would not like to make public.

## Check that your download is the real one

1. **Download only from this GitHub page** (`github.com/Safeisbetter/khana-reader`), not from channels, groups or other sites.
2. **Compare the file's SHA-256 with `SHA256SUMS.txt` of the same release.** On a Mac or Linux: `shasum -a 256 FILE`. On Windows
   (PowerShell): `Get-FileHash FILE`. The two codes must be exactly the same.
3. **Android:** every APK of ours is signed with one key. Its certificate's SHA-256 fingerprint is
   `32:44:BB:08:80:95:34:D3:42:78:5A:FD:CA:E9:C8:16:03:B9:B2:63:FC:7B:C2:3D:E5:78:32:D5:C1:D9:E0:F9`
   (`apksigner verify --print-certs FILE.apk` shows it). Android refuses to install an update signed with another key.

## Staying safe with books and PDFs

**Where a book comes from matters more than the app that reads it.**

- A site or channel you download from can see **that you downloaded** a book, and when. The app cannot change that. Get books from
  sources you know and trust, and where it matters, over a connection you trust.
- A file called a "book" may be something else: a program (`.apk`, `.exe`, `.scr`, `.dmg`) or a PDF built to attack the program
  that opens it. **Never install an "app needed to read this book".** Khana opens books; it never asks you to install anything.
- Any reading app that asks for the internet, your contacts or all your files deserves suspicion. Khana asks for none of them.

**What Khana does when a PDF is bad, broken or full of junk:**

- **Nothing in a PDF is ever run.** Scripts, forms, attachments and "launch" actions inside a PDF are ignored. Links are not
  opened: the app cannot open a web address at all.
- **Nothing can be sent out.** The Android app has no internet permission; on every platform the app has no network code, and its
  reading screen refuses every outside address. A hostile PDF cannot "phone home" through the app.
- **The PDF is read in a locked room.** PDFs are opened by Mozilla's pdf.js, kept up to date, in a sandbox with no permissions. On
  the Mac it runs in a separate process with its own address, cut off from the reading screen and from your saved corrections.
- **Limits protect the device:** a page that claims to be enormous is drawn smaller, huge pictures are refused, and a file over
  300 MB is not opened.
- **Advertising and junk text** (site addresses stamped on every page, "downloaded from…" lines) are left out of the reading where
  the app can tell; you can remove any paragraph yourself with «✎ ویرایش متن».
- **Honest limit:** no program can promise that a hostile file is harmless. Keep your system and (on Android) "Android System
  WebView" updated, and don't open files from people you have reason to fear.

**What stays on your device, and how to remove it:**

- The app keeps only: your settings, your own corrections to a book's text (they contain sentences of the book), your list of
  pronunciation fixes, and your recorded voice if you made one. Nothing else about a book is stored.
- **Settings → «پاک کردن همه‌ی ردها» (delete all traces)** removes all of that at once. The app also clears its unfinished
  temporary sound files every time it starts.
- **MP3, WAV and PDF files you saved, and the books themselves, are your own files:** the app does not delete them. Delete them
  yourself if they should not be found.
- Use a screen lock and your device's encryption. On Android, [GrapheneOS](https://grapheneos.org) gives the strongest protection we know of.

**What another app on the phone (a store app, for example) can see of Khana.** It cannot see what you read: the open book, your
corrections, settings and recorded voice are in Khana's private folder; by default no app can photograph or record Khana's screen, and
none may record the sound it plays. On Android a sound file Khana makes **stays inside the app** ("🎧 فایل‌های من"), is played there, and no
other app can read it. Another app **can** see that Khana is installed, and it can read **a copy you export into shared storage, or a PDF
you save there** (name and contents) if it holds the storage or audio permission, more widely on Android 10 and older; the
suggested file name comes from the book's name, so rename it when saving. On Android 9 and older any app can read the clipboard, so
avoid "copy page text" for sensitive text there. What you type passes through the phone's keyboard app, and an app you gave the
Accessibility permission can read the screen. A phone that no longer gets security updates protects all of this less.

**Cafe Bazaar and other Iranian app stores.** Khana is **not** published on Cafe Bazaar, Myket or any other store; this GitHub page
is its only official source, and a copy found elsewhere is not ours. We chose this because of what Bazaar's own documents say (read
November 2026): an account is tied to a phone number or email; its [privacy policy](https://cafebazaar.ir/privacy?l=en) lists the
device ID, advertising ID, **the list of installed apps**, searches and IP address among what it collects, and says information may be
shared where the law requires or permits, including under court orders; and apps must comply with the law of the Islamic Republic.
We found no independent, recent evidence that Bazaar has tampered with apps or handed over a particular user's data, and we do not
repeat claims we could not source. For automatic updates without a store account, [Obtainium](https://github.com/ImranR98/Obtainium)
can follow this repository's releases (we did not make or audit it). The Farsi guide below says more.

## What it does

- **Reads aloud** in Farsi with a natural-sounding voice (six to choose from; five were donated to the public domain by their owners), and saves the reading as **MP3** or **WAV** (on the Mac also **M4A**).
- **Reads a text you type or paste** («✎ نوشتن متن»), and reads the text of **pictures** (a photo of a page, a screenshot) with OCR.
- **Lets you correct the text in the page itself:** «✎ ویرایش متن» makes every paragraph a box you can type in, select in and
  delete from; a word can be changed or removed in the whole book. Corrections reach the reading, the MP3 and the PDF.
- **Opens** PDF, EPUB, FB2, PowerPoint (`.pptx`), Word (`.docx`) and plain text. On the Mac also older Word files, RTF, ODT and HTML.
- **Translates** from English, Dutch, German, French, Spanish, Italian, Portuguese, Turkish and Polish into Farsi, reads the
  translation, and can save it as a **Farsi PDF** (not in the light versions).
- **Reads scanned pages** with OCR, and **opens password-protected PDFs** (the password is used to open the file and is never
  written to storage or sent anywhere).
- **Leaves out what is not the book** as far as it can tell: page numbers, running heads, footnotes, indexes and the like.
- **Can read in your own voice** («صدای من»). Please use only your own voice: copying someone's voice without their consent is
  forbidden by the speech model's terms.

## Made to be usable without good eyesight

- Large buttons and text; text size up to 72; seven looks to choose from (light, dark, a warm night light, black, black-and-yellow, and two glass looks); follows the system's
  "increase contrast" and "reduce motion" settings.
- Everything works from the keyboard: <kbd>Space</kbd> read or pause, <kbd>←</kbd> <kbd>→</kbd> pages, <kbd>⌘[</kbd> <kbd>⌘]</kbd> speed, <kbd>⌘,</kbd> settings.
- Built for screen readers (VoiceOver, TalkBack): buttons are named, messages and page changes are announced, and a window that
  opens takes the focus and gives it back when it closes. This has not yet been checked by a blind reader: please tell us what fails.
- The word being read is marked, and the reading can start from any sentence you click.

## How it works

Everything runs on your device; nothing is sent anywhere.

- **The engine is written in Rust.** It works out a page's lines and paragraphs, decides what is body text, repairs words the PDF
  broke, translates, and makes the speech. All of it is safe Rust except one folder, `src/ffi`, where each call into a native
  library is wrapped once; the compiler refuses `unsafe` anywhere else.
- **Translation:** Meta's NLLB-200, run with CTranslate2 (the 3.3-billion-parameter model in the full Mac app, the 600-million one elsewhere).
- **Speech:** a Farsi text-to-speech model (pocket-tts-farsi-v2), run with ONNX Runtime (its usage reporting to Microsoft is switched
  off, and could not work without a network anyway), with rules for numbers, names and pauses.
- **Scanned pages:** a PaddleOCR recogniser, and on the Mac also Apple's Vision. **Dictation** (Mac, full): OpenAI's Whisper, on the device.
- **PDFs:** Mozilla's pdf.js. **MP3:** LAME.
- **The reading screen is a web page** shown in the system's web view (WebKit on the Mac, Android's WebView, WebView2 on Windows),
  with a strict content policy: no outside address, no inline script, no JavaScript evaluation.
- The models are data files (weights and tables), not programs. Every part and where it comes from is listed in the
  [credits](THIRD_PARTY_NOTICES.md). No part of the app is fetched from a server when it runs.

## Privacy

- Your books stay on your device. There is no account, no analytics and no server, anywhere.
- The Android app has no internet, no storage and no microphone permission. Files are opened and saved through Android's own screens.
- The text of the open book is kept in memory while it is open and overwritten when the book is closed.
- **App lock** (Settings, «قفل برنامه»), like the screen lock of Signal and off unless you turn it on: the app asks for a password
  you chose when it starts and after the time you pick without being touched (1 minute to 1 hour, or only at start). While it is
  locked the app itself refuses every request, not just the page. The password is never kept: only a random salt and its
  PBKDF2-HMAC-SHA256 hash (300,000 rounds) are stored, in a file only your account can read. After five wrong tries you must wait
  (30 seconds, then longer, up to an hour), and the wait survives a restart. **It is a lock on using the app on a device that is
  switched on and open; it does not encrypt the app's files** (use FileVault, or the phone's own encryption, for that). A
  forgotten password cannot be recovered: the lock screen offers «erase everything and start again», which removes the app's
  data (edits, your recorded voice, settings, sound files kept inside the app) and the lock itself, and leaves the books and
  files you keep elsewhere alone.
- On the Mac the app runs in Apple's App Sandbox. It holds Apple's "outgoing network" permission only because Apple's WebKit
  needs it to start its own helper processes (the app does not open without it); the program has no network code and its page
  refuses every outside address.
- The safety design follows [GrapheneOS's PDF Viewer](https://github.com/GrapheneOS/PdfViewer): a strict content policy, no
  JavaScript evaluation, no fonts loaded by the browser, a cap on the size of a drawn page, a minimum web view version, and
  reproducible Android builds. GrapheneOS does not endorse or support this app.
- Only the owner of this repository can change it. Others can only propose changes, and nothing enters without the owner's review.

## Limits

- **Translation is done by a machine and makes mistakes,** especially with names, old spelling, poetry and technical terms.
- **The voice speaks Farsi only.** Farsi writing leaves out short vowels, so the app has to guess how a rare word or a name
  sounds, and sometimes guesses wrong. A foreign name in Latin letters is read by the English dictionary, by Dutch rules in a
  Dutch book, and otherwise by plain letter rules. You can fix any word for good: select it, choose «اصلاح تلفظ».
- **How well a book reads depends on the file.** PDFs with broken fonts, bad scans or unusual layouts can come out wrong.
- **Dictation and voice editing** (Mac) hear clear, slow speech best and make mistakes with names and in noise. Check what was written.
- One document at a time, up to 300 MB. The app never updates itself: download a new release to update.
- The translation and speech models are for **non-commercial use only**, so the apps may not be sold.

<div dir="rtl">

## راهنمای فارسی

### چه چیزی را دانلود کنم؟

فایل‌ها در صفحه‌ی **[Releases](../../releases/latest)** هستند. برای هر دستگاه یک نسخه‌ی کوچک و یک نسخه‌ی بزرگ هست، چون اینترنت برای خیلی‌ها گران است. **اگر اینترنت شما کند یا گران است، نسخه‌ی سبک را بگیرید.**

| دستگاه | فایل | حجم | چه دارد |
|---|---|---|---|
| اندروید، سبک | `KhanaReader-Android-lite.apk` | حدود ۰٫۲۶ گیگابایت | کتاب فارسی، عکس و متن تایپ‌شده را می‌خواند و صدا ذخیره می‌کند. **بدون ترجمه.** |
| اندروید، کامل | `KhanaReader-Android.apk` | حدود ۰٫۸۶ گیگابایت | همان، به‌اضافه‌ی ترجمه‌ی کتاب‌های خارجی به فارسی. |
| مک، سبک | `KhanaReader-Mac-lite.dmg` | حدود ۰٫۴۵ گیگابایت، یک فایل | فارسی را می‌خواند، صدا ذخیره می‌کند، صفحه‌ی اسکن‌شده را می‌خواند. **بدون ترجمه و بدون تبدیل گفتار به نوشته.** |
| مک، متوسط | `KhanaReader-Mac-medium.dmg` | حدود ۱ گیگابایت، یک فایل | همان، به‌اضافه‌ی ترجمه با مدل کوچک‌تر (همان مدل گوشی). |
| مک، کامل | `KhanaReader-Mac.dmg` **و** همه‌ی فایل‌های `KhanaReader-Mac.00x.dmgpart` | حدود ۵ گیگابایت، سه فایل | بهترین مدل ترجمه و تبدیل گفتار به نوشته. |
| ویندوز، سبک (آزمایشی) | `KhanaReader-Windows-light-experimental.zip` | حدود ۰٫۲ گیگابایت | بدون ترجمه. |
| ویندوز، کامل (آزمایشی) | `KhanaReader-Windows-full-experimental.zip` | حدود ۰٫۷۵ گیگابایت | با ترجمه (مدل کوچک‌تر). |

- **مک:** مک با تراشه‌ی اپل (M1 یا جدیدتر) و macOS 13 یا جدیدتر. برای نسخه‌ی کامل هر سه فایل را در یک پوشه بگذارید و بعد `.dmg` را باز کنید. بار اول macOS هشدار می‌دهد، چون این برنامه‌ی رایگان امضای پولی اپل ندارد: برنامه را باز کنید، بعد در **System Settings → Privacy & Security** گزینه‌ی **Open Anyway** را بزنید.
- **اندروید، برای سرعت:** تا وقتی خوانا می‌خواند یا فایل می‌سازد، آن را روی صفحه نگه دارید (صفحه خودش روشن می‌ماند). اگر برنامه‌ی دیگری باز کنید یا صفحه خاموش شود، اندروید فقط هسته‌های کُند پردازنده را به آن می‌دهد و کار چند برابر طول می‌کشد.
- **اندروید:** گوشی ۶۴ بیتی (arm64) با اندروید ۸ یا بالاتر و «Android System WebView» نسخه‌ی ۱۳۳ یا جدیدتر. هنگام نصب، اندروید اجازه‌ی «نصب از این منبع» را می‌خواهد.
- **ویندوز:** ویندوز ۱۰ یا ۱۱ (۶۴ بیتی). پوشه را کامل از حالت فشرده درآورید و `KhanaReader.exe` را باز کنید.

> **صادقانه:** فقط نسخه‌ی مک هر روز به دست یک انسان استفاده می‌شود. نسخه‌ی اندروید هنوز روی گوشی واقعی آزمایش نشده و نسخه‌ی ویندوز فقط روی رایانه‌های ساخت GitHub اجرا شده است. اگر مشکلی دیدید در بخش [Issues](../../issues) بنویسید؛ کتابی را که نمی‌خواهید همه ببینند پیوست نکنید.

### چطور مطمئن شوم فایل دست‌کاری نشده است؟

۱. **فقط از همین صفحه‌ی GitHub دانلود کنید** (نشانی `github.com/Safeisbetter/khana-reader`)، نه از کانال، گروه یا سایت دیگر.

۲. **کد SHA-256 فایل را با فایل `SHA256SUMS.txt` همان نسخه مقایسه کنید.** در مک یا لینوکس: `shasum -a 256 نام‌فایل` · در ویندوز (PowerShell): `Get-FileHash نام‌فایل`. دو کد باید کاملاً یکی باشند.

۳. **اندروید:** همه‌ی فایل‌های APK ما با یک کلید امضا شده‌اند. اثر انگشت (SHA-256) گواهی آن:

`32:44:BB:08:80:95:34:D3:42:78:5A:FD:CA:E9:C8:16:03:B9:B2:63:FC:7B:C2:3D:E5:78:32:D5:C1:D9:E0:F9`

اگر این کد فرق داشت، فایل از ما نیست. اندروید هم اجازه نمی‌دهد نسخه‌ای با کلید دیگر روی این برنامه نصب شود.

### کتاب و PDF را چطور امن‌تر بگیریم؟

**این‌که کتاب از کجا می‌آید از برنامه‌ای که آن را می‌خواند مهم‌تر است.**

- سایت یا کانالی که از آن دانلود می‌کنید می‌تواند ببیند **که شما چه کتابی را چه وقت گرفته‌اید.** برنامه این را نمی‌تواند عوض کند. کتاب را از منبعی بگیرید که می‌شناسید و به آن اعتماد دارید، و هر جا مهم است، با اتصالی که به آن اعتماد دارید.
- فایلی که اسمش «کتاب» است ممکن است چیز دیگری باشد: یک برنامه (`.apk`، `.exe`، `.scr`، `.dmg`) یا PDFی که برای حمله به برنامه‌ی خواننده ساخته شده است. **هرگز «برنامه‌ای که برای خواندن این کتاب لازم است» نصب نکنید.** خوانا کتاب را باز می‌کند و هیچ‌وقت از شما نمی‌خواهد چیزی نصب کنید.
- هر برنامه‌ی کتاب‌خوانی که اینترنت، مخاطبان یا دسترسی به همه‌ی فایل‌های شما را می‌خواهد مشکوک است. خوانا هیچ‌کدام را نمی‌خواهد.

**اگر یک PDF خراب، آلوده یا پر از تبلیغ باشد، خوانا چه می‌کند؟**

- **هیچ چیزِ درون PDF اجرا نمی‌شود.** اسکریپت، فرم، پیوست و دستورهای «اجرا» در PDF نادیده گرفته می‌شوند. لینک‌ها باز نمی‌شوند: برنامه اصلاً نمی‌تواند نشانی اینترنتی باز کند.
- **هیچ چیز بیرون فرستاده نمی‌شود.** برنامه‌ی اندروید اجازه‌ی اینترنت ندارد؛ در همه‌ی دستگاه‌ها برنامه کد شبکه ندارد و صفحه‌ی آن هر نشانی بیرونی را رد می‌کند. یک PDF آلوده نمی‌تواند از راه این برنامه با جایی تماس بگیرد.
- **PDF در یک اتاق قفل‌شده خوانده می‌شود.** PDF را pdf.js (ساخت موزیلا، به‌روز) در محیطی جداشده و بدون هیچ مجوزی باز می‌کند. در مک، این بخش در پردازه‌ای جدا اجرا می‌شود و به صفحه‌ی خواندن و ویرایش‌های شما دسترسی ندارد.
- **محدودیت‌ها از دستگاه محافظت می‌کنند:** صفحه‌ای که ادعا می‌کند بسیار بزرگ است کوچک‌تر کشیده می‌شود، تصویر خیلی بزرگ پذیرفته نمی‌شود و فایل بزرگ‌تر از ۳۰۰ مگابایت باز نمی‌شود.
- **تبلیغ و متن اضافه** (نشانی سایت روی هر صفحه، خط «دانلود شده از…») تا جایی که برنامه تشخیص دهد خوانده نمی‌شود؛ هر بند را هم خودتان می‌توانید با «✎ ویرایش متن» حذف کنید.
- **محدودیت، صادقانه:** هیچ برنامه‌ای نمی‌تواند قول بدهد که یک فایل دشمنانه بی‌خطر است. سیستم‌عامل و (در اندروید) «Android System WebView» را به‌روز نگه دارید و فایلِ کسی را که از او می‌ترسید باز نکنید.

**چه چیزی روی دستگاه می‌ماند و چطور پاک می‌شود؟**

- برنامه فقط این‌ها را نگه می‌دارد: تنظیمات، ویرایش‌های خود شما در متن کتاب (که جمله‌هایی از کتاب در آن‌هاست)، فهرست اصلاح تلفظ، و صدای ضبط‌شده‌ی شما اگر ساخته باشید. هیچ چیز دیگری از کتاب ذخیره نمی‌شود.
- **تنظیمات ← «پاک کردن همه‌ی ردها»** همه‌ی این‌ها را یک‌جا پاک می‌کند. برنامه هر بار که باز می‌شود فایل‌های صوتی نیمه‌کاره‌ی موقت خودش را هم پاک می‌کند.
- **فایل‌های MP3، WAV و PDF که ذخیره کرده‌اید، و خودِ کتاب‌ها، فایل‌های خود شما هستند:** برنامه آن‌ها را پاک نمی‌کند. اگر نباید پیدا شوند، خودتان پاکشان کنید.
- قفل صفحه و رمزگذاری دستگاه را روشن نگه دارید. در اندروید، [GrapheneOS](https://grapheneos.org) قوی‌ترین محافظتی است که می‌شناسیم. (GrapheneOS این برنامه را تأیید یا پشتیبانی نمی‌کند.)

### کافه‌بازار و فروشگاه‌های داخلی

**خوانا در کافه‌بازار، مایکت یا هیچ فروشگاه دیگری منتشر نشده است.** تنها جای رسمی آن همین صفحه‌ی GitHub است. اگر جایی برنامه‌ای با این نام دیدید، از ما نیست؛ ممکن است دست‌کاری شده باشد. پیش از نصب، اثر انگشت امضا را که بالاتر آمده مقایسه کنید.

چرا خوانا را در بازار نگذاشته‌ایم؟ این‌ها را از نوشته‌های خودِ بازار برداشته‌ایم (نوامبر ۲۰۲۶)، نه از شنیده‌ها:

- **ورود با شماره‌ی تلفن یا ایمیل است.** طبق [سیاست حریم خصوصی بازار](https://cafebazaar.ir/privacy?l=en) برای ساختن حساب شماره‌ی موبایل یا ایمیل لازم است و ورود با رمز یک‌بارمصرف انجام می‌شود. اگر سیم‌کارت به نام شما ثبت شده باشد، حسابی که با آن شماره ساخته می‌شود می‌تواند به هویت شما وصل شود.
- **بازار می‌داند چه برنامه‌هایی دارید.** همان سیاست می‌گوید این‌ها جمع‌آوری می‌شود: شناسه‌ی دستگاه، شناسه‌ی تبلیغاتی، **فهرست برنامه‌های نصب‌شده**، جستجوها، نشانی IP و زمان استفاده.
- **داده‌ها می‌تواند به مراجع قانونی داده شود.** همان سیاست می‌گوید بازار ممکن است اطلاعات را هر جا قانون لازم بداند یا اجازه دهد، از جمله با حکم دادگاه، در اختیار بگذارد. مدت نگهداری داده‌ها در آن نیامده است.
- **برنامه‌ها باید با قانون جمهوری اسلامی سازگار باشند** ([شرایط بازار](https://cafebazaar.ir/press/agreement?l=en)). برنامه‌ای که کتاب‌های هر کس را، هر چه باشد، می‌خواند و ترجمه می‌کند، ممکن است برداشته شود یا از ما بخواهند آن را محدود کنیم.
- **ما کنترلی بر فایل نخواهیم داشت.** در یک فروشگاه، فایل و به‌روزرسانی از سرور آن فروشگاه می‌آید. در GitHub می‌توانید کد SHA-256 و امضای فایل را خودتان با آنچه ما منتشر کرده‌ایم مقایسه کنید.

**آنچه نمی‌دانیم، صادقانه:** مدرک مستقل و تازه‌ای پیدا نکردیم که نشان دهد بازار برنامه‌ها را دست‌کاری کرده یا داده‌ی کاربری مشخصی را به کسی داده است. بعضی راهنماهای امنیت دیجیتال چنین چیزی می‌گویند اما منبعی برایش نیاورده‌اند، پس ما آن را تکرار نمی‌کنیم. نگرانی ما از چیزهایی است که خود بازار نوشته: شماره‌ی تلفن، فهرست برنامه‌ها، و تحویل داده به حکم قانون.

**توصیه برای کسی که امنیتش مهم است:**

- خوانا را فقط از همین صفحه بگیرید و امضا و SHA-256 را بررسی کنید.
- اگر به‌روزرسانی خودکار می‌خواهید، برنامه‌ی آزاد [Obtainium](https://github.com/ImranR98/Obtainium) نسخه‌های تازه را مستقیم از GitHub می‌گیرد: نشانی `github.com/Safeisbetter/khana-reader` را در آن وارد کنید. (Obtainium را ما نساخته‌ایم و بررسی نکرده‌ایم.)
- فهرست برنامه‌های گوشی می‌تواند درباره‌ی شما چیزهایی بگوید. هر فروشگاهی که روی گوشی نصب است، چه داخلی چه خارجی، معمولاً این فهرست را می‌بیند.

### برنامه‌های دیگرِ گوشی (مثلاً یک فروشگاه) چه چیزی از خوانا می‌بینند؟

کوتاه: **آنچه می‌خوانید را نمی‌بینند؛ اما فایل‌هایی را که در حافظه‌ی مشترک گوشی ذخیره می‌کنید ممکن است ببینند.** به‌ویژه روی گوشی‌های قدیمی.

**آنچه برنامه‌ی دیگر نمی‌تواند ببیند** (مگر گوشی روت یا آلوده باشد):

- کتابی که باز کرده‌اید، متن آن، ویرایش‌ها، تنظیمات و صدای ضبط‌شده‌ی شما. این‌ها در پوشه‌ی خصوصی خوانا هستند و اندروید دست برنامه‌های دیگر را از آن کوتاه می‌کند.
- صفحه‌ی خوانا. به‌طور پیش‌فرض هیچ برنامه‌ای نمی‌تواند از آن عکس یا فیلم بگیرد و در فهرست برنامه‌های اخیر هم متن دیده نمی‌شود (در تنظیمات می‌شود این را برداشت).
- صدایی که خوانا پخش می‌کند: برنامه‌های دیگر اجازه‌ی ضبط آن را ندارند.
- خوانا اینترنت ندارد؛ پس خودش هم چیزی به جایی نمی‌فرستد.

**آنچه برنامه‌ی دیگر می‌تواند ببیند:**

- **این‌که خوانا روی گوشی نصب است.** هر فروشگاه و بسیاری از برنامه‌ها فهرست برنامه‌های نصب‌شده را می‌بینند. این را نمی‌شود پنهان کرد.
- **رونوشتی که از فایل‌ها بیرون می‌برید.** در اندروید، فایل صدایی که خوانا می‌سازد داخل خودِ برنامه می‌ماند («🎧 فایل‌های من») و همان‌جا پخش می‌شود؛ هیچ برنامه‌ی دیگری آن را نمی‌بیند. اما اگر با «ذخیره در گوشی…» رونوشتی در حافظه‌ی مشترک (مثل پوشه‌ی Download یا Music) بگذارید، یا یک PDF ذخیره کنید: هر برنامه‌ای که «اجازه‌ی حافظه» یا «دسترسی به موسیقی و صدا» دارد می‌تواند **نام و محتوای** آن‌ها را بخواند. نام پیشنهادی فایل از نام کتاب ساخته می‌شود. روی اندروید ۱۰ و قدیمی‌تر این دسترسی گسترده‌تر است.
- **متنی که با «کپی متن صفحه» کپی می‌کنید.** روی اندروید ۹ و قدیمی‌تر هر برنامه‌ای می‌تواند پنهانی حافظه‌ی کپی (کلیپ‌بورد) را بخواند.
- **آنچه تایپ می‌کنید** (متنی که می‌نویسید، رمز PDF) از صفحه‌کلید گوشی می‌گذرد. صفحه‌کلیدی که خودتان نصب کرده‌اید می‌تواند آن را ببیند.
- برنامه‌ای که به آن «دسترس‌پذیری» (Accessibility) داده‌اید می‌تواند هر چه روی صفحه است بخواند. این اجازه را به برنامه‌ای که نمی‌شناسید ندهید.

**توصیه:**

- فایل صدا را داخل خوانا نگه دارید و همان‌جا گوش کنید. اگر رونوشتی بیرون می‌برید، **نام فایل را عوض کنید** تا نام کتاب در آن نباشد، و بعد از شنیدن پاکش کنید.
- اجازه‌ی حافظه، موسیقی و دسترس‌پذیری را از برنامه‌هایی که به آن‌ها اعتماد ندارید بگیرید (تنظیمات گوشی ← برنامه‌ها ← مجوزها).
- روی گوشی قدیمی (اندروید ۹ و پایین‌تر) از «کپی متن صفحه» برای متن حساس استفاده نکنید.
- گوشی‌ای که دیگر به‌روزرسانی امنیتی نمی‌گیرد در برابر برنامه‌ی بدخواه محافظت کمتری دارد. اگر امنیتتان در خطر است، گوشی به‌روز و، اگر می‌توانید، [GrapheneOS](https://grapheneos.org) بهتر است.

### برنامه چه می‌کند؟

- متن فارسی را با صدای طبیعی **می‌خواند** (سه مدل صدا؛ دو تا را صاحبانشان به مالکیت عمومی بخشیده‌اند) و خواندن را به صورت **MP3** یا **WAV** ذخیره می‌کند.
- **متنی که خودتان می‌نویسید** («✎ نوشتن متن») و متن **عکس‌ها** را هم می‌خواند.
- **ویرایش در خودِ صفحه:** با «✎ ویرایش متن» هر بند جایی برای نوشتن می‌شود: انتخاب کنید، بنویسید، با کلید Delete پاک کنید. یک کلمه را می‌شود در همه‌ی کتاب عوض یا حذف کرد. ویرایش‌ها در خواندن، MP3 و PDF هم اثر می‌کنند.
- PDF، EPUB، FB2، پاورپوینت، ورد و متن ساده را **باز می‌کند**؛ صفحه‌ی اسکن‌شده و PDF رمزدار را هم.
- از انگلیسی، هلندی، آلمانی، فرانسوی، اسپانیایی، ایتالیایی، پرتغالی، ترکی و لهستانی به فارسی **ترجمه می‌کند** و ترجمه را به صورت **PDF فارسی** ذخیره می‌کند (در نسخه‌های سبک نیست).
- می‌تواند با **صدای خود شما** بخواند («صدای من»). فقط صدای خودتان را به کار ببرید.
- **قفل برنامه** (تنظیمات، «قفل برنامه»؛ مانند قفل صفحه‌ی سیگنال، و تا خودتان روشنش نکنید خاموش است): برنامه هنگام باز شدن و بعد از مدتی که انتخاب می‌کنید (از ۱ دقیقه تا ۱ ساعت، یا فقط هنگام باز شدن) رمزی را که خودتان گذاشته‌اید می‌پرسد. تا قفل است، خودِ برنامه همه‌ی درخواست‌ها را رد می‌کند، نه فقط صفحه. رمز هیچ‌جا نگه داشته نمی‌شود؛ فقط یک نمک تصادفی و چکیده‌ی PBKDF2-HMAC-SHA256 آن (۳۰۰٬۰۰۰ دور) در پرونده‌ای که فقط حساب خودتان می‌خواند. بعد از پنج رمز اشتباه باید صبر کنید (۳۰ ثانیه، و هر بار بیشتر تا یک ساعت)، و این صبر با بستن برنامه برنمی‌گردد. **این قفل جلوی استفاده‌ی دیگران از برنامه را روی دستگاهِ روشن و بازِ شما می‌گیرد؛ فایل‌های برنامه را رمزگذاری نمی‌کند** (برای آن، FileVault یا رمزگذاری خودِ گوشی را روشن کنید). رمزِ فراموش‌شده را نمی‌شود بازیابی کرد: صفحه‌ی قفل «پاک کردن همه‌ی اطلاعات و شروع دوباره» را دارد که اطلاعات برنامه (ویرایش‌ها، صدای ضبط‌شده، تنظیمات، فایل‌های صدایی که داخل خودِ برنامه‌اند) و خودِ قفل را پاک می‌کند و به کتاب‌ها و فایل‌هایی که جای دیگر دارید دست نمی‌زند.

### برای کسانی که خوب نمی‌بینند

- دکمه‌ها و نوشته‌های بزرگ؛ اندازه‌ی قلم تا ۷۲؛ هفت پوسته: روشن، تیره، «نور شب» (گرم، برای شب)، سیاه، سیاه‌وزرد و دو پوسته‌ی شیشه‌ای؛ از تنظیم «تضاد بیشتر» و «حرکت کمتر» سیستم پیروی می‌کند.
- همه‌چیز با صفحه‌کلید کار می‌کند: <kbd>Space</kbd> خواندن یا مکث، <kbd>←</kbd> <kbd>→</kbd> صفحه‌ی بعد و قبل.
- برای صفحه‌خوان‌ها (VoiceOver، TalkBack) ساخته شده: دکمه‌ها نام دارند، پیام‌ها و عوض شدن صفحه گفته می‌شود. هنوز یک کاربر نابینا آن را نیازموده است؛ اگر جایی کار نکرد به ما بگویید.
- کلمه‌ای که خوانده می‌شود نشان‌دار است و می‌شود خواندن را از هر جمله شروع کرد.

### محدودیت‌ها

- **ترجمه ماشینی است و اشتباه دارد،** به‌ویژه در نام‌ها، املای قدیمی، شعر و واژه‌های تخصصی.
- **صدا فقط فارسی است.** خط فارسی مصوت‌های کوتاه را نمی‌نویسد؛ پس برنامه باید تلفظ واژه‌ی کم‌کاربرد یا نام را حدس بزند و گاهی اشتباه می‌کند. نام خارجی با حروف لاتین با واژه‌نامه‌ی انگلیسی، در کتاب هلندی با قاعده‌های هلندی، و وگرنه با قاعده‌ی ساده‌ی حروف خوانده می‌شود. هر کلمه را می‌توانید برای همیشه درست کنید: آن را انتخاب کنید و «اصلاح تلفظ» را بزنید.
- **کیفیت خواندن به خود فایل بستگی دارد.** PDF با فونت خراب، اسکن بد یا صفحه‌آرایی غیرمعمول ممکن است درست خوانده نشود.
- **تبدیل گفتار به نوشته و ویرایش با صدا** (مک) گفتار روشن و آرام را بهتر می‌شنود و در نام‌ها و جای پرسروصدا اشتباه می‌کند. نوشته را بازبینی کنید.
- هر بار یک سند، تا ۳۰۰ مگابایت. برنامه خودش را به‌روز نمی‌کند: برای به‌روزرسانی نسخه‌ی تازه را دانلود کنید.
- مدل‌های ترجمه و گفتار فقط برای استفاده‌ی غیرتجاری‌اند؛ پس برنامه را نمی‌توان فروخت.

### مشکل یا پرسش؟

در بخش [Issues](../../issues) بنویسید. مشکل امنیتی را طبق [SECURITY.md](SECURITY.md) گزارش کنید.

</div>

## Licence

Our own code is under the [MIT licence](LICENSE). The translation and speech models are for **non-commercial use only**, so the
complete apps may not be sold. See [credits and licences](THIRD_PARTY_NOTICES.md), including our thanks to GrapheneOS.

## Building from source

The models and prebuilt libraries are too large for this repository; the credits list where each comes from.
`tools/make_app.sh` builds the Mac app, `tools/make_release.sh` the three Mac downloads, and `android/build.sh` the Android app
(`KHANA_LITE=1` for the light one). The Windows app is built by GitHub (`.github/workflows/windows.yml`), with every outside step
pinned to an exact commit. A release of your own needs your own Android signing key. The code has unit tests (`cargo test --lib`)
and a self-test that runs the real window (`KHANA_SELFTEST=<file>`).
