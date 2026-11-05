#!/bin/bash
# CTranslate2 4.5.0 for Android (arm64), CPU only with the Ruy GEMM backend and OpenMP, built with
# the NDK's clang alone (no CMake). Output: android/build/ct2/libctranslate2.a (with Ruy inside).
# The source list and definitions follow CTranslate2's CMakeLists.txt for an arm64 build with
# -DWITH_RUY=ON -DWITH_MKL=OFF -DOPENMP_RUNTIME=COMP.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$HERE/toolchain/ct2-src"
NDK="$HERE/toolchain/ndk-x/android-ndk-r27c"
TC="$NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin"
CXX="$TC/aarch64-linux-android26-clang++"
AR="$TC/llvm-ar"
OUT="$HERE/build/ct2"
mkdir -p "$OUT/obj"
# (make cannot take paths with spaces: the sources are named from $OUT, relatively)
REL=../../toolchain/ct2-src

VERSION=$(sed -n 's/^__version__ = "\(.*\)"/\1/p' "$SRC/python/ctranslate2/version.py")
FLAGS=(-std=c++17 -O3 -fPIC -DNDEBUG -fopenmp -ffunction-sections -fdata-sections
  -DCT2_ARM64_BUILD -DCT2_WITH_RUY -DCT2_WITH_CPU_DISPATCH "-DCTRANSLATE2_VERSION=\"$VERSION\""
  -I"$REL/include" -I"$REL/src" -I"$REL/third_party" -I"$REL/third_party/spdlog/include"
  -I"$REL/third_party/ruy" -w)

# CTranslate2's own sources (the CMake SOURCES list), plus the NEON copy of the kernels
CT2=$(sed -n '/^set(SOURCES/,/^)/p' "$SRC/CMakeLists.txt" | grep -o 'src/[A-Za-z0-9_/]*\.cc')
cp "$SRC/src/cpu/kernels.cc" "$OUT/kernels_neon.cc"
# Ruy: its library sources (no tests, benchmarks or profiler)
RUY=$(cd "$SRC" && ls third_party/ruy/ruy/*.cc | grep -v -E '_test\.cc$|/test_|benchmark\.cc$')

# a Makefile of the objects, so they are built in parallel and only again when their source changed
MK="$OUT/objects.mk"
{
  printf 'CXX := %q\nFLAGS := %s\nall:' "$CXX" "$(printf '%q ' "${FLAGS[@]}")"
  for f in $CT2 $RUY; do printf ' obj/%s' "$(echo "$f" | tr '/' '_' | sed 's/\.cc$/.o/')"; done
  printf ' obj/kernels_neon.o\n'
  for f in $CT2 $RUY; do
    o="obj/$(echo "$f" | tr '/' '_' | sed 's/\.cc$/.o/')"
    printf '%s: %s\n\t$(CXX) $(FLAGS) -c $< -o $@\n' "$o" "$REL/$f"
  done
  printf 'obj/kernels_neon.o: kernels_neon.cc\n\t$(CXX) $(FLAGS) -DUSE_NEON -c $< -o $@\n'
} > "$MK"
make -C "$OUT" -f objects.mk -j"$(sysctl -n hw.ncpu)" all

rm -f "$OUT/libctranslate2.a"
"$AR" rcs "$OUT/libctranslate2.a" "$OUT"/obj/*.o
cp "$NDK/toolchains/llvm/prebuilt/darwin-x86_64/lib/clang/18/lib/linux/aarch64/libomp.a" "$OUT/"
echo "built $OUT/libctranslate2.a ($VERSION)"
