#!/bin/bash

# Port Forward - macOS GUI Build Script
# Usage: ./build-macos-gui.sh

set -e

echo "=========================================="
echo "  Port Forward - macOS GUI Build"
echo "=========================================="

# Check Rust installation
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust is not installed."
    echo "Please install from https://rustup.rs"
    exit 1
fi

# Check Node.js installation
if ! command -v node &> /dev/null; then
    echo "Error: Node.js is not installed."
    echo "Please install from https://nodejs.org"
    exit 1
fi

# Navigate to project root
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo ""
echo "Installing dependencies..."
echo ""

# Install npm dependencies
npm install

echo ""
echo "Building frontend..."
echo ""

# Build frontend
npm run build

echo ""
echo "Building Tauri app for macOS..."
echo ""

# Build Tauri app (creates .app and .dmg)
npm run tauri build -- --target universal-apple-darwin

echo ""
echo "=========================================="
echo "  Build Complete!"
echo "=========================================="
echo ""

# Show output files
DMG_PATH="$SCRIPT_DIR/src-tauri/target/universal-apple-darwin/release/bundle/dmg"
APP_PATH="$SCRIPT_DIR/src-tauri/target/universal-apple-darwin/release/bundle/macos"

echo "Output files:"
echo ""

if [ -d "$DMG_PATH" ]; then
    echo "DMG (disk image):"
    ls -lh "$DMG_PATH"/*.dmg 2>/dev/null || echo "  DMG not found"
    echo ""
fi

if [ -d "$APP_PATH" ]; then
    echo "APP (application bundle):"
    ls -lh "$APP_PATH"/*.app 2>/dev/null || echo "  APP not found"
    echo ""
fi

echo "=========================================="
echo "To install: Open the DMG file and drag"
echo "PortForward.app to Applications folder"
echo "=========================================="
