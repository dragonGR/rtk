#!/usr/bin/env sh
# rtk local builder & installer - https://github.com/dragonGR/rtk
# Usage: ./install.sh

set -e

BINARY_NAME="rtk"
INSTALL_DIR="${RTK_INSTALL_DIR:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    printf "${GREEN}[INFO]${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}[WARN]${NC} %s\n" "$1"
}

error() {
    printf "${RED}[ERROR]${NC} %s\n" "$1"
    exit 1
}

# Check for cargo toolchain
check_cargo() {
    if ! command -v cargo >/dev/null 2>&1; then
        error "cargo is not installed or not in PATH. Please install Rust via https://rustup.rs"
    fi
}

# Clean previous build artifacts
clean_local() {
    info "Cleaning build directory..."
    cargo clean
}

# Build release binary locally
build_local() {
    if [ ! -f "Cargo.toml" ]; then
        error "Cargo.toml not found in current directory. Run install.sh from the rtk root folder."
    fi

    clean_local
    info "Building $BINARY_NAME in release mode..."
    cargo build --release ${RTK_BUILD_FLAGS}
}

# Copy compiled binary to INSTALL_DIR
install_local() {
    TARGET_BIN="target/release/${BINARY_NAME}"
    if [ ! -f "$TARGET_BIN" ]; then
        error "Compiled binary not found at $TARGET_BIN"
    fi

    info "Installing $BINARY_NAME to $INSTALL_DIR..."
    mkdir -p "$INSTALL_DIR"
    rm -f "${INSTALL_DIR}/${BINARY_NAME}"
    cp "$TARGET_BIN" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
}

# Verify installed binary
verify() {
    INSTALLED_BIN="${INSTALL_DIR}/${BINARY_NAME}"
    if [ -x "$INSTALLED_BIN" ]; then
        info "Verification: $("$INSTALLED_BIN" --version)"
    else
        error "Binary not found at expected location: $INSTALLED_BIN"
    fi

    case ":$PATH:" in
        *":$INSTALL_DIR:"*) ;;
        *)
            warn "Directory $INSTALL_DIR is not in your PATH. Add to your shell profile:"
            warn "  export PATH=\"$INSTALL_DIR:\$PATH\""
            ;;
    esac
}

main() {
    info "Installing $BINARY_NAME locally..."
    check_cargo
    build_local
    install_local
    verify

    echo ""
    info "Installation complete! Run '$BINARY_NAME --help' to get started."
}

main

