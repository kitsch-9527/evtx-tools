# Release 构建
cargo build --release
if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ Release 构建成功" -ForegroundColor Green
} else {
    Write-Host "❌ Release 构建失败" -ForegroundColor Red
    exit 1
}
