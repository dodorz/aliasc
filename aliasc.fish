#!/usr/bin/env fish
# aliasc binary downloader for fish.
# Ensures the matching release binary is cached locally.
# If called with extra arguments, downloads if needed then execs the binary.

if set -q ALIASC_REPOSITORY
    set -l repo $ALIASC_REPOSITORY
else
    set -l repo "dodorz/aliasc"
end

if set -q ALIASC_RELEASE
    set -l release $ALIASC_RELEASE
else
    set -l release "latest"
end

if set -q ALIASC_BINARY_CACHE_DIR
    set -l cache_root $ALIASC_BINARY_CACHE_DIR
else if set -q XDG_CACHE_HOME
    set -l cache_root $XDG_CACHE_HOME/aliasc/bin
else
    set -l cache_root $HOME/.cache/aliasc/bin
end

function fail
    printf 'aliasc: %s\n' $argv >&2
    exit 1
end

set -l os (uname -s 2>/dev/null || true)
set -l arch (uname -m 2>/dev/null || true)
set -l asset ""

switch $os
    case Linux
        switch $arch
            case x86_64 amd64
                set -l asset "aliasc-x86_64-unknown-linux-gnu"
            case aarch64 arm64
                if test "$PREFIX" = "/data/data/com.termux/files/usr"
                    set -l asset "aliasc-aarch64-linux-android"
                else if test -n "$TERMUX_VERSION"
                    set -l asset "aliasc-aarch64-linux-android"
                else
                    fail "no Linux ARM64 release asset; Android/Termux ARM64 requires a Termux environment"
                end
            case '*'
                fail "unsupported Linux architecture: $arch"
        end
    case Darwin
        switch $arch
            case x86_64
                set -l asset "aliasc-x86_64-apple-darwin"
            case arm64 aarch64
                set -l asset "aliasc-aarch64-apple-darwin"
            case '*'
                fail "unsupported macOS architecture: $arch"
        end
    case MINGW\* MSYS\* CYGWIN\*
        switch $arch
            case x86_64 amd64
                set -l asset "aliasc-x86_64-pc-windows-msvc.exe"
            case 'i?86'
                set -l asset "aliasc-i686-pc-windows-msvc.exe"
            case aarch64 arm64
                set -l asset "aliasc-aarch64-pc-windows-msvc.exe"
            case '*'
                fail "unsupported Windows architecture: $arch"
        end
    case '*'
        fail "unsupported platform: $os ($arch)"
end

if test -z "$asset"
    fail "could not determine a release asset"
end

mkdir -p "$cache_root" 2>/dev/null
set -l binary "$cache_root/$asset"

if not test -x "$binary"
    set -l tmp (mktemp "$cache_root/.${asset}.tmp.XXXXXX" 2>/dev/null)
    if test -z "$tmp"
        fail "cannot create download temporary file"
    end
    set -l url "https://github.com/$repo/releases/$release/download/$asset"
    if type -q curl
        curl --fail --location --silent --show-error --retry 2 --connect-timeout 10 --max-time 45 --output "$tmp" "$url" 2>/dev/null
        or fail "download failed: $url"
    else if type -q wget
        wget -q --timeout=15 --tries=2 -O "$tmp" "$url" 2>/dev/null
        or fail "download failed: $url"
    else
        fail "curl or wget is required to download $asset"
    end
    chmod 0755 "$tmp" 2>/dev/null
    or fail "cannot mark downloaded binary executable: $tmp"
    mv -f "$tmp" "$binary" 2>/dev/null
    or fail "cannot install downloaded binary: $binary"
end

if test (count $argv) -gt 0
    exec "$binary" $argv
end
