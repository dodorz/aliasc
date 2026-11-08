#!/usr/bin/env fish
# aliasc-activate.fish - fish shell activator.
#
# Responsibility: put the generated aliases into the CURRENT fish session.
# The shell-agnostic work (fetch compiler, staleness check, compile) is done by
# aliasc-core.sh through `sh`; this file only sources the artifact.
#
# There is no fish fallback parser: the POSIX fallback works by eval-ing POSIX
# function bodies, which fish cannot consume. If no artifact can be produced the
# session simply starts without aliases and prints why.
#
# Install as: ~/.config/fish/conf.d/aliasc.fish

if set -q ALIASDSL_NOLOAD; and test "$ALIASDSL_NOLOAD" = 1
    return 0
end

set -q ALIASC_LIB_DIR; or set -g ALIASC_LIB_DIR (set -q XDG_DATA_HOME; and echo $XDG_DATA_HOME/aliasc; or echo $HOME/.local/share/aliasc)
set -q ALIASC_CORE; or set -g ALIASC_CORE $ALIASC_LIB_DIR/aliasc-core.sh
set -q ALIASC_CACHE_DIR; or set -g ALIASC_CACHE_DIR (set -q XDG_CACHE_HOME; and echo $XDG_CACHE_HOME; or echo $HOME/.cache)/aliasc
set -q ALIAS_FILE; or set -g ALIAS_FILE $HOME/.config/alias

if not test -f "$ALIAS_FILE"
    echo "aliasc: alias source is missing: $ALIAS_FILE" >&2
    return 1
end
if not test -f "$ALIASC_CORE"
    echo "aliasc: core not found: $ALIASC_CORE" >&2
    return 1
end
if not type -q sh
    echo "aliasc: 'sh' is required to run $ALIASC_CORE" >&2
    return 1
end

set -l artifact (sh $ALIASC_CORE --shell fish --source $ALIAS_FILE --cache-dir $ALIASC_CACHE_DIR)
if test $status -ne 0; or not test -f "$artifact"
    echo "aliasc: no fish artifact available; aliases are not loaded" >&2
    return 1
end

source $artifact
