#!/usr/bin/env bash
#
# kdb installer — downloads the latest prebuilt binary from GitHub releases.
#
# Usage:
#   curl -fsSL https://kdb.kernl.sh/install | bash
#
set -euo pipefail

REPO="dremnik/kdb"
INSTALL_DIR="${KDB_INSTALL_DIR:-$HOME/.local/bin}"

main() {
    detect_platform
    fetch_latest_tag
    download_and_verify
    install_binary
    print_success
}

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os="apple-darwin" ;;
        Linux)  os="unknown-linux-gnu" ;;
        *)
            err "unsupported OS: $os"
            err "install from source: cargo install --git https://github.com/$REPO"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        arm64|aarch64)   arch="aarch64" ;;
        *)
            err "unsupported architecture: $arch"
            err "install from source: cargo install --git https://github.com/$REPO"
            exit 1
            ;;
    esac

    TARGET="${arch}-${os}"
    ARCHIVE="kdb-${TARGET}.tar.gz"
}

fetch_latest_tag() {
    info "fetching latest release..."
    TAG="$(
        curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
            | grep '"tag_name"' \
            | head -1 \
            | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
    )"

    if [ -z "$TAG" ]; then
        err "could not determine latest release"
        exit 1
    fi

    info "latest release: $TAG"
    BASE_URL="https://github.com/$REPO/releases/download/$TAG"
}

download_and_verify() {
    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR"' EXIT

    info "downloading $ARCHIVE..."
    curl -fsSL -o "$TMPDIR/$ARCHIVE" "$BASE_URL/$ARCHIVE"
    curl -fsSL -o "$TMPDIR/checksums.txt" "$BASE_URL/checksums.txt"

    info "verifying checksum..."
    local expected actual
    expected="$(grep "$ARCHIVE" "$TMPDIR/checksums.txt" | awk '{print $1}')"
    if [ -z "$expected" ]; then
        err "archive not found in checksums.txt"
        exit 1
    fi

    if command -v sha256sum &>/dev/null; then
        actual="$(sha256sum "$TMPDIR/$ARCHIVE" | awk '{print $1}')"
    elif command -v shasum &>/dev/null; then
        actual="$(shasum -a 256 "$TMPDIR/$ARCHIVE" | awk '{print $1}')"
    else
        err "no sha256sum or shasum found — cannot verify checksum"
        exit 1
    fi

    if [ "$expected" != "$actual" ]; then
        err "checksum mismatch"
        err "  expected: $expected"
        err "  actual:   $actual"
        exit 1
    fi

    info "checksum ok"

    tar xzf "$TMPDIR/$ARCHIVE" -C "$TMPDIR"
    BINARY="$TMPDIR/kdb"
}

install_binary() {
    mkdir -p "$INSTALL_DIR"
    mv "$BINARY" "$INSTALL_DIR/kdb"
    chmod +x "$INSTALL_DIR/kdb"
    info "installed to $INSTALL_DIR/kdb"
}

print_success() {
    echo ""
    echo "  kdb installed successfully!"
    echo ""

    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        echo "  Add $INSTALL_DIR to your PATH:"
        echo ""
        echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
        echo ""
    fi
}

info() { echo "  [+] $*"; }
err()  { echo "  [!] $*" >&2; }

main
