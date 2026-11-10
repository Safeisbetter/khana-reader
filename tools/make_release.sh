#!/bin/sh
# The public Mac downloads, in dist/: the built app without the voices whose source is unknown, as disk images. Three sizes,
# because many readers pay dearly for every megabyte:
#   KhanaReader-Mac-lite.dmg    about 0.5 GB, one file: reads Farsi aloud, MP3, scanned pages; no translator, no dictation
#   KhanaReader-Mac-medium.dmg  about 1.1 GB, one file: the same with the small translator (the phone's, 600M)
#   KhanaReader-Mac.dmg + parts about 5 GB, split under GitHub's 2 GiB limit per file: the large translator and dictation
# Run tools/make_app.sh first. The app in this folder is not changed.
set -e
cd "$(dirname "$0")/.."
SRC="Khana Reader (Rust).app"
STAGE="dist/stage"
SMALL="models/translate/nllb-200-600M"
rm -rf dist
mkdir -p "$STAGE"
cp -cR "$SRC" "$STAGE/"
APP="$STAGE/$SRC"
M="$APP/Contents/Resources/models"
rm -rf "$M/voices"
cp LICENSE THIRD_PARTY_NOTICES.md "$STAGE/"
find "$STAGE" -name .DS_Store -delete

# full
codesign -f -s - --entitlements packaging/entitlements.plist "$APP"
hdiutil create -quiet -volname "Khana Reader" -srcfolder "$STAGE" -format UDZO -ov dist/whole.dmg
hdiutil segment -quiet -segmentSize 1900m -o dist/KhanaReader-Mac dist/whole.dmg
rm -f dist/whole.dmg

# medium: the small translator in place of the large one, no dictation model
rm -rf "$M/whisper" "$M/translate"
if [ -f "$SMALL/model.bin" ]; then
  mkdir -p "$M/translate"
  cp -cR "$SMALL" "$M/translate/"
  codesign -f -s - --entitlements packaging/entitlements.plist "$APP"
  hdiutil create -quiet -volname "Khana Reader" -srcfolder "$STAGE" -format ULMO -ov dist/KhanaReader-Mac-medium.dmg
else
  echo "no small translator in $SMALL: the medium download is not made"
fi

# light: no translator at all
rm -rf "$M/translate"
codesign -f -s - --entitlements packaging/entitlements.plist "$APP"
hdiutil create -quiet -volname "Khana Reader" -srcfolder "$STAGE" -format ULMO -ov dist/KhanaReader-Mac-lite.dmg

rm -rf "$STAGE"
ls -la dist
shasum -a 256 dist/* > dist/SHA256SUMS.txt
cat dist/SHA256SUMS.txt
