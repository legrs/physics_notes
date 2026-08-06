#!/usr/bin/env bash
# Install physq (Physics Notes terminal search) from the latest stable
# release. One script for macOS (Apple Silicon / Intel), Linux (x86_64 /
# aarch64), and Windows via Git Bash (MSYS2) — PowerShell users should use
# install_physq.ps1 instead.
#
# Downloads the matching unarchived binary, verifies its SHA-256 against the
# release's checksums.txt, clears the macOS Gatekeeper quarantine flag, and
# installs it into DEST_DIR. On Intel Mac it also installs
# libonnxruntime.1.23.2.dylib next to the binary — that build is dynamically
# linked and the two files must always stay side by side.
#
# Usage:
#   bash install_physq.sh [DEST_DIR]     # default: ./bin
#   bash install_physq.sh --global       # ~/.local/bin (macOS/Linux) or
#                                        # ~/bin (Git Bash), then add it to PATH
set -euo pipefail

case "${1:-}" in
  -g|--global) GLOBAL=1 ;;
  "")          GLOBAL=0 ;;
  *)           GLOBAL=0; DEST="$1" ;;
esac
BASE_URL="https://github.com/legrs/physics_notes/releases/latest/download"

# --- detect OS/arch and pick the asset names -------------------------------
case "$(uname -s)" in
  Darwin)  OS=macOS ;;                                      # macOS
  Linux)   OS=linux ;;
  MINGW*|MSYS*|CYGWIN*) OS=windows ;;
  *) echo "error: unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64)          ARCH=x86_64 ;;
  aarch64|arm64)         ARCH=aarch64 ;;
  *) echo "error: unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

DYLIB=""   # non-Intel/non-mac platforms have no companion library
case "$OS" in
  macOS)
    BIN="physq-bin-$ARCH-apple-darwin"
    [ "$ARCH" = x86_64 ] && DYLIB="libonnxruntime.1.23.2.dylib"
    ;;
  linux)   BIN="physq-bin-$ARCH-unknown-linux-gnu" ;;
  windows) BIN="physq-bin-$ARCH-pc-windows-msvc.exe" ;;
esac
EXE="physq"; [ "$OS" = windows ] && EXE="physq.exe"

if [ "${GLOBAL:-0}" = 1 ]; then
  case "$OS" in
    windows) DEST="$HOME/bin" ;;
    *)       DEST="$HOME/.local/bin" ;;
  esac
fi
DEST="${DEST:-./bin}"

# sha256sum (GNU, Git Bash) or shasum -a 256 (macOS); compare manually so the
# whole thing works even where `-c` formats differ.
hash_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "error: need sha256sum or shasum" >&2; exit 1
  fi
}

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/physq.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT

echo "Downloading $BIN from the latest release..."
curl -fL --retry 3 -o "$tmpdir/$BIN" "$BASE_URL/$BIN"
[ -n "$DYLIB" ] && curl -fL --retry 3 -o "$tmpdir/$DYLIB" "$BASE_URL/$DYLIB"
curl -fL --retry 3 -o "$tmpdir/checksums.txt" "$BASE_URL/checksums.txt"

echo "Verifying SHA-256 against checksums.txt..."
(
  cd "$tmpdir"
  verify() {
    local file="$1"
    local want got
    want="$(grep -E " ${file}$" checksums.txt | awk '{print $1}')"
    [ -n "$want" ] || { echo "error: no checksum for $file in checksums.txt" >&2; exit 1; }
    if command -v sha256sum >/dev/null 2>&1; then
      got="$(sha256sum "$file" | awk '{print $1}')"
    else
      got="$(shasum -a 256 "$file" | awk '{print $1}')"
    fi
    [ "$got" = "$want" ] || { echo "error: checksum mismatch for $file" >&2; exit 1; }
    echo "$file: OK"
  }
  verify "$BIN"
  if [ -n "$DYLIB" ]; then verify "$DYLIB"; fi
  exit 0
)

# --- install ---------------------------------------------------------------
mkdir -p "$DEST"
chmod +x "$tmpdir/$BIN"
mv "$tmpdir/$BIN" "$DEST/$EXE"
if [ -n "$DYLIB" ]; then mv "$tmpdir/$DYLIB" "$DEST/$DYLIB"; fi

# Gatekeeper quarantine (set by the curl download) blocks unsigned binaries
# until cleared — both files, since dyld checks the dylib too.
if [ "$OS" = macOS ] && command -v xattr >/dev/null 2>&1; then
  xattr -d com.apple.quarantine "$DEST/$EXE" 2>/dev/null || true
  [ -n "$DYLIB" ] && xattr -d com.apple.quarantine "$DEST/$DYLIB" 2>/dev/null || true
fi

echo
echo "Installed:"
echo "  $DEST/$EXE"
[ -n "$DYLIB" ] && echo "  $DEST/$DYLIB"
if [ -n "$DYLIB" ]; then
  echo "Keep these two files side by side — copying or moving physq alone"
  echo "breaks it (dyld: Library not loaded)."
fi
echo
"$DEST/$EXE" --version
echo "To update later: $DEST/$EXE update"

# --- optional global install: register DEST in the shell's PATH ---------------
if [ "${GLOBAL:-0}" = 1 ]; then
  if [ "$OS" = windows ]; then
    rc="$HOME/.bashrc"
  elif [ "$(basename "${SHELL:-/bin/zsh}")" = zsh ]; then
    rc="$HOME/.zshrc"
  else
    rc="$HOME/.bashrc"
  fi
  if grep -qF "$DEST" "$rc" 2>/dev/null; then
    echo "$DEST is already on PATH ($rc)"
  else
    touch "$rc"
    printf 'export PATH="%s:$PATH"\n' "$DEST" >> "$rc"
    echo "Added $DEST to PATH in $rc"
    echo "Restart your terminal, or run: source $rc"
  fi
  echo "Then run: physq --help"
fi