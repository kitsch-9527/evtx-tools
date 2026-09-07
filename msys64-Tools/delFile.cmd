@echo off
setlocal enabledelayedexpansion

set file_list_del=%1
:: 检查 file_list_del.txt 是否存在
if not exist "%file_list_del%" (
    echo 错误: 找不到文件 %file_list_del%
    pause
    exit /b 1
)

echo 开始处理文件列表...
echo ================================

set count=0
set success=0
set failed=0

:: 循环读取 file_list_del.txt 的每一行
for /f "usebackq delims=" %%i in ("%file_list_del%") do (
    set "filepath=%%i"
    
    :: 跳过空行
    if not "!filepath!"=="" (
        set /a count+=1
        echo [!count!] 正在处理: !filepath!
        
        :: 检查文件是否存在
        if exist "!filepath!" (
            takeown /f "!filepath!" /a
            icacls "!filepath!" /reset /T /C /Q
            del /s /f /q "!filepath!" 2>nul
            if errorlevel 0 (
                ::echo      -> 删除成功
                set /a success+=1
            ) else (
                ::echo      -> 删除失败
                set /a failed+=1
            )
        ) else (
            ::echo      -> 文件不存在，跳过
            set /a failed+=1
        )
    )
)

echo ================================
echo 处理完成！
echo 总计: !count! 个文件
echo 成功: !success! 个
echo 失败: !failed! 个

pause
