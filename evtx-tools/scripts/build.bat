@echo off
REM Release 构建
cargo build --release
if %ERRORLEVEL% equ 0 (
    echo [OK] Release 构建成功
) else (
    echo [FAIL] Release 构建失败
    exit /b 1
)
