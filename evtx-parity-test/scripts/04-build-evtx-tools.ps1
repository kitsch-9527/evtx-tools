# 04-build-evtx-tools.ps1 — cargo build --release 重编 evtx-tools → tools/（Q5）
$ErrorActionPreference = 'Stop'
$testRoot = Split-Path -Parent $PSScriptRoot
$repoRoot = Split-Path -Parent $testRoot
$srcDir  = Join-Path $repoRoot 'evtx-tools'
$toolsDir = Join-Path $testRoot 'tools'
New-Item -ItemType Directory -Force -Path $toolsDir | Out-Null

Write-Host "[04] $(cargo --version) / $(rustc --version)"
Push-Location $srcDir
try { cargo build --release } finally { Pop-Location }
$built = Join-Path $srcDir 'target\release\evtx-tools.exe'
if (-not (Test-Path $built)) { throw "编译产物缺失：$built" }
Copy-Item -Force $built (Join-Path $toolsDir 'evtx-tools.exe')
$batchBuilt = Join-Path $srcDir 'target\release\evtx-batch.exe'
if (Test-Path $batchBuilt) { Copy-Item -Force $batchBuilt (Join-Path $toolsDir 'evtx-batch.exe') }
$hash = (Get-FileHash (Join-Path $toolsDir 'evtx-tools.exe') -Algorithm SHA256).Hash
@(
  "built_at_local=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
  "cargo_version=$(cargo --version)"
  "rustc_version=$(rustc --version)"
  "source_dir=../evtx-tools"
  'profile=release'
  "sha256=$hash"
  'vcs=none（仓库无 .git）'
) | Set-Content -Encoding UTF8 (Join-Path $toolsDir 'build-info.txt')
Write-Host '[04] 完成：tools/evtx-tools.exe'
