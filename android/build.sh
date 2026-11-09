#!/bin/bash
# Builds the Android app: android/build/KhanaReader.apk (arm64 phones, Android 10 or newer).
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
(cd "$ROOT" && cargo rustc --release --lib --target aarch64-linux-android --crate-type cdylib)
"$NDKB/llvm-strip" --strip-unneeded -o "$OUT/jniLibs/arm64-v8a/libkhana.so" "$ROOT/target/aarch64-linux-android/release/libkhana.so"

# 3. the Java shell
rm -rf "$OUT/classes" "$OUT/dex"
mkdir -p "$OUT/classes" "$OUT/dex"
javac -source 11 -target 11 -Xlint:-options -encoding UTF-8 -classpath "$JAR" -d "$OUT/classes" "$HERE"/java/org/khana/reader/*.java
jar cf "$OUT/classes.jar" -C "$OUT/classes" .

# 4. the assets: the reader page (as on the Mac, plus android.js), the models, the PDF font
A="$OUT/assets"
rm -rf "$A"
mkdir -p "$A/app" "$A/fonts"
cp -Rc "$ROOT/web" "$A/app/web"
cp "$HERE/web/android.js" "$A/app/web/"
if [ -n "${KHANA_PUBLIC:-}" ]; then printf '\nwindow.KHANA_NO_MIC = true;\n' >> "$A/app/web/android.js"; fi
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
[ -f "$OUT/fonts/Vazirmatn.ttf" ] || { mkdir -p "$OUT/fonts"; swift "$HERE/tools/woff2_to_ttf.swift" "$ROOT/web/Vazirmatn.woff2" "$OUT/fonts/Vazirmatn.ttf"; }
cp "$OUT/fonts/Vazirmatn.ttf" "$A/fonts/"
(cd "$A/app" && find . -type f | sed 's|^\./||' | LC_ALL=C sort | while IFS= read -r f; do printf '%s\t%s\n' "$f" "$(stat -f %z "$f")"; done) > "$A/app.files"

# 5. the APK
rm -f "$OUT/res.zip" "$OUT/unsigned.apk" "$OUT/aligned.apk"
"$BT/aapt2" compile --dir "$HERE/res" -o "$OUT/res.zip"
MANIFEST="$HERE/AndroidManifest.xml"
if [ -n "${KHANA_PUBLIC:-}" ]; then
  # (the public download asks for no microphone at all: a voice sample is chosen as a file instead, and the page hides recording)
  MANIFEST="$OUT/AndroidManifest.public.xml"
  grep -v -E 'android.permission.(RECORD_AUDIO|MODIFY_AUDIO_SETTINGS)' "$HERE/AndroidManifest.xml" > "$MANIFEST"
fi
"$BT/aapt2" link -o "$OUT/unsigned.apk" --manifest "$MANIFEST" -I "$JAR" -A "$A" --no-compress \
  --proguard "$OUT/aapt-rules.pro" "$OUT/res.zip"
# R8: the shell shrunk and its names shortened (release), keeping what the manifest and JNI name
java -cp "$BT/lib/d8.jar" com.android.tools.r8.R8 --release --min-api $API --lib "$JAR" \
  --pg-conf "$OUT/aapt-rules.pro" --pg-conf "$HERE/proguard-rules.pro" --pg-map-output "$OUT/mapping.txt" \
  --output "$OUT/dex" "$OUT/classes.jar"
mkdir -p "$OUT/extra/lib"
rm -rf "$OUT/extra/lib/arm64-v8a"
cp -R "$OUT/jniLibs/arm64-v8a" "$OUT/extra/lib/"
cp "$OUT/dex/classes.dex" "$OUT/extra/"
# (fixed times and a sorted order, no extra attributes: the same bytes each build)
(cd "$OUT/extra" && find classes.dex lib -exec touch -h -t 202511030000 {} + && \
  find classes.dex lib -type f | sort | zip -q -X -0 "$OUT/unsigned.apk" -@)
"$BT/zipalign" -P 16 -f 4 "$OUT/unsigned.apk" "$OUT/aligned.apk"

# 6. signed with the app's own key (made once, kept in android/keystore: back it up; an update must
# be signed with the same key)
APK=KhanaReader.apk; [ -n "${KHANA_LITE:-}" ] && APK=KhanaReader-lite.apk
K="$HERE/keystore"
if [ ! -f "$K/khana.jks" ]; then
  mkdir -p "$K"
  chmod 700 "$K"
  head -c 24 /dev/urandom | base64 | tr -d '/+=' > "$K/password.txt"
  chmod 600 "$K/password.txt"
  keytool -genkeypair -keystore "$K/khana.jks" -storepass:file "$K/password.txt" -keypass:file "$K/password.txt" \
    -alias khana -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=Khana Reader" >/dev/null
fi
"$BT/apksigner" sign --v1-signing-enabled false --v2-signing-enabled true --v3-signing-enabled true --ks "$K/khana.jks" --ks-pass "file:$K/password.txt" --ks-key-alias khana --out "$OUT/$APK" "$OUT/aligned.apk"
"$BT/apksigner" verify -v --print-certs "$OUT/$APK" | grep -E "Verified using v[23] |Signer #1 certificate SHA-256"
rm -f "$OUT/unsigned.apk" "$OUT/aligned.apk"
ls -la "$OUT/$APK"
