$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$msiDir = Join-Path $repoRoot "src-tauri\target\release\bundle\msi"

function Test-TimestampedMsiName {
  param([string] $Name)
  return $Name -match "_\d{8}-\d{6}(?:-\d+)?\.msi$"
}

function Move-MsiWithTimestamp {
  param(
    [Parameter(Mandatory = $true)] [System.IO.FileInfo] $File,
    [Parameter(Mandatory = $true)] [string] $Timestamp
  )

  if (Test-TimestampedMsiName $File.Name) {
    return $File.FullName
  }

  $baseName = [System.IO.Path]::GetFileNameWithoutExtension($File.Name)
  $candidate = Join-Path $File.DirectoryName "$baseName`_$Timestamp.msi"
  $index = 1

  while (Test-Path -LiteralPath $candidate) {
    $candidate = Join-Path $File.DirectoryName "$baseName`_$Timestamp-$index.msi"
    $index += 1
  }

  Move-Item -LiteralPath $File.FullName -Destination $candidate
  return $candidate
}

Set-Location $repoRoot

if (Test-Path -LiteralPath $msiDir) {
  Get-ChildItem -LiteralPath $msiDir -Filter "*.msi" |
    Where-Object { -not (Test-TimestampedMsiName $_.Name) } |
    ForEach-Object {
      $existingTimestamp = $_.LastWriteTime.ToString("yyyyMMdd-HHmmss")
      $preservedPath = Move-MsiWithTimestamp -File $_ -Timestamp $existingTimestamp
      Write-Host "Preserved existing MSI: $preservedPath"
    }
}

& npm.cmd run tauri build
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

if (-not (Test-Path -LiteralPath $msiDir)) {
  throw "MSI output directory was not created: $msiDir"
}

$buildTimestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$newArtifacts = Get-ChildItem -LiteralPath $msiDir -Filter "*.msi" |
  Where-Object { -not (Test-TimestampedMsiName $_.Name) }

if ($newArtifacts.Count -eq 0) {
  throw "No new non-timestamped MSI artifact was found in: $msiDir"
}

$newArtifacts | ForEach-Object {
  $timestampedPath = Move-MsiWithTimestamp -File $_ -Timestamp $buildTimestamp
  Write-Host "Timestamped MSI: $timestampedPath"
}
