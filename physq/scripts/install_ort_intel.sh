#!/usr/bin/env bash
# Downloads the Microsoft official ONNX Runtime for Intel Mac (the last
# x86_64 macOS release — ONNX Runtime >= 1.24 no longer ships x86_64 macOS
# binaries, and ort-sys's own prebuilts never covered x86_64-apple-darwin)
# and installs it into "$RUNNER_TEMP/onnxruntime" (or /tmp outside CI).
# Prints the library directory, which callers use to set ORT_LIB_LOCATION.
#
# The dylib is version-pinned here; physq/src/update.rs's INTEL_ORT_DYLIB_ASSET
# and the release workflow's bundling must match this VERSION.
set -euo pipefail

VERSION=1.23.2
SHA256=d10359e16347b57d9959f7e80a225a5b4a66ed7d7e007274a15cae86836485a6
TGZ="onnxruntime-osx-x86_64-${VERSION}.tgz"
URL="https://github.com/microsoft/onnxruntime/releases/download/v${VERSION}/${TGZ}"

DEST="${RUNNER_TEMP:-/tmp}/onnxruntime"
LIB_DIR="$DEST/lib"

if [ ! -f "$LIB_DIR/libonnxruntime.${VERSION}.dylib" ]; then
  rm -rf "$DEST" "$DEST.tgz"
  mkdir -p "$DEST"
  curl -fsSL -o "$DEST.tgz" "$URL"
  echo "$SHA256  $DEST.tgz" | shasum -a 256 -c - >/dev/null
  tar xzf "$DEST.tgz" -C "$DEST" --strip-components=1
  rm -f "$DEST.tgz"
fi

echo "$LIB_DIR"