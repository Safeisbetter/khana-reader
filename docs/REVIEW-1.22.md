# Version 1.22 phone update

Adds compact Page and Speed buttons beside Read on Android phones, with controls that open only on request. The normal row is about 68 pixels tall and about 75 pixels with enlarged controls. Touch targets remain at least 48 pixels. Settings use a conventional gear shared with the Mac interface.

Swipe right for the next page and left for the previous page in the Farsi reader. Swipes do not cross the first or last page. Vertical scrolling, text selection, editing, long presses, cancelled touches and multi-touch do not turn pages. A completed swipe suppresses the following click so it does not start reading accidentally.

Checks use synthetic text only: page and speed changes, Escape/focus return, swipe boundaries and selection protection, phone layouts at 320 and 390 pixels, normal and enlarged controls, playback-state layout, and hiding the dock while the editing keyboard is open. These are browser layout/gesture checks, not a physical Pixel 10 test. The tablet was disconnected during this update.

The normal source/privacy/build checks and the GrapheneOS PDF Viewer reference review from version 1.21 still apply. No deep scan was restarted. Build and upload hashes are recorded with the release. Checks do not guarantee that every bug or security issue has been found.
