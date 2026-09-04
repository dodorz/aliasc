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

Supported `--shell` values are:

```text
posix, bash, zsh, fish, nu, powershell, pwsh, cmd
```

`powershell` and `pwsh` require a Windows target context. Cross-generation is supported, for example a Linux host can compile with `--platform windows`.

## Source Resolution

- `include "path"` is expanded relative to the including file.
- Missing ordinary includes produce warnings; include cycles are errors.
- `alias.local` beside the main source is loaded automatically after the main include tree unless `--no-local` is supplied.
- `ShortcutMap.yaml` beside the source is read by default unless `--no-shortcut-map` is supplied.
- Section activation depends on platform, distro, and environment, not on `--shell`.
- Alias definition names are case-insensitive. A definition such as `PiWeb=...` can be invoked as `piweb`, `Piweb`, or any other ASCII case combination; on case-sensitive shells, aliasc emits the corresponding forwarding entry points.
- `[Windows]` is the v1 legacy cmd-template compatibility section. Shell-specific sections are reserved and inactive in v1.

Every successful compilation writes a manifest containing the resolved target context, tracked inputs, generated outputs, and content hashes.

## Rebuild Helpers

The repository provides PowerShell helpers for manifest-based stale checks and explicit recompilation:

```text
scripts/aliasc-status.ps1
scripts/aliasc-rebuild.ps1
```

Interactive profiles should source or install only the last successful generated artifact. Rebuild operations belong outside the shell startup path.

## Repository Layout

```text
src/          Compiler, resolver, IR, backend, and manifest implementation
tests/        Fixture-driven integration tests
fixtures/     Small DSL source fixtures
scripts/      Manifest status and explicit rebuild helpers
.github/      GitHub Actions shell-matrix workflow
```

## CI And Releases

GitHub Actions builds release binaries for Windows x64/x86/ARM64, macOS Intel/Apple Silicon, Linux x64, and Android ARM64 (Termux) on every branch push. Push a release tag in `MAJOR.MINOR.PATCH` form, such as `0.0.7` or `v0.0.7`, to create a GitHub Release containing all seven binaries. The workflow normalizes an optional `v` prefix and passes the resulting `VERSION` to every build; the binary version, manifest version, and release tag therefore use the same value. Invalid release tags fail before any asset is published.

## License

This project is licensed under the [MIT License](LICENSE).

## Security and Execution Model

Portable templates are parsed into structured IR rather than emitted through `eval` or `Invoke-Expression`. `FirstAvailable` is resolved on first alias invocation and cached for the current shell session; it is never probed during compilation or shell startup.
