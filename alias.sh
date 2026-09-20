#!/bin/sh
# aliasc POSIX startup loader.
# Uses the aliasc binary directly; cache staleness checked via file mtime.

if [ "${ALIASDSL_NOLOAD-}" = 1 ]; then
    return 0 2>/dev/null || exit 0
fi

__aliasc_fallback_define_first_available() {
    __aliasc_fallback_name=$1
    __aliasc_fallback_candidates=$2
    __aliasc_fallback_cache="__aliasc_first_${__aliasc_fallback_name}"
    __aliasc_fallback_index=0
    __aliasc_fallback_resolve=
    __aliasc_fallback_cases=
    while [ -n "$__aliasc_fallback_candidates" ]; do
        case "$__aliasc_fallback_candidates" in
            *,*)
                __aliasc_fallback_candidate=${__aliasc_fallback_candidates%%,*}
                __aliasc_fallback_candidates=${__aliasc_fallback_candidates#*,}
                ;;
            *)
                __aliasc_fallback_candidate=$__aliasc_fallback_candidates
                __aliasc_fallback_candidates=
                ;;
        esac
        __aliasc_fallback_candidate=$(printf '%s' "$__aliasc_fallback_candidate" | command sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        [ -n "$__aliasc_fallback_candidate" ] || continue
        case "$__aliasc_fallback_candidate" in
            \\*) __aliasc_fallback_command=${__aliasc_fallback_candidate#\\} ;;
            *) __aliasc_fallback_command=$__aliasc_fallback_candidate ;;
        esac
        __aliasc_fallback_command_name=${__aliasc_fallback_command%%[[:space:]]*}
        __aliasc_fallback_command_rest=${__aliasc_fallback_command#"$__aliasc_fallback_command_name"}
        __aliasc_fallback_index=$((__aliasc_fallback_index + 1))
        if [ "$__aliasc_fallback_index" -eq 1 ]; then
            __aliasc_fallback_resolve="${__aliasc_fallback_resolve}    if __aliasc_is_external '$__aliasc_fallback_command_name'; then ${__aliasc_fallback_cache}=1
"
        else
            __aliasc_fallback_resolve="${__aliasc_fallback_resolve}    elif __aliasc_is_external '$__aliasc_fallback_command_name'; then ${__aliasc_fallback_cache}=${__aliasc_fallback_index}
"
        fi
        __aliasc_fallback_cases="${__aliasc_fallback_cases}    ${__aliasc_fallback_index}) command ${__aliasc_fallback_command_name}${__aliasc_fallback_command_rest} \"\$@\"
       return \$? ;;
"
    done
    if [ "$__aliasc_fallback_index" -eq 0 ]; then
        eval "${__aliasc_fallback_name}() {
    printf '%s\\n' 'aliasc: no FirstAvailable candidate found' >&2
    return 127
}"
        return
    fi
    __aliasc_fallback_code=$(
        printf '%s\n' "${__aliasc_fallback_name}() {"
        printf '%s\n' "  if [ \"\${${__aliasc_fallback_cache}+x}\" != x ] || [ \"\${${__aliasc_fallback_cache}-}\" = none ]; then"
        printf '%s' "$__aliasc_fallback_resolve"
        printf '%s\n' "    else ${__aliasc_fallback_cache}=none"
        printf '%s\n' "    fi"
        printf '%s\n' "  fi"
        printf '%s\n' "  case \"\$${__aliasc_fallback_cache}\" in"
        printf '%s' "$__aliasc_fallback_cases"
        printf '%s\n' "    *) printf '%s\\n' 'aliasc: no FirstAvailable candidate found' >&2"
        printf '%s\n' "       return 127 ;;"
        printf '%s\n' "  esac"
        printf '%s\n' "}"
    )
    eval "$__aliasc_fallback_code"
}

__aliasc_fallback_define_error() {
    __aliasc_fallback_errmsg=$1
    eval "${__aliasc_fallback_name}() {
    printf '%s\\n' 'aliasc: ${__aliasc_fallback_name}: ${__aliasc_fallback_errmsg}' >&2
    return 127
}"
}

__aliasc_fallback_define() {
    __aliasc_fallback_name=$1
    __aliasc_fallback_body=$2
    unalias "$__aliasc_fallback_name" 2>/dev/null || :

    __aliasc_fallback_body=$(
        __aliasc_fallback_out=
        __aliasc_fallback_q=
        __aliasc_fallback_s=$__aliasc_fallback_body
        while [ -n "$__aliasc_fallback_s" ]; do
            __aliasc_fallback_c=${__aliasc_fallback_s%"${__aliasc_fallback_s#?}"}
            __aliasc_fallback_s=${__aliasc_fallback_s#?}
            if [ "$__aliasc_fallback_q" = "'" ]; then
                [ "$__aliasc_fallback_c" = "'" ] && __aliasc_fallback_q=
                __aliasc_fallback_out="$__aliasc_fallback_out$__aliasc_fallback_c"
            elif [ "$__aliasc_fallback_q" = '"' ]; then
                [ "$__aliasc_fallback_c" = '"' ] && __aliasc_fallback_q=
                __aliasc_fallback_out="$__aliasc_fallback_out$__aliasc_fallback_c"
            elif [ "$__aliasc_fallback_c" = "'" ]; then
                __aliasc_fallback_q=\'
                __aliasc_fallback_out="$__aliasc_fallback_out$__aliasc_fallback_c"
            elif [ "$__aliasc_fallback_c" = '"' ]; then
                __aliasc_fallback_q=\"
                __aliasc_fallback_out="$__aliasc_fallback_out$__aliasc_fallback_c"
            elif [ "$__aliasc_fallback_c" = '#' ]; then
                __aliasc_fallback_s=
            else
                __aliasc_fallback_out="$__aliasc_fallback_out$__aliasc_fallback_c"
            fi
        done
        printf '%s' "$__aliasc_fallback_out"
    )

    case "$__aliasc_fallback_body" in
        '?('*) __aliasc_fallback_body="FirstAvailable${__aliasc_fallback_body#?}" ;;
    esac

    if [ "${__aliasc_fallback_body#FirstAvailable}" != "$__aliasc_fallback_body" ]; then
        __aliasc_fallback_payload=${__aliasc_fallback_body#FirstAvailable}
        __aliasc_fallback_payload=${__aliasc_fallback_payload#?}
        __aliasc_fallback_payload=${__aliasc_fallback_payload%?}
        __aliasc_fallback_candidates=$__aliasc_fallback_payload
        __aliasc_fallback_define_first_available "$__aliasc_fallback_name" "$__aliasc_fallback_candidates"
        return
    fi

    __aliasc_fallback_paren=0
    __aliasc_fallback_q=
    __aliasc_fallback_s=$__aliasc_fallback_body
    while [ -n "$__aliasc_fallback_s" ]; do
        __aliasc_fallback_c=${__aliasc_fallback_s%"${__aliasc_fallback_s#?}"}
        __aliasc_fallback_s=${__aliasc_fallback_s#?}
        if [ "$__aliasc_fallback_q" = "'" ]; then
            [ "$__aliasc_fallback_c" = "'" ] && __aliasc_fallback_q=
        elif [ "$__aliasc_fallback_q" = '"' ]; then
            [ "$__aliasc_fallback_c" = '"' ] && __aliasc_fallback_q=
        elif [ "$__aliasc_fallback_c" = "'" ]; then
            __aliasc_fallback_q=\'
        elif [ "$__aliasc_fallback_c" = '"' ]; then
            __aliasc_fallback_q=\"
        elif [ "$__aliasc_fallback_c" = '(' ]; then
            __aliasc_fallback_paren=$((__aliasc_fallback_paren + 1))
        elif [ "$__aliasc_fallback_c" = ')' ]; then
            [ "$__aliasc_fallback_paren" -gt 0 ] && __aliasc_fallback_paren=$((__aliasc_fallback_paren - 1))
        fi
    done
    if [ "$__aliasc_fallback_paren" -gt 0 ]; then
        __aliasc_fallback_define_error 'multiline definition (unclosed `(`) is not supported by the shell fallback'
        return
    fi

    if [ "${__aliasc_fallback_body#SetEnv}" != "$__aliasc_fallback_body" ]; then
        __aliasc_fallback_payload=${__aliasc_fallback_body#SetEnv}
        __aliasc_fallback_payload=${__aliasc_fallback_payload#?}
        __aliasc_fallback_payload=${__aliasc_fallback_payload%?}
        eval "${__aliasc_fallback_name}() { export $__aliasc_fallback_payload; }"
        return
    elif [ "${__aliasc_fallback_body#UnsetEnv}" != "$__aliasc_fallback_body" ]; then
        __aliasc_fallback_payload=${__aliasc_fallback_body#UnsetEnv}
        __aliasc_fallback_payload=${__aliasc_fallback_payload#?}
        __aliasc_fallback_payload=${__aliasc_fallback_payload%?}
        eval "${__aliasc_fallback_name}() { unset $__aliasc_fallback_payload; }"
        return
    elif [ "${__aliasc_fallback_body#WithEnv}" != "$__aliasc_fallback_body" ]; then
        __aliasc_fallback_payload=${__aliasc_fallback_body#WithEnv}
        __aliasc_fallback_env=${__aliasc_fallback_payload%%)*}
        __aliasc_fallback_env=${__aliasc_fallback_env#?}
        __aliasc_fallback_command=${__aliasc_fallback_payload#*)}
        __aliasc_fallback_command=$(printf '%s' "$__aliasc_fallback_command" | command sed 's/^[[:space:]]*//')
        if [ -n "$__aliasc_fallback_command" ]; then
            eval "${__aliasc_fallback_name}() { ( export $__aliasc_fallback_env; $__aliasc_fallback_command \"\$@\" ); }"
        else
            eval "${__aliasc_fallback_name}() { ( export $__aliasc_fallback_env; \"\$@\" ); }"
        fi
        return
    fi

    case "$__aliasc_fallback_body" in
        *@@*)
            __aliasc_fallback_body=$(printf '%s' "$__aliasc_fallback_body" | command sed 's/@@/@/g')
            ;;
    esac

    case "$__aliasc_fallback_body" in
        *@\**|*@1*|*@2*|*@3*|*@4*|*@5*|*@6*|*@7*|*@8*|*@9*)
            __aliasc_fallback_body=$(printf '%s' "$__aliasc_fallback_body" | command sed \
                -e 's/@\*/"$@"/g' \
                -e 's/@1/"$1"/g' -e 's/@2/"$2"/g' \
                -e 's/@3/"$3"/g' -e 's/@4/"$4"/g' \
                -e 's/@5/"$5"/g' -e 's/@6/"$6"/g' \
                -e 's/@7/"$7"/g' -e 's/@8/"$8"/g' \
                -e 's/@9/"$9"/g')
            ;;
        *) __aliasc_fallback_body="$__aliasc_fallback_body \"\$@\"" ;;
    esac

    case "$__aliasc_fallback_body" in
        *';'*|*'|'*|*'&&'*|*'||'*) ;;
        command\ *|export\ *|unset\ *|return\ *|:* ) ;;
        *) __aliasc_fallback_body="command $__aliasc_fallback_body" ;;
    esac
    eval "${__aliasc_fallback_name}() { $__aliasc_fallback_body; }"
}

__aliasc_fallback_section_active() {
    case "$1" in
        ''|Common) return 0 ;;
        UNIX)
            case "$__aliasc_fallback_os" in
                Linux|Darwin) return 0 ;;
                *) return 1 ;;
            esac
            ;;
        macOS) [ "$__aliasc_fallback_os" = Darwin ] ;;
        Windows) return 1 ;;
        *) [ "$__aliasc_fallback_os" = Linux ] && [ "$1" = "$__aliasc_fallback_distro" ] ;;
    esac
}

__aliasc_fallback_parse_file() {
    [ -f "$1" ] || return 0
    __aliasc_fallback_section=
    while IFS= read -r __aliasc_fallback_line || [ -n "$__aliasc_fallback_line" ]; do
        __aliasc_fallback_line=$(printf '%s' "$__aliasc_fallback_line" | command sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        case "$__aliasc_fallback_line" in
            ''|\#*|include\ *) continue ;;
            \[*\])
                __aliasc_fallback_section=${__aliasc_fallback_line#\[}
                __aliasc_fallback_section=${__aliasc_fallback_section%\]}
                ;;
            *=*)
                __aliasc_fallback_section_active "$__aliasc_fallback_section" || continue
                __aliasc_fallback_name=${__aliasc_fallback_line%%=*}
                __aliasc_fallback_body=${__aliasc_fallback_line#*=}
                __aliasc_fallback_body=$(printf '%s' "$__aliasc_fallback_body" | command sed 's/ #.*//')
                [ -n "$__aliasc_fallback_name" ] || continue
                __aliasc_fallback_define "$__aliasc_fallback_name" "$__aliasc_fallback_body"
                ;;
        esac
    done < "$1"
}

__aliasc_fallback_load() {
    : "${ALIAS_FILE:=$HOME/.config/alias}"
    : "${ALIASC_LOCAL_FILE:=$HOME/.config/alias.local}"
    __aliasc_fallback_os=$(uname -s 2>/dev/null || printf '%s' unknown)
    __aliasc_fallback_distro=
    if [ -f /etc/os-release ]; then
        __aliasc_fallback_distro=$(command sed -n 's/^ID=//p' /etc/os-release | command sed 's/^"//;s/"$//' | command head -n 1)
    fi
    case "$__aliasc_fallback_distro" in
        ubuntu) __aliasc_fallback_distro=Ubuntu ;;
        debian) __aliasc_fallback_distro=Debian ;;
        fedora) __aliasc_fallback_distro=Fedora ;;
        arch) __aliasc_fallback_distro=Arch ;;
    esac
    __aliasc_fallback_section=
    __aliasc_fallback_parse_file "$ALIAS_FILE"
    __aliasc_fallback_parse_file "$ALIASC_LOCAL_FILE"
}

__aliasc_wait_after_error() {
    printf '%s' 'aliasc: press Enter to continue ...' >&2
    IFS= read -r
    printf '\n' >&2
}

# --- main logic ---

: "${ALIAS_FILE:=$HOME/.config/alias}"
: "${ALIASC_LOCAL_FILE:=$HOME/.config/alias.local}"
: "${ALIASC_CACHE_DIR:=${XDG_CACHE_HOME:-$HOME/.cache}/aliasc}"
: "${ALIASC_OUTPUT:=$ALIASC_CACHE_DIR/alias.posix}"
: "${ALIASC_MANIFEST:=$ALIASC_OUTPUT.manifest.json}"

if [ ! -f "$ALIAS_FILE" ]; then
    printf '%s\n' "aliasc: alias source is missing: $ALIAS_FILE" >&2
    return 1 2>/dev/null || exit 1
fi

__aliasc_source_dir=${ALIAS_FILE%/*}
[ "$__aliasc_source_dir" = "$ALIAS_FILE" ] && __aliasc_source_dir=.
: "${ALIASC_LOCAL_FILE:=$__aliasc_source_dir/alias.local}"
: "${ALIASC_SHORTCUT_MAP:=$__aliasc_source_dir/ShortcutMap.yaml}"

__aliasc_downloader=$HOME/.local/bin/aliasc.sh
if [ ! -x "$__aliasc_downloader" ]; then
    printf '%s\n' "aliasc: downloader not found: $__aliasc_downloader" >&2
    return 1 2>/dev/null || exit 1
fi

__aliasc_binary=$("$__aliasc_downloader" --print-binary-path 2>/dev/null) || true
if [ -z "$__aliasc_binary" ] || [ ! -x "$__aliasc_binary" ]; then
    printf '%s\n' "aliasc: binary not found; attempting download..." >&2
    "$__aliasc_downloader" >/dev/null || {
        printf '%s\n' 'aliasc: download failed; using shell fallback' >&2
        __aliasc_fallback=1
        __aliasc_wait_after_error
    }
fi

__aliasc_needs_compile=0
if [ "${__aliasc_fallback-0}" = 1 ]; then
    __aliasc_needs_compile=1
elif [ -z "$__aliasc_binary" ] || [ ! -x "$__aliasc_binary" ]; then
    __aliasc_needs_compile=1
elif [ ! -f "$ALIASC_OUTPUT" ] || [ ! -f "$ALIASC_MANIFEST" ]; then
    __aliasc_needs_compile=1
elif [ "$ALIAS_FILE" -nt "$ALIASC_OUTPUT" ]; then
    __aliasc_needs_compile=1
elif [ -f "$ALIASC_LOCAL_FILE" ] && [ "$ALIASC_LOCAL_FILE" -nt "$ALIASC_OUTPUT" ]; then
    __aliasc_needs_compile=1
elif [ -f "$ALIASC_SHORTCUT_MAP" ] && [ "$ALIASC_SHORTCUT_MAP" -nt "$ALIASC_OUTPUT" ]; then
    __aliasc_needs_compile=1
elif [ "$__aliasc_binary" -nt "$ALIASC_OUTPUT" ]; then
    __aliasc_needs_compile=1
fi

if [ "$__aliasc_needs_compile" -eq 1 ]; then
    if ! mkdir -p "$ALIASC_CACHE_DIR" 2>/dev/null; then
        printf '%s\n' "aliasc: cannot create cache directory: $ALIASC_CACHE_DIR" >&2
        return 1 2>/dev/null || exit 1
    fi
    if ! "$__aliasc_binary" compile \
        --shell posix \
        --platform auto \
        --distro auto \
        --environment auto \
        --source "$ALIAS_FILE" \
        --output "$ALIASC_OUTPUT" >/dev/null; then
        __aliasc_fallback=1
        printf '%s\n' 'aliasc: compilation failed; using shell fallback' >&2
        __aliasc_wait_after_error
    fi
fi

if [ "${__aliasc_fallback-0}" = 1 ]; then
    __aliasc_fallback_load
elif [ -f "$ALIASC_OUTPUT" ]; then
    __aliasc_function_list=$ALIASC_CACHE_DIR/.functions.$$
    command sed -n 's/^\([A-Za-z_][A-Za-z0-9_]*\)()[[:space:]]*{$/\1/p' "$ALIASC_OUTPUT" >"$__aliasc_function_list"
    while IFS= read -r __aliasc_function_name; do
        [ -n "$__aliasc_function_name" ] || continue
        unalias "$__aliasc_function_name" 2>/dev/null || :
    done <"$__aliasc_function_list"
    rm -f "$__aliasc_function_list"
    . "$ALIASC_OUTPUT"
    __aliasc_source_status=$?
    if [ "$__aliasc_source_status" -ne 0 ]; then
        printf '%s\n' 'aliasc: generated output could not be loaded' >&2
        return 1 2>/dev/null || exit 1
    fi
else
    printf '%s\n' "aliasc: generated output is missing: $ALIASC_OUTPUT" >&2
    return 1 2>/dev/null || exit 1
fi

unset __aliasc_source_dir __aliasc_binary __aliasc_downloader __aliasc_function_list
unset __aliasc_function_name __aliasc_source_status __aliasc_needs_compile
unset ALIASC_LOCAL_FILE ALIASC_SHORTCUT_MAP ALIASC_CACHE_DIR ALIASC_MANIFEST

# Must be (re)installed last: the generated output redefines this helper with a
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
