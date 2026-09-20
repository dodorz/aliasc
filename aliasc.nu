#!/usr/bin/env nu
# aliasc binary downloader for nushell.
# Ensures the matching release binary is cached locally.
# If called with extra arguments, downloads if needed then execs the binary.

let repo = ($env.ALIASC_REPOSITORY | default "dodorz/aliasc")
let release = ($env.ALIASC_RELEASE | default "latest")
let cache_root = ($env.XDG_CACHE_HOME | default ($env.HOME | path join ".cache")) | path join "aliasc" "bin"

fn fail [msg: string] {
    print ("aliasc: " ++ $msg) | to error
    exit 1
}

let os_name = (sys | get kernel.name | str to-lowercase)
let arch_name = (sys | get cpu.current-arch | str to-lowercase)
mut asset = ""

if $os_name == "linux" {
    if $arch_name == "x86_64" or $arch_name == "amd64" {
        asset = "aliasc-x86_64-unknown-linux-gnu"
    } else if $arch_name == "aarch64" or $arch_name == "arm64" {
        if (try { $env.PREFIX } catch { "" }) == "/data/data/com.termux/files/usr" or (try { $env.TERMUX_VERSION } catch { "" }) != "" {
            asset = "aliasc-aarch64-linux-android"
        } else {
            fail "no Linux ARM64 release asset; Android/Termux ARM64 requires a Termux environment"
        }
    } else {
        fail ("unsupported Linux architecture: " ++ $arch_name)
    }
} elif $os_name == "darwin" or $os_name == "macos" {
    if $arch_name == "x86_64" {
        asset = "aliasc-x86_64-apple-darwin"
    } elif $arch_name == "aarch64" or $arch_name == "arm64" {
        asset = "aliasc-aarch64-apple-darwin"
    } else {
        fail ("unsupported macOS architecture: " ++ $arch_name)
    }
} else {
    fail ("unsupported platform: " ++ $os_name ++ " (" ++ $arch_name ++ ")")
}

if $asset == "" {
    fail "could not determine a release asset"
}

mkdir -p $cache_root
let binary = ($cache_root | path join $asset)

if not (path exists $binary) {
    let tmp = ($cache_root | path join (".${asset}.tmp"))
    let url = $"https://github.com/($repo)/releases/($release)/download/($asset)"
    try {
        http get $url | save --force $tmp
    } catch {
        fail ("download failed: " ++ $url)
    }
    cp $tmp $binary
    rm $tmp
    chmod +x $binary
}

let args = ($env | get NUSHELL_ARGS | default [])
if (length $args) > 0 {
    ^$binary $args
}
