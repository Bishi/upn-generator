$ErrorActionPreference = "Stop"

$dbPath = Join-Path $env:APPDATA "si.upn-generator\upn-generator.db"
$env:UPN_GENERATOR_DB_PATH = $dbPath

Write-Host "Using UPN Generator DB: $dbPath"
npm.cmd run tauri dev
