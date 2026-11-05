#!/bin/bash
# Builds "Khana Reader (Rust).app" in this folder: the program, its libraries, the web page and the
# models (APFS clones of models/, so they take no extra disk space), sandboxed and signed ad hoc.
set -e
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
# (no folder names of this computer inside the program)
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$PWD=/khana"$'\x1f'"--remap-path-prefix=$HOME/.cargo=/cargo"$'\x1f'"--remap-path-prefix=$HOME/.rustup=/rustup"
cargo build --release --bin khana
APP="Khana Reader (Rust).app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp target/release/khana "$APP/Contents/MacOS/KhanaReaderRust"
# only the app's own Frameworks folder: never the build folder's libraries
install_name_tool -delete_rpath "$PWD/vendor/lib" "$APP/Contents/MacOS/KhanaReaderRust"
cp packaging/Info.plist "$APP/Contents/"
cp packaging/AppIcon.icns "$APP/Contents/Resources/"
cp -R web "$APP/Contents/Resources/web"
cp -cRL models "$APP/Contents/Resources/models"
rm -f "$APP/Contents/Resources/models/parsigo/model/normalize_fa.py"
rm -rf "$APP/Contents/Resources/models/translate/nllb-200-1.3B"
rm -rf "$APP/Contents/Resources/models/translate/nllb-200-600M"  # (the Mac uses the larger model; the phone this one)
rm -rf "$APP/Contents/Resources/models/asr/vosk-model-small-fa-0.5/test"  # (the model's own Python example)
for l in libvosk libctranslate2 libonnxruntime; do
  cp vendor/lib/$l.dylib "$APP/Contents/Frameworks/"
  codesign -f -s - "$APP/Contents/Frameworks/$l.dylib"
done
codesign -f -s - --entitlements packaging/entitlements.plist "$APP/Contents/MacOS/KhanaReaderRust"
codesign -f -s - --entitlements packaging/entitlements.plist "$APP"
codesign -dv --entitlements - "$APP" 2>&1 | grep -E "Identifier|app-sandbox" || true
echo "built $APP"
