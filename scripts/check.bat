@echo off
REM 快速编译检查
cargo check
if %ERRORLEVEL% equ 0 (
    echo [OK] 编译检查通过
) else (
    echo [FAIL] 编译检查失败
    exit /b 1
)
