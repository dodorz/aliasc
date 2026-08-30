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
  [switch]$NoLocal,
  [string]$ShortcutMap,
  [switch]$NoShortcutMap,
  [switch]$Force
)

$status = Join-Path $PSScriptRoot 'aliasc-status.ps1'
$statusArgs = @{ Shell=$Shell; Source=$Source; Output=$Output; Platform=$Platform; Distro=$Distro; Environment=$Environment; Aliasc=$Aliasc; Quiet=$true }
& $status @statusArgs
if ($Force -or $LASTEXITCODE -ne 0) {
  $arguments = @('compile', '--shell', $Shell, '--platform', $Platform, '--distro', $Distro, '--environment', $Environment, '--source', $Source, '--output', $Output)
  if ($NoLocal) { $arguments += '--no-local' }
  if ($NoShortcutMap) { $arguments += '--no-shortcut-map' } elseif ($ShortcutMap) { $arguments += @('--shortcut-map', $ShortcutMap) }
  & $Aliasc @arguments
  exit $LASTEXITCODE
}
