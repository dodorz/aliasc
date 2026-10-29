# aliasc

`aliasc` is a Rust compiler for Alias DSL v2. It compiles one shared alias source file into a sourceable, target-shell-specific runtime artifact. Shell startup loads only generated output; it does not parse the DSL, start Rust, or probe `FirstAvailable` candidates.

The Alias DSL v2 specification is authoritative. Existing PowerShell and shell loaders are migration references only.

## Status

This is an in-progress implementation. The command-line interface, source resolver, manifest generation, and initial shell backends are present. The test suite and CI workflow are being expanded toward the full v2 shell behavior matrix.

## Build

```text
cargo build --release
```

The binary is written to `target/release/aliasc` on Unix-like systems and `target/release/aliasc.exe` on Windows.

Run the automated tests with:

```text
cargo test
```

## CLI

Compile a DOSKEY macro file for Windows cmd:

```text
aliasc compile --shell cmd --platform windows \
  --source C:\\path\\to\\alias \
  --output C:\\path\\to\\alias.doskey
```

The cmd backend also writes a sibling `aliasc-runtime.cmd` and an `<output>.manifest.json` file.

Validate a target context without writing an output:

```text
aliasc validate --shell pwsh --platform windows --source C:\\path\\to\\alias
```

Inspect active definitions:

```text
aliasc list --shell bash --platform linux --source /path/to/alias
aliasc search --shell cmd --platform windows --source C:\\path\\to\\alias proxy
aliasc explain --shell cmd --platform windows --source C:\\path\\to\\alias pkg
```

`aliasc list` prints one definition name per line. Add `--format tsv` to emit `name<TAB>body`, where `body` is the single-line `display_body()` form used by the generated `alias-search` builtin. Fields escape backslash, tab, LF, and CR as `\\`, `\t`, `\n`, and `\r`. The default remains name-only for backward compatibility.

Supported `--shell` values are:

```text
posix, bash, zsh, fish, nu, powershell, pwsh, cmd
```

`powershell` and `pwsh` require a Windows target context. Cross-generation is supported, for example a Linux host can compile with `--platform windows`.

## Bypassing Aliases

Each backend has a supported spelling for running the command underneath an alias. These spellings are pinned by the test suite and the CI shell matrix wherever the runner provides the shell.

Every generated artifact also defines `raw`, a uniform escape hatch that works the same way in every target shell:

```text
raw XXX
```

`raw XXX` runs the external command `XXX` instead of the alias generated for it, so it does not depend on remembering the shell-native spelling below. The per-shell forms remain supported for direct use.

| Target shell | Run the real command | Notes |
| --- | --- | --- |
| `posix`, `bash`, `zsh` | `command XXX` | The generated artifact defines shell functions. |
| `fish` | `command XXX` | fish has no alias/function distinction; its `alias` is function syntax. |
| `nu` | `^XXX` | The generated artifact defines `def` commands. |
| `powershell`, `pwsh` | `XXX.exe`, or `& (Get-Command XXX -CommandType Application).Path` | `\` is not an escape character in PowerShell. |
| `cmd` | `cmd /c XXX` | doskey macros are session-scoped; a fresh `cmd /c` only sees the real command. |

On POSIX shells, `\XXX` is **not** a bypass and must not be treated as one. A backslash only suppresses alias expansion, and aliasc defines shell functions rather than aliases, so `\XXX` silently executes the alias body instead of the external command. Use `command XXX`.

The `\` prefix inside an alias body (`cat=\cat -n`) means something different: it is DSL syntax that makes the compiler emit `command`, so the alias body calls the external command instead of recursing into the alias. The two uses are unrelated, and the body form is unaffected by the limitation above.

## Source Resolution

- `include "path"` is expanded relative to the including file.
- Missing ordinary includes produce warnings; include cycles are errors.
- `alias.local` beside the main source is loaded automatically after the main include tree unless `--no-local` is supplied.
- `ShortcutMap.yaml` is read from `$HOME/.config/ShortcutMap.yaml` by default; when that file does not exist, a `ShortcutMap.yaml` beside the alias source is used instead, unless `--shortcut-map` supplies another path or `--no-shortcut-map` disables it.
- Section activation depends on platform, distro, and environment, not on `--shell`.
- Alias definition names are case-insensitive when definitions are resolved, but generated entry points are intentionally limited for compactness. A definition such as `PiWeb=...` can be invoked as `PiWeb` or `piweb`; mixed-case variants such as `Piweb` are not generated on case-sensitive shells.
- Definitions may span multiple lines while parentheses are unbalanced. This is mostly useful for listing one `FirstAvailable` candidate per line:

  ```text
  cat=FirstAvailable(
    batcat
    bat
    cat
  )
  ```

  `SetEnv(...)`, `UnsetEnv(...)`, and `WithEnv(...)` entries may be listed the same way, and comma and newline separators can be mixed. Inside a multi-line definition, blank lines and lines starting with `#` or `;` are ignored, and an inline `#` comment may also follow the opening parenthesis or a candidate. A definition that reaches the end of the file with an unclosed `(` is an error. The POSIX shell fallback in `alias.sh` parses the same multi-line forms for `FirstAvailable`, `SetEnv`, `UnsetEnv`, and `WithEnv`, so behavior does not depend on whether the compiler binary is available.
- `?(...)` is a shorthand for `FirstAvailable(...)`: `cat=?(batcat, bat, cat)` is equivalent to `cat=FirstAvailable(batcat, bat, cat)`. A `?` anywhere other than immediately before the opening parenthesis at the start of a definition body is a literal character.
- `[Windows]` is the v1 legacy cmd-template compatibility section; portable `SetEnv(...)`, `UnsetEnv(...)`, and `WithEnv(...)` templates are still parsed there. Shell-specific sections are reserved and inactive in v1.

Every successful compilation writes a manifest containing the resolved target context, tracked inputs, generated outputs, and content hashes.

## ShortcutMap

`ShortcutMap.yaml` is an optional compile-time definition source, not a runtime mechanism. It is expanded into ordinary alias definitions before section selection and duplicate resolution. Definitions in the main alias source and `alias.local` therefore override generated ShortcutMap definitions with the same name.

Built-in sections:

```yaml
Config:
  Foo: ~/.foo

History:
  Bash: ~/.bash_history

Log:
  App: /var/log/app.log
```

They expand to `EditFooConfig`, `EditBashHistory`, and `ViewAppLog`, using `${EDITOR}` or `${LOGVIEWER}`. ShortcutMap values are treated as data and quoted before being inserted into the DSL.

Values that look like Windows paths keep their backslashes literally, even inside double quotes. For example, `NotepadPlus: "%APPDATA%\Notepad++\config.xml"` works without doubling every backslash. Plain unquoted paths, single-quoted paths, and standard YAML escapes in other double-quoted scalars are left unchanged.

The generic section provides an escape hatch for definitions that do not fit a built-in verb:

```yaml
Config:
  Foo: ~/.foo
aliases:
  Custom: printf custom
```

Unknown sections produce warnings instead of being silently ignored.

### Forward

`Forward` reads concrete `Host` aliases from `~/.ssh/config` (override with `ALIASC_SSH_CONFIG`) and expands each app into a host-specific alias:

```yaml
Forward:
  Syncthing: 8384
```

With `Host mac`, this generates `ForwardMacSyncthing`, whose body is effectively:

```sh
ssh -N -o ExitOnForwardFailure=yes -L 8384:127.0.0.1:8384 mac
```

The command still runs `ssh mac`, so `HostName`, `User`, `Port`, `ProxyJump`, and other SSH options are resolved by SSH at runtime. `Host *`, wildcard patterns, and negated patterns are skipped. `Include` files are expanded and tracked as dependencies.

`port` (or `remote_port`) is the remote target port. `local_port` is the local listening port and defaults to the remote port. `remote_host` defaults to `127.0.0.1`, and `bind` is the local bind address, also defaulting to `127.0.0.1`.

To restrict hosts or control the alias fragment:

```yaml
Forward:
  hosts:
    mac: Mac
    ecs: ECS
  exclude_hosts: ["github.com"]
  exclude_users: [git]
  apps:
    Syncthing: 8384
    Jellyfin:
      remote_port: 8096
      local_port: 18096
      bind: 0.0.0.0
      remote_host: 192.168.1.10
```

`hosts` may also be a list of SSH host names. `exclude_hosts` supports `*` and `?` patterns. `exclude_users` filters hosts by their effective SSH `User`, using the first matching `Host` rule in configuration order.

Architecturally, `ShortcutMap` is an input adapter: `src/shortcut.rs` parses YAML, dispatches each top-level key to a `ShortcutHandler`, and returns `RawDefinition` values plus diagnostics and tracked dependencies. The resolver only merges definitions and never needs to know what a section such as `Config` or `Forward` means. New verbs are added by implementing a handler rather than changing the resolver.

## Rebuild Helpers

The repository provides PowerShell helpers for manifest-based stale checks and explicit recompilation:

```text
scripts/aliasc-status.ps1
scripts/aliasc-rebuild.ps1
```

Interactive profiles should source or install only the last successful generated artifact. Rebuild operations belong outside the shell startup path.

## Repository Layout

```text
src/          Compiler, resolver, IR, backend, ShortcutMap adapter, SSH config reader, and manifest implementation
tests/        Fixture-driven integration tests
fixtures/     Small DSL source fixtures
scripts/      Manifest status and explicit rebuild helpers
.github/      GitHub Actions shell-matrix workflow
```

## CI And Releases

GitHub Actions builds release binaries for Windows x64/x86/ARM64, macOS Intel/Apple Silicon, Linux x64, and Android ARM64 (Termux) on every branch push. `Cargo.toml` is the single source of truth for the version: every build embeds it, and on a release tag push the workflow fails unless the tag (with an optional `v` prefix) names exactly that version. Push a release tag in `MAJOR.MINOR.PATCH` form, such as `0.2.1` or `v0.2.1`, to create a GitHub Release containing all seven binaries. The binary version, manifest version, and release tag therefore always agree. Before publishing, the release job generates a `checksums.txt` asset containing SHA-256 values for all binaries. GitHub additionally exposes each uploaded asset's SHA-256 as the `digest` field in the Release API, so Scoop manifests can use either source. Invalid or mismatched release tags fail before any asset is published.

## License

This project is licensed under the [MIT License](LICENSE).

## Security and Execution Model

Portable templates are parsed into structured IR rather than emitted through `eval` or `Invoke-Expression`. `FirstAvailable` is resolved on first alias invocation and cached for the current shell session; it is never probed during compilation or shell startup.
