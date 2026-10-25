#!/usr/bin/env fish
# aliasc binary downloader for fish.
# Ensures the matching release binary is cached locally.
# If called with extra arguments, downloads if needed then execs the binary.

set -l repo "dodorz/aliasc"
if set -q ALIASC_REPOSITORY
    set repo $ALIASC_REPOSITORY
end

set -l release "latest"
if set -q ALIASC_RELEASE
    set release $ALIASC_RELEASE
end

set -l cache_root ""
if set -q ALIASC_BINARY_CACHE_DIR
    set cache_root $ALIASC_BINARY_CACHE_DIR
else if set -q XDG_CACHE_HOME
    set cache_root $XDG_CACHE_HOME/aliasc/bin
else
    set cache_root $HOME/.cache/aliasc/bin
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
                set asset "aliasc-x86_64-unknown-linux-gnu"
            case aarch64 arm64
                if test "$PREFIX" = "/data/data/com.termux/files/usr"
                    set asset "aliasc-aarch64-linux-android"
                else if test -n "$TERMUX_VERSION"
                    set asset "aliasc-aarch64-linux-android"
                else
                    fail "no Linux ARM64 release asset; Android/Termux ARM64 requires a Termux environment"
                end
            case '*'
                fail "unsupported Linux architecture: $arch"
        end
    case Darwin
        switch $arch
            case x86_64
                set asset "aliasc-x86_64-apple-darwin"
            case arm64 aarch64
                set asset "aliasc-aarch64-apple-darwin"
            case '*'
                fail "unsupported macOS architecture: $arch"
        end
    case MINGW\* MSYS\* CYGWIN\*
        switch $arch
            case x86_64 amd64
                set asset "aliasc-x86_64-pc-windows-msvc.exe"
            case 'i?86'
                set asset "aliasc-i686-pc-windows-msvc.exe"
            case aarch64 arm64
                set asset "aliasc-aarch64-pc-windows-msvc.exe"
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
    set -l tmp (mktemp "$cache_root/.$asset.tmp.XXXXXX" 2>/dev/null)
    if test -z "$tmp"
        fail "cannot create download temporary file"
    end
    set -l url "https://github.com/$repo/releases/$release/download/$asset"
    set -l have_tool 0
    type -q curl; and set have_tool 1
    type -q wget; and set have_tool 1
    if test $have_tool -eq 0
        fail "curl or wget is required to download $asset"
    end
    set -l downloaded 0
    set -l curl_failed 0
    if type -q curl
        curl --fail --location --silent --show-error --retry 1 --retry-delay 1 --connect-timeout 10 --max-time 600 --speed-limit 512 --speed-time 60 --output "$tmp" "$url" 2>/dev/null
        and set downloaded 1
        or set curl_failed 1
    end
    if test $downloaded -eq 0; and type -q wget
        if test $curl_failed -eq 1
            echo 'aliasc: curl failed; retrying with wget' >&2
        end
        wget -q --timeout=20 --tries=2 -O "$tmp" "$url" 2>/dev/null
        and set downloaded 1
    end
    if test $downloaded -eq 0
        fail "download failed: $url"
    end
    if not test -s "$tmp"
        fail "downloaded file is empty: $url"
    end
    chmod 0755 "$tmp" 2>/dev/null
    or fail "cannot mark downloaded binary executable: $tmp"
    mv -f "$tmp" "$binary" 2>/dev/null
    or fail "cannot install downloaded binary: $binary"
end

if test (count $argv) -gt 0
    exec "$binary" $argv
end
