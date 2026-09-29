# Elevated wrapper for 01-export.ps1 (ASCII only: PS5.1 reads no-BOM ps1 as GBK)
$ErrorActionPreference = 'Stop'
$log = Join-Path $PSScriptRoot '..\baseline-evtx\01-export-elevated.log'
try {
  & (Join-Path $PSScriptRoot '01-export.ps1') *>&1 | Out-File -FilePath $log -Encoding utf8
  exit 0
} catch {
  "FAILED: $_" | Out-File -FilePath $log -Encoding utf8 -Append
  exit 1
}
