# 开发循环：check -> test
param(
    [switch]$Release
)

$mode = if ($Release) { "--release" } else { "" }

Write-Host "=== cargo check ===" -ForegroundColor Cyan
cargo check $mode
if ($LASTEXITCODE -ne 0) { Write-Host "❌ check 失败" -ForegroundColor Red; exit 1 }

Write-Host "`n=== cargo test ===" -ForegroundColor Cyan
cargo test $mode
if ($LASTEXITCODE -ne 0) { Write-Host "❌ 测试失败" -ForegroundColor Red; exit 1 }

Write-Host "`n✅ 全部通过" -ForegroundColor Green
