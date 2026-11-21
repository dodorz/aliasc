class Aliasc < Formula
  desc "Compile a small alias DSL into per-shell alias definitions (bash/zsh/fish/nushell/pwsh)"
  homepage "https://github.com/dodorz/aliasc"
  version "0.2.2"
  license "MIT"

  livecheck do
    url :stable
    strategy :github_latest
  end

  # The release zips bundle the compiler together with the glue scripts, so
  # installing one archive is enough for a fully offline setup: the fetcher
  # finds the compiler beside itself and never downloads.
  # The class-level URL doubles as the fallback brew requires so the formula
  # parses on every platform combination; unsupported ones bail in #install.
  url "https://github.com/dodorz/aliasc/releases/download/#{version}/aliasc-x86_64-unknown-linux-gnu.zip"
  sha256 "e4537bbc7a0c148e1fba22e991673b52f5342cfb6be3c8a60b843c0c5919e55d"

  on_macos do
    on_arm do
      url "https://github.com/dodorz/aliasc/releases/download/#{version}/aliasc-aarch64-apple-darwin.zip"
      sha256 "8a162a282da5eb1e77a3ac4891d07c10a4d5f04793ad759f050e43bb8aa17c44"
    end
    on_intel do
      url "https://github.com/dodorz/aliasc/releases/download/#{version}/aliasc-x86_64-apple-darwin.zip"
      sha256 "a3195258787028bc1860679d117ffdd678f06013c951fb81b6442e44785f9f29"
    end
  end

  def install
    # No glibc aarch64 asset exists yet (upstream ships Android/Termux only);
    # fail here instead of installing an x86_64 binary that cannot exec.
    # Adding a linux-aarch64 target to build-release.yml removes this guard.
    if OS.linux? && Hardware::CPU.arm?
      odie "aliasc does not ship a Linux ARM64 build yet (see the release assets)"
    end

    # The glue must stay together in one directory: each activator locates its
    # siblings beside itself, and alias-core.sh finds alias-fetch.sh the same
    # way. libexec keeps these support scripts out of PATH while opt_libexec
    # gives profiles a path that survives upgrades.
    %w[
      alias.sh
      alias.fish
      alias.nu
      alias.ps1
      alias-core.sh
      alias-fetch.sh
      alias-fallback.sh
    ].each { |f| libexec.install f }

    libexec.install "aliasc"
    # Zip archives may lose the executable bit depending on the extractor.
    FileUtils.chmod 0o755, [libexec/"aliasc", libexec/"alias-fetch.sh"]
    bin.install_symlink "../libexec/aliasc"
  end

  def caveats
    <<~EOS
      Alias sources live in ~/.config/alias (plus optional alias.local and
      ShortcutMap.yaml beside it); create one to get started.

      Activate aliasc by adding ONE line to your shell profile. The path below
      is stable across upgrades:

        bash/zsh/sh:
          . #{opt_libexec}/alias.sh
        fish:
          source #{opt_libexec}/alias.fish
        nushell:
          source #{opt_libexec}/alias.nu      # in config.nu
        PowerShell:
          . #{opt_libexec}/alias.ps1          # in $PROFILE
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{libexec}/aliasc --version")
    (testpath/"alias").write("[Common]\nhello=echo world\n")
    system libexec/"aliasc", "compile", "--shell", "posix",
           "--source", testpath/"alias", "--output", testpath/"out.sh"
    assert_match "hello()", (testpath/"out.sh").read
  end
end
