#!/usr/bin/env bash
set -e

ROOT_DIR=$(pwd)
echo "Root directory: $ROOT_DIR"

# Parse --platforms argument
PLATFORMS="linux-x64,osx-x64,osx-arm64"
while [[ $# -gt 0 ]]; do
  case $1 in
    --platforms)
      PLATFORMS="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

IFS=',' read -ra PLATFORM_ARRAY <<< "$PLATFORMS"
echo "Requested platforms: ${PLATFORM_ARRAY[@]}"

# Cross-platform copy function
do_copy() {
  src="$1"
  dst="$2"
  if command -v cp >/dev/null 2>&1; then
    cp "$src" "$dst"
  else
    python -c "import shutil; shutil.copyfile('$src', '$dst')"
  fi
}

# Install required targets
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Create output directories
mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/linux-x64"
mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/osx-x64"
mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/osx-arm64"

if [ ! -f "$ROOT_DIR/hypertrie/Cargo.toml" ]; then
    echo "Error: Cargo.toml not found at $ROOT_DIR/hypertrie/Cargo.toml"
    exit 1
fi

cd "$ROOT_DIR/hypertrie" || exit 1

for platform in "${PLATFORM_ARRAY[@]}"; do
  case $platform in
    linux-x64)
      echo "Building for Linux x64..."
      RUSTFLAGS="-C target-feature=+aes,+sse2" cargo build --release --target x86_64-unknown-linux-gnu
      do_copy target/x86_64-unknown-linux-gnu/release/libhypertrie.so "$ROOT_DIR/HyperTrieCore/target/release/linux-x64/libhypertrie.so"
      ;;
    osx-x64)
      echo "Building for macOS x64..."
      cargo build --release --target x86_64-apple-darwin
      do_copy target/x86_64-apple-darwin/release/libhypertrie.dylib "$ROOT_DIR/HyperTrieCore/target/release/osx-x64/libhypertrie.dylib"
      ;;
    osx-arm64)
      echo "Building for macOS ARM64..."
      cargo build --release --target aarch64-apple-darwin
      do_copy target/aarch64-apple-darwin/release/libhypertrie.dylib "$ROOT_DIR/HyperTrieCore/target/release/osx-arm64/libhypertrie.dylib"
      ;;
    *)
      echo "Unknown platform: $platform"
      ;;
  esac
done

cd "$ROOT_DIR" || exit 1
echo "Build complete!"
echo "Listing build output:"
ls -R "$ROOT_DIR/HyperTrieCore/target/release" || true 