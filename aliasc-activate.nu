#!/usr/bin/env nu
# aliasc-activate.nu - nushell activator.
#
# Responsibility: put the generated aliases into the CURRENT nushell session.
# The shell-agnostic work (fetch compiler, staleness check, compile) is done by
# aliasc-core.sh through `sh`; this file only sources the artifact.
#
# There is no nushell fallback parser: the POSIX fallback works by eval-ing
# POSIX function bodies, which nushell cannot consume.
#
# Install from config.nu:  source ~/.config/nushell/aliasc.nu

if (($env.ALIASDSL_NOLOAD? | default '') == '1') {
    return
}

let lib_dir = ($env.ALIASC_LIB_DIR? | default (
    if ('XDG_DATA_HOME' in $env) { $env.XDG_DATA_HOME | path join aliasc } else { $env.HOME | path join .local/share/aliasc }
))
let core = ($env.ALIASC_CORE? | default ($lib_dir | path join aliasc-core.sh))
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
