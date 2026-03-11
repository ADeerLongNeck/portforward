#!/bin/bash

# Port Forward CLI - Cross-platform Build Script
# Usage: ./build-cli.sh

set -e

echo "=========================================="
echo "  Port Forward CLI Build"
echo "=========================================="

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Darwin) OS_NAME="macOS" ;;
    Linux)  OS_NAME="Linux" ;;
    MINGW*|MSYS*|CYGWIN*) OS_NAME="Windows" ;;
    *) OS_NAME="$OS" ;;
esac

echo "Detected OS: $OS_NAME"
echo ""

# Check Rust installation
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust is not installed."
    echo "Please install from https://rustup.rs"
    exit 1
fi

# Navigate to src-tauri directory
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/src-tauri"

echo "Building CLI release for $OS_NAME..."
echo ""

# Build CLI version (without GUI/tauri)
cargo build --release --no-default-features --features cli

echo ""
echo "=========================================="
echo "  Build Complete!"
echo "=========================================="
echo ""

# Determine binary name based on OS
if [[ "$OS" == MINGW* ]] || [[ "$OS" == MSYS* ]] || [[ "$OS" == CYGWIN* ]]; then
    BINARY="$SCRIPT_DIR/src-tauri/target/release/port-forward.exe"
else
    BINARY="$SCRIPT_DIR/src-tauri/target/release/port-forward"
fi

if [ -f "$BINARY" ]; then
    SIZE=$(ls -lh "$BINARY" | awk '{print $5}')
    echo "Binary: $BINARY"
    echo "Size: $SIZE"
    echo ""
    echo "Usage:"
    echo "  # Server mode"
    echo "  ./port-forward server --port 5173 --password mypass --forward 1080,8080"
    echo ""
    echo "  # Client mode"
    echo "  ./port-forward client --host server.com --port 5173 --password mypass"
    echo ""

    # On Unix systems, suggest installation
    if [[ "$OS" != MINGW* ]] && [[ "$OS" != MSYS* ]] && [[ "$OS" != CYGWIN* ]]; then
        echo "Install to /usr/local/bin (optional):"
        echo "  sudo cp $BINARY /usr/local/bin/port-forward"
        echo "  sudo chmod +x /usr/local/bin/port-forward"
    fi
fi
