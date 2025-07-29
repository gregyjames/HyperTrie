#!/bin/bash
set -e

ROOT_DIR=$(pwd)
echo "Root directory: $ROOT_DIR"

# Install required targets
rustup target add x86_64-pc-windows-msvc
rustup target add i686-pc-windows-msvc
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Create output directories
mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/windows-x64"
mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/windows-x86"
mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/linux-x64"
mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/osx-x64"
mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/osx-arm64"

if [ ! -f "$ROOT_DIR/hypertrie/Cargo.toml" ]; then
    echo "Error: Cargo.toml not found at $ROOT_DIR/hypertrie/Cargo.toml"
    exit 1
fi

cd "$ROOT_DIR/hypertrie" || exit 1

# Build for Windows x64
echo "Building for Windows x64..."
RUSTFLAGS="-C target-feature=+aes,+sse2" cargo build --release --target x86_64-pc-windows-msvc
cp target/x86_64-pc-windows-msvc/release/hypertrie.dll "$ROOT_DIR/HyperTrieCore/target/release/windows-x64/hypertrie.dll"

# Build for Windows x86
echo "Building for Windows x86..."
RUSTFLAGS="-C target-feature=+aes,+sse2" cargo build --release --target i686-pc-windows-msvc
cp target/i686-pc-windows-msvc/release/hypertrie.dll "$ROOT_DIR/HyperTrieCore/target/release/windows-x86/hypertrie.dll"

# Build for Linux x64
echo "Building for Linux x64..."
RUSTFLAGS="-C target-feature=+aes,+sse2" cargo build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/libhypertrie.so "$ROOT_DIR/HyperTrieCore/target/release/linux-x64/libhypertrie.so"

# Build for macOS x64
echo "Building for macOS x64..."
cargo build --release --target x86_64-apple-darwin
cp target/x86_64-apple-darwin/release/libhypertrie.dylib "$ROOT_DIR/HyperTrieCore/target/release/osx-x64/libhypertrie.dylib"

# Build for macOS ARM64
echo "Building for macOS ARM64..."
cargo build --release --target aarch64-apple-darwin
cp target/aarch64-apple-darwin/release/libhypertrie.dylib "$ROOT_DIR/HyperTrieCore/target/release/osx-arm64/libhypertrie.dylib"

cd "$ROOT_DIR" || exit 1
echo "Build complete!" 