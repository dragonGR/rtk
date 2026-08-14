#!/usr/bin/env sh
# Sync Cargo.toml version with latest release from rtk-ai/rtk
set -e

REPO="rtk-ai/rtk"

info() {
    printf "\033[0;32m[INFO]\033[0m %s\n" "$1"
}

warn() {
    printf "\033[1;33m[WARN]\033[0m %s\n" "$1"
}

# Fetch latest version tag from upstream GitHub repository
UPSTREAM_TAG=$(curl -sI "https://github.com/${REPO}/releases/latest" \
    | grep -i '^location:' \
    | sed -E 's|.*/tag/([^[:space:]]+).*|\1|' \
    | tr -d '\r')

if [ -z "$UPSTREAM_TAG" ]; then
    warn "Redirect lookup failed, falling back to GitHub API..."
    UPSTREAM_TAG=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name":' \
        | sed -E 's/.*"([^"]+)".*/\1/')
fi

if [ -z "$UPSTREAM_TAG" ]; then
    printf "\033[0;31m[ERROR]\033[0m Could not determine latest upstream version.\n" >&2
    exit 1
fi

UPSTREAM_VERSION=$(echo "$UPSTREAM_TAG" | sed 's/^v//')

CURRENT_VERSION=$(grep '^version =' Cargo.toml | head -n 1 | sed -E 's/version = "([^"]+)"/\1/')

info "Current Cargo.toml version: $CURRENT_VERSION"
info "Latest upstream version:     $UPSTREAM_VERSION ($UPSTREAM_TAG)"

if [ "$CURRENT_VERSION" != "$UPSTREAM_VERSION" ]; then
    info "Updating Cargo.toml version from $CURRENT_VERSION to $UPSTREAM_VERSION..."
    sed -i -E "s/^(version = \")[^\"]+(\")/\1${UPSTREAM_VERSION}\2/" Cargo.toml
    info "Cargo.toml updated successfully."
else
    info "Cargo.toml version is already up to date."
fi
