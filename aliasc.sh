#!/bin/sh
# aliasc binary downloader.
# Ensures the matching release binary is cached locally.
# If called with extra arguments, downloads if needed then execs the binary.

set -u

repo=${ALIASC_REPOSITORY:-dodorz/aliasc}
release=${ALIASC_RELEASE:-latest}
cache_root=${ALIASC_BINARY_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/aliasc/bin}

fail() {
    printf '%s\n' "aliasc: $*" >&2
    exit 1
}

os=$(uname -s 2>/dev/null || true)
arch=$(uname -m 2>/dev/null || true)
asset=

case "$os" in
    Linux)
        case "$arch" in
            x86_64|amd64) asset=aliasc-x86_64-unknown-linux-gnu ;;
            aarch64|arm64)
                if [ "${PREFIX-}" = "/data/data/com.termux/files/usr" ] || [ -n "${TERMUX_VERSION-}" ]; then
                    asset=aliasc-aarch64-linux-android
                else
                    fail "no Linux ARM64 release asset; Android/Termux ARM64 requires a Termux environment"
                fi
                ;;
            *) fail "unsupported Linux architecture: $arch" ;;
        esac
        ;;
    Darwin)
        case "$arch" in
            x86_64) asset=aliasc-x86_64-apple-darwin ;;
            arm64|aarch64) asset=aliasc-aarch64-apple-darwin ;;
            *) fail "unsupported macOS architecture: $arch" ;;
        esac
        ;;
    MINGW*|MSYS*|CYGWIN*)
        case "$arch" in
            x86_64|amd64) asset=aliasc-x86_64-pc-windows-msvc.exe ;;
            i?86) asset=aliasc-i686-pc-windows-msvc.exe ;;
            aarch64|arm64) asset=aliasc-aarch64-pc-windows-msvc.exe ;;
            *) fail "unsupported Windows architecture: $arch" ;;
        esac
        ;;
    *) fail "unsupported platform: $os ($arch)" ;;
esac

[ -n "$asset" ] || fail "could not determine a release asset"
mkdir -p "$cache_root" || fail "cannot create cache directory: $cache_root"
binary=$cache_root/$asset

if [ ! -x "$binary" ]; then
    tmp=$(mktemp "$cache_root/.${asset}.tmp.XXXXXX") || fail "cannot create download temporary file"
    trap 'rm -f "$tmp"' EXIT HUP INT TERM
    url="https://github.com/$repo/releases/$release/download/$asset"
    if command -v curl >/dev/null 2>&1; then
        curl --fail --location --silent --show-error --retry 2 --connect-timeout 10 --max-time 45 --output "$tmp" "$url" || fail "download failed: $url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q --timeout=15 --tries=2 -O "$tmp" "$url" || fail "download failed: $url"
    else
        fail "curl or wget is required to download $asset"
    fi
    chmod 0755 "$tmp" || fail "cannot mark downloaded binary executable: $tmp"
    mv -f "$tmp" "$binary" || fail "cannot install downloaded binary: $binary"
    trap - EXIT HUP INT TERM
fi

if [ $# -gt 0 ]; then
    case "${1-}" in
        --print-binary-path) printf '%s\n' "$binary"; exit 0 ;;
        *) exec "$binary" "$@" ;;
    esac
fi
