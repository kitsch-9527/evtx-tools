# run-all.ps1 — 总控：01→06 顺序执行（Q12 方案 A），任一步失败即停，可单步重跑。
# 步骤 01 需要管理员终端；步骤 06 需 Python 3.12+（仅标准库，无 uv 也可跑）。
$ErrorActionPreference = 'Stop'
$scripts = Join-Path $PSScriptRoot 'scripts'
& (Join-Path $scripts '01-export.ps1')
& (Join-Path $scripts '02-baseline-parse.ps1')
& (Join-Path $scripts '03-build-msgdb.ps1')
& (Join-Path $scripts '04-build-evtx-tools.ps1')
& (Join-Path $scripts '05-new-parse.ps1')
python (Join-Path $scripts '06-compare.py')
Write-Host 'run-all 完成 → 查看 report.md（结论）与 report.diff（明细）。'
