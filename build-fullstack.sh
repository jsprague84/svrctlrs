#!/usr/bin/env bash
#
# Fullstack Build Script for SvrCtlRS
#
# Workaround for Dioxus CLI 0.7.1 "Could not automatically detect target triple" error
# This script manually builds both server and WASM client components
#

set -e  # Exit on error

echo "🔨 Building SvrCtlRS Fullstack Application..."
echo

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
RELEASE=${1:-release}  # Use --release by default, or pass "debug" as argument
BUILD_FLAG=""
if [ "$RELEASE" = "release" ]; then
    BUILD_FLAG="--release"
    BUILD_DIR="release"
else
    BUILD_DIR="debug"
fi

echo -e "${BLUE}Build mode:${NC} $RELEASE"
echo

# Step 1: Build Server Binary
echo -e "${GREEN}[1/3]${NC} Building server binary..."
cargo build $BUILD_FLAG \
    --package server \
    --bin server \
    --features server

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Server binary built successfully"
    echo -e "   Location: target/$BUILD_DIR/server"
else
    echo -e "${YELLOW}✗${NC} Server build failed"
    exit 1
fi
echo

# Step 2: Build WASM Client
echo -e "${GREEN}[2/3]${NC} Building WASM client..."
cargo build $BUILD_FLAG \
    --package server \
    --bin server \
    --target wasm32-unknown-unknown \
    --no-default-features \
    --features web

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} WASM client built successfully"
    echo -e "   Location: target/wasm32-unknown-unknown/$BUILD_DIR/server.wasm"
else
    echo -e "${YELLOW}✗${NC} WASM build failed"
    exit 1
fi
echo

# Step 3: Generate JavaScript bindings with wasm-bindgen
echo -e "${GREEN}[3/3]${NC} Generating JavaScript bindings..."

# Check if wasm-bindgen-cli is installed
if ! command -v wasm-bindgen &> /dev/null; then
    echo -e "${YELLOW}⚠${NC}  wasm-bindgen not found. Installing..."
    cargo install wasm-bindgen-cli
fi

# Create dist directory
mkdir -p dist

# Generate bindings
wasm-bindgen \
    --target web \
    --out-dir dist \
    --out-name svrctlrs \
    target/wasm32-unknown-unknown/$BUILD_DIR/server.wasm

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} JavaScript bindings generated"
    echo -e "   Location: dist/"
else
    echo -e "${YELLOW}✗${NC} wasm-bindgen failed"
    exit 1
fi
echo

# Summary
echo -e "${GREEN}═══════════════════════════════════════${NC}"
echo -e "${GREEN}✓ Fullstack build completed successfully!${NC}"
echo -e "${GREEN}═══════════════════════════════════════${NC}"
echo
echo "Artifacts:"
echo "  • Server binary:  target/$BUILD_DIR/server"
echo "  • WASM client:    dist/svrctlrs_bg.wasm"
echo "  • JS bindings:    dist/svrctlrs.js"
echo
echo "To run the server:"
echo "  ./target/$BUILD_DIR/server"
echo
