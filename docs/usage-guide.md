# evet 使用流程说明（实操手册）

> 面向使用者：怎么跑、怎么读结果、出问题怎么查。
> 术语与架构见 `CONTEXT.md` / `README.md`；修复机理与勿回退事实见 `skills/parity-test-skill.md`。

---

## 0. 快速导航（按目的找流程）

| 我想…… | 去哪节 | 耗时 |
|---|---|---|
| 完整验证一遍（本机真实数据） | §2 全链路 | ~15 分钟 |
| 改了 evtx-tools（渲染/解析）代码，验证改动 | §3.1 迭代循环 | ~6 分钟 |
| 改了 msgdb-dump（萃取）代码，验证改动 | §3.2 迭代循环 | ~9 分钟 |
| 只看当前匹配状态 | §4 结果解读 | 10 秒 |
| 某一步报错/结果异常 | §7 坑位速查 | — |
| 深挖某条差异 | §8 取证探针 | — |

---

## 1. 环境前提（一次性）

| 前提 | 说明 |
|---|---|
| Windows + 管理员能力 | Security 通道导出必须提权（普通会话会"拒绝访问"）；提权用 `scripts/_01-security-elevated.ps1`（会弹 UAC，点"是"） |
| 工具链 | Go 1.2x（msgdb-dump）、Rust 1.75+（evtx-tools）、Python 3.12+（仅标准库） |
| PowerShell 执行策略 | 脚本统一用 `-ExecutionPolicy Bypass` 运行，无需改全局设置 |
| 本机 UI 语言 | 基线渲染语言跟随本机显示语言（当前 zh-CN）。**改动显示语言后必须重跑 01/02**（G0 门会拦截不一致） |
| 磁盘 | 管线数据约 2.3 GB（msgdb + 导出 + 解析产物，均已 gitignore） |

一次性初始化（首次或换机器）：

```powershell
cd E:\Code\evet\msgdb-dump ;  go build ./...   # 或依赖管线 03 自动构建
cd E:\Code\evet\evtx-tools ;  cargo build --release
```

---

## 2. 完整链路（01→06）

所有命令在 `E:\Code\evet\evtx-parity-test\` 下执行。

### 01 — 导出本机四通道真实 EVTX

```bash
# 先删旧文件（wevtutil epl 不覆盖已存在文件，不删会报"文件存在"）
rm -f baseline-evtx/Application.evtx baseline-evtx/Setup.evtx baseline-evtx/System.evtx baseline-evtx/Security.evtx
# 三通道普通权限即可
for ch in Application Setup System; do wevtutil.exe epl $ch baseline-evtx/$ch.evtx; done
# Security 必须提权（弹 UAC）
powershell -NoProfile -Command "$p = Start-Process powershell -Verb RunAs -Wait -PassThru -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','E:\Code\evet\evtx-parity-test\scripts\_01-security-elevated.ps1'; Write-Host ('exit=' + $p.ExitCode)"
```

**验证点**：四个 `.evtx` 的 mtime 都是刚刚（`ls -la baseline-evtx/`）。

### 02 — Lua 基线解析（路径 A：真值）

```powershell
powershell -ExecutionPolicy Bypass -File scripts/02-baseline-parse.ps1
```

做什么：把 4 个 evtx 中转到 `msys64-Tools/evtx/`，跑原 `evtx2text.lua`（内部走 wevtutil/wevtapi 渲染），产物 `*.evtx.txt` 搬到 `baseline-parsed/`。
**验证点**：日志末尾 `4/4 个 *.evtx.txt 已搬`。耗时 ~5 分钟。

### 03 — 重建消息库（msgdb-dump，Go）

```powershell
powershell -ExecutionPolicy Bypass -File scripts/03-build-msgdb.ps1
```

做什么：扫注册表全部 ETW provider → 解析 PE/CRIM/MUI/参数文件 → 产出单文件 SQLite 消息库 `msgdb/hotech-msg-<系统>-<build>.db`（schema v9，约 350 MB）。脚本自动删旧库。
**验证点**：`[done] dump complete`；行数参考：paramrefs ~39 万 / value maps ~35 万 / sources 1273。耗时 ~3 分钟。

### 04 — 重建渲染器（evtx-tools，Rust）

```powershell
powershell -ExecutionPolicy Bypass -File scripts/04-build-evtx-tools.ps1
```

**验证点（必做）**：`ls -la tools/evtx-tools.exe` mtime 已更新。cargo 无需重编时脚本仍报"完成"，但 tools/ 下的 exe 可能没被替换——**改动过 Rust 代码后这一步必须人工核对 mtime**，否则 05 跑的是旧渲染器（"修复无效"假象的第一大根因）。也可 `md5sum` 对比 `evtx-tools/target/release/evtx-tools.exe` 与 `tools/evtx-tools.exe`。

### 05 — B 路解析（离线渲染）

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/05-new-parse.ps1
```

做什么：用 evtx-tools + 消息库把 4 个 evtx 解析两遍（en-US 与 zh-CN 各一遍），产物 `new-parsed/{en,zh}/*.{csv,db}`。耗时 ~4-5 分钟。

### 06 — 比对闭环

```bash
python scripts/06-compare.py
```

做什么：Lua 基线（真值）vs `new-parsed/` 同语言侧逐条对齐比对（主语言由 G0 从基线文本自动探测，当前 zh-CN；另一语言只计覆盖率）。产出 `report.json`（机器可读）/ `report.md`（结论）/ `report.diff`（逐条明细）。
**结果**：末行打印 `PASS`（diff=0）/ `DONE-WITH-DIFF`（有差异，exit=1）/ `BLOCKED`。

一键等价：`powershell -ExecutionPolicy Bypass -File run-all.ps1`（含 01/02，需要管理员会话跑全程）。

---

## 3. 增量迭代（改了什么 → 重跑到哪）

改动后**只重跑受影响的步骤**，比对永远最后跑：

| 改动位置 | 重跑 | 耗时 |
|---|---|---|
| 无代码改动，只想刷新真实数据 | 01 → 02 → 06* | ~10 分钟 |
| 只改 evtx-tools（渲染/解析/查询） | 04 → 05 → 06 | ~6 分钟 |
| 只改 msgdb-dump（萃取/schema） | 03 → 05 → 06 | ~9 分钟 |
| 两边都改 | 03 → 04 → 05 → 06 | ~10 分钟 |
| 只改 06 比对口径 | 06 | ~1 分钟 |

\* 纯刷新数据时 03/04 可跳过（代码没变）；但**机器打过补丁/装过驱动后建议连 03 一起重跑**（注册表与 DLL 会漂移）。

**TDD 纪律**：任何修复先在单测层加失败测试（`msgdb-dump: go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1`；`evtx-tools: cargo test --workspace`），改到全绿后再进闭环。

---

## 4. 结果解读（report.json / report.md）

关键字段：

```jsonc
{
  "verdict": "PASS",            // PASS=diff 0；DIFF=有差异；BLOCKED=环境问题
  "compared": 45166,            // 对齐比对总条数（基线 vs 新解析，逐条对齐）
  "matched": 45149,             // 五字段（EventID/Provider/Channel/TimeCreated/Message）全一致的条数
  "diff": 0,                    // 差异条数（默认口径，已扣除豁免命中）
  "exempted": 17,               // 受控豁免命中（登记在册的已知分歧，ADR 0006）
  "whitelist_diff_hits": 17,    // 白名单命中（关豁免复核用）
  "baseline_lang": "zh-CN",     // G0 探测的基线渲染语言
  "by_channel": { ... }         // 分通道 compared/matched/diff/categories
}
```

- **M1** = matched/compared（主线匹配率；G1 门下限 97.53%）。
- **M2** = D 闭环率（硬门 80%）。
- **M5** = diff − 豁免命中（有效分歧）。
- 差异分类：**A** 渲染格式一致（豁免）/ **B** 裸参数未渲染 / **C** 无模板 fallback / **D** 实质分歧。B/C 全场为 0 是当前红线基线。

`report.md` 是人类可读汇总；`report.diff` 每条差异含两侧全文（每条截断 500 字，深挖用 importlib 复用 06 的解析函数，勿抽样下结论）。

---

## 5. 门禁（acceptance-check.py）

```bash
python scripts/acceptance-check.py --prev report.prev.json ; echo $?
python scripts/06-compare.py --no-exemptions   # G6 关豁免复核（跑完重跑默认口径恢复 report.json）
```

| 门 | 含义 |
|---|---|
| G0 | 基线渲染语言已知且与 prev 一致（语言翻转拦截器） |
| G1 | M1 ≥ 97.53%；M2 ≥ 80% |
| G2 | 红线：Security diff=0、Setup diff=0、四通道 B/C 不新增、无缺失类 |
| G3 | 豁免命中与登记表一致（EXEMPTIONS 表） |
| G6 | 关豁免后 diff == 默认 diff + 白名单条数（防豁免掩盖回归） |
| 确定性门 | 重跑 05→06，两次 report.json 数字逐通道全等（防迭代序不确定） |

`report.prev.json` 是上一轮快照；**每轮 PASS 后 `cp report.json report.prev.json`** 作为新基线。

---

## 6. 单元测试（改代码必跑，秒级）

```bash
cd msgdb-dump && go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1   # Go 137 用例
cd evtx-tools && cargo test --workspace                            # Rust 246 用例
```

覆盖：schema 契约（版本硬校验/缺表即错）、渲染规则（typed 渲染、N/A parity、%%N 语言层、值映射双层）、取值层（UserData 展平、Binary 参数、GUID-span 保护）。

---

## 7. 坑位速查（症状 → 原因 → 处置）

| 症状 | 原因 | 处置 |
|---|---|---|
| 01 报"文件存在" | `wevtutil epl` 不覆盖已存在文件 | 删目标文件再导 |
| 01 Security 报"拒绝访问" | 需要管理员 | 走 `_01-security-elevated.ps1`（UAC） |
| 02 产出 0 文件 | 裸 `cmd` 被 msys 垫片遮蔽 | 02 脚本已用 System32 全路径，勿改回 |
| 06 基线语言与预期不符 / G0 FAIL | 本机显示语言变了 | 重跑 01/02（真值语言跟机器走） |
| 修复后 diff 纹丝不动 | 04 没真正重编 / tools exe 未更新 | 核对 exe mtime 或 md5；重跑 04 |
| 输出出现 `;;` 连接的裸参数（如 6009 `10.00.;;26220;;…`） | 渲染器没找到消息模板：跑了**旧版/孤儿二进制**（如 evtx-batch.exe 09-03 化石），或未带 `--msg-db` | 只用 `tools/evtx-tools.exe`（核对 mtime）+ `--msg-db`；孤儿 exe 已隔离为 `*.stale-*` |
| `.db` 输出里同一 event_record_id 出现多次/旧渲染并存 | R35 前的 SQLite 写入是追加语义（跨运行累积化石行；CSV 覆盖写所以不受影响，06 只读 CSV 故门禁不可见） | R35 已修（快照语义）；旧污染 .db 重跑 05 即净 |
| 05 全量挂死（进程 CPU≈0） | resolved 闭包内二次 `db.lock()`（Mutex 不可重入） | 已修（R26）；新增闭包内代码严禁再 `db.lock()` |
| 06 出现"缺失"类 diff | 两侧事件数不对齐 | 查 02/05 是否完整跑完（4 个文件都有产物） |
| `--no-exemptions` diff ≠ 默认 diff + 白名单条数 | 豁免表两处不同步 | EXEMPTIONS 在 06 与 acceptance-check 两处，必须同一提交同步 |
| Go 全仓构建失败 | 历史坏探针包 | 只测指定包：`go test ./dump/ ./schema/ ./crim/ ./pe/` |
| 控制台/重定向中文乱码 | wevtutil 重定向到文件是 GBK | python 解码先试 GBK |
| 查导出的 .evtx 文件报"通道路径无效" | 缺 `/lf:true` 或相对路径 | `wevtutil qe <绝对路径> /lf:true '/q:...'` |
| 提权 .ps1 静默不执行 | 无 BOM UTF-8 中文注释被 PS5.1 按 GBK 读吞换行 | 提权运行的 .ps1 保持纯 ASCII |

---

## 8. 取证探针（改代码前先实证）

```bash
# raw XML（本机通道）
MSYS_NO_PATHCONV=1 wevtutil qe System '/q:*[System[Provider[@Name="X"] and (EventID=1)]]' /c:2 /f:xml
# raw XML（导出文件——必须绝对路径 + /lf:true）
MSYS_NO_PATHCONV=1 wevtutil qe "E:\Code\evet\evtx-parity-test\baseline-evtx\System.evtx" /lf:true '/q:*[System[(EventID=4624)]]' /c:2 /f:xml
# 消息库 SQL（python+sqlite3；DB=glob evtx-parity-test/msgdb/*.db）
# CRIM InType/OutType 探针
msgdb-dump/crim-probe-intype.exe <dll> <guid|-> <eid>
# 注意 DUMP_STRINGS 只扫 RT_STRING，不能作为 RT_MESSAGETABLE(type 11) 不存在的证据
```

深差异全量比对：importlib 加载 `scripts/06-compare.py`，复用 `parse_baseline`/`load_new_csv`/`cmp_pair`/`canon_msg`。

---

## 9. 提交纪律

- 修复必须 TDD（先失败测试 → 实现 → 全绿）再闭环（改哪层跑到哪步）。
- 红线一票否决：Security/Setup diff=0、四通道 B/C 不新增、无缺失类、G0 语言一致；破线即回退。
- `EXEMPTIONS` 表在 `06-compare.py` 与 `acceptance-check.py` 两处，**同一提交同步**；登记表真源 `docs/residual-zeroing-plan-2026-09-07.md` §4。
- 每轮 PASS 后刷新 `report.prev.json`；提交信息含目标簇前后条数与红线结论。

---

## 10. 相关文档

| 文档 | 内容 |
|---|---|
| `STATUS.md` | 项目当前状态、基线数字、修复史 |
| `CONTEXT.md` | 领域术语（Provider/Variant Identity/Matching Contract…） |
| `skills/parity-test-skill.md`（三镜像） | 勿回退事实 #1–#23（全部实测机理） |
| `docs/residual-zeroing-plan-2026-09-07.md` | 豁免登记表（§4） |
| `docs/phase-reports/` | 各阶段报告（含 2026-09-30-r26-r29-pass.md 终态） |
| `docs/adr/` | 架构决策记录 |
