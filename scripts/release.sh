#!/bin/bash
set -e

# Get version from Cargo.toml
VERSION=$(grep '^version' crates/bricklogo/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')

# Detect platform and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    darwin)  PLATFORM="macos" ;;
    linux)   PLATFORM="linux" ;;
    mingw*|msys*|cygwin*) PLATFORM="windows" ;;
    *)       PLATFORM="$OS" ;;
esac

case "$ARCH" in
    x86_64|amd64)  ARCH="x64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    armv7l)        ARCH="armv7" ;;
esac

BINARY="bricklogo"
if [ "$PLATFORM" = "windows" ]; then
    BINARY="bricklogo.exe"
fi

ZIP_NAME="bricklogo-v${VERSION}-${PLATFORM}-${ARCH}.zip"

echo "Building BrickLogo v${VERSION} for ${PLATFORM}-${ARCH}..."
cargo build --release --bin bricklogo

echo "Creating ${ZIP_NAME}..."

# Create a temp directory for the zip contents
STAGING=$(mktemp -d)
mkdir -p "$STAGING/bricklogo"

cp "target/release/${BINARY}" "$STAGING/bricklogo/"
cp bricklogo.config.json.example "$STAGING/bricklogo/"
cp -r examples "$STAGING/bricklogo/"
cp -r assets "$STAGING/bricklogo/"

cd "$STAGING"
zip -r "${ZIP_NAME}" bricklogo/
cd -

mkdir -p releases
mv "$STAGING/${ZIP_NAME}" releases/
rm -rf "$STAGING"

echo "Done: releases/${ZIP_NAME}"
