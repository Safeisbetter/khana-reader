# Security / امنیت

## English

**Download safely.** Only download the app from this repository's [Releases](../../releases) page. Each release has a `SHA256SUMS.txt`; compare the file's SHA-256 with it
(`shasum -a 256 file` on Mac/Linux, `Get-FileHash file` in Windows PowerShell). The Android APKs are all signed with one key; its SHA-256 certificate fingerprint is
`32:44:BB:08:80:95:34:D3:42:78:5A:FD:CA:E9:C8:16:03:B9:B2:63:FC:7B:C2:3D:E5:78:32:D5:C1:D9:E0:F9`
(`apksigner verify --print-certs file.apk`). A file with another fingerprint is not from this project.

**What the app does.** It works fully offline. The Android app has no internet permission. Nothing you open is sent anywhere.

**App lock.** Optional (Settings, «قفل برنامه»). It protects the use of the app on a device that is switched on and open; it does not encrypt files. The password is never stored: a random salt and a PBKDF2-HMAC-SHA256 hash (300,000 rounds) are kept in `lock.json`, readable only by your account; wrong tries are slowed down (from the fifth, 30 seconds, then longer, up to an hour) and the count survives a restart. While the app is locked it refuses every request except unlock, status, lock and «erase everything». A forgotten password cannot be recovered; erasing the app's data is the way out.

**Reporting a problem.** Please use GitHub's *Report a vulnerability* button on the Security tab (private), or open an issue for a problem that is not sensitive.
Do not attach books you would not like to make public.

**Who can change the code.** Only the owner can push to this repository. Proposals from other people arrive as pull requests and are reviewed before anything is merged.

## فارسی

**امن دانلود کنید.** برنامه را فقط از بخش [Releases](../../releases) همین مخزن بگیرید. هر نسخه فایل `SHA256SUMS.txt` دارد؛ کد SHA-256 فایل خود را با آن مقایسه کنید.
فایل‌های APK اندروید همگی با یک کلید امضا شده‌اند و اثر انگشت گواهی آن در بالا آمده است. فایلی با اثر انگشت دیگر از این پروژه نیست.

**برنامه چه می‌کند؟** کاملاً آفلاین کار می‌کند؛ برنامه‌ی اندروید اجازه‌ی اینترنت ندارد و هیچ‌چیز از آنچه باز می‌کنید جایی فرستاده نمی‌شود.

**گزارش مشکل.** از دکمه‌ی «Report a vulnerability» در زبانه‌ی Security (خصوصی) استفاده کنید، یا برای مشکل غیرحساس یک Issue باز کنید. کتاب‌هایی را که نمی‌خواهید همه ببینند پیوست نکنید.

**قفل برنامه.** اختیاری است (تنظیمات، «قفل برنامه»). جلوی استفاده‌ی دیگران را روی دستگاهِ روشن و باز می‌گیرد؛ فایل‌ها را رمزگذاری نمی‌کند. رمز نگه داشته نمی‌شود؛ فقط نمک و چکیده‌ی PBKDF2 آن در پرونده‌ی `lock.json`. رمز اشتباه کند می‌شود و تا قفل است برنامه همه‌ی درخواست‌ها را رد می‌کند. رمز فراموش‌شده بازیابی نمی‌شود؛ راه، پاک کردن اطلاعات برنامه است.

**چه کسی کد را تغییر می‌دهد؟** فقط صاحب مخزن می‌تواند در آن بنویسد. پیشنهاد دیگران به صورت Pull request می‌آید و پیش از پذیرفته شدن بررسی می‌شود.
