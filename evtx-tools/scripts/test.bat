@echo off
REM 运行全量测试
cargo test
if %ERRORLEVEL% equ 0 (
    echo [OK] 全部测试通过
) else (
    echo [FAIL] 存在测试失败
    exit /b 1
)
