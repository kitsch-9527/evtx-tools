# Compare hotech-evtx message DB with native PowerShell Get-WinEvent
$db = "E:\Code\rust\hotech-analyzer-gui\crates\hotech-evtx\target\mui_test\hotech-msg-Windows_10_Education-25H2-26220.7052.db"

Write-Host "=== Step 1: Get latest Kernel-Power events from System log ===" -ForegroundColor Cyan
$events = Get-WinEvent -LogName System -ProviderName "Microsoft-Windows-Kernel-Power" -MaxEvents 5
$eid_list = @()
foreach ($e in $events) {
    $eid = $e.Id
    $eid_list += $eid
    $msg = $e.Message -replace "`n"," " -replace "`r",""
    if ($msg.Length -gt 120) { $msg = $msg.Substring(0, 120) + "..." }
    Write-Host "`nPowerShell Event $eid :" -ForegroundColor Yellow
    Write-Host "  $msg"
    Write-Host "  Time: $($e.TimeCreated)"
}

Write-Host "`n=== Step 2: Our message DB for same event IDs ===" -ForegroundColor Cyan
$unique_ids = $eid_list | Select-Object -Unique
foreach ($eid in $unique_ids) {
    $row = sqlite3.exe $db "SELECT message, task, opcode, parameters FROM messages WHERE event_id = $eid" 2>$null
    if ($LASTEXITCODE -eq 0 -and $row) {
        Write-Host "`nOur DB Event $eid :" -ForegroundColor Green
        $msg = $row[0]
        $task = $row[1]
        $opcode = $row[2]
        $params = $row[3]
        if ($msg.Length -gt 120) { $msg = $msg.Substring(0, 120) + "..." }
        Write-Host "  message: $msg"
        Write-Host "  task: $task"
        Write-Host "  opcode: $opcode"
        Write-Host "  parameters: $params"
    } else {
        Write-Host "Event $eid not found in our DB" -ForegroundColor Red
    }
}

Write-Host "`n=== Step 3: Full DB listing ===" -ForegroundColor Cyan
$rows = sqlite3.exe $db "SELECT event_id, task, opcode, substr(parameters,1,40), substr(message,1,80) FROM messages ORDER BY event_id" 2>$null
if ($LASTEXITCODE -eq 0) {
    foreach ($r in $rows) {
        Write-Host "  Event $r"
    }
}
