#!/usr/bin/env pwsh
# alias-install.ps1 - install the aliasc glue from a Windows release bundle.
#
# Copies the compiler and the PowerShell activator into one directory, then
# prints (or appends) the line to add to $PROFILE.
#
# Usage:
#   .\alias-install.ps1 [-Shell pwsh|powershell] [-Dir PATH] [-Profile FILE]
#                       [-NoCompiler] [-Uninstall] [-Force] [-DryRun]
#
# Exit status: 0 on success, 1 on any failure.

[CmdletBinding()]
param(
    [ValidateSet('pwsh', 'powershell')]
    [string]$Shell = $(if ($PSVersionTable.PSEdition -eq 'Desktop') { 'powershell' } else { 'pwsh' }),
    [string]$Dir,
    [string]$Profile,
    [switch]$NoCompiler,
    [switch]$Uninstall,
    [switch]$Force,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

function Fail([string]$Message) {
    [Console]::Error.WriteLine("aliasc: $Message")
    exit 1
}

function Note([string]$Message) {
    Write-Output $Message
}

$bundleDir = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }
if ($PSCommandPath) {
    $resolved = (Resolve-Path -LiteralPath $PSCommandPath -ErrorAction SilentlyContinue).Path
    if ($resolved) { $bundleDir = Split-Path -Parent $resolved }
}

# Every glue file is installed even on Windows: the POSIX ones are tiny and let
# the same directory serve Git for Windows, WSL, or a dual-boot checkout.
$glueFiles = @('alias.sh', 'alias.fish', 'alias.nu', 'alias.ps1', 'alias-core.sh', 'alias-fetch.sh', 'alias-fallback.sh', 'alias-install.sh')

if (-not (Test-Path -LiteralPath (Join-Path $bundleDir 'alias.ps1') -PathType Leaf)) {
    Fail "bundle is incomplete: missing $(Join-Path $bundleDir 'alias.ps1')"
}

if (-not $Dir) {
    $dataHome = if ($env:XDG_DATA_HOME) { $env:XDG_DATA_HOME } else { Join-Path $HOME '.local/share' }
    $Dir = Join-Path $dataHome 'aliasc'
}

$compiler = $null
foreach ($candidate in @('aliasc.exe', 'aliasc')) {
    $path = Join-Path $bundleDir $candidate
    if (Test-Path -LiteralPath $path -PathType Leaf) { $compiler = $path; break }
}
if (-not $NoCompiler -and -not $compiler) {
    Fail 'bundle has no compiler; pass -NoCompiler to install the glue only'
}

$activator = 'alias.ps1'
$marker = "# aliasc ($Shell)"
$line = ". '$Dir/$activator'"

function Remove-AliascBlock([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return }
    $content = Get-Content -LiteralPath $Path
    $kept = New-Object System.Collections.Generic.List[string]
    $skip = 0
    $pendingBlanks = 0
    foreach ($entry in $content) {
        if ($skip -gt 0) { $skip--; continue }
        if ($entry -eq $marker) { $skip = 1; $pendingBlanks = 0; continue }
        if ($entry -match '^\s*$') { $pendingBlanks++; continue }
        for ($i = 0; $i -lt $pendingBlanks; $i++) { $kept.Add('') }
        $pendingBlanks = 0
        $kept.Add($entry)
    }
    Set-Content -LiteralPath $Path -Value $kept -Encoding utf8
}

if ($Uninstall) {
    if ($DryRun) {
        Note "would remove $Dir"
        if ($Profile) { Note "would remove the activation block from $Profile" }
        exit 0
    }
    if (-not (Test-Path -LiteralPath $Dir -PathType Container)) { Fail "nothing installed at $Dir" }
    foreach ($f in $glueFiles) { Remove-Item -LiteralPath (Join-Path $Dir $f) -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath (Join-Path $Dir 'aliasc.exe') -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath (Join-Path $Dir 'aliasc') -ErrorAction SilentlyContinue
    if (-not (Get-ChildItem -LiteralPath $Dir -ErrorAction SilentlyContinue)) { Remove-Item -LiteralPath $Dir -ErrorAction SilentlyContinue }
    Note "removed aliasc glue from $Dir"
    if ($Profile -and (Test-Path -LiteralPath $Profile -PathType Leaf)) {
        Remove-AliascBlock $Profile
        Note "removed the activation line from $Profile"
    }
    exit 0
}

if ((Test-Path -LiteralPath $Dir -PathType Container) -and -not $Force) {
    foreach ($f in $glueFiles) {
        if (Test-Path -LiteralPath (Join-Path $Dir $f)) {
            Fail "$Dir already contains $f; pass -Force to overwrite"
        }
    }
}

if ($DryRun) {
    Note "would install to $Dir:"
    foreach ($f in $glueFiles) { Note "  $f" }
    if ($compiler) { Note "  $(Split-Path -Leaf $compiler)" }
    Note "would activate with: $Dir/$activator"
    if ($Profile) { Note "would append to ${Profile}: $marker / $line" }
    exit 0
}

New-Item -ItemType Directory -Path $Dir -Force | Out-Null
foreach ($f in $glueFiles) {
    $source = Join-Path $bundleDir $f
    if (Test-Path -LiteralPath $source -PathType Leaf) {
        Copy-Item -LiteralPath $source -Destination (Join-Path $Dir $f) -Force
    }
}
if ($compiler) {
    Copy-Item -LiteralPath $compiler -Destination (Join-Path $Dir (Split-Path -Leaf $compiler)) -Force
}
Note "installed the aliasc glue into $Dir ($Shell)"

if (-not $Profile) {
    Write-Output @"

Add this to your `$PROFILE:

  $marker
  $line

Then start a new PowerShell session, or dot-source it now to try it.
"@
    exit 0
}

if ((Test-Path -LiteralPath $Profile -PathType Leaf) -and
    (Select-String -LiteralPath $Profile -SimpleMatch $marker -Quiet)) {
    Note "activation already present in $Profile"
    exit 0
}

New-Item -ItemType Directory -Path (Split-Path -Parent $Profile) -Force | Out-Null
Add-Content -LiteralPath $Profile -Value "`n$marker`n$line"
Note "appended the activation line to $Profile"
