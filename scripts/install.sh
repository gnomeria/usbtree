#!/bin/sh
# usbtree installer — https://github.com/gnomeria/usbtree
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/gnomeria/usbtree/main/scripts/install.sh | sh
#
# Environment variables:
#   USBTREE_VERSION      install a specific version (e.g. "0.1.0"), default: latest release
#   USBTREE_INSTALL_DIR  install directory, default: /usr/local/bin if writable, else ~/.local/bin
set -eu

REPO="gnomeria/usbtree"
BIN="usbtree"

# ---- styled output ---------------------------------------------------------
if [ -t 1 ] && [ "${NO_COLOR:-}" = "" ]; then
    BOLD="$(printf '\033[1m')" DIM="$(printf '\033[2m')"
    RED="$(printf '\033[31m')" GREEN="$(printf '\033[32m')"
    YELLOW="$(printf '\033[33m')" RESET="$(printf '\033[0m')"
else
    BOLD="" DIM="" RED="" GREEN="" YELLOW="" RESET=""
fi

info() { printf '%s\n' "${DIM}·${RESET} $*"; }
ok()   { printf '%s\n' "${GREEN}✓${RESET} $*"; }
warn() { printf '%s\n' "${YELLOW}!${RESET} $*" >&2; }
die()  { printf '%s\n' "${RED}✗${RESET} $*" >&2; exit 1; }

# ---- download helper (curl or wget) ----------------------------------------
if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL "$1"; }
    fetch_to() { curl -fsSL -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -qO- "$1"; }
    fetch_to() { wget -qO "$2" "$1"; }
else
    die "need curl or wget to download $BIN"
fi

# ---- platform detection ----------------------------------------------------
OS="$(uname -s)"
case "$OS" in
    Linux)  OS=linux ;;
    Darwin) OS=darwin ;;
    MINGW* | MSYS* | CYGWIN*)
        die "this installer is for Linux/macOS — on Windows run in PowerShell: irm https://raw.githubusercontent.com/$REPO/main/scripts/install.ps1 | iex" ;;
    *) die "unsupported OS: $OS" ;;
esac

ARCH="$(uname -m)"
case "$ARCH" in
    x86_64 | amd64)  ARCH=amd64 ;;
    aarch64 | arm64) ARCH=arm64 ;;
    *) die "unsupported architecture: $ARCH" ;;
esac

if [ "$OS" = "darwin" ] && [ "$ARCH" = "amd64" ]; then
    die "prebuilt macOS binaries are Apple Silicon only — on Intel Macs run: cargo install --git https://github.com/$REPO"
fi

# ---- resolve version -------------------------------------------------------
VERSION="${USBTREE_VERSION:-}"
if [ -z "$VERSION" ]; then
    info "resolving latest release…"
    VERSION="$(fetch "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' | head -n1 | sed -E 's/.*"v?([^"]+)".*/\1/')" || true
    [ -n "$VERSION" ] || die "couldn't determine the latest release — is one published? Set USBTREE_VERSION or check https://github.com/$REPO/releases"
fi
VERSION="${VERSION#v}"

ASSET="${BIN}_${VERSION}_${OS}-${ARCH}.tar.gz"
BASE_URL="https://github.com/$REPO/releases/download/v$VERSION"

info "installing $BOLD$BIN v$VERSION$RESET ($OS-$ARCH)"

# ---- download + verify -----------------------------------------------------
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM

info "downloading $ASSET…"
fetch_to "$BASE_URL/$ASSET" "$TMP/$ASSET" \
    || die "download failed: $BASE_URL/$ASSET"

if fetch_to "$BASE_URL/checksums.txt" "$TMP/checksums.txt" 2>/dev/null; then
    EXPECTED="$(grep "  $ASSET\$" "$TMP/checksums.txt" | awk '{print $1}')"
    if [ -n "$EXPECTED" ]; then
        if command -v sha256sum >/dev/null 2>&1; then
            ACTUAL="$(sha256sum "$TMP/$ASSET" | awk '{print $1}')"
        elif command -v shasum >/dev/null 2>&1; then
            ACTUAL="$(shasum -a 256 "$TMP/$ASSET" | awk '{print $1}')"
        else
            ACTUAL=""
        fi
        if [ -z "$ACTUAL" ]; then
            warn "no sha256sum/shasum found — skipping checksum verification"
        elif [ "$ACTUAL" != "$EXPECTED" ]; then
            die "checksum mismatch for $ASSET (expected $EXPECTED, got $ACTUAL)"
        else
            ok "checksum verified"
        fi
    else
        warn "$ASSET not listed in checksums.txt — skipping verification"
    fi
else
    warn "checksums.txt not found in release — skipping verification"
fi

tar -xzf "$TMP/$ASSET" -C "$TMP" || die "couldn't extract $ASSET"
[ -f "$TMP/$BIN" ] || die "archive didn't contain the $BIN binary"

# ---- pick install dir ------------------------------------------------------
INSTALL_DIR="${USBTREE_INSTALL_DIR:-}"
if [ -z "$INSTALL_DIR" ]; then
    if [ -d /usr/local/bin ] && [ -w /usr/local/bin ]; then
        INSTALL_DIR=/usr/local/bin
    else
        INSTALL_DIR="$HOME/.local/bin"
    fi
fi
mkdir -p "$INSTALL_DIR" || die "couldn't create $INSTALL_DIR"

install -m 755 "$TMP/$BIN" "$INSTALL_DIR/$BIN" 2>/dev/null \
    || { cp "$TMP/$BIN" "$INSTALL_DIR/$BIN" && chmod 755 "$INSTALL_DIR/$BIN"; } \
    || die "couldn't install to $INSTALL_DIR (try USBTREE_INSTALL_DIR=~/.local/bin)"

# macOS Gatekeeper quarantines downloaded binaries. usbtree binaries are not
# code-signed or notarized, so clear the flag (the sha256 was verified above).
if [ "$OS" = "darwin" ]; then
    if command -v xattr >/dev/null 2>&1; then
        xattr -d com.apple.quarantine "$INSTALL_DIR/$BIN" 2>/dev/null || true
        warn "binaries are not notarized by Apple — quarantine flag cleared after checksum verification"
    else
        warn "binaries are not notarized by Apple — if Gatekeeper blocks it, run: xattr -d com.apple.quarantine $INSTALL_DIR/$BIN"
    fi
fi

ok "installed $BOLD$INSTALL_DIR/$BIN$RESET"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) warn "$INSTALL_DIR is not in your PATH — add: export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac

# Real bytes/s (usbmon) needs root, Linux only. sudo resolves commands against
# secure_path in /etc/sudoers, which excludes ~/.local/bin — so `sudo usbtree`
# fails with "command not found" when we install there. /usr/local/bin *is* in
# secure_path; offer an opt-in symlink so plain `sudo usbtree` works.
if [ "$OS" = linux ] && [ "$INSTALL_DIR" != /usr/local/bin ]; then
    DO_SYMLINK=""
    if [ "${USBTREE_SUDO_SYMLINK:-}" = 1 ]; then
        DO_SYMLINK=1                       # non-interactive opt-in (CI, curl|sh with no tty)
    elif [ -r /dev/tty ]; then
        # Ask on the terminal directly — stdin is the piped script under curl|sh,
        # so read from /dev/tty (same trick sudo uses for the password prompt).
        printf '%s' "${DIM}·${RESET} Real bytes/s (usbmon) needs root. Symlink into /usr/local/bin so ${BOLD}sudo $BIN${RESET} works? [y/N] "
        read -r ANS </dev/tty || ANS=""
        case "$ANS" in [yY]*) DO_SYMLINK=1 ;; esac
    fi
    if [ -n "$DO_SYMLINK" ]; then
        if sudo ln -sf "$INSTALL_DIR/$BIN" "/usr/local/bin/$BIN"; then
            ok "symlinked /usr/local/bin/$BIN — ${BOLD}sudo $BIN${RESET} now works"
        else
            warn "couldn't create /usr/local/bin/$BIN symlink"
        fi
    else
        info "for real bytes/s (usbmon, needs root): ${BOLD}sudo \"\$(command -v $BIN)\"${RESET}"
    fi
fi

printf '\n%s\n' "Run ${BOLD}usbtree${RESET} for the TUI, ${BOLD}usbtree --dump${RESET} to print the tree once, or ${BOLD}usbtree --updatelist${RESET} to refresh the usb.ids database."
