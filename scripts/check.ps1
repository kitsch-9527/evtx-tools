# 快速编译检查
Write-Host "=== cargo check ===" -ForegroundColor Cyan
cargo check 2>&1 | ForEach-Object {
    if ($_ -match "error") { Write-Host $_ -ForegroundColor Red }
    elseif ($_ -match "warning") { Write-Host $_ -ForegroundColor Yellow }
    else { Write-Host $_ }
}

if ($LASTEXITCODE -eq 0) {
    Write-Host "`n✅ 编译检查通过" -ForegroundColor Green
} else {
    Write-Host "`n❌ 编译检查失败" -ForegroundColor Red
    exit 1
}
