#!/bin/sh
# aliasc binary downloader.
# Ensures the matching release binary is cached locally.
# If called with extra arguments, downloads if needed then execs the binary.
#
# Usage:
#   aliasc.sh [OPTIONS] [-- COMMAND...]
#
# Options:
#   --force              Force re-download even if binary already exists
#   --platform PLATFORM  Override platform detection (e.g. linux-x86_64-musl)
#   --list-platforms     List available platform identifiers and exit
#   --print-binary-path  Print the cached binary path and exit
#   --help               Show this help message and exit
#
# Platform identifiers:
#   linux-x86_64-gnu, linux-x86_64-musl, linux-aarch64-android,
#   darwin-x86_64, darwin-aarch64,
#   windows-x86_64, windows-i686, windows-aarch64

set -u

repo=${ALIASC_REPOSITORY:-dodorz/aliasc}
release=${ALIASC_RELEASE:-latest}
cache_root=${ALIASC_BINARY_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/aliasc/bin}

fail() {
    printf '%s\n' "aliasc: $*" >&2
    exit 1
}

usage() {
    sed -n '2,/^$/{ s/^# \?//; p; }' "$0" >&2
    exit 0
}

list_platforms() {
    printf '%s\n' \
        "linux-x86_64-gnu" \
        "linux-x86_64-musl" \
        "linux-aarch64-android" \
        "darwin-x86_64" \
        "darwin-aarch64" \
        "windows-x86_64" \
        "windows-i686" \
        "windows-aarch64"
    exit 0
}

__aliasc_have_glibc() {
    major=$1
    minor=$2
    ldd_out=$(ldd --version 2>&1 | head -n1) || return 1
    ver=$(printf '%s' "$ldd_out" | sed -n 's/.*GLIBC \([0-9]\+\)\.\([0-9]\+\).*/\1 \2/p')
    [ -n "$ver" ] || return 1
    cur_major=${ver%% *}
    cur_minor=${ver#* }
    [ "$cur_major" -gt "$major" ] || { [ "$cur_major" -eq "$major" ] && [ "$cur_minor" -ge "$minor" ]; }
}

__aliasc_download() {
    _asset=$1
    _url="https://github.com/$repo/releases/$release/download/$_asset"
    _tmp=$(mktemp "$cache_root/.${_asset}.tmp.XXXXXX") || fail "cannot create download temporary file"
    trap 'rm -f "$_tmp"' EXIT HUP INT TERM
    if command -v curl >/dev/null 2>&1; then
        curl --fail --location --silent --show-error --retry 2 --connect-timeout 10 --max-time 45 --output "$_tmp" "$_url" || fail "download failed: $_url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q --timeout=15 --tries=2 -O "$_tmp" "$_url" || fail "download failed: $_url"
    else
        fail "curl or wget is required to download $_asset"
    fi
    chmod 0755 "$_tmp" || fail "cannot mark downloaded binary executable: $_tmp"
    mv -f "$_tmp" "$cache_root/$_asset" || fail "cannot install downloaded binary: $cache_root/$_asset"
    trap - EXIT HUP INT TERM
}

force=0
platform=""

while [ $# -gt 0 ]; do
    case "$1" in
        --force) force=1; shift ;;
        --platform)
            [ $# -ge 2 ] || fail "--platform requires an argument"
            platform=$2; shift 2 ;;
        --help) usage ;;
        --list-platforms) list_platforms ;;
        --print-binary-path) break ;;
        --) shift; break ;;
        *) break ;;
    esac
done

if [ -n "$platform" ]; then
    case "$platform" in
        linux-x86_64-gnu)    asset=aliasc-x86_64-unknown-linux-gnu ;;
        linux-x86_64-musl)   asset=aliasc-x86_64-unknown-linux-musl ;;
        linux-aarch64-android) asset=aliasc-aarch64-linux-android ;;
        darwin-x86_64)       asset=aliasc-x86_64-apple-darwin ;;
        darwin-aarch64)      asset=aliasc-aarch64-apple-darwin ;;
        windows-x86_64)      asset=aliasc-x86_64-pc-windows-msvc.exe ;;
        windows-i686)        asset=aliasc-i686-pc-windows-msvc.exe ;;
        windows-aarch64)     asset=aliasc-aarch64-pc-windows-msvc.exe ;;
        *) fail "unknown platform: $platform" ;;
    esac
else
    os=$(uname -s 2>/dev/null || true)
    arch=$(uname -m 2>/dev/null || true)
    asset=

    case "$os" in
        Linux)
            case "$arch" in
                x86_64|amd64)
                    if __aliasc_have_glibc 2 31; then
                        asset=aliasc-x86_64-unknown-linux-gnu
                    else
                        asset=aliasc-x86_64-unknown-linux-musl
                    fi
                    ;;
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
fi

[ -n "$asset" ] || fail "could not determine a release asset"
mkdir -p "$cache_root" || fail "cannot create cache directory: $cache_root"
binary=$cache_root/$asset

if [ "$force" -eq 1 ] || [ ! -x "$binary" ]; then
    __aliasc_download "$asset"

    case "$asset" in
        *-unknown-linux-gnu)
            if ! "$binary" --version >/dev/null 2>&1; then
                printf '%s\n' "aliasc: gnu binary failed to run; falling back to musl" >&2
                rm -f "$binary"
                asset=aliasc-x86_64-unknown-linux-musl
                binary=$cache_root/$asset
                if [ ! -x "$binary" ]; then
                    __aliasc_download "$asset"
                fi
            fi
            ;;
    esac
fi

if [ $# -gt 0 ]; then
    case "${1-}" in
        --print-binary-path) printf '%s\n' "$binary"; exit 0 ;;
        *) exec "$binary" "$@" ;;
    esac
fi
