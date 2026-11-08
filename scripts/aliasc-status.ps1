#!/usr/bin/env pwsh
[CmdletBinding()]
param(
  [Parameter(Mandatory)][ValidateSet('posix','bash','zsh','fish','nu','powershell','pwsh','cmd')][string]$Shell,
  [Parameter(Mandatory)][string]$Source,
  [Parameter(Mandatory)][string]$Output,
  [string]$Platform = 'auto',
  [string]$Distro = 'auto',
  [string]$Environment = 'auto',
  [string]$Aliasc = 'aliasc',
  [switch]$Quiet
)

$manifestPath = "$Output.manifest.json"
$stale = $false
$reason = @()
if (-not (Test-Path -LiteralPath $Output -PathType Leaf)) { $stale = $true; $reason += 'output missing' }
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { $stale = $true; $reason += 'manifest missing' }
if (-not $stale) {
  try { $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json } catch { $stale = $true; $reason += 'manifest invalid' }
}
if (-not $stale) {
  $expectedPlatform = if ($Platform -eq 'auto') { if ($IsWindows -or $env:OS -eq 'Windows_NT') { 'windows' } elseif ($IsMacOS) { 'macos' } else { 'linux' } } else { $Platform }
  if ($manifest.target.shell -ne $Shell -or $manifest.target.platform -ne $expectedPlatform) { $stale = $true; $reason += 'target context differs' }
  $compiler = Get-Command $Aliasc -CommandType Application -ErrorAction SilentlyContinue
  if (-not $compiler) { $stale = $true; $reason += 'compiler unavailable' }
  elseif ((Get-Item -LiteralPath $compiler.Source).LastWriteTimeUtc -gt (Get-Item -LiteralPath $Output).LastWriteTimeUtc) { $stale = $true; $reason += 'compiler newer' }
  foreach ($input in $manifest.inputs) {
    # mtime_ns is the signal, not `exists`. The compiler records exists=true with
    # mtime_ns=null for optional-but-positional inputs (a configured ShortcutMap
    # or include that is absent), so comparing `exists` marks such a manifest
    # stale forever.
    if ($null -eq $input.mtime_ns) {
      if (Test-Path -LiteralPath $input.path) { $stale = $true; $reason += "input appeared: $($input.path)" }
      continue
    }
    if (-not (Test-Path -LiteralPath $input.path -PathType Leaf)) { $stale = $true; $reason += "input missing: $($input.path)"; continue }
    $item = Get-Item -LiteralPath $input.path
    if ($item.Length -ne [int64]$input.size -or (($item.LastWriteTimeUtc.Ticks - 621355968000000000) * 100) -ne [int64]$input.mtime_ns) { $stale = $true; $reason += "input changed: $($input.path)" }
  }
  foreach ($generated in $manifest.outputs) { if (-not (Test-Path -LiteralPath $generated.path -PathType Leaf)) { $stale = $true; $reason += "generated output missing: $($generated.path)" } }
}
if ($Quiet) { if ($stale) { exit 1 }; exit 0 }
if ($stale) { "stale: $($reason -join '; ')"; exit 1 }
'fresh'
