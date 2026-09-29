# 夜间持续执行计划（2026-09-20 23:00 → 2026-09-21 09:00）

> 版本：2026-09-20。上游：`docs/six-hour-execution-plan.md`（A–H 已完成，本档为其续篇）、
> `docs/residual-zeroing-plan-2026-09-07.md`（登记表唯一真源）、
> `docs/acceptance-criteria.md`（指标 M1–M6 / 门禁 G1–G7）、
> `docs/execution-runbook.md`（命令与坑位）、`skills/parity-test-skill.md`（勿回退事实 #1–#16）。
> 执行者：ZCode（编排者会话 + 每阶段一个子代理）。入口提示词见
> `docs/orchestrator-prompt-2026-09-20.md`。
>
> **本档是自包含操作手册**：子代理只读本档对应 §N 全部 + §4/§5/§7 通用节，不读别的计划档。

---

## 0. 起点实测（2026-09-20 现场 recon，非引用旧数字）

**终态锚点（`evtx-parity-test/report.json`，2026-09-15 生成）**：
compared=32801、matched=32020、**diff=771**、**M1=97.6190%**、M2=99.43%、
exempted=6 项/10 条、M5=761、verdict=DIFF。分通道：
Application 3925/3836（B74 C11 D2）、Setup 222/222、Security 26031/26031、System 2623/1931（B644 C40）。

**实测簇分解（本次现场解析 `report.diff`，B718/C51/D2）**：

| 类 | 簇（provider, eid） | 条数 | 基线形态 | 现渲染形态 |
|---|---|---|---|---|
| B | **Ntfs, 50** | **515** | {Delayed Write Failed} …（经典消息表） | 裸参数 + 二进制 hex |
| B | hcmon, 0 | 89 | Detected unrecognized USB driver (\Driver\hrdevmon). | 裸参数 |
| B | WXWorkUpgrader, 0 | 27 | 操作成功完成。 | 裸参数 |
| B | asus, 0 | 16 | （经典消息） | 裸参数 |
| B | Win32k, 267 | 14 | Touch/Touchpad HQA verification succeeded. | 裸 hex |
| B | igfxCUIService2.0.0.0, 0 | 9 | 操作成功完成。 | 裸参数 |
| B | PDFShellExt, 0 | 9 | 操作成功完成。 | 裸参数 |
| B | 其余 17 个 (provider,eid) | 39 | — | 裸参数/hex |
| C | **Microsoft-Windows-Configuration-Change-Monitor, 1** | **40** | **N/A** | 无模板 fallback 长文 |
| C | WMIRegistrationService, 0/2 + IntelDalJhi, 0 | 9 | 服务启动/停止真消息 | 无模板 fallback |
| C | EventSystem, 4625 + Search, 1003 | 2 | 真消息 | 无模板 fallback |
| D | Windows Error Reporting, 1001 | 2 | 真消息 | 十进制串被上游 hex 化 |

**根因 recon（已实证，子代理勿重复第一步）**：

1. **B 类 = 经典 EventLog 源消息表未落库**（residual plan Q1 明确"另立专项"的那一档）。
   注册表实证：
   - `Ntfs`（System 通道）`EventMessageFile = C:\WINDOWS\system32\drivers\ntfs.sys`；
   - `hcmon` `EventMessageFile = C:\WINDOWS\System32\IoLogMsg.dll;C:\WINDOWS\System32\drivers\hcmon.sys`；
   - `Win32k` → `win32kbase.sys`；`NetBT` → `netevent.dll`；
   - `WMIRegistrationService` → `…DriverStore\…\WMIRegistrationService.exe`；`IntelDalJhi` → `…DriverStore\…\jhi_service.exe`。
   - msgdb 实证：`dll_entries` **已含** ntfs.sys(895)/IoLogMsg.dll(290)/win32kbase.sys(906)/netevent.dll(260) 等；
     `providers` 有 `Ntfs(guid=NULL, dll_id=895)`、`hcmon(dll_id=290)`、`Win32k(dll_id=906)`、`NetBT(dll_id=260)`；
     但 **classic 维度的 eid=50 变体 = 0**（`provider_message_variants` 里 Ntfs eid=50 只有
     `Microsoft-Windows-NtfsLog_114163…` 的现代变体 "NtfsConsolidateAllFileRecords…"）。
   - 即：**文件采集到了、经典源的 RT_MESSAGETABLE（低 ID 消息，如 50 → 0x40000032）没有归属到
     guid=NULL 的 classic provider**。三个候选断点（N3 逐一定位）：
     (a) 萃取时 .sys / 无 CRIM 的 RT_MESSAGETABLE 低 ID 未映射到 `event_id`；
     (b) 归属时消息挂到了同 DLL 的现代 GUID provider（Attribution Contamination 的镜像）；
     (c) 渲染时 guid=NULL 的 8 元组查询路径 miss。
2. **C 簇 CfgChangeMonitor**：`eventlog_sources` 里它是 `('Microsoft-Windows-Configuration-Change-Monitor','classic')`，
   但注册表里它其实是 WINEVT publisher `{a148cf02-be6d-5f08-94e3-b68de60d8422}`，**0 条变体**，
   基线渲染 **N/A**。我们因"在 eventlog_sources 表内"而未走 N/A 臂，改走 fallback 长文 → 40 条全错。
   这是 R12 N/A 模型的**精度缺口**（表只记"是否注册"，未记"是否有消息资源"），不是新机制。
3. **D 簇 WER×2**：上游 `format_for_display` 把报告文件名 GUID 尾段十进制串 hex 化
   （`3003916586`→`0xb30c212a`，实证相等；TS35 同域，保护名单有三处调用点，勿只改一处）。
4. **管线现状**：`evtx-parity-test/msgdb/hotech-msg-…-26220.9343.db` 存在（307MB）；
   `tools/evtx-tools.exe` 存在；`report.prev.json` 是 **E 前旧快照(diff=799)**——N0 必须刷新。

---

## 1. 本轮目标与红线

**主目标**：攻下经典源消息表专项（B 类主体），把 M1 从 97.6190% 推高。
**副目标**：C 簇 N/A 精度缺口清零（-40）、WER 上游 hex 保护落地（-2，D 清零 → M2=100%）。

**数字验收（硬性）**：
- 阶段级：每个阶段必须给出该簇的 `report.json` 前后条数（目标簇清零）。
- 轮次级（N7）：
  - Security **26031 全绿**、Setup **全绿**（红线，一票否决）；
  - 四通道 B/C 类**不得新增**（基线 B 74/0/0/644、C 11/0/0/40 为上限）；
  - 无"缺失"类 diff；
  - `acceptance-check.py --prev report.prev.json` 全门 PASS，exit=0；
  - G6 关豁免口径复核：`--no-exemptions` 时 diff == 默认 diff + 10，无意外掩盖。
- 单测：Rust `cargo test --workspace` 全绿（基线 225）、Go `go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1` 全绿（基线 125）。

**红线处置**：任一阶段后破线 → `jj undo`（或 `jj restore`）回退该阶段提交 → 重派一次 → 仍失败登记转后续。
**禁止**：改动/回退 skill 基线 #1–#16 的任何既有修复；改 `EXEMPTIONS` 双表不同步；只改 `format_for_display` 三处调用中的一处。

---

## 2. 编排模型（沿用六小时计划已验证的总则）

- **编排者（ZCode 主会话）** 只做四件事：读阶段报告 → 生成派发提示词 → 派发子代理 → 核对验收。
  **不亲自执行阶段任务**（避免主上下文膨胀）。
- **子代理**：每阶段一个、全新上下文。若 ZCode 支持指定模型则用 `glm-5.3-flash`；否则继承并在提示词首行声明。
- **为 flash 的设计补偿**：简报自包含、步骤原子化、验收数字写死、开放式决策一律按"退出条件"走。
- **状态全部落盘**：阶段结束写 `docs/phase-reports/overnight-2026-09-20/night-N<X>-report.md`（模板见 §6）；
  下一阶段只依赖磁盘，不依赖任何会话记忆。
- **失败不阻塞**：阶段内失败/超时 → 按退出条件登记（转豁免/转后续）→ 写报告 → 正常结束。禁止为单簇突破时间盒。
- **提交纪律**：每阶段一个 jj commit；提交信息含簇条数前后值 + 红线结论；提交前过该阶段门（G1/G2 必跑）。

### 2.1 每阶段固定"取证 → 修复 → 闭环"三步
1. **取证**（先实证，不凭记忆）：wevtutil raw XML + `crim-probe-intype.exe` + SQL 探针 + dry-run 脚本。
   **未取证不得改代码**。
2. **修复**：TDD——先加失败单测（§3.1 层），再改实现到绿。
3. **闭环**：按 §3.2 重跑到 06，核对目标簇与红线。

### 2.2 改哪个上游 → 重跑哪几步（省时关键）
| 改动位置 | 重跑 |
|---|---|
| `msgdb-dump/**`（萃取/归属/落库） | 03 → 05 → 06 |
| `evtx-tools/**`（渲染/查询/取值） | 04 → 05 → 06 |
| `evtx-parity-test/scripts/06-compare.py`（比对层） | 只跑 06 |
| 两类都改 | 03 → 04 → 05 → 06 |

---

## 3. 环境、命令与坑位（子代理必读）

### 3.1 单测（秒级）
```powershell
cd E:\Code\evet\msgdb-dump;  go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1
cd E:\Code\evet\evtx-tools; cargo test --workspace
```

### 3.2 全链路（`evtx-parity-test/` 下）
```powershell
powershell -ExecutionPolicy Bypass -File scripts/03-build-msgdb.ps1        # ~3min，改 msgdb-dump 后
powershell -ExecutionPolicy Bypass -File scripts/04-build-evtx-tools.ps1  # ~1min，改 evtx-tools 后
powershell -ExecutionPolicy Bypass -File scripts/05-new-parse.ps1         # ~4min，03/04 之后
python scripts/06-compare.py                                              # ~1min，每轮必跑
python scripts/acceptance-check.py --prev report.prev.json                # 门禁；echo $LASTEXITCODE
python scripts/acceptance-check.py --no-exemptions                        # G6 关豁免
```

### 3.3 取证探针
| 用途 | 命令 |
|---|---|
| raw XML 形态 | `MSYS_NO_PATHCONV=1 wevtutil qe System '/q:*[System[Provider[@Name="Ntfs"] and (EventID=50)]]' /c:1 /f:xml` |
| 参数 InType/OutType + mapRef | `msgdb-dump\crim-probe-intype.exe <dll> <guid|-> <eid>` |
| 全 variant 模板 | `set ALL_TPL=1` 后同上 |
| 字符串表（0xDn） | `set DUMP_STRINGS=1` + `<dll-or-mui> x`（主 DLL 与 MUI 分开跑） |
| 子块 hexdump | `set DUMP_MAP_OFF=0x<off>` |
| 库内容 SQL 探针 | 见 §3.4 |

### 3.4 库内容 SQL 探针（python + sqlite3，DB 路径 glob `evtx-parity-test/msgdb/hotech-msg-*.db`）
关键表列（实测）：
`providers(provider_name, guid, channel, dll_id)`、
`provider_message_variants(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message, parameters, level_name, keywords_name)`、
`dll_entries(dll_id, dll_name, dll_path, pe_version)`、
`eventlog_sources(source_name, source_type)`。
自检：`param_types` / `param_value_maps` 必须含 `param_name` 列（缺列=旧库残留=重跑 03）。

### 3.5 坑位（全部实测，勿踩）
1. **03 重建期间禁止并发 05 / 直跑渲染器**（会读半写 DB，症状：types/链接随机失效）。
2. 03 控制台输出是 **GBK**——grep 中文可能失配，以 SQL 探针为准。
3. `--format both` 会抢写同一文件；05 已按 db/csv 分两次跑，勿改。
4. 裸 `cmd` 会被 msys64 垫片遮蔽；02 脚本已用 System32 全路径。
5. `report.diff` 每条 Msg 截断 500 字；深差异用 importlib 复用 06 的解析函数做全消息比对，**勿抽样下结论**。
6. `go build ./...` 会因历史坏探针 `crim-probe-566/main.go` 失败；只测指定包即可（管线 03 不受影响）。
7. 改簇前先跑 dry-run 探针实测影响面（阶段 A 的两个根因假设曾被实测证伪）。
8. 03 重建前必须删旧 DB（脚本已固化；若见 `no such column: param_name` 即删除逻辑被绕过）。

### 3.6 禁读（子代理）
`report.diff` 全文、`evtx-parity-test/msgdb|new-parsed` 下 DB 全量、handoff 全文、
`docs/` 下与本阶段无关的其他计划档。**核对只看**：阶段报告、`report.json` 关键字段、验收脚本输出。

---

## 4. 阶段总表（10h = N0–N9）

| 阶段 | 时间盒 | 目标 | 数字验收（硬性） | 上游 | 退出条件 |
|---|---|---|---|---|---|
| **N0** | 23:00–23:20 | 就绪：刷 prev、基线复核、锁清理 | prev 刷新；基线四通道数字与 §0 一致 | — | 基线破线 → 停并报告 |
| **N1** | 23:20–00:15 | C-40：CfgChangeMonitor N/A parity | 该簇 40→0；红线全过 | 06 or evtx-tools | 60min 未定位 → 登记 PARSE 转后续 |
| **N2** | 00:15–01:05 | D-2：WER 上游 hex 保护 | D 残留 2→0；M2=100% | evtx-tools | 保护名单引入回归 → 回退并登记 |
| **N3** | 01:05–02:05 | 经典源取证（Ntfs 50×515 为靶） | 断点定位到 (a)/(b)/(c) 之一并写出证据 | 只读探针 | 60min 未定位 → 转 N5 次优簇 |
| **N4** | 02:05–03:35 | 经典源落地（Ntfs 50） | 该簇 515→0（或部分清零 + 余量登记） | msgdb-dump | 破线或超盒 → 登记转后续 |
| **N5** | 03:35–05:05 | 经典源扩展（其余 B ~203） | B 总量 718→≤100（目标） | msgdb-dump | 超盒 → 剩余登记转后续 |
| **N6** | 05:05–05:45 | C 剩余（第三方服务源 11 条） | C 总量 51→≤2 | msgdb-dump | 超盒 → 登记 SRC |
| **N7** | 05:45–06:45 | 全量验收 G1–G7 + 确定性门 | acceptance-check 全 PASS ×2；关豁免 = diff+10 | — | 任一 FAIL → 定位回退 |
| **N8** | 06:45–07:45 | 定稿：三镜像 / STATUS / README / 登记表 / 报告 | 文档同步完成 + 最终 jj commit | — | — |
| **N9** | 07:45–09:00 | 缓冲 / 收尾 / 最终报告 | 收尾落盘；09:00 硬停 | — | — |

**墙钟纪律**：本地 ≥07:30 只允许 ≤30min 的收尾动作；≥08:00 **不开新阶段**，只做 N8/N9；≥09:00 一律停止。

---

## §N0 阶段 N0 简报 — 就绪门（编排者亲自做，不派子代理）

1. `date '+%F %T'`；确认工作目录 `E:\Code\evet`；记录启动时间。
2. **检查完成哨兵**：`docs/phase-reports/overnight-2026-09-20/FINAL.md` 存在 → 本轮已完结，直接退出。
3. **并发锁**：`docs/phase-reports/overnight-2026-09-20/.lock` 存在且 mtime <90min → 有别的执行会话，退出；
   过期/残留 → 删旧领新（写入当前时间）。**锁文件不入库**，每阶段 commit 前删除。
4. **刷新 prev**：`cd evtx-parity-test; cp report.json report.prev.json`（现 prev 是 E 前旧快照 799，必须刷新）。
5. **基线复核**：读 `report.json`，确认 compared=32801 / matched=32020 / diff=771 / Security=0 / Setup=0。
   若基线与此不符 → 先跑 `python scripts/06-compare.py` 复现；仍不符 → **停止并报告**（环境已漂移，勿盲跑）。
6. **环境检查**：`go version` / `cargo --version` / `python --version` / `jj --version`；
   `evtx-parity-test/msgdb/hotech-msg-*.db` 与 `tools/evtx-tools.exe` 存在。
7. 建目录 `docs/phase-reports/overnight-2026-09-20/`；写 `N0-readiness.md`（含 1–6 实测值）。

---

## §N1 阶段 N1 简报 — C-40：Configuration-Change-Monitor N/A parity（55min）

**背景**（已 recon）：该 provider 40 条，基线全部渲染 `N/A`；我们走 fallback 长文。
它在 `eventlog_sources` 中记为 `classic`，实为 WINEVT publisher `{a148cf02-be6d-5f08-94e3-b68de60d8422}`，0 条变体。

**任务**：
1. **取证**（≤20min）：确认 wevtapi 对该 provider 的行为——它是否 `PublisherGuid` 注册但
   `MessageFileName/ResourceFileName` 缺失或指向不存在的资源？用 §3.3 raw XML + registry 探查；
   复用 `evtx-parity-test/scripts/probe-r12-na-model.py`（importlib 复用 06 解析函数）跑 N/A 模型全量验证，
   给出"若规则改为 X，则命中多少条、有无误伤"的实证数字。
2. **修复**（TDD）：在 `evtx-tools` 的 R12 N/A 判定处（`cmd_parse.rs` miss-cohort 分支）收紧规则：
   注册为 winevt 但**无可用消息资源** → 与未注册同判 `N/A`。
   **必须先加失败单测**（构造该形态的入库/查询夹具），再改实现。
   **不得**放宽既有 R12（未注册 + EID≠0 → N/A）语义；不得影响 Security/Setup。
3. **闭环**：04 → 05 → 06；核对该簇 40→0、B/C 不新增、Security/Setup=0。
4. 提交 + 写报告。

**必读**：本档 §0/§1/§2/§3/§6；`docs/adr/0004`；`skills/parity-test-skill.md` 事实 #7（R12）。
**退出条件**：60min 内无法实证"无消息资源"判别口径 → **登记豁免（PARSE）**，勿硬编码 provider 名单。

---

## §N2 阶段 N2 简报 — D-2：WER 1001 上游 hex 保护（50min）

**背景**：App #2972/#3429，上游 `format_for_display` 把 WER 报告文件名 GUID 尾段十进制串
`3003916586` 渲染成 `0xb30c212a`（与基线不等）。TS35 同域修复的教训：**大十进制→hex 转换有三处调用点**
（`substitute_format_message` 内部 pass1、final formatting、Step 3 本体），且已有
`format_for_display_protected` / `substitute_format_message_protected` 保护名单机制。

**任务**：
1. **取证**（≤15min）：raw XML 取该事件两个参数的形态；`crim-probe-intype.exe` 取
   InType/OutType（先探针实证，不凭记忆）；确认该值是否为"owned typed 参数最终值"。
2. **修复**（TDD）：把该参数纳入保护名单（或收窄 Step 3 的触发条件），**三处调用点一起处理**。
   先加失败单测（给定 `3003916586` 形态输出为十进制原样）。
3. **闭环**：04 → 05 → 06；核对 D 残留 2→0、M2=100%、TS35 等历史已修簇不回退、红线全过。
4. 提交 + 写报告。

**必读**：本档 §0/§1/§2/§3/§6；`skills/parity-test-skill.md` 事实 #8/#11；handoff 的 TS35 小节。
**退出条件**：改动触发任何已完成簇回退 → 回退本次改动，登记"后续专项"，写报告。

---

## §N3 阶段 N3 简报 — 经典源消息表取证：Ntfs 50 ×515（60min，有界 research，只读）

**断点假设**（§0 recon 已给 (a)(b)(c)）：必须用证据**收敛到唯一一个**，并写出可复现命令与结论。

**任务（按序，命中即停）**：
1. **确认消息文本在哪个文件**：对 ntfs.sys 跑 `DUMP_STRINGS=1 crim-probe-intype.exe C:\WINDOWS\system32\drivers\ntfs.sys x`，
   搜 "Delayed Write Failed"；同时对其 MUI（`zh-CN\ntfs.sys.mui` / `en-US\...`）各跑一次（探针不合并 MUI）。
2. **确认消息 ID**：找到的条目 ID 是否为 `0x40000032`（=0x40000000|50）或 50；记录原始 key。
3. **确认落库断点**：SQL 查 `dll_entries`/`providers`/`provider_message_variants`：
   - 该消息是否已在库中但挂到别的 provider（→ (b) 归属污染）；
   - 或根本没落库（→ (a) 萃取/映射）；
   - 或在库但 renderer 查不到（→ (c) 渲染，用 `evtx-tools parse` 直跑单文件 CSV 抽查验证）。
4. **对照第二个簇**（hcmon / Win32k）交叉验证断点是否同源。
5. **产出**：`N3-classic-source-forensics.md`——断点编号 + 证据命令 + 原始输出片段 + 影响面估算
   （可清零条数）+ 建议修复点（文件/函数）。**本阶段不改任何代码。**

**退出条件**：60min 未收敛 → 写出"已排除的假设 + 剩余假设 + 下一步"，转 N4 按最可能假设实现（风险自担，仍受红线约束）。
**禁读**：`report.diff` 全文、DB 全量。

---

## §N4 阶段 N4 简报 — 经典源落地：Ntfs 50（90min）

**任务**：
1. 读 `N3-classic-source-forensics.md`，按其中断点实施修复。
   - 若 (a) 萃取/映射：改 `msgdb-dump/{pe,dump}` 的经典 RT_MESSAGETABLE 路径，把低 ID 消息
     映射为 classic provider（`guid=NULL`）的 `event_id`；写 Go 单测。
   - 若 (b) 归属：改 `dump/provider_extractor.go` / `run_variants.go` 的归属逻辑，
     按"经典源名称 + EventMessageFile"归属，勿挂到现代 GUID provider；写单测。
   - 若 (c) 渲染：改 `evtx-tools` 的 guid=NULL 查询路径（`resolver.rs`/`variant_resolver.rs`）；写 Rust 单测。
   - **严守** `CONTEXT.md` 的 Provider Attribution 定义（按 provider_guid→WEVT 归属，非 dll_path→DllGroup）。
2. **闭环**：按 §2.2 选步（msgdb-dump 改 → 03→05→06；evtx-tools 改 → 04→05→06）。
3. 核对：Ntfs 50 **515→0**（允许部分，余量须写明原因）；红线全过；单测全绿。
4. 提交 + 写报告。

**退出条件**：破线 → `jj undo` 回退重做一次；仍破线 → 回退 + 登记转后续。
**注意**：这是本轮最大单簇，若修复面过大，允许只做"最小可验证修复"并在报告写明剩余。

---

## §N5 阶段 N5 简报 — 经典源扩展：其余 B ≈203 条（90min）

**任务**：把 N4 的机制推广到其余经典源簇（hcmon 89、WXWorkUpgrader 27、asus 16、
Win32k 267 14、igfxCUIService 9、PDFShellExt 9、NetBT 4311 4、EventLog 6005/6006/6008/6009 8、
Chrome 256、MEIx64、VMnetAdapter、e1dexpress 等）。
1. 复用 N4 的探针脚本，批量确认各源的 `EventMessageFile` 与消息 ID 映射形态。
2. 对能走同一机制的直接清零；对形态不同的（如 Win32k 纯 hex、NetBT 长 hex 串）单独取证：
   可能是取值层/渲染臂问题，**先探针再定方案**。
3. 批量闭环：03 → 05 → 06（若还改了 evtx-tools 则加 04）。
4. 目标 B 总量 718 → ≤100（保守：写出每簇归属，未清零者登记）。
5. 提交 + 写报告。

**退出条件**：90min 到点即收尾，剩余逐簇登记（转后续/豁免），**不得拖尾**。
**必读**：N4 报告；本档 §0/§3。

---

## §N6 阶段 N6 简报 — C 剩余：第三方服务源（40min）

**目标簇**：WMIRegistrationService 0/2（6）、IntelDalJhi 0（3）、EventSystem 4625（1）、Search 1003（1）。
**WMIRegistrationService/IntelDalJhi 的 EMF 是 DriverStore 下的 .exe**——先确认 03 是否已把这些 .exe 纳入采集；
若否，补采集路径（注意归属仍按注册源名）。
EventSystem/Search 为 winevt 有真实消息但 miss，按 N1 的 N/A 模型与模板匹配排查。

**任务**：取证 → 修复（TDD）→ 03→05（→04）→06 → 目标 C 总量 51→≤2。
**退出条件**：40min 到点即登记转后续。

---

## §N7 阶段 N7 简报 — 全量验收 G1–G7 + 确定性门（60min）

1. `cd evtx-parity-test; python scripts/acceptance-check.py --prev report.prev.json`；`echo $LASTEXITCODE`。
2. **G6 关豁免**：`python scripts/acceptance-check.py --no-exemptions` 全量复核；
   确认关豁免 diff == 默认 diff + 白名单条数（防豁免掩盖回归）。
3. **确定性门**：重跑 `05 → 06` 一次，比对两次 `report.json` 的 diff/matched/by_channel 完全一致
   （防 HashMap 序不确定性，R8 先例）。
4. **红线复核**：Security=0、Setup=0、四通道 B/C 不新增、无缺失类。
5. 结果写入 `night-N7-report.md`，并在提交信息中写明验收记录（§5 模板）。

**退出条件**：任一 FAIL → 定位到具体提交并回退（`jj undo`）→ 复跑确认绿；无法定位 → 保留最后绿提交并登记。

---

## §N8 阶段 N8 简报 — 定稿与文档同步（60min）

1. `skills/parity-test-skill.md` **三镜像同步**（→ `.agents/skills/`、`.dsh/skills/`），
   新增本轮"勿回退事实 #17+"：经典源消息表归属机制、N/A 精度规则、WER 保护名单扩展；
   基线表更新为 N7 实测数字。三份 md5 必须一致。
2. `STATUS.md`：§3 消息库、§5 基线、§6 修复表、§7 残留、§8.1 单测数字全部更新。
3. **`README.md` 口径修正**（历史 stale）：schema v4→v8、169 tests→当前值、
   表结构补 eventlog_sources/param_types、工具描述与 STATUS 对齐（本轮附带清理项）。
4. `docs/residual-zeroing-plan-2026-09-07.md` §4 登记表：新增/关闭项与 EXEMPTIONS 双表**同一提交**同步。
5. `docs/phase-reports/final-report.md`（或新建本轮 final）写终态锚点；`C:\msys64\tmp\evet-handoff-2026-09-07.md` 更新终态节。
6. 写 `docs/phase-reports/overnight-2026-09-20/FINAL.md`（**完成哨兵**），最终 jj commit。

**注意**：`EXEMPTIONS` 在 `06-compare.py` 与 `scripts/acceptance-check.py` 两处，必须同步。

---

## §N9 阶段 N9 — 缓冲 / 收尾（75min）

- 吸收前序溢出：若某阶段超盒，在此补完或登记。
- 清理：删除临时探针脚本、锁文件；确认 `jj status` 干净、无大文件误入跟踪。
- 汇总本轮最终执行报告（各阶段数字、终态 M1/M2/D/豁免、红线结论、后续专项清单）。
- **09:00 硬停**：无论如何停止，确保磁盘状态一致（报告 + 提交落盘）。

---

## 5. 心跳看护协议（23:00–09:00 无人值守）

> 若 ZCode 能稳定跑长会话，可省心跳；但首夜按**心跳自愈**配置（六小时计划实测：单会话马拉松易被限流/中断）。

```
心跳触发(每20min) → 完成哨兵(FINAL.md 在?) → 并发锁检查(90min 过期)
  → 领锁 → 找第一个缺报告的阶段 → 派发1个子代理(一个阶段) → 编排者核对
  → jj提交 + 阶段报告落盘 → 删锁 → 退出
```

- **cron**：`*/20 23,0-8 * * *`（23:00–08:59 每 20 分钟）。
- **状态传递只认磁盘**：阶段报告 + jj 提交。任何一环死亡，下一心跳从磁盘续跑，损失 ≤20min。
- **限流处置**：派发遇速率限制 → 等 5min 重试，至多 3 次；仍失败 → 删锁退出，下次心跳续跑。不硬扛。
- **并发锁**：`docs/phase-reports/overnight-2026-09-20/.lock`；**不入库**，commit 前删。
- **墙钟纪律**：见 §4 表下说明。

### 5.1 运行前提（用户侧一次性检查）
1. **机器不休眠**：电源设置"睡眠=从不"（接通电源），否则心跳不触发。
2. **ZCode 保持运行**、本工作区 `E:\Code\evet` 保持打开。
3. **权限放行**：夜间不能弹确认——把本工作区工具权限（Bash/jj/python/cargo/go）设为自动允许 / always-allow。
4. **窗口内不要手动跑编排者**（锁会挡心跳；人为介入请先删锁）。
5. **清理旧心跳残留**：计划任务 `UIEOrchestrator`（action = `UIEOrchestratorStub.exe /SendHeartbeat`，
   目标 exe 已不存在）与 `docs/phase-reports/.orchestrator-lock` 属上一轮残留，建议删除；
   本档的心跳用新建的 ZCode 自动化，勿复用旧的。
6. **配额**：单个子代理实测约 100 万 token（glm-5.3-flash）；心跳机制容忍限流。

---

## 6. 阶段报告模板（`docs/phase-reports/overnight-2026-09-20/night-N<X>-report.md`）

```markdown
# Night <X> 报告（<日期时间>）
## 结论
- 验收数字：<目标簇前→后；diff/matched/M1；M2；exempted>
- 门：G1 ✓/✗  G2 ✓/✗（Security/Setup/B/C/缺失 逐项）
## 取证
- <断点/证据命令/原始输出片段>
## 完成
- <任务 → 结果（含改动文件）>
## 登记/转出
- <簇 → 豁免(根因码 PARSE/SRC/BIN/RO)/后续专项 + 关闭条件>
## 未决与建议
- <坑位、给下阶段的提示>
## 变更文件
- <清单>（jj change id：<id>）
```

---

## 7. 子代理派发提示词模板（编排者用）

```text
你是夜间计划阶段 <X> 的执行代理。严格按以下简报执行，时间盒 <N> 分钟。
工作目录 E:\Code\evet。先读简报，再取证，再改代码（TDD），最后闭环。

<粘贴本档 §N<X> 全文>

通用必读（只读这些）：
- docs/overnight-plan-2026-09-20.md §0/§1/§2/§3/§6（起点数字、红线、命令、坑位、报告模板）
- skills/parity-test-skill.md 的"已核实的关键事实（勿回退）"一节
- 上一阶段报告 docs/phase-reports/overnight-2026-09-20/night-N<X-1>-report.md
禁读：report.diff 全文、evtx-parity-test/msgdb|new-parsed 下 DB 全量、handoff 全文、其他计划档。

铁律：
- 先探针实证，不凭记忆；未取证不得改代码。
- 任何渲染/萃取修复先加失败单测再改实现，单测全绿才算改完。
- 每阶段一个 jj commit（信息含簇条数前后 + 红线结论）。
- 破红线（Security/Setup>0、B/C 新增、缺失类）→ jj undo 回退，重做一次，仍失败登记转后续。
- 时间盒到点即收尾：宁可登记豁免/后续，不拖尾，不为单簇突破时间盒。

完成后必须：
1. 写 docs/phase-reports/overnight-2026-09-20/night-N<X>-report.md（按模板，字段缺一不可）；
2. jj commit；
3. 回报：jj change id、目标簇前→后条数、G1/G2 结论、单测数字、未决项。
```

---

## 8. 相关文档
- 入口提示词：`docs/orchestrator-prompt-2026-09-20.md`
- 指标与门禁：`docs/acceptance-criteria.md`
- 命令与坑位：`docs/execution-runbook.md`
- 登记表真源：`docs/residual-zeroing-plan-2026-09-07.md` §4
- 勿回退事实：`skills/parity-test-skill.md`（三镜像）
- 上一轮计划（历史）：`docs/six-hour-execution-plan.md`、`docs/overnight-watchdog.md`
