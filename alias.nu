#!/usr/bin/env nu
# alias.nu - nushell activator.
#
# Responsibility: put the generated aliases into the CURRENT nushell session.
# The shell-agnostic work (fetch compiler, staleness check, compile) is done by
# alias-core.sh through `sh`; this file only sources the artifact.
#
# There is no nushell fallback parser: the POSIX fallback works by eval-ing
# POSIX function bodies, which nushell cannot consume.
#
# Install it yourself from config.nu:  source ~/proj/aliasc/alias.nu

if (($env.ALIASDSL_NOLOAD? | default '') == '1') {
    return
}

# These scripts live in the aliasc checkout and are installed by hand, so look
# next to this file first instead of assuming a fixed install prefix.
# Follow symlinks: the siblings live at the target, not next to the link.
let self_file = ($env.CURRENT_FILE? | default '')
let self_dir = (
    if $self_file == '' { '' } else {
        let resolved = (try { ^readlink -f $self_file | str trim } catch { '' })
        if $resolved == '' { $self_file | path dirname } else { $resolved | path dirname }
    }
)
let candidates = ([
    $self_dir
    ($env.HOME | path join proj/aliasc)
    (if ('XDG_DATA_HOME' in $env) { $env.XDG_DATA_HOME | path join aliasc } else { $env.HOME | path join .local/share/aliasc })
])
let found = ($candidates | where {|d| ($d | path join alias-core.sh) | path exists } | first | default '')
let lib_dir = ($env.ALIASC_LIB_DIR? | default (if $found == '' { $env.HOME | path join proj/aliasc } else { $found }))
let core = ($env.ALIASC_CORE? | default ($lib_dir | path join alias-core.sh))
let cache_dir = ($env.ALIASC_CACHE_DIR? | default ((if ('XDG_CACHE_HOME' in $env) { $env.XDG_CACHE_HOME } else { $env.HOME | path join .cache }) | path join aliasc))
let source_file = ($env.ALIAS_FILE? | default ($env.HOME | path join .config/alias))

if not ($source_file | path exists) {
    print --stderr $"aliasc: alias source is missing: ($source_file)"
    return
}
if not ($core | path exists) {
    print --stderr $"aliasc: core not found: ($core)"
    return
}
if (which ^sh | is-empty) {
    print --stderr $"aliasc: 'sh' is required to run ($core)"
    return
}

let fetcher_args = (if (($env.ALIASC_FETCHER? | default '') | is-empty) { [] } else { [--fetcher $env.ALIASC_FETCHER] })
let artifact = (^sh $core --shell nu --source $source_file --cache-dir $cache_dir ...$fetcher_args | complete)
if $artifact.exit_code != 0 {
    print --stderr "aliasc: no nu artifact available; aliases are not loaded"
    return
}

let path = ($artifact.stdout | str trim)
if not ($path | path exists) {
    print --stderr "aliasc: no nu artifact available; aliases are not loaded"
    return
}

source $path
