# evet 完整执行计划与测试循环手册（Runbook）

> 版本：2026-09-08。所有命令、耗时、验证点均为本会话实证值。
> 配套：`STATUS.md`（现状总览）、`docs/residual-zeroing-plan-2026-09-07.md`（批次计划）、
> `skills/parity-test-skill.md`（勿回退事实 #1-#11）、`docs/adr/0001-0006`。

---

## 0. 当前基线（写本手册时）

| 指标 | 值 |
|---|---|
| 比对总量 | 32801（Application 3925 / Setup 222 / Security 26031 / System 2623） |
| matched | 31992（**97.53%**） |
| 总 diff | 809 |
| D 类残留 | 30 条 / 12 簇（详见 §6 工作项清单） |
| 消息库 | schema_version=8，9 张表（v7 七表 + eventlog_sources + param_types） |

---

## 1. 环境与前置条件

| 项 | 要求 | 核实命令 |
|---|---|---|
| PowerShell | 步骤 01 需**管理员**（其余步骤普通终端即可） | `whoami /groups \| findstr S-1-16-12288` |
| Go | 1.20+ | `go version` |
| Rust | cargo/rustc 稳定版 | `cargo --version` |
| Python | 3.12+（仅标准库） | `python --version` |
| jj | 0.40+（提交用） | `jj --version` |
| Git Bash 注意 | heredoc 内写 Go 代码时 `\\n` 会被吃掉——改用 Edit 工具直接编辑 | — |

工作目录约定：`E:\Code\evet` 为仓库根；下文相对路径均以此为基准。

---

## 2. 层次一：代码级单元测试循环（改代码后必跑，秒级）

### 2.1 Go 侧（msgdb-dump）

```bash
cd E:/Code/evet/msgdb-dump
go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1
```

- 预期：四包全部 `ok`（当前 192+ 用例）。
- **已知陷阱**：`go build ./...` / `go test ./...` 会因历史坏探针
  `crim-probe-566/main.go` 报语法错误——只测指定包即可，管线不受影响。
- 覆盖：v8 schema 契约、TTBL 解析（InType/OutType）、0xDn 字符串表家族、
  param_value_maps 写入、模板参数链接采集。

### 2.2 Rust 侧（evtx-tools）

```bash
cd E:/Code/evet/evtx-tools
cargo test --workspace
```

- 预期：`core 193 + cli 28 + 集成 3` 全部 ok。
- 覆盖：类型化渲染规则表、0xDn 家族、保护名单（decimal 不被 hex 化）、
  N/A parity、模板级参数名校验、v8 打开契约（旧 v7 拒开）。
- 疑难事件用 live-DB 诊断测试（`#[ignore]`）：

```bash
cargo test -p evtx-tools-cli --bin evtx-tools debug_kga25 -- --ignored --nocapture
```

### 2.3 规则

- **任何渲染修复先加失败测试再改实现**（仓库纪律，全部既有修复均如此）。
- 单测绿 ≠ 闭环：必须进入 §3 全链路。

---

## 3. 层次二：全链路管线（`evtx-parity-test/`，分钟级）

### 3.0 前置一次性（基线不变则永不重跑）

```powershell
# 管理员 PowerShell
powershell -ExecutionPolicy Bypass -File scripts/01-export.ps1    # wevtutil epl 四通道 → baseline-evtx/（~1min）
powershell -ExecutionPolicy Bypass -File scripts/02-baseline-parse.ps1  # evtx2text.lua → baseline-parsed/（~2min）
```

### 3.1 每轮六步（改哪个上游就重跑到哪步，06 永远最后）

| 步 | 命令（evtx-parity-test/ 下） | 耗时 | 产物 | 重跑条件 |
|---|---|---|---|---|
| 03 | `powershell -ExecutionPolicy Bypass -File scripts/03-build-msgdb.ps1` | ~3min | `msgdb/hotech-msg-*.db`（~305MB） | 改 msgdb-dump/** |
| 04 | `powershell -ExecutionPolicy Bypass -File scripts/04-build-evtx-tools.ps1` | ~20s-1min | `tools/evtx-tools.exe` | 改 evtx-tools/** |
| 05 | `powershell -ExecutionPolicy Bypass -File scripts/05-new-parse.ps1` | ~4min | `new-parsed/{en,zh}/*.{db,csv}` | 03 或 04 之后 |
| 06 | `python scripts/06-compare.py` | ~1min | `report.{md,json,diff}` | 每轮 |

一键等价：`.\run-all.ps1`（含 01/02）。
**单步直跑渲染器（调试快捷方式，绕过脚本）**：

```bash
cd evtx-parity-test
./tools/evtx-tools.exe parse baseline-evtx/System.evtx -o "$TEMP/out.csv" \
  --format csv --msg-db msgdb/hotech-msg-*.db --lang en-US
```

### 3.2 每步之后的验证点（缺一不可）

**03 之后**——库内容探针（SQL）：

```python
import sqlite3
db = sqlite3.connect('msgdb/hotech-msg-Windows_10_Education-25H2-26220.9223.db')
db.execute("SELECT COUNT(*) FROM value_maps").fetchone()        # 预期 ≈29718
db.execute("SELECT COUNT(*) FROM param_value_maps").fetchone()  # 预期 ≈4219
db.execute("SELECT COUNT(*) FROM param_types").fetchone()       # 预期 ≈71598
# 抽查目标簇的链接与条目（示例）：
db.execute("SELECT param_index, param_name, map_name FROM param_value_maps WHERE provider_name='Microsoft-Windows-Kernel-Boot' AND event_id=153").fetchall()
```

- schema 漂移检查：`param_value_maps`/`param_types` 必须含 `param_name` 列
  （03 现在会先删旧库；若见 `no such column: param_name` = 删除逻辑被绕过/旧库残留）。

**05 之后**——渲染抽查（CSV）：

```python
import csv
# 按簇抽查 message 字段（Time-Service 35 应见十进制 reference id；Ntfs 98 应见 "is healthy."）
```

**06 之后**——三查：
1. `report.json` 的 `verdict` / `diff` / `matched`；
2. `by_channel.*.categories` 与上轮对比（目标簇条数、有无新增分类）；
3. **回归红线**：Security diff=0、Setup diff=0、B/C 不得新增。破线立即 `jj` 回退。

### 3.3 已知坑位（全部实测）

1. 03 重建期间**不要并发跑 05/直跑渲染器**——会读到半写状态的 DB
   （症状：types/链接随机失效）。
2. 03 的控制台输出是 GBK——`grep` 中文/英文关键字可能失配，以 SQL 探针为准。
3. `--format both` 会抢写同一文件——05 按 db/csv 分两次跑（脚本已固化，勿改）。
4. 裸 `cmd` 会被 msys64 垫片遮蔽——02 脚本已用 System32 全路径（脚本已固化）。

---

## 4. 一轮完整迭代 SOP（取证 → 修复 → 闭环）

以"簇 X 条数 N"为输入：

1. **选题**：解析 `report.diff`（`[System #i] D-实质分歧` 分组，每组看首条）；
   `report-exemplars.md` 有分组样本。按条数降序挑簇。
2. **取证**（先实证，不凭记忆）：
   - raw 值形态：`MSYS_NO_PATHCONV=1 wevtutil qe System '/q:*[System[Provider[@Name="X"] and (EventID=N)]]' /c:1 /f:xml`
   - 参数类型/地图：`msgdb-dump/crim-probe-intype.exe <dll> <guid> <eid>`
     （GUID/DLL 从 msgdb providers/dll_entries 查；变体模板 dump 见 ALL_TPL 模式）
   - 字符串表：`DUMP_STRINGS=1 crim-probe-intype.exe <dll-or-mui> x`（注意探针
     不合并 MUI——主 DLL 与 MUI 分别跑）
   - sub-block 原始字节：`DUMP_MAP_OFF=0x<hex>` hexdump
   - 全量模型验证：仿 `scripts/probe-r12-na-model.py`（importlib 复用 06 的解析函数）
3. **归因**：明确是 (a) 萃取缺数据、(b) 渲染规则缺臂、(c) 比对口径、(d) 不可修。
4. **修复**：TDD——先在 §2 对应包加失败测试，再改实现到绿。
5. **闭环**：按 §3 重跑受影响步骤 + 06；对照红线。
6. **沉淀**：skill 基线三镜像加"勿回退"事实；schema/口径变更先写 ADR。

**取证探针速查**（crim-probe-intype，环境变量触发）：

| 模式 | 命令 | 用途 |
|---|---|---|
| 默认 | `crim-probe-intype.exe <dll> <guid> [eid...]` | 参数 InType/OutType + mapRef 解析 + 条目数 |
| 全 variant | `ALL_TPL=1 …` | 该 EID 的所有模板变体（不同 variant 可能模板不同） |
| 字符串表 | `DUMP_STRINGS=1 … <dll-or-mui> x` | 0xDn 字符串表全量（主 DLL 与 MUI 分开跑） |
| 块 hexdump | `DUMP_MAP_OFF=0x<off> …` | VMAP/BMAP 子块原始字节 |

---

## 5. 验收门与指标口径

- **主线**：matched/compared（当前 97.53%）——机器口径，report.json 同源。
- **副线**：D 闭环率（已清 D / 原 D 348）。
- **回归红线（一票否决）**：Security 26031 全绿、Setup 全绿、B/C 不新增。
- **豁免纪律**（ADR 0006）：豁免条目必须有根因码（PARSE/SRC/BIN/RO）+ 关闭条件；
  批次验收时**关闭豁免跑全量**复核一次。
- 本轮目标：主线 ≈97.59%（差 6013×16 一簇）、D 闭环率 86%。

---

## 6. 剩余工作项执行计划（按序执行）

### 6.1 6013×16 — 有界 research（预算：一轮会话）

1. 列候选源：注册表 `HKLM\SYSTEM\CCS\Services\EventLog\System` 下 EventLog 源的
   EventMessageFile；`netevent.dll` MUI 已排除。
2. 反向定位：对 `wevtutil qe System /q:...[EventID=6013]` 的事件做
   `EvtFormatMessage` 实验（PowerShell `Get-WinEvent -FilterHashtable` 渲染对照），
   或枚举系统全部 DLL 的 message table 搜 "system uptime"。
3. 找到 → 常规萃取路径接入（照 EventLog 6005/6009 模式）；找不到 → 转豁免
   （根因码 SRC，关闭条件"定位到源"）。

### 6.2 WPD 24577/24579×4 — 取值层取证

1. raw XML 看 UserData/Data 形态（WPD 是 manifest+UserData 复合）。
2. 对比 extract_subst_params 各策略命中情况（疑似 Named-Data 策略漏）。
3. 修取值层 + 单测；闭环。

### 6.3 Ntfs 140×1 — NTSTATUS 消息化

1. 取 raw：`Failure status` 参数值（0xc000000e）与其 InType/OutType（探针）。
2. 若 out=0x1F（NTSTATUS）：需要 NTSTATUS→消息源（R15 表只盖 Win32 0..16000）。
   候选：ntdll `SysMsg` 表 / 离线枚举。1 条 ROI 低——允许直接转豁免（RO）。

### 6.4 WER 1001×5 — 比对层归一（ADR 0006 首个实施项）

1. 06-compare.py `canon_msg` 或专用归一：`\\?\` ≡ `\\\`（双向，注释注明
   R5 live 取证依据 + 影响面 5 条）。
2. 跑 06 验证 -5；确认无其他簇受影响。

### 6.5 SAM×3 / disk×1 / SPP×1 / CAPI2×1 / Wininit 26×1 — 豁免落地

1. 按登记表逐条核对根因码与关闭条件。
2. 06 白名单实施：键 = 通道+provider+eid+根因码；报告统计豁免命中数；
   提供 `--no-exemptions` 开关。
3. 关闭豁免跑全量复核一次（防豁免掩盖回归）。

### 6.6 定稿

1. STATUS.md / skill 基线三镜像数字更新。
2. `docs/residual-zeroing-plan-2026-09-07.md` 勾销完成项。
3. jj commit（信息含 diff 前后值与红线复核结论）。

---

## 7. 提交规范

- 每个逻辑单元一个 `jj commit -m`（描述含动机/方案/结果数字）。
- 提交前：§2 两层单测绿 + §3 全链路过 + 红线复核。
- 文档三镜像（skills/parity-test-skill.md → .agents/.dsh）必须同步。
