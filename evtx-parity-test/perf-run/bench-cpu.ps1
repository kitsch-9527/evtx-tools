# bench-cpu.ps1 — run one parse, report wall (from exe's own [done] log) + TotalProcessorTime.
param(
    [string]$Exe = "..\tools\evtx-tools.exe",
    [string]$MsgDb = "msgdb\hotech-msg-Windows_10_Education-25H2-26220.9343.db",
    [string]$EvtxFile = "big-evtx\Security.evtx",
    [string]$Out = "perf-run\cold\ab-bench.db",
    [string]$Tag = "run"
)
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = Start-Process -FilePath $Exe -ArgumentList @(
    "parse", "--format", "db", "--threads", "4", "--defer-indexes",
    "--msg-db", $MsgDb, "-o", $Out, $EvtxFile
) -NoNewWindow -PassThru -RedirectStandardError "$env:TEMP\bench-$Tag.err"
$p.WaitForExit()
$sw.Stop()
Write-Output ("[{0}] wall={1:N1}s cpu={2:N1}s exit={3}" -f $Tag, $sw.Elapsed.TotalSeconds, $p.TotalProcessorTime.TotalSeconds, $p.ExitCode)
Get-Content "$env:TEMP\bench-$Tag.err" | Select-String "\[done\]" | ForEach-Object { $_.Line }
Remove-Item "$env:TEMP\bench-$Tag.err" -ErrorAction SilentlyContinue
