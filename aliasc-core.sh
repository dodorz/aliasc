#!/bin/sh
# aliasc-core.sh - shared POSIX core: fetch compiler -> check staleness -> compile.
#
# Responsibility: produce a fresh, shell-specific artifact and print its path
# on stdout. It never loads anything into a shell: the per-shell activators do
# that, because "install into the current session" is the only part that cannot
# be shared.
#
# Why POSIX sh and not one implementation per shell: the work is process-level
# (find, compare, spawn the compiler) and identical for every Unix shell, while
# sh is guaranteed present wherever fish, nushell, bash, or zsh run. PowerShell
# is the one exception - Windows has no reliable /bin/sh - so pwsh uses
# aliasc-activate.ps1 instead of this script.
#
# Usage:
#   sh aliasc-core.sh --shell NAME [options]
#
# Options:
#   --shell NAME           Language to generate (posix|bash|zsh|fish|nu)
#   --source PATH          Alias source (default $ALIAS_FILE, else ~/.config/alias)
#   --cache-dir PATH       Artifact cache (default ${XDG_CACHE_HOME:-$HOME/.cache}/aliasc)
#   --fetcher PATH         Fetcher script (default $HOME/.local/bin/aliasc-fetch.sh)
#   --platform VALUE       Passed to the compiler (default auto)
#   --distro VALUE         Passed to the compiler (default auto)
#   --environment VALUE    Passed to the compiler (default auto)
#   --force                Recompile even when the artifact is fresh
#   --quiet                Suppress progress messages on stderr
#
# Exit status: 0 with the artifact path on stdout, 1 on any failure.
#
# Degraded mode is deliberately limited to "reuse the existing artifact". A
# hand-written DSL parser cannot be shared with fish, nu, or pwsh anyway: the
# POSIX fallback works by eval-ing POSIX function bodies, which those shells
# cannot consume. Only the POSIX activator carries a parser fallback.

fail() {
    printf 'aliasc: %s\n' "$*" >&2
    exit 1
}

note() {
    [ "$quiet" -eq 1 ] || printf 'aliasc: %s\n' "$*" >&2
}

shell=
source_file=
cache_dir=
fetcher=
platform=auto
distro=auto
environment=auto
force=0
quiet=0

while [ $# -gt 0 ]; do
    case "$1" in
        --shell) [ $# -ge 2 ] || fail "--shell requires a value"; shell=$2; shift 2 ;;
        --shell=*) shell=${1#*=}; shift ;;
        --source) [ $# -ge 2 ] || fail "--source requires a value"; source_file=$2; shift 2 ;;
        --source=*) source_file=${1#*=}; shift ;;
        --cache-dir) [ $# -ge 2 ] || fail "--cache-dir requires a value"; cache_dir=$2; shift 2 ;;
        --cache-dir=*) cache_dir=${1#*=}; shift ;;
        --fetcher) [ $# -ge 2 ] || fail "--fetcher requires a value"; fetcher=$2; shift 2 ;;
        --fetcher=*) fetcher=${1#*=}; shift ;;
        --platform) [ $# -ge 2 ] || fail "--platform requires a value"; platform=$2; shift 2 ;;
        --platform=*) platform=${1#*=}; shift ;;
        --distro) [ $# -ge 2 ] || fail "--distro requires a value"; distro=$2; shift 2 ;;
        --distro=*) distro=${1#*=}; shift ;;
        --environment) [ $# -ge 2 ] || fail "--environment requires a value"; environment=$2; shift 2 ;;
        --environment=*) environment=${1#*=}; shift ;;
        --force) force=1; shift ;;
        --quiet) quiet=1; shift ;;
        --help|-h) sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) fail "unknown option: $1" ;;
    esac
done

[ -n "$shell" ] || fail "--shell is required"
case "$shell" in
    posix|bash|zsh|fish|nu) ;;
    *) fail "unsupported shell for the POSIX core: $shell" ;;
esac

: "${source_file:=${ALIAS_FILE:-$HOME/.config/alias}}"
: "${cache_dir:=${XDG_CACHE_HOME:-$HOME/.cache}/aliasc}"
: "${fetcher:=$HOME/.local/bin/aliasc-fetch.sh}"

[ -f "$source_file" ] || fail "alias source is missing: $source_file"
[ -x "$fetcher" ] || fail "fetcher not found: $fetcher"

artifact=$cache_dir/alias.$shell
manifest=$artifact.manifest.json

# --- compiler -------------------------------------------------------------

binary=$("$fetcher" --print-binary-path 2>/dev/null) || binary=
if [ -z "$binary" ] || [ ! -x "$binary" ]; then
    note 'compiler not cached; fetching'
    if ! "$fetcher" >/dev/null 2>&1; then
        # Degraded path: reuse whatever a previous successful run produced.
        if [ -f "$artifact" ]; then
            note "download failed; reusing existing artifact $artifact"
            printf '%s\n' "$artifact"
            exit 0
        fi
        fail 'download failed and no cached artifact exists'
    fi
    binary=$("$fetcher" --print-binary-path 2>/dev/null) || binary=
fi
[ -n "$binary" ] && [ -x "$binary" ] || {
    if [ -f "$artifact" ]; then
        note "compiler unavailable; reusing existing artifact $artifact"
        printf '%s\n' "$artifact"
        exit 0
    fi
    fail 'compiler unavailable and no cached artifact exists'
}

# --- staleness ------------------------------------------------------------

# Inputs are read from the manifest rather than guessed from the source path so
# that `include` files and ShortcutMap dependencies are covered. Each line is
# "<mtime_ns|->\t<path>".
#
# mtime_ns is the signal, not `exists`: the compiler records exists=true for
# files that are optional-but-positional (a configured ShortcutMap or include
# that is absent) and leaves mtime_ns null for them. Trusting `exists` would
# mark such a manifest stale forever and recompile on every shell startup.
read_inputs() {
    awk '
        /^[[:space:]]*"inputs"[[:space:]]*:/ { inp = 1; next }
        inp && /^[[:space:]]*\][[:space:]]*,?[[:space:]]*$/ { inp = 0; next }
        inp && match($0, /"path"[[:space:]]*:[[:space:]]*"/) {
            v = substr($0, RSTART + RLENGTH); sub(/".*$/, "", v); path = v; next
        }
        inp && match($0, /"mtime_ns"[[:space:]]*:[[:space:]]*[0-9]+/) {
            v = substr($0, RSTART, RLENGTH); sub(/.*:[[:space:]]*/, "", v)
            print v "\t" path; next
        }
        inp && /"mtime_ns"[[:space:]]*:[[:space:]]*null/ { print "-\t" path }
    ' "$1" 2>/dev/null
}

manifest_shell() {
    awk -F'"' '/"shell"[[:space:]]*:/ { print $4; exit }' "$1" 2>/dev/null
}

manifest_compiler() {
    awk -F'"' '/^[[:space:]]*"compiler"[[:space:]]*:/ { inp = 1; next }
                inp && /^[[:space:]]*\}[[:space:]]*,?[[:space:]]*$/ { exit }
                inp && /"path"[[:space:]]*:/ { print $4; exit }' "$1" 2>/dev/null
}

needs_compile() {
    [ "$force" -eq 1 ] && return 0
    [ -f "$artifact" ] && [ -f "$manifest" ] || return 0

    recorded_shell=$(manifest_shell "$manifest")
    [ "$recorded_shell" = "$shell" ] || return 0

    recorded_compiler=$(manifest_compiler "$manifest")
    if [ -n "$recorded_compiler" ] && [ -f "$recorded_compiler" ]; then
        [ -n "$(find "$recorded_compiler" -newer "$artifact" -print 2>/dev/null)" ] && return 0
    fi
    if [ -n "$recorded_compiler" ] && [ "$recorded_compiler" != "$binary" ]; then
        return 0
    fi

    tab=$(printf '\t')
    read_inputs "$manifest" >"$cache_dir/.inputs.$$" || return 0
    stale=0
    while IFS="$tab" read -r recorded_mtime path; do
        [ -n "$path" ] || continue
        if [ "$recorded_mtime" = - ]; then
            # Absent at compile time. Appearing later changes the result, but
            # only for files the compiler actually resolved as a dependency.
            [ -e "$path" ] && { stale=1; break; }
            continue
        fi
        [ -f "$path" ] || { stale=1; break; }
        [ -n "$(find "$path" -newer "$artifact" -print 2>/dev/null)" ] && { stale=1; break; }
    done <"$cache_dir/.inputs.$$"
    rm -f "$cache_dir/.inputs.$$"
    [ "$stale" -eq 1 ] && return 0

    # A newly created local/shortcut file is a compile input even when the
    # previous manifest predates it, so check them directly as well.
    source_dir=${source_file%/*}
    [ "$source_dir" = "$source_file" ] && source_dir=.
    for extra in "$source_dir/alias.local" "$source_dir/ShortcutMap.yaml"; do
        [ -f "$extra" ] || continue
        [ -n "$(find "$extra" -newer "$artifact" -print 2>/dev/null)" ] && return 0
    done

    return 1
}

# --- compile --------------------------------------------------------------

mkdir -p "$cache_dir" 2>/dev/null || fail "cannot create cache directory: $cache_dir"

if needs_compile; then
    note "compiling $shell artifact"
    if ! "$binary" compile \
        --shell "$shell" \
        --platform "$platform" \
        --distro "$distro" \
        --environment "$environment" \
        --source "$source_file" \
        --output "$artifact" >/dev/null; then
        if [ -f "$artifact" ]; then
            note "compilation failed; reusing existing artifact $artifact"
            printf '%s\n' "$artifact"
            exit 0
        fi
        fail 'compilation failed and no cached artifact exists'
    fi
fi

[ -f "$artifact" ] || fail "artifact missing after compile: $artifact"
printf '%s\n' "$artifact"
