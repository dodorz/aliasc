# Homebrew tap for aliasc

This directory holds the Homebrew formula for macOS (Apple Silicon / Intel) and
Linux x64 (Linuxbrew). Installing it puts the compiler **and** the glue scripts
in place, so the only thing left for the user is one profile line.

## Publishing the tap

The formula must live in a repository named `homebrew-tap` under the `dodorz`
GitHub org, with this layout:

```
dodorz/homebrew-tap
└── Formula
    └── aliasc.rb      <- copy of packaging/homebrew/Formula/aliasc.rb
```

## Installing (after the tap is published)

```sh
brew tap dodorz/aliasc https://github.com/dodorz/homebrew-tap
brew install aliasc
```

or the short form `brew install dodorz/aliasc/aliasc`.

## What the formula installs

- `${prefix}/opt/aliasc/libexec/` — `aliasc` plus `alias.sh`, `alias-core.sh`,
  `alias-fetch.sh`, `alias-fallback.sh`, `alias.fish`, `alias.nu`, `alias.ps1`.
  The glue must stay together in one directory (activators locate their
  siblings beside themselves), and the compiler sits beside the glue, so
  `alias-fetch.sh` never needs to download anything: the install is fully
  offline, including recompiles after edits to `~/.config/alias`.
- `${prefix}/bin/aliasc` — a symlink for direct CLI use.
- The printed caveats contain the one profile line per shell, anchored at
  `opt_libexec`, which stays valid across version upgrades.

Users then add to their profile (exactly as with starship):

```sh
# bash/zsh/sh
. "$(brew --prefix)/opt/aliasc/libexec/alias.sh"
```

## Updating the formula for a release

`url`/`sha256` pairs are pinned per platform; refresh them from the release's
`checksums.txt` when versioning (bump `version`, replace the three zip
checksums). `brew bump-formula-pr` can automate this once the tap is public.
`livecheck` (`strategy :github_latest`) keeps `brew livecheck aliasc` working.

## Platform coverage

| Platform | Asset |
| --- | --- |
| macOS Apple Silicon | `aliasc-aarch64-apple-darwin.zip` |
| macOS Intel | `aliasc-x86_64-apple-darwin.zip` |
| Linux x64 | `aliasc-x86_64-unknown-linux-gnu.zip` |
| Linux ARM64 | not packaged yet (upstream only ships an Android/Termux binary) |
