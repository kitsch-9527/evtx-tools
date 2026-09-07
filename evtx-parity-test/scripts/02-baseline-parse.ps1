# 02-baseline-parse.ps1 — Q7 方案 A：中转跑原 evtx2text.lua → baseline-parsed/（原脚本不动）
# 事实：脚本输入写死 msys64-Tools\evtx\（目录批量、无参数），输出 *.evtx.txt 到其当前目录，
# 依赖 lfs（已验证可用）+ wevtutil/tr/sed/perl/iconv（已验证在 usr\bin），结尾有 PAUSE。
$ErrorActionPreference = 'Stop'
$testRoot = Split-Path -Parent $PSScriptRoot
$repoRoot = Split-Path -Parent $testRoot
$msysDir  = Join-Path $repoRoot 'msys64-Tools'
$stageDir = Join-Path $msysDir 'evtx'
$luaExe   = Join-Path $msysDir 'usr\bin\lua.exe'
$srcDir   = Join-Path $testRoot 'baseline-evtx'
$dstDir   = Join-Path $testRoot 'baseline-parsed'
New-Item -ItemType Directory -Force -Path $dstDir | Out-Null

$srcFiles = Get-ChildItem $srcDir -Filter *.evtx -File -ErrorAction SilentlyContinue
if (-not $srcFiles -or $srcFiles.Count -eq 0) { throw "baseline-evtx/ 为空：请先跑 scripts\\01-export.ps1（管理员）。" }
if (-not (Test-Path $luaExe)) { throw "找不到 lua: $luaExe" }
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null

# 清理上次 staging 与上次输出（只动四个通道同名文件，不动脚本）
Get-ChildItem $stageDir -Filter *.evtx -File -ErrorAction SilentlyContinue | Remove-Item -Force
foreach ($f in $srcFiles) {
  $old = Join-Path $msysDir ($f.Name + '.txt')
  if (Test-Path $old) { Remove-Item -Force $old }
}
Copy-Item (Join-Path $srcDir '*.evtx') $stageDir -Force
Write-Host ("[02] 已中转 {0} 个文件到 msys64-Tools/evtx/" -f $srcFiles.Count)

# msys 工具链（tr/sed/perl/iconv）在 cmd 内前置到 PATH；工作目录必须是 msys64-Tools（相对路径 evtx\ 生效）。
# 注意：必须用 System32\cmd.exe 全路径——裸 `cmd` 会被 PATH 上某 msys64 的 cmd bash 垫片遮蔽，
# 导致 lua 根本没跑起来（2026-09-03 实测 0 产出根因）。原脚本不动。
$sysCmd = Join-Path $env:SystemRoot 'System32\cmd.exe'
$msysBin = Join-Path $msysDir 'usr\bin'
Push-Location $msysDir
try {
  Write-Host '[02] 运行原脚本：lua evtx2text.lua（stdin=NUL 跳过结尾 PAUSE）'
  & $sysCmd /d /c "set PATH=$msysBin;%PATH% && lua.exe evtx2text.lua < NUL"
} finally { Pop-Location }

$moved = 0
foreach ($f in $srcFiles) {
  $gen = Join-Path $msysDir ($f.Name + '.txt')
  if (Test-Path $gen) {
    Move-Item -Force $gen (Join-Path $dstDir ($f.Name + '.txt'))
    $moved++
  } else {
    Write-Warning ("未生成：{0}（{1} 无输出）" -f $gen, $f.Name)
  }
}
# 清理 staging，还原 msys64-Tools/evtx/ 为空（核实过：此前为空）
Get-ChildItem $stageDir -Filter *.evtx -File -ErrorAction SilentlyContinue | Remove-Item -Force
Write-Host ("[02] 完成：{0}/{1} 个 *.evtx.txt 已搬到 baseline-parsed/" -f $moved, $srcFiles.Count)
if ($moved -ne $srcFiles.Count) { throw '部分文件未生成基线解析 output，请检查上方警告。' }
