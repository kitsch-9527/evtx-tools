
set path="%~dp0usr\bin";%path%;

%~d0
cd "%~dp0"

::统一换行
dos2unix file_list.txt 
::排序去重
sort file_list.txt | uniq > file_list.tmp.txt 

set _RANDOM=%RANDOM%
cp file_list.txt archive%_RANDOM%.7z.txt

7z a -ssw -mtm -mta -mtc -spf -t7z archive%_RANDOM%.7z @file_list.tmp.txt 

7z rn archive%_RANDOM%.7z C: C
7z rn archive%_RANDOM%.7z D: D
7z rn archive%_RANDOM%.7z E: E
7z rn archive%_RANDOM%.7z F: F
7z rn archive%_RANDOM%.7z G: G
::删除临时文件
del file_list.tmp.txt 

:: -mx=9            压缩等级 9是极限压缩
:: -tzip            指定格式
:: -mtm -mta -mtc   保留时间
:: @file_list.txt   需要打包的文件列表
:: -spf 启用完全限定路径（绝对路径）
:: -ssw 共享文件

pause