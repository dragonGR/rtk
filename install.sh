#!/usr/bin/env sh
# Local installer for dragonGR/rtk
# Usage:
#   ./install.sh
#   ./install.sh /path/to/rtk

set -e

BINARY_NAME="rtk"
INSTALL_DIR="${RTK_INSTALL_DIR:-$HOME/.local/bin}"
REPO_DIR="${1:-$(pwd)}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

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

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        error "Missing required command: $1"
    fi
}

verify_repo() {
    if [ ! -f "${REPO_DIR}/Cargo.toml" ]; then
        error "No Cargo.toml found in ${REPO_DIR}"
    fi

    if [ ! -f "${REPO_DIR}/src/main.rs" ]; then
        error "No src/main.rs found in ${REPO_DIR}"
    fi
}

clean_existing() {
    mkdir -p "$INSTALL_DIR"

    info "Cleaning previous local build artifacts"
    cargo clean --manifest-path "${REPO_DIR}/Cargo.toml"
}

install_local() {
    mkdir -p "$INSTALL_DIR"

    info "Installing ${BINARY_NAME} from local checkout: ${REPO_DIR}"
    info "Install dir: ${INSTALL_DIR}"

    cargo install \
        --path "$REPO_DIR" \
        --locked \
        --force \
        --root "${INSTALL_DIR%/bin}"
}

verify_install() {
    if [ -x "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        info "Installed binary: ${INSTALL_DIR}/${BINARY_NAME}"
        info "Version: $("${INSTALL_DIR}/${BINARY_NAME}" --version)"
    else
        error "Install finished but ${INSTALL_DIR}/${BINARY_NAME} was not created"
    fi

    case ":$PATH:" in
        *":${INSTALL_DIR}:"*)
            ;;
        *)
            warn "${INSTALL_DIR} is not on PATH"
            warn "Add this to your shell profile:"
            warn "  export PATH=\"${INSTALL_DIR}:\$PATH\""
            ;;
    esac
}

main() {
    require_command cargo
    verify_repo
    clean_existing
    install_local
    verify_install

    echo ""
    info "Local installation complete."
}

main
