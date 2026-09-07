# 05-new-parse.ps1 — evtx-tools 对基线逐文件 en-US/zh-CN 各跑一遍 → new-parsed/（Q8/Q4，Q6 口径）
# 关键修正：--format both 与 -o <单路径> 会让 db 与 csv 抢写同一文件，故按 db/csv 分两次跑
# （意图 = both：db 留全 fidelity + csv 给比对用），各落自家文件。
$ErrorActionPreference = 'Stop'
$testRoot = Split-Path -Parent $PSScriptRoot
$evtxDir  = Join-Path $testRoot 'baseline-evtx'
$msgdbDir  = Join-Path $testRoot 'msgdb'
$toolsExe  = Join-Path $testRoot 'tools\evtx-tools.exe'
$enDir     = Join-Path $testRoot 'new-parsed\en'
$zhDir     = Join-Path $testRoot 'new-parsed\zh'
New-Item -ItemType Directory -Force -Path $enDir, $zhDir | Out-Null

if (-not (Test-Path $toolsExe)) { throw "找不到 tools/evtx-tools.exe：请先跑 scripts\\04-build-evtx-tools.ps1。" }
$dbs = @(Get-ChildItem $msgdbDir -Filter hotech-msg-*.db -File)
if ($dbs.Count -ne 1) { throw "单文件契约破坏：msgdb/ 下应恰好 1 个 hotech-msg-*.db，实得 $($dbs.Count) 个：请先跑 scripts\\03-build-msgdb.ps1。" }
$msgDb = $dbs[0].FullName
Write-Host "[05] 使用消息库：$($dbs[0].Name)（单文件 v7）"
$files = Get-ChildItem $evtxDir -Filter *.evtx -File
if (-not $files) { throw 'baseline-evtx/ 为空：请先跑 scripts\\01-export.ps1。' }

foreach ($lang in @('en-US', 'zh-CN')) {
  $destDir = if ($lang -eq 'en-US') { $enDir } else { $zhDir }
  foreach ($f in $files) {
    $stem = [IO.Path]::GetFileNameWithoutExtension($f.Name)
    $dbOut  = Join-Path $destDir ($stem + '.db')
    $csvOut = Join-Path $destDir ($stem + '.csv')
    Write-Host "[05] [$lang] $($f.Name) -> db"
    & $toolsExe parse $f.FullName -o $dbOut --format db --msg-db $msgDb --lang $lang
    if ($LASTEXITCODE -ne 0) { throw "解析失败：$($f.Name) [$lang/db]（exit=$LASTEXITCODE）" }
    Write-Host "[05] [$lang] $($f.Name) -> csv"
    & $toolsExe parse $f.FullName -o $csvOut --format csv --msg-db $msgDb --lang $lang
    if ($LASTEXITCODE -ne 0) { throw "解析失败：$($f.Name) [$lang/csv]（exit=$LASTEXITCODE）" }
  }
}
Write-Host '[05] 完成：new-parsed/en + new-parsed/zh（各 .db + .csv，不过滤全量）'
