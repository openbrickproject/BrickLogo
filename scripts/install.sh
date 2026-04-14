#!/bin/sh
# BrickLogo installer
#
# Install or update BrickLogo with:
#
#   curl -fsSL https://raw.githubusercontent.com/openbrickproject/BrickLogo/main/scripts/install.sh | sh
#
# Installs to ~/.bricklogo with a symlink on PATH. Preserves any existing
# bricklogo.config.json on upgrade.

set -e

REPO="openbrickproject/BrickLogo"
INSTALL_DIR="$HOME/.bricklogo"

# ── Detect platform ──────────────────────────────

OS=$(uname -s)
ARCH=$(uname -m)

case "$OS-$ARCH" in
    Darwin-arm64)     PLATFORM="macos-arm64" ;;
    Linux-aarch64)    PLATFORM="linux-arm64" ;;
    MINGW*|MSYS*)     echo "On Windows, download the release zip from https://github.com/$REPO/releases"; exit 1 ;;
    *)                echo "Unsupported platform: $OS $ARCH"; exit 1 ;;
esac

# ── Fetch latest release tag ─────────────────────

echo "Fetching latest release..."
VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)

if [ -z "$VERSION" ]; then
    echo "Failed to determine latest release version."
    exit 1
fi

echo "Installing BrickLogo $VERSION for $PLATFORM..."

# ── Download and extract ─────────────────────────

URL="https://github.com/$REPO/releases/download/$VERSION/bricklogo-$VERSION-$PLATFORM.zip"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL" -o "$TMPDIR/bricklogo.zip"
unzip -q "$TMPDIR/bricklogo.zip" -d "$TMPDIR"

# ── Preserve user config on upgrade ──────────────

SAVED_CONFIG=""
if [ -f "$INSTALL_DIR/bricklogo.config.json" ]; then
    cp "$INSTALL_DIR/bricklogo.config.json" "$TMPDIR/bricklogo.config.json.bak"
    SAVED_CONFIG="$TMPDIR/bricklogo.config.json.bak"
fi

# ── Install to ~/.bricklogo ──────────────────────

rm -rf "$INSTALL_DIR"
mv "$TMPDIR/bricklogo" "$INSTALL_DIR"

if [ -n "$SAVED_CONFIG" ]; then
    cp "$SAVED_CONFIG" "$INSTALL_DIR/bricklogo.config.json"
fi

chmod +x "$INSTALL_DIR/bricklogo"

# ── macOS: strip quarantine flag ─────────────────

if [ "$OS" = "Darwin" ]; then
    xattr -d com.apple.quarantine "$INSTALL_DIR/bricklogo" 2>/dev/null || true
fi

# ── Symlink onto PATH ────────────────────────────

LINK_DIR="/usr/local/bin"
if [ ! -w "$LINK_DIR" ]; then
    LINK_DIR="$HOME/.local/bin"
    mkdir -p "$LINK_DIR"
fi

ln -sf "$INSTALL_DIR/bricklogo" "$LINK_DIR/bricklogo"

# ── Done ─────────────────────────────────────────

echo ""
echo "BrickLogo $VERSION installed successfully."
echo ""
echo "  Examples: $INSTALL_DIR/examples/"
echo "  Docs:     $INSTALL_DIR/docs/"
echo ""

# Check if the symlink dir is on PATH
case ":$PATH:" in
    *":$LINK_DIR:"*)
        echo "Run 'bricklogo' to get started."
        ;;
    *)
        echo "Note: $LINK_DIR is not on your PATH."
        echo "Add it with:  export PATH=\"$LINK_DIR:\$PATH\""
        echo "Then run 'bricklogo' to get started."
        ;;
esac
echo ""
