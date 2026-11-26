#!/bin/bash
# Builds "Khana Reader (Rust).app" in this folder. Local builds default to ad hoc signing;
# set MACOS_SIGN_IDENTITY to a Developer ID Application identity for distribution builds.
set -e
cd "$(dirname "$0")/.."
source tools/macos_signing.sh
export PATH="$HOME/.cargo/bin:$PATH"
# (no folder names of this computer inside the program)
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$PWD=/khana"$'\x1f'"--remap-path-prefix=$HOME/.cargo=/cargo"$'\x1f'"--remap-path-prefix=$HOME/.rustup=/rustup"
VENDOR_ARGS=()
if [ -d "$PWD/vendor/crates" ]; then
  VENDOR_ARGS=(--offline --config "source.crates-io.replace-with='vendored-sources'" --config "source.vendored-sources.directory='$PWD/vendor/crates'")
fi
cargo build --locked --release --bin khana "${VENDOR_ARGS[@]}"
APP="Khana Reader (Rust).app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp target/release/khana "$APP/Contents/MacOS/KhanaReaderRust"
# only the app's own Frameworks folder: never the build folder's libraries
install_name_tool -delete_rpath "$PWD/vendor/lib" "$APP/Contents/MacOS/KhanaReaderRust" 2>/dev/null || true
# (checked: no folder name of this computer, which holds the user's name, is left inside the program)
if LC_ALL=C grep -q -a "$HOME" "$APP/Contents/MacOS/KhanaReaderRust"; then echo "A PATH OF THIS COMPUTER IS IN THE PROGRAM"; exit 1; fi
cp packaging/Info.plist "$APP/Contents/"
cp packaging/AppIcon.icns "$APP/Contents/Resources/"
cp -R web "$APP/Contents/Resources/web"
if [ -f MODELS.sha256 ]; then
  # Verify every bundled model and native library listed in MODELS.sha256 that exists in this tree.
  grep -v '^#' MODELS.sha256 | while IFS= read -r line; do
    [ -z "$line" ] && continue
    f="${line#*  }"
    if [ -f "$f" ]; then printf '%s\n' "$line" | shasum -a 256 -c - >/dev/null || { echo "CHECKSUM MISMATCH: $f"; exit 1; }; fi
  done
fi
cp -cRL models "$APP/Contents/Resources/models"
rm -f "$APP/Contents/Resources/models/parsigo/model/normalize_fa.py"
rm -rf "$APP/Contents/Resources/models/translate/nllb-200-1.3B"
rm -rf "$APP/Contents/Resources/models/translate/nllb-200-600M"  # (the Mac uses the larger model; the phone this one)
rm -rf "$APP/Contents/Resources/models/asr/vosk-model-small-fa-0.5/test"  # (the model's own Python example)
for l in libvosk libctranslate2 libonnxruntime; do
  cp vendor/lib/$l.dylib "$APP/Contents/Frameworks/"
done
macos_sign_bundle "$APP"
if [[ "$MACOS_SIGN_IDENTITY" == "-" ]]; then
  echo "Built with ad hoc signing for local use. Downloadable releases need Developer ID signing and notarization."
fi
echo "built $APP"
