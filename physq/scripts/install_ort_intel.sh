#!/usr/bin/env bash
# Downloads the Microsoft official ONNX Runtime for Intel Mac (the last
# x86_64 macOS release — ONNX Runtime >= 1.24 no longer ships x86_64 macOS
# binaries, and ort-sys's own prebuilts never covered x86_64-apple-darwin)
# and installs it into "$RUNNER_TEMP/onnxruntime" (or /tmp outside CI).
# Prints the $LIB_DIR directory, which callers use to set ORT_LIB_LOCATION
# (and copy the dylib into the release bundle).
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
  # GitHub Releases の配信元はときどき数十秒〜数分 5xx（504 等）を返す。
  # curl 自身のリトライ（指数バックオフで ~30 秒）だけだと短い障害でも
  # 落ちるので、外側でも間隔を空けて数回やり直す（合計で最大 ~10 分）。
  ok=0
  for attempt in 1 2 3 4; do
    if curl -fL --connect-timeout 30 --retry 6 --retry-all-errors --retry-max-time 120 \
        -o "$DEST.tgz" "$URL"; then
      ok=1
      break
    fi
    echo "ONNX Runtime download failed (attempt $attempt/4); retrying in $((attempt * 30))s" >&2
    sleep $((attempt * 30))
  done
  [ "$ok" = 1 ] || { echo "ONNX Runtime download failed: $URL" >&2; exit 1; }
  echo "$SHA256  $DEST.tgz" | shasum -a 256 -c - >/dev/null
  # The archive has a single top-level directory. bsdtar (macOS) doesn't
  # support GNU tar's --strip-components, so extract and hoist manually.
  EXTRACT="$(mktemp -d "$DEST.extract.XXXXXX")"
  tar xzf "$DEST.tgz" -C "$EXTRACT"
  mv "$EXTRACT"/onnxruntime-osx-x86_64-${VERSION}/* "$DEST"/
  rm -rf "$EXTRACT" "$DEST.tgz"
fi

echo "$LIB_DIR"