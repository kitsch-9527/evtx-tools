
set path="%~dp0usr\bin";%path%;

%~d0
cd "%~dp0"

set _RANDOM=%RANDOM%

7z a -ssw -mtm -mta -mtc -spf -t7z archive.7z @%1

7z rn archive.7z C: C
7z rn archive.7z D: D
7z rn archive.7z E: E
7z rn archive.7z F: F
7z rn archive.7z G: G

 