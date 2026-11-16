# Version 1.21 review

Reference: GrapheneOS/PdfViewer commit 9e343963ac6e17872f60ffa6af285b584b24afa9, reviewed 2026-11-15.
https://github.com/GrapheneOS/PdfViewer/tree/9e343963ac6e17872f60ffa6af285b584b24afa9

Khana retains local request interception, no Android Internet permission, blocked WebView network/file/content access, a separate sandboxed PDF frame, restrictive page and worker policies, disabled PDF dynamic evaluation and an 8,388,608-pixel render cap. Its PDF engine is 6.5.438; no dependency downgrade or wholesale replacement was made. GrapheneOS now permits WebAssembly in its PDF worker; Khana keeps its existing JavaScript decoder path and disabled WebAssembly.

Android responses now consistently include no-store, nosniff and no-referrer headers. The Windows build now requires its window-opening smoke test to succeed before uploading a package. Model downloads remain pinned by SHA-256.

The interface uses a red raised-style icon, a compact settings gear, grouped tools and a compact reading player. File reading starts with content rather than the filename. Imported files end with «پایان متن» in playback and MP3 export; typed or dictated text has no spoken title or ending announcement.

Validation is recorded in the release provenance and checksums. The Pixel Tablet was used for Android checks, including a phone-sized viewport and enlarged controls; a physical Pixel 10 has not been tested. Windows packages remain experimental. Translation, OCR, speech recognition and pronunciation can make mistakes. Normal checks are not an exhaustive security audit or a guarantee that every bug has been found.
