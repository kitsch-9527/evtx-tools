# Elevated Security-only export (ASCII only: PS5.1 reads no-BOM ps1 as GBK)
$ErrorActionPreference = 'Stop'
$dest = Join-Path $PSScriptRoot '..\baseline-evtx\Security.evtx'
$log = Join-Path $PSScriptRoot '..\baseline-evtx\01-security-elevated.log'
try {
  & wevtutil.exe epl Security $dest *>&1 | Out-File -FilePath $log -Encoding utf8
  if ($LASTEXITCODE -ne 0) { throw "wevtutil exit=$LASTEXITCODE" }
  exit 0
} catch {
  "FAILED: $_" | Out-File -FilePath $log -Encoding utf8 -Append
  exit 1
}
