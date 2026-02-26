#!/usr/bin/env sh
# rtk installer - local source build only
# Usage:
#   ./install.sh

set -eu

BINARY_NAME="rtk"
INSTALL_DIR="${RTK_INSTALL_DIR:-$HOME/.local/bin}"
DEST_BIN="${INSTALL_DIR}/${BINARY_NAME}"
CLEAN_OLD="${RTK_CLEAN_OLD:-1}"
AUTO_INIT="${RTK_AUTO_INIT:-0}"
BUILD_JOBS="${RTK_BUILD_JOBS:-}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { printf "${GREEN}[INFO]${NC} %s\n" "$1"; }
warn() { printf "${YELLOW}[WARN]${NC} %s\n" "$1"; }
error() { printf "${RED}[ERROR]${NC} %s\n" "$1"; exit 1; }

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || error "Missing required command: $1"
}

timestamp() {
    date +%Y%m%d-%H%M%S
}

ensure_local_repo() {
    [ -f "./Cargo.toml" ] || error "Run this script from the rtk repo root (Cargo.toml not found)."
    grep -q '^name = "rtk"$' "./Cargo.toml" || error "Current Cargo.toml is not the rtk package."
    SOURCE_DIR="$(pwd)"
}

build_release_binary() {
    require_cmd cargo
    require_cmd rustc

    info "Building optimized release binary from local source..."
    (
        cd "${SOURCE_DIR}"
        BUILD_BASE_ARGS="build --release"
        if [ -n "${BUILD_JOBS}" ]; then
            export CARGO_BUILD_JOBS="${BUILD_JOBS}"
        fi

        # 1) Reproducible when lockfile is already up-to-date.
        if cargo ${BUILD_BASE_ARGS} --locked; then
            exit 0
        fi

        warn "Locked build failed (likely lockfile needs refresh). Retrying offline..."

        # 2) Refresh lockfile without network if possible.
        if cargo ${BUILD_BASE_ARGS} --offline; then
            exit 0
        fi

        warn "Offline build failed (cache may be incomplete). Retrying with normal Cargo resolution..."

        # 3) Final fallback.
        cargo ${BUILD_BASE_ARGS}
    ) || error "Cargo build failed"

    SOURCE_BIN="${SOURCE_DIR}/target/release/${BINARY_NAME}"
    [ -x "${SOURCE_BIN}" ] || error "Built binary not found at ${SOURCE_BIN}"

    if command -v strip >/dev/null 2>&1; then
        strip "${SOURCE_BIN}" >/dev/null 2>&1 || true
    fi
}

backup_if_exists() {
    target="$1"
    if [ -e "$target" ]; then
        [ -w "$target" ] || error "Cannot write to existing binary: $target (check permissions or use RTK_INSTALL_DIR)"
        backup="${target}.bak.$(timestamp)"
        mv "$target" "$backup" || error "Failed to backup existing binary: $target"
        info "Backed up existing $(basename "$target") to $backup"
    fi
}

clean_old_installations() {
    [ "${CLEAN_OLD}" = "1" ] || return 0

    for old in "$HOME/.cargo/bin/rtk" "$HOME/.local/bin/rtk" "/usr/local/bin/rtk"; do
        [ "$old" = "${DEST_BIN}" ] && continue
        [ -e "$old" ] || continue

        if [ -w "$old" ] || [ -w "$(dirname "$old")" ]; then
            backup="${old}.old.$(timestamp)"
            if mv "$old" "$backup"; then
                info "Archived old installation: $old -> $backup"
            else
                warn "Could not archive old installation (permission/sandbox): $old"
            fi
        else
            warn "Found old installation but cannot archive (permissions): $old"
        fi
    done
}

install_binary() {
    mkdir -p "${INSTALL_DIR}" || error "Cannot create install directory: ${INSTALL_DIR}"
    [ -w "${INSTALL_DIR}" ] || error "Install directory is not writable: ${INSTALL_DIR} (set RTK_INSTALL_DIR to a writable path)"
    backup_if_exists "${DEST_BIN}"
    cp "${SOURCE_BIN}" "${DEST_BIN}" || error "Failed to copy binary to ${DEST_BIN}"
    chmod +x "${DEST_BIN}" || error "Failed to set executable bit on ${DEST_BIN}"
    info "Installed ${BINARY_NAME} to ${DEST_BIN}"
}

verify_installation() {
    VERSION_OUTPUT="$("${DEST_BIN}" --version 2>/dev/null || true)"
    [ -n "${VERSION_OUTPUT}" ] || error "Installed binary failed to run: ${DEST_BIN}"
    info "Verification: ${VERSION_OUTPUT}"

    if "${DEST_BIN}" gain --help >/dev/null 2>&1; then
        info "Verified: 'rtk gain' command is available"
    else
        warn "Installed binary works, but 'rtk gain' check failed"
    fi

    if command -v rtk >/dev/null 2>&1; then
        ACTIVE_BIN="$(command -v rtk)"
        if [ "${ACTIVE_BIN}" = "${DEST_BIN}" ]; then
            info "PATH check: rtk resolves to ${DEST_BIN}"
        else
            warn "PATH check: rtk currently resolves to ${ACTIVE_BIN}"
            warn "Add ${INSTALL_DIR} earlier in PATH to prefer this install."
        fi
    else
        warn "Binary installed but not in PATH."
        warn "Add this to your shell profile:"
        warn "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    fi
}

maybe_init_claude() {
    [ "${AUTO_INIT}" = "1" ] || return 0

    if [ -x "${DEST_BIN}" ]; then
        info "Applying Claude hook setup (rtk init --global --auto-patch)..."
        "${DEST_BIN}" init --global --auto-patch || warn "Auto init failed. Run manually: rtk init --global --auto-patch"
    fi
}

main() {
    info "Installing ${BINARY_NAME} from local source..."
    ensure_local_repo
    build_release_binary
    clean_old_installations
    install_binary
    verify_installation
    maybe_init_claude

    echo ""
    info "Done."
    if [ "${AUTO_INIT}" = "1" ]; then
        info "Claude hook setup attempted automatically."
    else
        info "Optional next step: RTK_AUTO_INIT=1 ./install.sh  (or run: rtk init --global --auto-patch)"
    fi
}

main
