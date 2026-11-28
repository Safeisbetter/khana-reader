#!/bin/bash
# Rebuilds all local Khana Reader packages (Mac + Android + SHA256SUMS.txt) into dist/ completely offline.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "=== 1/4 Building macOS app bundle ==="
bash tools/make_app.sh

echo "=== 2/4 Building macOS DMG releases ==="
bash tools/make_release.sh

echo "=== 3/4 Building Android full APK ==="
KHANA_PUBLIC=1 bash android/build.sh
cp -cf android/build/KhanaReader-Android-next.apk dist/KhanaReader-Android-next.apk

echo "=== 4/4 Building Android lite APK ==="
KHANA_PUBLIC=1 KHANA_LITE=1 bash android/build.sh
cp -cf android/build/KhanaReader-Android-next-lite.apk dist/KhanaReader-Android-next-lite.apk

rm -f dist/SHA256SUMS.txt
(cd dist && shasum -a 256 * > SHA256SUMS.txt)
echo "=== All release packages ready in dist/ ==="
ls -lh dist/
cat dist/SHA256SUMS.txt
