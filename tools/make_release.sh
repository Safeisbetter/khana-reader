#!/bin/sh
# The public Mac download, in dist/: the built app without the voices whose source is unknown, in a disk image split into
# parts under GitHub's 2 GiB limit per file. Run tools/make_app.sh first. The app in this folder is not changed.
set -e
cd "$(dirname "$0")/.."
SRC="Khana Reader (Rust).app"
STAGE="dist/stage"
rm -rf dist
mkdir -p "$STAGE"
cp -cR "$SRC" "$STAGE/"
APP="$STAGE/$SRC"
rm -rf "$APP/Contents/Resources/models/voices"
rm -rf "$APP/Contents/Resources/models/whisper"  # (speech to text: 1.6 GB, only in the builds made for oneself)
cp LICENSE THIRD_PARTY_NOTICES.md "$STAGE/"
find "$STAGE" -name .DS_Store -delete
codesign -f -s - --entitlements packaging/entitlements.plist "$APP"
hdiutil create -quiet -volname "Khana Reader" -srcfolder "$STAGE" -format UDZO -ov dist/whole.dmg
hdiutil segment -quiet -segmentSize 1900m -o dist/KhanaReader-Mac dist/whole.dmg
rm -rf "$STAGE" dist/whole.dmg
ls -la dist
shasum -a 256 dist/* > dist/SHA256SUMS.txt
cat dist/SHA256SUMS.txt
