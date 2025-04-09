#!/bin/bash
set -e

VERSION="0.1.0"
NAME="pathranger"

# Create a dist directory
mkdir -p dist

# Define target architectures
TARGETS=(
    # Linux
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    
    # macOS
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    
    # Windows
    "x86_64-pc-windows-msvc"
    "aarch64-pc-windows-msvc"
)

# Check if we're on Linux and need to install Windows cross-compilation tools
if [[ "$(uname)" == "Linux" ]]; then
    if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
        echo "Installing Windows cross-compilation tools..."
        sudo apt-get update
        sudo apt-get install -y mingw-w64
    fi
fi

# Install SQLite development libraries if on Linux
if [[ "$(uname)" == "Linux" ]]; then
    if ! dpkg -l | grep -q libsqlite3-dev; then
        echo "Installing SQLite development libraries..."
        sudo apt-get update
        sudo apt-get install -y libsqlite3-dev
    fi
fi

# Build for each target
for TARGET in "${TARGETS[@]}"; do
    echo "Building for $TARGET..."
    
    # Add appropriate target if not already installed
    rustup target add "$TARGET" || true
    
    # Build
    cargo build --release --target "$TARGET"
    
    # Get platform info from target
    if [[ "$TARGET" == *"linux"* ]]; then
        PLATFORM="linux"
        EXT=""
    elif [[ "$TARGET" == *"apple"* ]]; then
        PLATFORM="macos"
        EXT=""
    elif [[ "$TARGET" == *"windows"* ]]; then
        PLATFORM="windows"
        EXT=".exe"
    else
        echo "Unknown platform in target: $TARGET"
        continue
    fi
    
    # Get architecture from target
    if [[ "$TARGET" == "x86_64"* ]]; then
        ARCH="x86_64"
    elif [[ "$TARGET" == "aarch64"* ]]; then
        ARCH="arm64"
    else
        echo "Unknown architecture in target: $TARGET"
        continue
    fi
    
    # Create package name
    PACKAGE_NAME="${NAME}-v${VERSION}-${ARCH}-${PLATFORM}"
    
    # Create archive
    if [[ "$PLATFORM" == "windows" ]]; then
        BINARY_PATH="target/$TARGET/release/${NAME}${EXT}"
        if [[ "$(uname)" == "Darwin" ]] || [[ "$(uname)" == "Linux" ]]; then
            # On macOS or Linux creating a zip for Windows
            zip -j "dist/${PACKAGE_NAME}.zip" "$BINARY_PATH"
        else
            # On Windows
            powershell Compress-Archive -Path "$BINARY_PATH" -DestinationPath "dist/${PACKAGE_NAME}.zip"
        fi
    else
        # For Linux and macOS
        tar -czf "dist/${PACKAGE_NAME}.tar.gz" -C "target/$TARGET/release" "${NAME}${EXT}"
    fi
    
    echo "Created dist/${PACKAGE_NAME} archive"
done

echo "All builds complete!"