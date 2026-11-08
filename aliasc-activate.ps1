#!/usr/bin/env pwsh
# aliasc-activate.ps1 - PowerShell / pwsh activator.
#
# Responsibility: put the generated aliases into the CURRENT PowerShell session.
#
# Windows has no dependable /bin/sh, so this activator cannot reuse
# aliasc-core.sh and implements the same three steps itself: fetch the compiler,
# compare the manifest inputs against the artifact, then compile and dot-source
# the result.
#
# It still calls the POSIX fetcher through `sh` rather than carrying a second
# download implementation: Git for Windows, WSL, and every Unix PowerShell host
# provide sh. Set ALIASC_BINARY to an existing compiler to skip fetching
# entirely - that is the supported setup when no sh is available.
#
# There is no PowerShell fallback parser: the POSIX fallback works by eval-ing
# POSIX function bodies, which PowerShell cannot consume.
#
# Install from $PROFILE:
#   . ~/.config/powershell/aliasc.ps1

if ($env:ALIASDSL_NOLOAD -eq '1') { return }

$libDir = if ($env:ALIASC_LIB_DIR) { $env:ALIASC_LIB_DIR }
          elseif ($env:XDG_DATA_HOME) { Join-Path $env:XDG_DATA_HOME 'aliasc' }
          else { Join-Path $HOME '.local/share/aliasc' }
$fetcher = if ($env:ALIASC_FETCHER) { $env:ALIASC_FETCHER } else { Join-Path $HOME '.local/bin/aliasc-fetch.sh' }
$cacheDir = if ($env:ALIASC_CACHE_DIR) { $env:ALIASC_CACHE_DIR }
            elseif ($env:XDG_CACHE_HOME) { Join-Path $env:XDG_CACHE_HOME 'aliasc' }
            else { Join-Path $HOME '.cache/aliasc' }
$sourceFile = if ($env:ALIAS_FILE) { $env:ALIAS_FILE } else { Join-Path $HOME '.config/alias' }
$shellName = if ($PSVersionTable.PSEdition -eq 'Desktop') { 'powershell' } else { 'pwsh' }

function Write-AliascError([string]$Message) {
    [Console]::Error.WriteLine("aliasc: $Message")
}

if (-not (Test-Path -LiteralPath $sourceFile -PathType Leaf)) {
    Write-AliascError "alias source is missing: $sourceFile"
    return
}

# --- compiler -------------------------------------------------------------

$binary = $env:ALIASC_BINARY
if (-not $binary -or -not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    $binary = $null
    $sh = Get-Command sh -CommandType Application -ErrorAction SilentlyContinue
    if (-not $sh) {
        Write-AliascError "no 'sh' for $fetcher; set ALIASC_BINARY to an existing aliasc executable"
        return
    }
    $binary = (& $sh.Source $fetcher --print-binary-path 2>$null | Select-Object -First 1)
    if (-not $binary -or -not (Test-Path -LiteralPath $binary -PathType Leaf)) {
        if (-not (Test-Path -LiteralPath $fetcher -PathType Leaf)) {
            Write-AliascError "fetcher not found: $fetcher; set ALIASC_BINARY"
            return
        }
        Write-AliascError 'compiler not cached; fetching'
        & $sh.Source $fetcher > $null 2>&1
        $binary = (& $sh.Source $fetcher --print-binary-path 2>$null | Select-Object -First 1)
    }
}
if (-not $binary -or -not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    Write-AliascError "compiler unavailable; aliases are not loaded"
    return
}

# --- staleness ------------------------------------------------------------

$artifact = Join-Path $cacheDir "alias.$shellName"
$manifestPath = "$artifact.manifest.json"

function Test-ArtifactStale {
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) { return $true }
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { return $true }
    try { $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json } catch { return $true }

    if ($manifest.target.shell -ne $shellName) { return $true }
    if ($manifest.compiler.path -ne $binary) { return $true }
    if ((Test-Path -LiteralPath $binary -PathType Leaf) -and
        (Get-Item -LiteralPath $binary).LastWriteTimeUtc -gt (Get-Item -LiteralPath $artifact).LastWriteTimeUtc) { return $true }

    # mtime_ns is the signal, not `exists`: optional-but-positional inputs are
    # recorded with exists=true and mtime_ns=null, so trusting `exists` would
    # force a recompile on every startup.
    $artifactTime = (Get-Item -LiteralPath $artifact).LastWriteTimeUtc
    foreach ($input in $manifest.inputs) {
        if ($null -eq $input.mtime_ns) {
            if (Test-Path -LiteralPath $input.path) { return $true }
            continue
        }
        if (-not (Test-Path -LiteralPath $input.path -PathType Leaf)) { return $true }
        if ((Get-Item -LiteralPath $input.path).LastWriteTimeUtc -gt $artifactTime) { return $true }
    }
    return $false
}

# --- compile and load -----------------------------------------------------

if (-not (Test-Path -LiteralPath $cacheDir)) { New-Item -ItemType Directory -Path $cacheDir -Force | Out-Null }

if (Test-ArtifactStale) {
    Write-AliascError "compiling $shellName artifact"
    $arguments = @('compile', '--shell', $shellName, '--platform', 'auto', '--distro', 'auto',
                   '--environment', 'auto', '--source', $sourceFile, '--output', $artifact)
    & $binary @arguments > $null 2>&1
    if ($LASTEXITCODE -ne 0) {
        if (Test-Path -LiteralPath $artifact -PathType Leaf) {
            Write-AliascError "compilation failed; reusing existing artifact $artifact"
        } else {
            Write-AliascError 'compilation failed; aliases are not loaded'
            return
        }
    }
}

if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
    Write-AliascError "artifact missing: $artifact"
    return
}

# Drop the previous generation so renamed or deleted definitions do not linger.
$previous = $script:AliascLoadedFunctions
if ($previous) {
    foreach ($name in $previous) {
        Remove-Item -Path "Function:$name" -ErrorAction SilentlyContinue
        Remove-Item -Path "Alias:$name" -ErrorAction SilentlyContinue
    }
}
$script:AliascLoadedFunctions = @(
    Select-String -LiteralPath $artifact -Pattern '^function\s+([A-Za-z_][A-Za-z0-9_-]*)' |
        ForEach-Object { $_.Matches[0].Groups[1].Value }
)

. $artifact
