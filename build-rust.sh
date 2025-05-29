#!/bin/bash
set -e

# Parse command line arguments
PLATFORMS=""
while [[ $# -gt 0 ]]; do
  case $1 in
    --platforms)
      PLATFORMS="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

# Store the root directory
ROOT_DIR=$(pwd)
echo "Root directory: $ROOT_DIR"

# Function to build for a specific platform
build_platform() {
    local platform=$1
    local target=$2
    local output_dir=$3
    local rustflags=$4

    echo "Building for $platform..."
    echo "Current directory: $(pwd)"
    
    if [ -n "$rustflags" ]; then
        RUSTFLAGS="$rustflags" cargo build --release --target "$target"
    else
        cargo build --release --target "$target"
    fi

    # Copy the built library to the output directory
    case $platform in
        windows-*)
            cp "target/$target/release/hypertrie.dll" "$ROOT_DIR/HyperTrieCore/target/release/$output_dir/libhypertrie.dll"
            ;;
        linux-*)
            cp "target/$target/release/libhypertrie.so" "$ROOT_DIR/HyperTrieCore/target/release/$output_dir/libhypertrie.so"
            ;;
        osx-*)
            cp "target/$target/release/libhypertrie.dylib" "$ROOT_DIR/HyperTrieCore/target/release/$output_dir/libhypertrie.dylib"
            ;;
    esac
}

# Create output directories for requested platforms
if [ -z "$PLATFORMS" ]; then
    echo "No platforms specified, building for all platforms"
    PLATFORMS="windows-x64,windows-x86,linux-x64,osx-x64,osx-arm64"
fi

IFS=',' read -ra PLATFORM_ARRAY <<< "$PLATFORMS"
for platform in "${PLATFORM_ARRAY[@]}"; do
    mkdir -p "$ROOT_DIR/HyperTrieCore/target/release/$platform"
done

# Verify Cargo.toml exists
if [ ! -f "$ROOT_DIR/hypertrie/Cargo.toml" ]; then
    echo "Error: Cargo.toml not found at $ROOT_DIR/hypertrie/Cargo.toml"
    exit 1
fi

cd "$ROOT_DIR/hypertrie" || exit 1

# Build for each requested platform
for platform in "${PLATFORM_ARRAY[@]}"; do
    case $platform in
        windows-x64)
            build_platform "$platform" "x86_64-pc-windows-msvc" "windows-x64" "-C target-feature=+aes,+sse2"
            ;;
        windows-x86)
            build_platform "$platform" "i686-pc-windows-msvc" "windows-x86" "-C target-feature=+aes,+sse2"
            ;;
        linux-x64)
            build_platform "$platform" "x86_64-unknown-linux-gnu" "linux-x64" "-C target-feature=+aes,+sse2"
            ;;
        osx-x64)
            build_platform "$platform" "x86_64-apple-darwin" "osx-x64" "-C target-feature=+aes,+sse2"
            ;;
        osx-arm64)
            build_platform "$platform" "aarch64-apple-darwin" "osx-arm64" ""
            ;;
    esac
done

cd "$ROOT_DIR" || exit 1
echo "Build complete!" 