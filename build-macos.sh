#!/bin/bash

# Port Forward CLI - macOS Build Script
# Usage: ./build-macos.sh

set -e

echo "=========================================="
echo "  Port Forward CLI - macOS Build"
echo "=========================================="

# Check Rust installation
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust is not installed."
    echo "Please install from https://rustup.rs"
    exit 1
fi

# Navigate to src-tauri directory
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/src-tauri"

echo ""
echo "Building CLI release for macOS..."
echo ""

# Build CLI version (without GUI/tauri)
cargo build --release --no-default-features --features cli

echo ""
echo "=========================================="
echo "  Build Complete!"
echo "=========================================="
echo ""
echo "Binary: $SCRIPT_DIR/src-tauri/target/release/port-forward"
echo ""

# Show binary info
BINARY="$SCRIPT_DIR/src-tauri/target/release/port-forward"
if [ -f "$BINARY" ]; then
    SIZE=$(ls -lh "$BINARY" | awk '{print $5}')
    echo "Size: $SIZE"
    echo ""
    echo "Usage:"
    echo "  ./port-forward server --port 5173 --password mypass --forward 1080,8080"
    echo "  ./port-forward client --host server.com --port 5173 --password mypass"
    echo ""
    echo "Copy binary to /usr/local/bin (optional):"
    echo "  sudo cp $BINARY /usr/local/bin/port-forward"
fi
