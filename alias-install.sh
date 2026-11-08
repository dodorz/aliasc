#!/bin/sh
# alias-install.sh - install the aliasc glue scripts from a release bundle.
#
# Copies the compiler and the glue into one directory, then tells you which line
# to add to your shell profile (or adds it for you with --profile).
#
# Usage:
#   ./alias-install.sh [OPTIONS]
#
# Options:
#   --shell NAME     Shell to activate: posix, bash, zsh, fish, nu (default: infer
#                    from $SHELL, else posix)
#   --dir PATH       Install directory (default ${XDG_DATA_HOME:-~/.local/share}/aliasc)
#   --profile FILE   Append the activation line to FILE (created if missing).
#                    Nothing is written unless this option is given.
#   --no-compiler    Do not copy the compiler binary; rely on alias-fetch.sh
#                    downloading it on first use
#   --uninstall      Remove the installed glue (and the profile line, if --profile)
#   --force          Overwrite an existing install
#   --dry-run        Print what would happen without touching the filesystem
#   --help           Show this help
#
# Exit status: 0 on success, 1 on any failure.

set -u

fail() {
    printf 'aliasc: %s\n' "$*" >&2
    exit 1
}

usage() {
    sed -n '2,/^$/{ s/^# \{0,1\}//; p; }' "$0"
}

note() { printf '%s\n' "$*"; }

shell=
install_dir=
profile=
copy_compiler=1
uninstall=0
force=0
dry_run=0

while [ $# -gt 0 ]; do
    case "$1" in
        --shell) [ $# -ge 2 ] || fail "--shell requires a value"; shell=$2; shift 2 ;;
        --shell=*) shell=${1#*=}; shift ;;
        --dir) [ $# -ge 2 ] || fail "--dir requires a value"; install_dir=$2; shift 2 ;;
        --dir=*) install_dir=${1#*=}; shift ;;
        --profile) [ $# -ge 2 ] || fail "--profile requires a value"; profile=$2; shift 2 ;;
        --profile=*) profile=${1#*=}; shift ;;
        --no-compiler) copy_compiler=0; shift ;;
        --uninstall) uninstall=1; shift ;;
        --force) force=1; shift ;;
        --dry-run) dry_run=1; shift ;;
        --help|-h) usage; exit 0 ;;
        *) fail "unknown option: $1" ;;
    esac
done

# --- locate the bundle ----------------------------------------------------

self=$0
self_dir=${self%/*}
[ "$self_dir" = "$self" ] && self_dir=.
resolved=$(readlink -f "$self" 2>/dev/null) || resolved=
[ -n "$resolved" ] && self_dir=${resolved%/*}

for needed in alias.sh alias-core.sh alias-fetch.sh alias-fallback.sh; do
    [ -f "$self_dir/$needed" ] || fail "bundle is incomplete: missing $self_dir/$needed"
done

# --- work out the shell ---------------------------------------------------

if [ -z "$shell" ]; then
    case "${SHELL-}" in
        */fish) shell=fish ;;
        */nu) shell=nu ;;
        */zsh) shell=zsh ;;
        */bash) shell=bash ;;
        *osh) shell=posix ;;
        *) shell=posix ;;
    esac
    note "aliasc: no --shell given, inferred '$shell' from \$SHELL"
fi

case "$shell" in
    posix|bash|zsh) activator=alias.sh ;;
    fish) activator=alias.fish ;;
    nu) activator=alias.nu ;;
    pwsh|powershell) activator=alias.ps1 ;;
    *) fail "unsupported shell: $shell (expected posix, bash, zsh, fish, nu, pwsh)" ;;
esac

# --- destination ----------------------------------------------------------

: "${install_dir:=${XDG_DATA_HOME:-$HOME/.local/share}/aliasc}"

# A fish/nu install still needs the POSIX core and fetcher, so every glue file
# is always installed; only the activation line differs.
glue_files="alias.sh alias.fish alias.nu alias.ps1 alias-core.sh alias-fetch.sh alias-fallback.sh"

compiler=
for candidate in aliasc aliasc.exe; do
    if [ -f "$self_dir/$candidate" ]; then
        compiler=$self_dir/$candidate
        break
    fi
done

if [ "$copy_compiler" -eq 1 ] && [ -z "$compiler" ]; then
    fail "bundle has no compiler; pass --no-compiler to install the glue only"
fi

# --- activation line ------------------------------------------------------

case "$shell" in
    posix|bash|zsh) line=". $install_dir/$activator" ;;
    *) line="source $install_dir/$activator" ;;
esac
marker="# aliasc ($shell)"

# --- uninstall ------------------------------------------------------------

# Remove the marker and the single line that follows it, leaving the rest of the
# profile untouched. Repeated runs are harmless.
__remove_block() {
    _file=$1
    _tmp=$_file.aliasc.$$
    awk -v marker="# aliasc ($shell)" '
        $0 == marker { skip = 2; next }
        skip > 0 { skip--; next }
        { print }
    ' "$_file" >"$_tmp" || { rm -f "$_tmp"; return 1; }
    # Drop the trailing blank separator the install adds (never runs of blanks
    # inside the file), so install followed by uninstall leaves the profile
    # byte-identical.
    awk '/^[[:space:]]*$/ { pending = pending "\n"; next }
         { printf "%s%s\n", pending, $0; pending = "" }' "$_tmp" >"$_tmp.2" \
        && mv -f "$_tmp.2" "$_tmp"
    mv -f "$_tmp" "$_file"
}

if [ "$uninstall" -eq 1 ]; then
    if [ "$dry_run" -eq 1 ]; then
        note "would remove $install_dir"
    else
        [ -d "$install_dir" ] || fail "nothing installed at $install_dir"
        for f in $glue_files; do
            rm -f "$install_dir/$f"
        done
        rm -f "$install_dir/aliasc" "$install_dir/aliasc.exe"
        # Only drop the directory when nothing else lives there.
        rmdir "$install_dir" 2>/dev/null || :
        note "removed aliasc glue from $install_dir"
    fi
    if [ -n "$profile" ] && [ -f "$profile" ]; then
        if [ "$dry_run" -eq 1 ]; then
            note "would remove the activation block from $profile"
        else
            __remove_block "$profile"
            note "removed the activation line from $profile"
        fi
    fi
    exit 0
fi

# --- install --------------------------------------------------------------

if [ -d "$install_dir" ] && [ "$force" -ne 1 ]; then
    for f in $glue_files; do
        if [ -e "$install_dir/$f" ]; then
            fail "$install_dir already contains $f; pass --force to overwrite"
        fi
    done
fi

if [ "$dry_run" -eq 1 ]; then
    note "would install to $install_dir:"
    for f in $glue_files; do
        note "  $f"
    done
    [ -n "$compiler" ] && note "  $(basename "$compiler")"
    note "would activate with: $install_dir/$activator"
else
    mkdir -p "$install_dir" || fail "cannot create $install_dir"
    for f in $glue_files; do
        cp "$self_dir/$f" "$install_dir/$f" || fail "cannot copy $f"
    done
    chmod 0644 "$install_dir"/*.sh "$install_dir"/*.fish "$install_dir"/*.nu "$install_dir"/*.ps1 2>/dev/null || :
    if [ -n "$compiler" ]; then
        cp "$compiler" "$install_dir/$(basename "$compiler")" || fail "cannot copy the compiler"
        chmod 0755 "$install_dir/$(basename "$compiler")" || fail "cannot mark the compiler executable"
    fi
    chmod 0755 "$install_dir/alias-fetch.sh" "$install_dir/alias-install.sh" 2>/dev/null || :
    note "installed the aliasc glue into $install_dir ($shell)"
fi

# --- activation line ------------------------------------------------------

if [ -n "$profile" ]; then
    if [ "$dry_run" -eq 1 ]; then
        note "would append to $profile:"
        note "  $marker"
        note "  $line"
    else
        if [ -f "$profile" ] && grep -qF "$marker" "$profile"; then
            note "activation already present in $profile"
        else
            mkdir -p "${profile%/*}" 2>/dev/null || :
            {
                printf '\n%s\n%s\n' "$marker" "$line"
            } >>"$profile" || fail "cannot write $profile"
            note "appended the activation line to $profile"
        fi
    fi
    exit 0
fi

cat <<EOF

Add this to your shell profile:

  $marker
  $line

Then start a new shell, or run it now to try it.
EOF
