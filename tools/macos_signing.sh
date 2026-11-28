#!/bin/bash
# Shared signing helpers for local macOS builds and public releases.

MACOS_SIGN_IDENTITY="${MACOS_SIGN_IDENTITY:--}"
MACOS_NOTARY_PROFILE="${MACOS_NOTARY_PROFILE:-}"

macos_sign_code() {
  local path="$1"
  local entitlements="${2:-}"
  local args=(--force --sign "$MACOS_SIGN_IDENTITY")
  if [[ "$MACOS_SIGN_IDENTITY" != "-" ]]; then
    args+=(--options runtime --timestamp)
  fi
  if [[ -n "$entitlements" ]]; then
    args+=(--entitlements "$entitlements")
  fi
  codesign "${args[@]}" "$path"
}

macos_sign_dmg() {
  local path="$1"
  if [[ "$MACOS_SIGN_IDENTITY" == "-" ]]; then
    codesign --force --sign - "$path"
  else
    codesign --force --sign "$MACOS_SIGN_IDENTITY" --timestamp "$path"
  fi
}

macos_sign_bundle() {
  local app="$1"
  local contents="$app/Contents"
  # Shared installations must be readable from every user account.
  chmod -R a+rX "$app"
  local lib
  for lib in "$contents"/Frameworks/*.dylib; do
    [[ -f "$lib" ]] || continue
    macos_sign_code "$lib"
  done
  macos_sign_code "$contents/MacOS/KhanaReaderRust" packaging/entitlements.plist
  macos_sign_code "$app" packaging/entitlements.plist
  codesign --verify --deep --strict --verbose=2 "$app"
}

macos_require_release_signing() {
  if [[ "$MACOS_SIGN_IDENTITY" == "-" || -z "$MACOS_NOTARY_PROFILE" ]]; then
    if [[ "${MACOS_ALLOW_UNNOTARIZED:-0}" != "1" ]]; then
      cat >&2 <<'EOF'
Refusing to create public macOS release images without Developer ID signing and notarization.
Set MACOS_SIGN_IDENTITY to the installed "Developer ID Application" identity and
MACOS_NOTARY_PROFILE to a saved `notarytool` keychain profile. For a local test image only,
set MACOS_ALLOW_UNNOTARIZED=1; Gatekeeper may block that image when downloaded.
EOF
      return 1
    fi
    echo "WARNING: creating an unnotarized macOS image; Gatekeeper may block downloaded copies." >&2
  fi
}

macos_notarize_dmg() {
  local path="$1"
  [[ -n "$MACOS_NOTARY_PROFILE" ]] || return 0
  [[ "$MACOS_SIGN_IDENTITY" != "-" ]] || {
    echo "A Developer ID identity is required for notarization." >&2
    return 1
  }
  xcrun notarytool submit "$path" --keychain-profile "$MACOS_NOTARY_PROFILE" --wait
  xcrun stapler staple "$path"
  xcrun stapler validate "$path"
}
