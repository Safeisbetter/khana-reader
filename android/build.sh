#!/bin/bash
# Builds the new Android app (arm64 phones, Android 8 or newer).
# No Gradle: the Rust core (libkhana.so) with cargo and the NDK, the Java shell with javac and d8,
# the APK with aapt2, zipalign and apksigner. Everything it uses is in android/toolchain (JDK 17,
# Android SDK platform 37 and build-tools 35, NDK r27c, CTranslate2 4.5.0's source); nothing is fetched while building.
# Reproducible, as GrapheneOS builds its apps: the same sources and toolchain give the same APK, byte
# for byte (fixed times, a sorted order of the added files, no build paths in the Rust code; R8,
# aapt2 and the v2/v3 signature are deterministic). The signing key stays outside the APK.
#   android/build.sh
set -euo pipefail
export SOURCE_DATE_EPOCH=1762128000 # (2025-11-03 00:00 UTC: the time stamped on the APK's files)
export TZ=UTC LC_ALL=C
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$HERE")"
T="$HERE/toolchain"
OUT="$HERE/build"
export JAVA_HOME="$T/jdk/jdk-17.0.13+11/Contents/Home"
export PATH="$JAVA_HOME/bin:$HOME/.cargo/bin:$PATH"
SDK="$T/sdk"
BT="$SDK/build-tools/35.0.0"
JAR="$SDK/platforms/android-37.0/android.jar"
NDKB="$T/ndk-x/android-ndk-r27c/toolchains/llvm/prebuilt/darwin-x86_64/bin"
API=26

# Keep signing material out of Git. Public builds use the new app's release key.
K="${KHANA_ANDROID_SIGNING_DIR:-$HERE/keystore}"
if [ ! -r "$K/khana.jks" ] || [ ! -r "$K/password.txt" ]; then
  echo "Set KHANA_ANDROID_SIGNING_DIR to the directory containing khana.jks and password.txt." >&2
  exit 1
fi
if [ -n "${KHANA_PUBLIC:-}" ]; then
  expected=$(cat "$HERE/RELEASE_CERT_SHA256")
  actual=$("$JAVA_HOME/bin/keytool" -list -v -keystore "$K/khana.jks" -storepass:file "$K/password.txt" -alias khana | awk '$1 == "SHA256:" { print $2 }')
  if [ "$actual" != "$expected" ]; then
    echo "The Android signing key does not match the published release certificate." >&2
    exit 1
  fi
fi

# 1. CTranslate2 (once)
[ -f "$OUT/ct2/libctranslate2.a" ] || "$HERE/build_ct2.sh"

# 2. the Rust core
export CC_aarch64_linux_android="$NDKB/aarch64-linux-android$API-clang"
export CXX_aarch64_linux_android="$NDKB/aarch64-linux-android$API-clang++"
export AR_aarch64_linux_android="$NDKB/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDKB/aarch64-linux-android$API-clang"
mkdir -p "$OUT/jniLibs/arm64-v8a"
cp "$HERE/prebuilt/arm64-v8a/"*.so "$OUT/jniLibs/arm64-v8a/"
# (paths of this Mac left out of the library: the source and cargo folders by fixed names)
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$ROOT=/khana"$'\x1f'"--remap-path-prefix=$HOME/.cargo=/cargo"$'\x1f'"--remap-path-prefix=$HOME/.rustup=/rustup"
VENDOR_ARGS=()
if [ -d "$ROOT/vendor/crates" ]; then
  VENDOR_ARGS=(--offline --config "source.crates-io.replace-with='vendored-sources'" --config "source.vendored-sources.directory='$ROOT/vendor/crates'")
fi
(cd "$ROOT" && cargo rustc --locked --release --lib --target aarch64-linux-android --crate-type cdylib "${VENDOR_ARGS[@]}")
"$NDKB/llvm-strip" --strip-unneeded -o "$OUT/jniLibs/arm64-v8a/libkhana.so" "$ROOT/target/aarch64-linux-android/release/libkhana.so"

# 3. the Java shell
rm -rf "$OUT/classes" "$OUT/dex"
mkdir -p "$OUT/classes" "$OUT/dex"
JAVA_SRC="$HERE/java/org/khana/reader"
if [ -n "${KHANA_DEV:-}" ] && [ -z "${KHANA_PUBLIC:-}" ]; then
  # (KHANA_DEV=1: a developer's build with the speed test and UI checks; never for distribution: its APK is named KhanaReader-dev.apk)
  JAVA_SRC="$OUT/java-dev"
  rm -rf "$JAVA_SRC"; mkdir -p "$JAVA_SRC"
  cp "$HERE"/java/org/khana/reader/*.java "$JAVA_SRC/"
  sed -i '' 's/static final boolean DEV_TOOLS = false;/static final boolean DEV_TOOLS = true;/' "$JAVA_SRC/MainActivity.java"
  grep -q 'DEV_TOOLS = true;' "$JAVA_SRC/MainActivity.java" || { echo "DEV_TOOLS switch not found"; exit 1; }
fi
if [ -n "${KHANA_PUBLIC:-}" ]; then
  # (the public download: the developer's USB tests are compiled out, so no other app on the phone can start them)
  JAVA_SRC="$OUT/java-public"
  rm -rf "$JAVA_SRC"; mkdir -p "$JAVA_SRC"
  cp "$HERE"/java/org/khana/reader/*.java "$JAVA_SRC/"
  sed -i '' 's/static final boolean DEV_TOOLS = true;/static final boolean DEV_TOOLS = false;/' "$JAVA_SRC/MainActivity.java"
  grep -q 'DEV_TOOLS = false;' "$JAVA_SRC/MainActivity.java" || { echo "DEV_TOOLS switch not found"; exit 1; }
fi
javac -source 11 -target 11 -Xlint:-options -encoding UTF-8 -classpath "$JAR" -d "$OUT/classes" "$JAVA_SRC"/*.java
jar cf "$OUT/classes.jar" -C "$OUT/classes" .

# 4. the assets: the reader page (as on the Mac, plus android.js), the models, the PDF font
A="$OUT/assets"
rm -rf "$A"
mkdir -p "$A/app" "$A/fonts"
cp -Rc "$ROOT/web" "$A/app/web"
cp "$HERE/web/android.js" "$A/app/web/"
cat "$HERE/web/keyboard.css" >> "$A/app/web/app.css"
# (the microphone is kept in every build now, voice editing and «صدای من» recording included; KHANA_NO_MIC=1 makes a light build without it)
if [ -n "${KHANA_PUBLIC:-}" ] && [ -n "${KHANA_LITE:-}" ] && [ -n "${KHANA_NO_MIC:-}" ]; then printf '\nwindow.KHANA_NO_MIC = true;\n' >> "$A/app/web/android.js"; fi
cp -RcL "$ROOT/models" "$A/app/models"
find "$A" -name '.DS_Store' -delete
# (as tools/make_app.sh: not the model's Python helper nor Vosk's own example)
rm -f "$A/app/models/parsigo/model/normalize_fa.py"
rm -rf "$A/app/models/translate/nllb-200-3.3B"
# (KHANA_PUBLIC=1: the public download, without the voices whose source is unknown)
if [ -n "${KHANA_PUBLIC:-}" ]; then rm -rf "$A/app/models/voices"; fi
rm -rf "$A/app/models/translate/nllb-200-1.3B"  # (the Mac's larger model: too heavy for a phone, 3.4 GB at its peak)
# (KHANA_LITE=1: the light download, without the translator: about half the size; foreign-language books are not translated)
if [ -n "${KHANA_LITE:-}" ]; then rm -rf "$A/app/models/translate"; fi
rm -rf "$A/app/models/asr/vosk-model-small-fa-0.5/test"
rm -rf "$A/app/models/whisper"  # (speech to text is on the Mac only: 1.6 GB)
# The voice model's 8-bit copy (android/prebuilt/voice8, made from the 32-bit one by dynamic quantization of its matrices) in
# place of the 32-bit one: a quarter of its size (341 MB -> 86 MB of download) and, measured on one tablet, speech made
# about half again as fast. Two listeners heard no difference in what is said. KHANA_LM32=1 keeps the 32-bit model.
LM8="$HERE/prebuilt/voice8/flow_lm_step.onnx"
if [ -z "${KHANA_LM32:-}" ] && [ -f "$LM8" ]; then cp "$LM8" "$A/app/models/parsigo/model/onnx/flow_lm_step.onnx"; rm -f "$A/app/models/parsigo/model/onnx/flow_lm_step.onnx.data"; fi
[ -f "$OUT/fonts/Vazirmatn.ttf" ] || { mkdir -p "$OUT/fonts"; swift "$HERE/tools/woff2_to_ttf.swift" "$ROOT/web/Vazirmatn.woff2" "$OUT/fonts/Vazirmatn.ttf"; }
cp "$OUT/fonts/Vazirmatn.ttf" "$A/fonts/"
(cd "$A/app" && find . -type f | sed 's|^\./||' | LC_ALL=C sort | while IFS= read -r f; do printf '%s\t%s\n' "$f" "$(stat -f %z "$f")"; done) > "$A/app.files"

# 5. the APK
rm -f "$OUT/res.zip" "$OUT/unsigned.apk" "$OUT/aligned.apk"
"$BT/aapt2" compile --dir "$HERE/res" -o "$OUT/res.zip"
MANIFEST="$HERE/AndroidManifest.xml"
if [ -n "${KHANA_PUBLIC:-}" ] && [ -n "${KHANA_LITE:-}" ] && [ -n "${KHANA_NO_MIC:-}" ]; then
  # (KHANA_NO_MIC=1: a light download that asks for no microphone at all: a voice sample is chosen as a file instead, and the page hides recording)
  MANIFEST="$OUT/AndroidManifest.public.xml"
  grep -v -E 'android.permission.(RECORD_AUDIO|MODIFY_AUDIO_SETTINGS)' "$HERE/AndroidManifest.xml" > "$MANIFEST"
fi
# (the models are compressed inside the APK: about a quarter less to download, where every megabyte costs; the app unpacks them
# on its first start as before. resources.arsc stays uncompressed, as Android 11 and newer require)
"$BT/aapt2" link -o "$OUT/unsigned.apk" --manifest "$MANIFEST" -I "$JAR" -A "$A" -0 .arsc \
  --proguard "$OUT/aapt-rules.pro" "$OUT/res.zip"
# R8: the shell shrunk and its names shortened (release), keeping what the manifest and JNI name; non-dev builds
# also strip every log call (proguard-public.pro) so default production builds never leak logs.
PG_PUBLIC=()
if [ -z "${KHANA_DEV:-}" ] || [ -n "${KHANA_PUBLIC:-}" ]; then
  PG_PUBLIC=(--pg-conf "$HERE/proguard-public.pro")
fi
java -cp "$BT/lib/d8.jar" com.android.tools.r8.R8 --release --min-api $API --lib "$JAR" \
  --pg-conf "$OUT/aapt-rules.pro" --pg-conf "$HERE/proguard-rules.pro" ${PG_PUBLIC[@]+"${PG_PUBLIC[@]}"} --pg-map-output "$OUT/mapping.txt" \
  --output "$OUT/dex" "$OUT/classes.jar"
if [ -z "${KHANA_DEV:-}" ] || [ -n "${KHANA_PUBLIC:-}" ]; then
  # (checked, not hoped: no reference to Android's log, nor to the developer's tests, is left in non-dev code)
  if LC_ALL=C grep -q -a 'Landroid/util/Log;' "$OUT/dex/classes.dex"; then echo "RELEASE BUILD STILL CALLS android.util.Log"; exit 1; fi
  if LC_ALL=C grep -q -a 'KhanaBench' "$OUT/dex/classes.dex"; then echo "RELEASE BUILD STILL HAS THE DEVELOPER TESTS"; exit 1; fi
fi
mkdir -p "$OUT/extra/lib"
rm -rf "$OUT/extra/lib/arm64-v8a"
cp -R "$OUT/jniLibs/arm64-v8a" "$OUT/extra/lib/"
cp "$OUT/dex/classes.dex" "$OUT/extra/"
# (fixed times and a sorted order, no extra attributes: the same bytes each build)
(cd "$OUT/extra" && find classes.dex lib -exec touch -h -t 202511030000 {} + && \
  find classes.dex lib -type f | sort | zip -q -X -0 "$OUT/unsigned.apk" -@)
"$BT/zipalign" -P 16 -f 4 "$OUT/unsigned.apk" "$OUT/aligned.apk"

# 6. Sign with the new app's key. Back up this key and its password separately;
# future updates of org.khana.reader.next require the same signing identity.
APK=KhanaReader-Android-next.apk; [ -n "${KHANA_LITE:-}" ] && APK=KhanaReader-Android-next-lite.apk
if [ -n "${KHANA_DEV:-}" ] && [ -z "${KHANA_PUBLIC:-}" ]; then APK=KhanaReader-dev.apk; fi
"$BT/apksigner" sign --v1-signing-enabled false --v2-signing-enabled true --v3-signing-enabled true --ks "$K/khana.jks" --ks-pass "file:$K/password.txt" --ks-key-alias khana --out "$OUT/$APK" "$OUT/aligned.apk"
"$BT/apksigner" verify -v --print-certs "$OUT/$APK" | grep -E "Verified using v[23] |Signer #1 certificate SHA-256"
# (never a debuggable APK: any app could then be attached to it over USB and its memory, the open book, read)
if "$BT/aapt2" dump badging "$OUT/$APK" 2>/dev/null | grep -q "application-debuggable"; then echo "APK IS DEBUGGABLE"; rm -f "$OUT/$APK"; exit 1; fi
# (and no folder name of this computer inside the program: the user's own name is in it)
if LC_ALL=C grep -q -a "$HOME" "$OUT/jniLibs/arm64-v8a/libkhana.so" "$OUT/dex/classes.dex"; then echo "A PATH OF THIS COMPUTER IS IN THE APP"; rm -f "$OUT/$APK"; exit 1; fi
rm -f "$OUT/unsigned.apk" "$OUT/aligned.apk"
ls -la "$OUT/$APK"
