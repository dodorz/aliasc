#!/bin/sh
# aliasc-activate.sh - POSIX shell activator (bash, zsh, dash, sh).
#
# Responsibility: put the generated aliases into the CURRENT shell. All the
# shell-agnostic work (fetch compiler, staleness check, compile) lives in
# aliasc-core.sh; this file only decides what to load and how.
#
# Unlike the fish/nu/pwsh activators this one carries a pure-shell parser
# fallback (aliasc-fallback.sh), because POSIX function bodies can be eval-ed
# directly - the other shells cannot consume them.
#
# Install as: ~/.config/interactive.d/10-aliasc-activate.sh

if [ "${ALIASDSL_NOLOAD-}" = 1 ]; then
    return 0 2>/dev/null || exit 0
fi

: "${ALIASC_LIB_DIR:=${XDG_DATA_HOME:-$HOME/.local/share}/aliasc}"
: "${ALIASC_CORE:=${ALIASC_LIB_DIR}/aliasc-core.sh}"
: "${ALIASC_FALLBACK:=${ALIASC_LIB_DIR}/aliasc-fallback.sh}"
: "${ALIASC_CACHE_DIR:=${XDG_CACHE_HOME:-$HOME/.cache}/aliasc}"
: "${ALIAS_FILE:=$HOME/.config/alias}"

# Sourced by an interactive shell, so plain `return` is the correct exit.
__aliasc_activate() {
    if [ ! -f "$ALIAS_FILE" ]; then
        printf 'aliasc: alias source is missing: %s\n' "$ALIAS_FILE" >&2
        return 1
    fi
    if [ ! -f "$ALIASC_CORE" ]; then
        printf 'aliasc: core not found: %s\n' "$ALIASC_CORE" >&2
        return 1
    fi

    __aliasc_output=$(sh "$ALIASC_CORE" --shell posix --source "$ALIAS_FILE" --cache-dir "$ALIASC_CACHE_DIR")
    if [ $? -ne 0 ] || [ ! -f "$__aliasc_output" ]; then
        if [ -f "$ALIASC_FALLBACK" ]; then
            printf '%s\n' 'aliasc: core failed; using the shell fallback' >&2
            . "$ALIASC_FALLBACK"
            __aliasc_fallback_load
            return 0
        fi
        printf '%s\n' 'aliasc: core failed and no fallback is installed' >&2
        return 1
    fi

    # Drop the previous generation before sourcing the new one so that renamed
    # or deleted definitions do not survive from an earlier artifact.
    __aliasc_function_list=$ALIASC_CACHE_DIR/.functions.$$
    command sed -n 's/^\([A-Za-z_][A-Za-z0-9_]*\)()[[:space:]]*{$/\1/p' "$__aliasc_output" >"$__aliasc_function_list"
    while IFS= read -r __aliasc_function_name; do
        [ -n "$__aliasc_function_name" ] || continue
        unalias "$__aliasc_function_name" 2>/dev/null || :
    done <"$__aliasc_function_list"
    rm -f "$__aliasc_function_list"

    if ! . "$__aliasc_output"; then
        printf '%s\n' 'aliasc: generated output could not be loaded' >&2
        return 1
    fi
    return 0
}

__aliasc_activate
__aliasc_activate_status=$?
unset -f __aliasc_activate
unset __aliasc_output __aliasc_function_list __aliasc_function_name

if [ "$__aliasc_activate_status" -ne 0 ]; then
    unset __aliasc_activate_status
    return 1 2>/dev/null || exit 1
fi
unset __aliasc_activate_status

# Must be (re)installed last: the generated output defines this helper with a
# "for dir in $PATH" loop that relies on word-splitting, which zsh does not do
# by default, causing every FirstAvailable alias to report 'no candidate'.
__aliasc_is_external() {
    case "$1" in
        */*) [ -f "$1" ] && [ -x "$1" ] ;;
        *)
            __aliasc_path=$PATH
            while :; do
                case "$__aliasc_path" in
                    *:*)
                        __aliasc_dir=${__aliasc_path%%:*}
                        __aliasc_path=${__aliasc_path#*:}
                        __aliasc_more=1
                        ;;
                    *)
                        __aliasc_dir=$__aliasc_path
                        __aliasc_path=
                        __aliasc_more=0
                        ;;
                esac
                [ -n "$__aliasc_dir" ] || __aliasc_dir=.
                if [ -f "$__aliasc_dir/$1" ] && [ -x "$__aliasc_dir/$1" ]; then
                    return 0
                fi
                [ "$__aliasc_more" -eq 1 ] || break
            done
            return 1
            ;;
    esac
}
