# 01-export.ps1 — 管理员：wevtutil epl 四通道 → baseline-evtx/（Q2/Q3）
# 通道映射：应用=Application / 补丁安装=Setup / 安全=Security / 系统=System
$ErrorActionPreference = 'Stop'
$testRoot = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $testRoot 'baseline-evtx'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { Write-Warning '非管理员终端：Security 通道导出很可能失败，建议右键“以管理员身份运行”后重跑本脚本。' }

$channels = @(
  @{ Log = 'Application'; File = 'Application.evtx' },
  @{ Log = 'Setup';       File = 'Setup.evtx' },
  @{ Log = 'Security';     File = 'Security.evtx' },
  @{ Log = 'System';       File = 'System.evtx' }
)
foreach ($c in $channels) {
  $dest = Join-Path $outDir $c.File
  Write-Host "[01] wevtutil epl $($c.Log) -> $dest"
  wevtutil epl $c.Log $dest
  if ($LASTEXITCODE -ne 0) { throw "导出失败：$($c.Log)（exit=$LASTEXITCODE）" }
  Get-Item $dest | Format-Table Name, Length, LastWriteTime -AutoSize | Out-String -Width 200 | Write-Host
}
Write-Host '[01] 完成：4 个通道已导出到 baseline-evtx/'
