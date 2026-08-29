#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Installer for repolish.
#
#   curl -fsSL https://raw.githubusercontent.com/asale-ai/repolish/main/install.sh | sh
#
# Options (environment variables):
#   REPOLISH_VERSION    tag to install, e.g. v0.2.0 (default: latest release)
#   REPOLISH_BIN_DIR    install directory          (default: ~/.local/bin)
#   REPOLISH_TARGET     which agents get the skill:
#                         detect (default) only the agents installed here
#                         all              every agent repolish knows about
#                         claude|codex|gemini|opencode|agents
#                         none             binary only
#   REPOLISH_NO_SKILL   set to 1 to skip the skill entirely
#
# POSIX sh on purpose: this has to run on Alpine, on minimal CI images, and on
# macOS's ancient bash. No arrays, no [[ ]], no process substitution.

set -eu

REPO="asale-ai/repolish"
BIN_NAME="repolish"
BIN_DIR="${REPOLISH_BIN_DIR:-$HOME/.local/bin}"
SKILL_TARGET="${REPOLISH_TARGET:-detect}"

RED=''; GREEN=''; YELLOW=''; BOLD=''; RESET=''
if [ -t 1 ] && [ "${NO_COLOR:-}" = "" ]; then
  RED=$(printf '\033[31m'); GREEN=$(printf '\033[32m')
  YELLOW=$(printf '\033[33m'); BOLD=$(printf '\033[1m'); RESET=$(printf '\033[0m')
fi

info() { printf '%s\n' "$*"; }
step() { printf '%s==>%s %s\n' "$BOLD" "$RESET" "$*"; }
warn() { printf '%swarning:%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
die()  { printf '%serror:%s %s\n' "$RED" "$RESET" "$*" >&2; exit 1; }

have() { command -v "$1" > /dev/null 2>&1; }

cleanup() { [ -n "${TMP_DIR:-}" ] && rm -rf "$TMP_DIR"; }
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------- platform

detect_target() {
  os=$(uname -s)
  arch=$(uname -m)

  case "$os" in
    Darwin) os_part="apple-darwin" ;;
    Linux)  os_part="unknown-linux-gnu" ;;
    MINGW*|MSYS*|CYGWIN*)
      die "Windows detected. Download the .zip from the releases page, or:
    cargo install repolish
  https://github.com/$REPO/releases" ;;
    *) die "unsupported operating system: $os" ;;
  esac

  case "$arch" in
    x86_64|amd64)  arch_part="x86_64" ;;
    arm64|aarch64) arch_part="aarch64" ;;
    *) die "unsupported architecture: $arch
repolish ships x86_64 and aarch64 builds. Build from source instead:
    cargo install repolish" ;;
  esac

  # The Linux builds are glibc-only. Saying so beats installing a binary that
  # dies with a linker error the first time it runs.
  if [ "$os_part" = "unknown-linux-gnu" ]; then
    if have ldd && ldd --version 2>&1 | grep -qi 'musl'; then
      die "musl libc detected (Alpine or similar).
repolish only ships glibc builds today, and a gnu binary will not run here.
Build it instead:
    cargo install repolish"
    fi
  fi

  # macOS on Apple silicon can run the x86_64 build under Rosetta, but the
  # native one is right there — only fall back if the arch build is missing.
  TARGET="${arch_part}-${os_part}"
}

# ------------------------------------------------------------------ helpers

download() {
  url="$1"; dest="$2"
  if have curl; then
    curl -fsSL --retry 3 --retry-delay 2 -o "$dest" "$url" || return 1
  elif have wget; then
    wget -qO "$dest" "$url" || return 1
  else
    die "neither curl nor wget is available"
  fi
}

sha256_of() {
  if have sha256sum; then sha256sum "$1" | cut -d' ' -f1
  elif have shasum; then shasum -a 256 "$1" | cut -d' ' -f1
  elif have openssl; then openssl dgst -sha256 "$1" | awk '{print $NF}'
  else return 1
  fi
}

resolve_version() {
  if [ -n "${REPOLISH_VERSION:-}" ]; then
    VERSION="$REPOLISH_VERSION"
    return
  fi
  step "Resolving the latest release"
  api="https://api.github.com/repos/$REPO/releases/latest"
  if have curl; then body=$(curl -fsSL "$api" 2>/dev/null || true)
  else body=$(wget -qO- "$api" 2>/dev/null || true)
  fi
  VERSION=$(printf '%s' "$body" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)
  [ -n "$VERSION" ] || die "could not determine the latest release from $api.
Set REPOLISH_VERSION=vX.Y.Z to install a specific version."
}

# ------------------------------------------------------------------- install

main() {
  detect_target
  resolve_version

  # The archive name is a contract with .github/workflows/release.yml:
  #   repolish-{tag}-{target}.tar.gz     — note the tag keeps its v prefix
  ASSET="${BIN_NAME}-${VERSION}-${TARGET}.tar.gz"
  BASE="https://github.com/$REPO/releases/download/$VERSION"

  TMP_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t repolish)
  info "  version:  $VERSION"
  info "  platform: $TARGET"
  info "  install:  $BIN_DIR"
  info ""

  step "Downloading $ASSET"
  if ! download "$BASE/$ASSET" "$TMP_DIR/$ASSET"; then
    die "download failed: $BASE/$ASSET
The release may not include a build for $TARGET.
See https://github.com/$REPO/releases/tag/$VERSION"
  fi

  step "Verifying checksum"
  # Each archive ships its own .sha256 next to it, in `shasum` format:
  #   <hex>  <filename>
  if download "$BASE/$ASSET.sha256" "$TMP_DIR/$ASSET.sha256"; then
    expected=$(cut -d' ' -f1 < "$TMP_DIR/$ASSET.sha256" | tr -d '\r\n')
    if actual=$(sha256_of "$TMP_DIR/$ASSET"); then
      if [ "$expected" != "$actual" ]; then
        die "checksum mismatch for $ASSET
  expected: $expected
  actual:   $actual
Nothing was installed. That means a corrupted download or a tampered artifact —
re-run, and report it if it persists."
      fi
      info "  ok: $actual"
    else
      warn "no SHA-256 tool found (sha256sum, shasum, or openssl); skipping verification"
    fi
  else
    warn "could not download $ASSET.sha256; continuing unverified"
  fi

  step "Extracting"
  tar xzf "$TMP_DIR/$ASSET" -C "$TMP_DIR" || die "could not extract $ASSET"
  SRC="$TMP_DIR/${BIN_NAME}-${VERSION}-${TARGET}/$BIN_NAME"
  [ -f "$SRC" ] || SRC=$(find "$TMP_DIR" -name "$BIN_NAME" -type f | head -n1)
  [ -f "$SRC" ] || die "the archive did not contain a $BIN_NAME binary"

  step "Installing to $BIN_DIR"
  mkdir -p "$BIN_DIR" || die "cannot create $BIN_DIR
Set REPOLISH_BIN_DIR to a writable directory and re-run."
  # Write to a temp name and rename: replacing a running binary in place
  # truncates it, and `mv` within one directory is atomic.
  install_tmp="$BIN_DIR/.$BIN_NAME.$$"
  cp "$SRC" "$install_tmp" || die "cannot write to $BIN_DIR
Set REPOLISH_BIN_DIR to a writable directory and re-run."
  chmod 755 "$install_tmp"
  mv -f "$install_tmp" "$BIN_DIR/$BIN_NAME" || die "cannot replace $BIN_DIR/$BIN_NAME"

  if ! "$BIN_DIR/$BIN_NAME" --version > /dev/null 2>&1; then
    die "the installed binary did not run.
That usually means an architecture mismatch — detected $TARGET."
  fi
  info "  ${GREEN}installed${RESET} $("$BIN_DIR/$BIN_NAME" --version)"

  if [ "${REPOLISH_NO_SKILL:-}" != "1" ] && [ "$SKILL_TARGET" != "none" ]; then
    info ""
    step "Installing the agent skill"
    # The binary owns the target list, so this script never has to know where
    # any agent keeps its skills. One place to update when a new one appears.
    "$BIN_DIR/$BIN_NAME" . --stages skill --target "$SKILL_TARGET" --force --apply \
      || warn "skill installation failed; run '$BIN_NAME --list' to see the options"
  fi

  info ""
  case ":$PATH:" in
    *":$BIN_DIR:"*)
      info "${GREEN}Done.${RESET} Try: ${BOLD}$BIN_NAME .${RESET}"
      ;;
    *)
      shell_name=$(basename "${SHELL:-sh}")
      case "$shell_name" in
        zsh)  rc="~/.zshrc" ;;
        bash) rc="~/.bashrc" ;;
        fish) rc="~/.config/fish/config.fish" ;;
        *)    rc="your shell profile" ;;
      esac
      info "${GREEN}Done.${RESET}"
      info ""
      info "${YELLOW}$BIN_DIR is not on your PATH.${RESET} The skill calls '$BIN_NAME' by"
      info "name, so add it before an agent tries to use it:"
      info ""
      if [ "$shell_name" = "fish" ]; then
        info "  ${BOLD}fish_add_path $BIN_DIR${RESET}"
      else
        info "  ${BOLD}echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> $rc${RESET}"
        info "  ${BOLD}exec \$SHELL${RESET}"
      fi
      ;;
  esac
}

main "$@"
