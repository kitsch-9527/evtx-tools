# evtx-parity-test — 基线解析 vs 新解析比对测试

## 目的：测试循环闭合

本项目是 `evtx-tools` / `msgdb-dump` 双链路的**可重复闭环验证台**：每次改动上游（消息库生成、
解析器、替换引擎）后，重跑 `run-all.ps1` 即可得到同一口径的 `report.md`（人类结论）+
`report.json`（机器可读汇总，下轮迭代的对比基线）+ `report.diff`（差异明细）。

闭环语义：`report.json.verdict` 为 `PASS` 即闭合；为 `DIFF` 则按 `by_channel.*.categories`
定位回归分类（A 豁免 / B 裸参数 / C 无模板 / D 实质分歧），修上游后再跑下一轮。

本机四通道事件查看器 → 两条解析路径 → 归一化比对 → 对比报告。

## 目录（英文为磁盘正名，中文为用词语别名，见 CONTEXT.md）

```
evtx-parity-test/
├── baseline-evtx/      # 基线文件夹：wevtutil epl 导出的 4 个 .evtx
├── baseline-parsed/    # 基线解析：evtx2text.lua 输出的 *.evtx.txt（中文文本）
├── msgdb/              # msgdb-dump 本机生成的双语单库（1033+2052）
├── new-parsed/en|zh/   # 新解析：evtx-tools --lang en-US / zh-CN 输出（.db + .csv）
├── tools/              # 本项目内重编的 exe（msgdb-dump-x64.exe、evtx-tools.exe）
├── scripts/            # 分步脚本 01..06
├── report.md           # 对比报告（Q6 口径）
├── report.diff         # 原始差异明细
└── run-all.ps1         # 总控：一键顺序执行
```

## 前置条件

- 管理员 PowerShell（步骤 01 导出 Security 通道需要；已核实当前终端是管理员）
- Go 1.20+（步骤 03；已核实 go1.20）
- cargo / rustc（步骤 04；已核实 1.94.1）
- Python 3.12+（步骤 06，仅标准库；已核实 3.12.11；本机暂无 uv，`pyproject.toml` 已预留迁移位）

## 执行顺序

```powershell
# 单步（可单独重跑）
scripts\01-export.ps1            # 管理员：wevtutil epl 四通道 → baseline-evtx/
scripts\02-baseline-parse.ps1    # 中转跑原 evtx2text.lua → baseline-parsed/
scripts\03-build-msgdb.ps1       # go build + 本机双语库 → msgdb/
scripts\04-build-evtx-tools.ps1  # cargo build --release → tools/
scripts\05-new-parse.ps1         # en-US / zh-CN 各跑一遍 → new-parsed/
python scripts\06-compare.py     # 顺序对齐比对 → report.md + report.diff

# 一键全链路
.\run-all.ps1
```

## 已知事项（写脚本前核实的事实）

1. `evtx2text.lua` 无参数、输入写死 `msys64-Tools\evtx\`、GBK 编码、依赖 `lfs`（已验证可用）+
   `wevtutil/tr/sed/perl/iconv`（已验证四者皆在 `usr\bin`）；结尾有 `PAUSE`，02 脚本以 `NUL` 作 stdin
   重定向避免挂起，原脚本不动。
2. `wevtutil /f:text` 的 `Event[N]` 是**查询序号**（从 0 起），不是 `event_record_id`，故两边按**出现顺序**
   对齐（基线第 N 个块 ↔ 新解析 `sequence_number` 第 N 条），见 Q10 修正。
3. 新解析 `event_time` 存的是**本地时间** `YYYY-MM-DD HH:MM:SS.ffffff`；基线 `Date` 是 UTC。比对时基线
   时间转本地后再比，容差 2 秒。
4. `evtx-tools --format both` 与 `-o <单路径>` 同写一个文件（db 与 csv 抢同一路径），疑似损坏输出，
   故 05 脚本按 `--format db` / `--format csv` **分两次跑**（与 both 意图等价，各落自家文件）。
5. 本机 `msgdb-dump` 默认 `--lang 1033,2052` 即一次产出**双语单库**；`evtx-tools` 以 `--lang` 切换语言。
6. 仓库无 `.git`（`fatal: not a git repository`），版本信息记入 `msgdb/build-info.txt` 与
   `tools/build-info.txt`，不依赖 VCS。
