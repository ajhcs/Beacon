#!/usr/bin/env pwsh
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$SourceDir,

  [Parameter(Mandatory = $true)]
  [string]$DestinationDir,

  [Parameter(Mandatory = $true)]
  [string]$ManifestPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $SourceDir -PathType Container)) {
  throw "Source directory does not exist: $SourceDir"
}

$sourceRoot = (Resolve-Path -LiteralPath $SourceDir).Path
$destinationRoot = [System.IO.Path]::GetFullPath($DestinationDir)
$manifestFullPath = [System.IO.Path]::GetFullPath($ManifestPath)

New-Item -ItemType Directory -Path $destinationRoot -Force | Out-Null

$manifestDirectory = Split-Path -Path $manifestFullPath -Parent
if ($manifestDirectory) {
  New-Item -ItemType Directory -Path $manifestDirectory -Force | Out-Null
}

$sourceFiles = @(Get-ChildItem -LiteralPath $sourceRoot -Recurse -File | Sort-Object FullName)
if ($sourceFiles.Count -eq 0) {
  throw "No files found to install from source directory: $sourceRoot"
}

$manifestLines = New-Object System.Collections.Generic.List[string]
$manifestLines.Add("# fresnel-fir installer manifest")
$manifestLines.Add("source=$sourceRoot")
$manifestLines.Add("destination=$destinationRoot")
$manifestLines.Add("generated_utc=$((Get-Date).ToUniversalTime().ToString('o'))")
$manifestLines.Add("")
$manifestLines.Add("path|bytes|sha256")

foreach ($sourceFile in $sourceFiles) {
  $relativePath = [System.IO.Path]::GetRelativePath($sourceRoot, $sourceFile.FullName)
  $destinationPath = Join-Path $destinationRoot $relativePath
  $destinationParent = Split-Path -Path $destinationPath -Parent
  if ($destinationParent) {
    New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
  }

  Copy-Item -LiteralPath $sourceFile.FullName -Destination $destinationPath -Force

  $hash = (Get-FileHash -LiteralPath $destinationPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $sizeBytes = (Get-Item -LiteralPath $destinationPath).Length
  $manifestLines.Add("$relativePath|$sizeBytes|$hash")
}

Set-Content -LiteralPath $manifestFullPath -Value $manifestLines
Write-Host "Installed $($sourceFiles.Count) files to $destinationRoot"
Write-Host "Manifest written to $manifestFullPath"
