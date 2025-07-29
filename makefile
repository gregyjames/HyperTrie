# Define build mode (default to release)
BUILD_MODE ?= release  # Set to 'debug' or 'release'

# Define project paths
C_SHARP_PROJECT = HyperTrieCore
RUST_PROJECT = hypertrie

# Define output paths based on build mode
ifeq ($(BUILD_MODE), debug)
    RUST_OUTPUT_DIR = $(RUST_PROJECT)/target/debug
    C_SHARP_OUTPUT_DIR = $(C_SHARP_PROJECT)/bin/Debug/net8.0
    CARGO_BUILD_FLAG =
    DOTNET_BUILD_FLAG =
else
    RUST_OUTPUT_DIR = $(RUST_PROJECT)/target/release
    C_SHARP_OUTPUT_DIR = $(C_SHARP_PROJECT)/bin/Release/net8.0
    CARGO_BUILD_FLAG = --release
    DOTNET_BUILD_FLAG = --configuration Release
endif

# Rust library name
RUST_LIB_NAME = libhypertrie

# Detect OS and set shared library extension
ifeq ($(OS), Windows_NT)
    RUST_LIB_EXT = dll
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S), Linux)
        RUST_LIB_EXT = so
    else ifeq ($(UNAME_S), Darwin)
        RUST_LIB_EXT = dylib
    endif
endif

# Full path to the compiled Rust shared library
RUST_LIB_PATH = $(RUST_OUTPUT_DIR)/$(RUST_LIB_NAME).$(RUST_LIB_EXT)

# Build Rust library
.PHONY: rust
rust:
	@echo "Building Rust library in $(BUILD_MODE) mode..."
	cd $(RUST_PROJECT) && cargo build $(CARGO_BUILD_FLAG)

# Build C# project
.PHONY: csharp
csharp: copy_rust_lib
	@echo "Building C# project in $(BUILD_MODE) mode..."
	dotnet build $(C_SHARP_PROJECT) $(DOTNET_BUILD_FLAG)

# Copy Rust library to C# output folder
.PHONY: copy_rust_lib
copy_rust_lib: rust
	@echo "Copying Rust library to C# output directory: $(C_SHARP_OUTPUT_DIR)"
	mkdir -p $(C_SHARP_OUTPUT_DIR)  # Ensure directory exists
	cp $(RUST_LIB_PATH) HyperTrieCore/src/$(C_SHARP_OUTPUT_DIR)/
	
	# Also copy to the GitHub Actions build structure for local development
	@echo "Copying Rust library to GitHub Actions build structure..."
	mkdir -p HyperTrieCore/target/release/osx-x64
	mkdir -p HyperTrieCore/target/release/osx-arm64
	cp $(RUST_LIB_PATH) HyperTrieCore/target/release/osx-x64/libhypertrie.dylib
	cp $(RUST_LIB_PATH) HyperTrieCore/target/release/osx-arm64/libhypertrie.dylib

# Clean both projects
.PHONY: clean
clean:
	@echo "Cleaning Rust and C# projects..."
	cd $(RUST_PROJECT) && cargo clean
	dotnet clean $(C_SHARP_PROJECT)

# Run C# application
.PHONY: run
run: csharp copy_rust_lib
	@echo "Running C# application in $(BUILD_MODE) mode..."
	cd $(C_SHARP_PROJECT)/src/HyperTrieTester && dotnet run $(DOTNET_BUILD_FLAG)

# Clean both projects (debug & release)
.PHONY: clean
clean:
	@echo "Cleaning Rust and C# projects..."
	cd $(RUST_PROJECT) && cargo clean
	dotnet clean $(C_SHARP_PROJECT)
	rm -rf $(C_SHARP_PROJECT)/bin $(C_SHARP_PROJECT)/obj
	rm -rf $(RUST_PROJECT)/target
	
# Full build
.PHONY: all
all: rust csharp copy_rust_lib

# Run everything
.PHONY: full_run
full_run: all run
