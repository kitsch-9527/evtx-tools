# 全量测试
Write-Host "=== 运行全量测试 ===" -ForegroundColor Cyan
cargo test 2>&1 | ForEach-Object {
    if ($_ -match "FAILED") { Write-Host $_ -ForegroundColor Red }
    elseif ($_ -match "ok") { Write-Host $_ -ForegroundColor Green }
    elseif ($_ -match "running") { Write-Host $_ -ForegroundColor Yellow }
    else { Write-Host $_ }
}

if ($LASTEXITCODE -eq 0) {
    Write-Host "`n✅ 全部测试通过" -ForegroundColor Green
} else {
    Write-Host "`n❌ 存在测试失败" -ForegroundColor Red
    exit 1
}
