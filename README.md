# Khana Reader (خوانا)

Reads books aloud in Farsi and translates foreign books into Farsi, entirely on your own device. Free, with no account, no ads
and no tracking.

<div dir="rtl">

خوانا کتاب‌ها را به فارسی با صدای بلند می‌خواند و کتاب‌های خارجی را به فارسی ترجمه می‌کند؛ همه‌چیز فقط روی دستگاه خود شما انجام می‌شود. رایگان است، و حساب کاربری، تبلیغ یا ردیابی ندارد.

</div>

## Download

Get the files from the **[Releases page](../../releases/latest)**.

### Mac

Needs an Apple Silicon Mac (M1 or newer) with macOS 13 or newer and about 4 GB of disk space. 16 GB of memory is recommended,
because translating uses about 7 GB.

1. Download **both** `.dmg` and `.dmgpart` files into the same folder.
2. Double-click the `.dmg` file and drag the app to Applications.
3. The first time, macOS warns you, because this free app is not signed with a paid Apple Developer ID. Try to open the app,
   then go to **System Settings → Privacy & Security** and choose **Open Anyway**.

### Android

Needs an arm64 phone with Android 8 or newer and a recent Android System WebView (version 133 or newer).

1. Download `KhanaReader.apk` (about 1.3 GB) on your phone.
2. Open it and allow installing from this source when Android asks.

> **Please note:** the Android app has not yet been tried on a real phone. If something goes wrong, please open an
> [issue](../../issues).

## What it does

- Reads PDF, EPUB, Word and text files aloud in Farsi, and saves MP3s
- Translates from English, Dutch, German, French, Spanish, Italian, Portuguese, Turkish and Polish into Farsi
- Reads scanned pages with OCR
- Opens password-protected PDFs: it asks for the password in a small dialog. The password goes only to the PDF reader inside the app, never to the rest of the app, to a file, or anywhere else
- Can read in your own voice («صدای من»). Please use only your own voice: copying someone's voice without their consent is forbidden by the speech model's terms

## Privacy

- Your books stay on your device, and there is no account, analytics or server.
- The Android app has no internet permission.
- The text of the book you have open is kept in memory while it is open, because reading aloud, searching and translating need
  it. The app overwrites its own copies when it lets the book go (another book opened, the window closed). It keeps no copy of
  the book on storage: an MP3 or PDF you ask for is made in the app's private storage until you save it where you choose, and on
  the Mac a Word or RTF file is copied for a moment to a temporary file for Apple's converter and deleted at once. Copies inside
  the web view's page and Android's Java layer are out of the app's reach, and Android keeps other apps out of this app's memory.
- On the Mac, the app runs in Apple's App Sandbox. It holds Apple's "outgoing network" permission only because Apple's WebKit
  needs it to start its own helper processes; the program has no network code and its page refuses every outside address.

## Limits

- Translation is done by a machine and makes mistakes, especially with names and old spelling. Don't rely on it where accuracy matters.
- Android uses a smaller translation model than the Mac, so its translations have more errors.

## Licence

Our own code is under the [MIT licence](LICENSE). The translation and speech models are for **non-commercial use only**, so the
complete apps may not be sold. See [credits and licences](THIRD_PARTY_NOTICES.md), including our thanks to GrapheneOS.

## Building from source

The models and prebuilt libraries are too large for this repository; the credits list where each comes from.
`tools/make_app.sh` builds the Mac app and `android/build.sh` builds the Android app. A release of your own needs your own
Android signing key.
