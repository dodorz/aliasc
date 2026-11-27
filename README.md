# aliasc

[English](README.md) | [中文](README.zh-CN.md)

`aliasc` compiles one plain-text alias source file into fast, per-shell startup
artifacts — for bash, zsh, sh/dash, fish, nushell, PowerShell, and cmd.

**The problem it solves.** Aliases multiply across machines and shells, each
shell wants its own syntax (`alias x=...` vs `def x` vs `Set-Alias` vs doskey),
and portable aliases can't say things like "use `bat` if it exists, else `cat`".
aliasc keeps a single source of truth and pays the parsing cost once, at
compile time: your shell startup only sources a generated file of native
definitions — no DSL parser, no subprocess, no candidate probing at startup.

```
~/.config/alias   (one DSL file, all shells)
        │  aliasc compile --shell <shell> --platform <platform>
        ▼
~/.cache/aliasc/alias.<shell>            ← what the shell actually loads
~/.cache/aliasc/alias.<shell>.manifest.json  ← freshness metadata
```

## Install

### Homebrew (macOS, Linux x64)

This repository doubles as its own Homebrew tap:

```sh
brew tap dodorz/aliasc https://github.com/dodorz/aliasc
brew install aliasc
```

The install puts the compiler and the glue scripts together in
`$(brew --prefix)/opt/aliasc/libexec`, so everything — including recompiles
after you edit the alias source — works offline. Add one line to your profile
(the path follows upgrades automatically):

```sh
# bash / zsh / sh
. "$(brew --prefix)/opt/aliasc/libexec/alias.sh"
```

fish: `source .../alias.fish` · nushell: `source .../alias.nu` ·
PowerShell: `. .../alias.ps1`

### Release bundle (any platform, no package manager)

Each release publishes one `.zip` per platform containing the compiled binary
plus the glue:

```sh
unzip -d aliasc aliasc-x86_64-unknown-linux-gnu.zip
cd aliasc
./alias-install.sh --shell bash --profile ~/.bashrc   # copies glue, writes the activation line
```

```powershell
Expand-Archive aliasc-x86_64-pc-windows-msvc.zip -DestinationPath aliasc
cd aliasc
.\alias-install.ps1 -Shell pwsh -Profile $PROFILE
```

`./alias-install.sh --no-compiler` installs the glue only; the fetcher then
downloads the binary on first shell startup. Bare binaries are also published
beside the zips (used by the bootstrap path and Scoop manifests).

Every bundle is self-contained: `alias-fetch.sh` finds the compiler beside
itself and never downloads. The fetcher can also bootstrap a glue-only install
from the loose binaries.

## The alias source

`~/.config/alias` (override with `ALIAS_FILE`). Format: `name=body`, grouped
into `[Section]`s selected by target platform, distro, and environment.

```ini
include "shared.alias"          # expanded relative to the including file

[Common]                        # every target
greet=printf "hello ${USER}\n" @1
cat=?(bat, cat)                 # FirstAvailable shorthand: bat if installed, else cat
pip=WithEnv(PIP_REQUIRE_VIRTUALENV=false) pip @*
work=SetEnv(HTTP_PROXY=http://127.0.0.1:7890)
nowork=UnsetEnv(HTTP_PROXY HTTPS_PROXY)
pipe=grep -c @@                 # @@ is a literal @

[UNIX]                          # Linux + macOS
[macOS] [Ubuntu] [Debian] [Fedora] [Arch] [Windows] [WSL] [GitBash] [MSYS2]
```

- **Sections** are platform/distro/environment selectors, not shell selectors:
  `Common`, `UNIX`, `macOS`, `Windows`, `Ubuntu`, `Debian`, `Fedora`, `Arch`,
  `WSL`, `GitBash`, `MSYS2`. Last definition of the same name (case-insensitive)
  wins. `[Bash]`-style shell sections are reserved for future use.
- **Arguments**: `@1`…`@9`, `@*` (all arguments). Bodies without any `@`
  placeholder get `@*` appended automatically.
- **Templates**: `FirstAvailable(a, b, …)` resolves to the first installed
  command at *first invocation* (cached per session); `SetEnv(...)` /
  `UnsetEnv(...)` / `WithEnv(...)` scope environment changes; `\cmd` in a body
  forces the external command instead of recursing into the alias.
- **Multi-line definitions**: entries may span lines while parentheses are
  unbalanced — one `FirstAvailable` candidate per line reads best. Blank lines,
  `#`/`;` comments, and trailing inline `#` comments are ignored inside them.
- `alias.local` beside the main source is appended automatically (machine-local
  overrides); `ShortcutMap.yaml` (see below) is another optional input.
- Names are case-insensitive at resolution; generated entry points cover the
  exact spelling and its all-lowercase form.

## ShortcutMap (optional)

A YAML file (default `$HOME/.config/ShortcutMap.yaml`, else beside the alias
source) expanded at compile time into ordinary definitions — data, not runtime
machinery:

```yaml
Config:
  Foo: ~/.foo            # → EditFooConfig (opens ${EDITOR})

History:
  Bash: ~/.bash_history

Log:
  App: /var/log/app.log  # → ViewAppLog (uses ${LOGVIEWER})

aliases:                  # free-form escape hatch
  Custom: printf custom

Forward:                  # reads Host entries from ~/.ssh/config
  Syncthing: 8384         # → ForwardMacSyncthing = ssh -N -L 8384:... mac
  apps:
    Jellyfin:
      remote_port: 8096
      local_port: 18096
```

Definitions from the main source and `alias.local` override ShortcutMap
entries with the same name. Values are treated as data and quoted before being
inserted into the DSL; Windows-style paths keep their backslashes literally.

## CLI

```sh
aliasc compile --shell posix --platform linux --source ~/.config/alias --output out.sh
aliasc validate --shell pwsh --platform windows --source C:\path\to\alias
aliasc list --shell bash --platform linux --source ...        # names (--format tsv: name<TAB>body)
aliasc search --shell bash --platform linux --source ... proxy
aliasc explain --shell bash --platform linux --source ... pkg  # what an alias expands to
aliasc run update -- --deep            # execute a definition outside any shell
```

### Running aliases outside the shell

Shell functions do not cross process boundaries, so parents that exec programs
directly never see your aliases: `sudo`, `env`, `xargs`, cron, `ssh host cmd`.
`aliasc run NAME [-- args...]` closes that gap: it resolves the definition for
the current target context (sections, `FirstAvailable` probing, `WithEnv`)
and executes it in a fresh `sh`. Because `aliasc` itself is a real binary on
PATH, `sudo` is happy to run it:

```sh
sudo aliasc run update-system
sudo aliasc run pip -- install -U rich
sudo "$(command -v aliasc)" run backup   # when the brew bin is not in sudo's secure_path
```

`--source` defaults to `$ALIAS_FILE`, then `~/.config/alias`; child exit codes
propagate. `SetEnv`/`UnsetEnv` definitions have no command to execute and are
rejected. (Remote hosts are a deployment concern: they need their own aliasc
or shim copy.)

`--shell` accepts `posix`, `bash`, `zsh`, `fish`, `nu`, `powershell`/`pwsh`,
`cmd`. `powershell`/`pwsh` require a Windows target context; cross-compilation
from any host is supported (`--platform`, `--distro`, `--environment` default
to auto-detection, `--no-local` / `--shortcut-map` / `--no-shortcut-map`
control extra inputs). Every compile writes a manifest recording the resolved
context, tracked inputs (mtimes), and content hashes.

## Shell integration

Exactly one file per shell is sourced from the profile; it loads the cached
artifact into the **current session** and drops definitions from the previous
generation first:

| Shell | Profile line | Artifact |
| --- | --- | --- |
| sh, bash, zsh, dash | `. /path/to/aliasc/alias.sh` | `alias.posix` |
| fish | `source /path/to/aliasc/alias.fish` | `alias.fish` |
| nushell | `source /path/to/aliasc/alias.nu` | `alias.nu` |
| PowerShell 5.1 / 7+ | `. /path/to/aliasc/alias.ps1` | `alias.pwsh` |

The glue files (below) must stay together in one directory; each activator
finds its siblings beside itself, falling back to
`${XDG_DATA_HOME:-~/.local/share}/aliasc` (set `ALIASC_LIB_DIR` to win
outright). Under dash, which cannot report the sourced path, only the
data-directory candidate works — install the glue there or export
`ALIASC_LIB_DIR`.

### The glue

| File | Role |
| --- | --- |
| `alias.sh` / `alias.fish` / `alias.nu` / `alias.ps1` | Per-shell activators: fetch → staleness check → load the artifact into the current session. Loading is shell-private, hence one file each. |
| `alias-core.sh` | Shared POSIX core: locate the compiler, decide freshness from the manifest, compile, print the artifact path on stdout. Loads nothing itself. |
| `alias-fetch.sh` | Downloader: reports `--print-binary-path`, downloads the platform-matched binary (with glibc/musl detection, wget fallback) when missing, caches it under `${XDG_CACHE_HOME:-~/.cache}/aliasc/bin`. |
| `alias-fallback.sh` | POSIX-only degraded parser: when no compiler can be obtained *and* no artifact exists, parses the DSL with pure `sh` so a broken network never leaves the shell without aliases. |
| `alias-install.sh` / `alias-install.ps1` | Bundle installers: copy the glue (and bundled compiler) into one directory and write the activation line; `--uninstall` restores the profile byte-for-byte. |

`alias.ps1` cannot assume `/bin/sh`, so it performs the core/fetcher steps
itself in PowerShell (`ALIASC_BINARY` skips fetching entirely); the fish/nu
activators just call `alias-core.sh` through `sh`.

### Degraded paths

| Situation | POSIX | fish / nu / PowerShell |
| --- | --- | --- |
| Compiler present, artifact fresh | load artifact | load artifact |
| Compiler present, artifact stale | recompile, then load | recompile, then load |
| Download fails, artifact exists | reuse artifact | reuse artifact |
| Download fails, no artifact | `alias-fallback.sh` parses the DSL | error, no aliases |

Only POSIX degrades into a parser: the fallback works by `eval`-ing POSIX
function bodies, which the other shells cannot consume.

## Bypassing an alias

Every artifact defines a uniform escape hatch:

```sh
raw ls --all      # runs the external ls, ignoring the alias
```

Per-shell native spellings: `command ls` (POSIX shells, fish), `^ls`
(nushell), `ls.exe` or `& (Get-Command ls -CommandType Application).Path`
(PowerShell), `cmd /c ls` (cmd). On POSIX shells `\ls` is *not* a bypass —
aliasc emits functions, and a backslash only affects alias expansion, so it
still runs the alias body.

For process boundaries that functions cannot cross (`sudo`, cron, `xargs`),
use `aliasc run` — see [Running aliases outside the shell](#running-aliases-outside-the-shell).

## Environment variables

| Variable | Meaning |
| --- | --- |
| `ALIAS_FILE` | Alias source path (default `~/.config/alias`) |
| `ALIASDSL_NOLOAD=1` | Activators exit without loading (for one-off tests) |
| `ALIASC_CACHE_DIR` | Artifact cache (default `${XDG_CACHE_HOME:-~/.cache}/aliasc`) |
| `ALIASC_LIB_DIR` | Directory holding the glue (activators normally locate it themselves) |
| `ALIASC_CORE`, `ALIASC_FALLBACK`, `ALIASC_FETCHER` | Explicit paths for the core, fallback parser, fetcher |
| `ALIASC_BINARY` | Use this existing compiler; skip downloading (PowerShell activator) |
| `ALIASC_BINARY_CACHE_DIR` | Compiler download cache (default `.../aliasc/bin`) |
| `ALIASC_REPOSITORY`, `ALIASC_RELEASE` | Download source (default `dodorz/aliasc`, `latest`) |
| `ALIASC_PLATFORM` | Platform override for the PowerShell activator |
| `ALIASC_SSH_CONFIG` | SSH config read by the ShortcutMap `Forward` section |

## Build from source

```sh
cargo build --release        # target/release/aliasc
cargo test                   # unit + fixture-driven shell integration tests
```

Rust stable; no runtime dependencies outside the standard library beyond
`clap` and `serde`/`serde_yaml`.

## Releases

GitHub Actions builds Linux x64 (glibc + musl), macOS Intel/Apple Silicon,
Windows x64/x86/ARM64, and Android ARM64 (Termux) on every push; a
`MAJOR.MINOR.PATCH` tag (optionally `v`-prefixed) matching the version in
`Cargo.toml` — the single source of truth — publishes a release with all
binaries, per-platform zip bundles, and `checksums.txt`. `Cargo.lock` must
carry the same version or `--locked` builds fail. Because the Homebrew formula
in this repository's default branch points at release assets, tag and publish
the release first, then commit the formula's version/checksum bump.

## License

MIT
