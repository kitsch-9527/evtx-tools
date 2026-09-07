# 03-build-msgdb.ps1 — go build 重编 msgdb-dump + 本机双语单库 → msgdb/（Q5/Q9/Q4）
# 事实：Go 项目（非 cargo），默认 --lang 1033,2052 一次产出双语单库。
$ErrorActionPreference = 'Stop'
$testRoot = Split-Path -Parent $PSScriptRoot
$repoRoot = Split-Path -Parent $testRoot
$srcDir  = Join-Path $repoRoot 'msgdb-dump'
$toolsDir = Join-Path $testRoot 'tools'
$outDir   = Join-Path $testRoot 'msgdb'
New-Item -ItemType Directory -Force -Path $toolsDir, $outDir | Out-Null
$exe = Join-Path $toolsDir 'msgdb-dump-x64.exe'

Write-Host "[03] go version: $(go version)"
Push-Location $srcDir
try { go build -o $exe . } finally { Pop-Location }
if (-not (Test-Path $exe)) { throw "编译产物缺失：$exe" }

$hash = (Get-FileHash $exe -Algorithm SHA256).Hash
@(
  "built_at_local=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
  "go_version=$(go version)"
  "source_dir=../msgdb-dump"
  "exe=tools/msgdb-dump-x64.exe"
  "sha256=$hash"
  'vcs=none（仓库无 .git，2026-09-03 核实 fatal: not a git repository）'
) | Set-Content -Encoding UTF8 (Join-Path $outDir 'build-info.txt')

Write-Host '[03] 生成单文件双语库：--lang 1033,2052（en-US + zh-CN，schema_version 7，ADR 0003）'
& $exe --out-dir $outDir --lang 1033,2052
if ($LASTEXITCODE -ne 0) { throw "msgdb-dump 运行失败（exit=$LASTEXITCODE）" }
$dbs = @(Get-ChildItem $outDir -Filter hotech-msg-*.db -File)
if ($dbs.Count -ne 1) { throw "单文件契约破坏：msgdb/ 下应恰好 1 个 hotech-msg-*.db，实得 $($dbs.Count) 个" }
$db = $dbs[0]
$db | Format-Table Name, Length, LastWriteTime -AutoSize | Out-String -Width 220 | Write-Host
Write-Host '[03] 完成'
