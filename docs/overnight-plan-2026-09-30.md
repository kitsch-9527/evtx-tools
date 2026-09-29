# 夜间持续执行计划（2026-09-29 23:30 → 2026-09-30 08:30）

> 版本：2026-09-29 夜。上游：`docs/phase-reports/2026-09-29-r21-classic-fixes.md`（R21 四修复后终态）、
> `docs/overnight-plan-2026-09-20.md`（编排协议模板）、`skills/parity-test-skill.md`（勿回退事实 #1–#20）。
> 执行者：ZCode 心跳编排会话（每 30 分钟）+ 每阶段一个 general-purpose 子代理。
>
> **本档是自包含操作手册**：子代理只读对应 §P<N> 全部 + §1/§2/§3/§6 通用节，不读别的计划档。

---

## 0. 起点锚点（2026-09-29 18:33 终态实测，report.json=report.prev.json）

- compared=44890、matched=44635、**diff=237**（全部 D 类）、**M1=99.4319%**、exempted=18 条/5 项、
  baseline_lang=**zh-CN**、verdict=DIFF。
- 分通道：Application 10728/10742（D14）、Setup **676/676 全绿**、Security **27366/27366 全绿**、
  System 5855/6106（D223）。**B/C 类全场为 0**（红线基线）。
- 残留簇分解（实测）：CfgChangeMonitor 1×80、Ntfs(Microsoft-Windows-) 98×74、NetBT 4311×38、
  Resource-Exhaustion-Detector 2004×9、SAM 16962×8(豁免BIN)、VMnetAdapter 36×8、e1dexpress 32×8、
  WPD 24577×4/24579×4(豁免PARSE)、Chrome 256×8、WER 1001×2、EventLog 6008×2、CodexSandbox 1001/1002×2、
  SPP 1004×1(豁免)、CAPI2 4111×1(豁免)、Edge 256×2。
- 单测基线：Go 130 绿、Rust 228 绿（bin 29 + core 196 + doctest 3）。
- 工具链就绪：msgdb 18:11 新建、tools/evtx-tools.exe 18:14、`scripts/_01-security-elevated.ps1`（UAC）。

---

## 1. 目标与红线

**主目标**：残留 237 条 D 逐簇清零（目标 ≥150 条，乐观全清），M1 向 100% 推进。
**数字验收（硬性）**：
- 阶段级：目标簇条数前后值必须写进报告与提交信息。
- 轮次级：Security=0、Setup=0（一票否决）；**B/C 不得新增**（当前 0/0/0/0）；无缺失类；
  `acceptance-check.py --prev report.prev.json` 的 G0/G2/G3 必过（G1 的 M2 用 en 基线 348 属已知失义，
  只披露不否决）；单测 Go 130+/Rust 228+ 全绿。
**红线处置**：任一阶段后破线 → 回退该阶段改动（jj restore 到阶段前 change）→ 重派一次 → 仍失败登记转后续。
**禁止**：回退 skill 事实 #1–#20 任何既有修复；EXEMPTIONS 双表不同步改动（P8 前禁碰豁免表）；
只改 format_for_display 三处调用之一（P5）；放宽 R12 既有语义（P1）。

---

## 2. 编排模型

- **心跳（每 30 分钟 cron）**：完成哨兵检查 → 并发锁（90min 过期）→ 找第一个缺报告的阶段 →
  派发 **1 个**子代理 → 核对 → jj 提交 → 删锁 → 退出。状态只认磁盘。
- **失败不阻塞**：阶段失败/超时 → 报告写 FAIL 登记 → 仍提交（仅报告）→ 正常结束，下一心跳继续。
- **墙钟纪律**：本地 ≥07:00 只允许 ≤30min 收尾（P8/补报告）；≥08:00 不开新阶段只做 P8；**08:30 一律停止**。

## 3. 环境、命令与坑位（子代理必读）

### 3.1 单测（秒级）
```powershell
cd E:\Code\evet\msgdb-dump;  go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1
cd E:\Code\evet\evtx-tools;  cargo test --workspace
```
### 3.2 全链路（`evtx-parity-test/` 下）
```powershell
powershell -ExecutionPolicy Bypass -File scripts/03-build-msgdb.ps1        # ~3min，改 msgdb-dump 后
powershell -ExecutionPolicy Bypass -File scripts/04-build-evtx-tools.ps1   # ~1min，改 evtx-tools 后
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/05-new-parse.ps1  # ~4min
python scripts/06-compare.py                                               # ~1min 每轮必跑
python scripts/acceptance-check.py --prev report.prev.json                 # 门禁
```
### 3.3 坑位（全部实测，勿踩）
1. **重跑 05 前必须核对 `tools/evtx-tools.exe` mtime**——04 未真正重编译会造成"修复无效"假象（09-29 踩坑）。
2. `wevtutil qe <文件>` 查导出的 evtx 必须**绝对路径 + `/lf:true`**，否则报"指定的通道路径无效"。
3. wevtutil 输出**重定向到文件是 GBK**（控制台管道才是 UTF-16），python 解码先试 GBK。
4. `wevtutil epl` 不覆盖已存在文件，重导前先删。
5. 提权运行的 .ps1 必须**纯 ASCII**（无 BOM UTF-8 中文注释被 PS5.1 按 GBK 读吞换行）。
6. 03 重建期间禁止并发 05；03 控制台输出 GBK；`go build ./...` 因历史坏探针会挂，只测指定包。
7. `report.diff` 每条 Msg 截断 500 字；深差异用 importlib 复用 06 解析函数全量比对，勿抽样下结论。
8. **禁读**：`report.diff` 全文、msgdb/new-parsed 下 DB 全量、与本阶段无关的其他计划档。
9. 主比对语言 zh-CN（G0）；基线 Date 为本地时间带 Z 后缀；比对容差 2s。
10. Security 通道重导需 UAC（`scripts/_01-security-elevated.ps1`）——夜间**不需要**（01 不在计划内）。

### 3.4 取证探针
| 用途 | 命令 |
|---|---|
| raw XML（本机通道） | `MSYS_NO_PATHCONV=1 wevtutil qe System '/q:...' /c:2 /f:xml` |
| raw XML（导出文件） | `MSYS_NO_PATHCONV=1 wevtutil qe "E:\Code\evet\evtx-parity-test\baseline-evtx\System.evtx" /lf:true '/q:...' /c:2 /f:xml` |
| 06 解析函数复用 | importlib 加载 `scripts/06-compare.py`（参照 /tmp/classify2.py 模式） |
| 库内容 SQL | python+sqlite3，DB=glob `evtx-parity-test/msgdb/hotech-msg-*.db` |
| CRIM 探针 | `msgdb-dump/crim-probe-intype.exe <dll> <guid|-> <eid>`（DUMP_STRINGS 只扫 RT_STRING，勿作 type11 证据） |

## 4. 阶段总表（时间盒为上限）

| 阶段 | 时间盒 | 目标 | 验收 | 上游 | 退出条件 |
|---|---|---|---|---|---|
| P0 | 23:25 就绪 | 锚点复核+锁+本档提交 | 基线与 §0 一致 | — | 不符 → 停并报告 |
| **P1** | 60min | R12 N/A 精度：CfgChangeMonitor 80→0 | 该簇清零；红线全过 | evtx-tools | 回归/未定位 → 登记转后续 |
| **P2** | 90min | value_maps 双语化 schema v9：Ntfs 98 74→0 | 该簇清零或部分+登记 | msgdb-dump+evtx-tools | 回归 → 回退登记 |
| **P3** | 60min | hex 格式族：NetBT 4311×38/VMnet 36×8/e1d 32×8 | ≥1 簇清零，余登记 | 取证后定 | 60min 未定位 → 登记 |
| **P4** | 45min | Resource-Exhaustion 2004×9 参数空置 | 9→0 或登记 PARSE | evtx-tools | 超盒 → 登记 |
| **P5** | 60min | WER 1001×2 上游 hex 保护 | 2→0；TS35 等不回退 | evtx-tools | 回归 → 回退登记 |
| **P6** | 60min | 小簇定性：6008×2/Codex×2/Chrome、Edge 256×10 | 逐条定性+可机械修的清零 | 视根因 | 超盒 → 登记 |
| P7 | 60min | 全量验收 ×2 + 确定性门 + G6 关豁免 | 两轮全等；关豁免=diff+18 | — | FAIL → 定位回退 |
| P8 | 60min | 定稿：skill 三镜像/STATUS/登记表/FINAL | 文档同步+最终提交 | — | — |
| 缓冲 | →08:30 | 吸收溢出/补登记 | — | — | 08:30 硬停 |

---

## §P1 简报 — R12 N/A 精度：registered-but-no-resources（60min，evtx-tools）

**背景（已 recon）**：`Microsoft-Windows-Configuration-Change-Monitor` eid=1 ×80 条，基线渲染 **N/A**；
该 provider 在 `eventlog_sources` 有注册行（classic 误标，实为无消息资源的 WINEVT publisher），
`provider_message_variants` 0 行 → 我方走 fallback 长文全错。R12 现规则：未注册 ∧ EID≠0 → N/A；
缺的精度是：**注册但无任何消息资源** → wevtapi 同样 N/A。
**任务**：
1. 取证（≤15min）：SQL 实证该 provider：eventlog_sources 有行、variants=0；用 06 解析函数全量枚举
   "已注册且 variants=0"的 provider 清单，核对它们出现在比对中的行基线是否全为 N/A
   （**dry-run 模型**：若存在基线非 N/A 的此类 provider，规则须收窄并报告，不得硬上）。
2. 修复（TDD）：`resolver.rs` 打开时加载 `providers_with_variants: HashSet<String>`
   （`SELECT DISTINCT provider_name FROM provider_message_variants`，空表=未知=一律 true 保守）；
   新增 `pub fn has_any_variant(&self, name) -> bool`；`cmd_parse.rs` miss 臂
   （eid≠0 ∧ registry_known ∧ registered）追加 `&& has_any_variant(prov)` 才走 fallback，
   否则置 "N/A"。先加失败单测（registered+0 变体→N/A 臂的谓词函数级测试；有变体→false）。
   **不得**改既有"未注册→N/A"语义。
3. 闭环：04（核对 exe mtime）→ 05 → 06。验收：该簇 80→0；无新增 N/A 类 diff 行
   （D 计数只减不增）；Security/Setup=0；B/C=0。
4. 提交 + 写 `docs/phase-reports/overnight-2026-09-30/night-P1-report.md`（§6 模板）。

## §P2 简报 — value_maps 双语化 schema v9（90min，msgdb-dump+evtx-tools）

**背景（已 recon）**：Ntfs 98×74 基线"运行状况良好"vs 我方 "is healthy"——VMAP 0xDn 字符串表
en 偏置（extractor/provider_extractor 的 d2Strings 合并是主 DLL 先填、MUI 只补缺）。
`value_maps` 无 lang 列（PK=provider,map,value），渲染端无从按语言取。
**任务**：
1. 取证（≤15min）：SQL 确认 Ntfs 98 相关 map 的 display 全 en；`grep -rn "get_value_map\|resolve_value" evtx-tools/evtx-tools-core/src/` 审计全部消费点（改动必须全部经 lang 感知）。
2. schema v9（TDD）：`value_maps` 加 `lang TEXT NOT NULL DEFAULT 'en-US'`，PK 加 lang；
   metadata `schema_version=9`；Rust `open()` want "9"（reject 更旧）；测试夹具
   `insert_metadata_8` 改 v9（或加参）。**两个写入路径都要带 lang**：`InsertValueMap`/
   `writeValueMaps`（run_windows.go）与 WELM import Step 6（lang 字面量）。
3. 萃取端（TDD）：per-lang stringTable——对每个 langCode：`main RTStrings+D2Strings` 为底、
   该语言 MUI 的 D2Strings **全覆盖 overlay**（镜像 MUI 整族本地化语义）；对每语言分别跑
   MAPS 解析得 `ValueMapsByLang[lang][mapName][value]=display`（en 层同时留 legacy 字段亦可）；
   provider_extractor.go 同步改造。先加失败单测（zh MUI 字符串覆盖 en 的夹具级断言）。
4. 渲染端（TDD）：`load_value_maps()` 按 lang 加载首选层 + en-US 回退层（镜像 param_refs v9 模式）；
   `get_value_map`/`resolve_value` 先首选后回退。
5. 闭环：03 → 04（核 mtime）→ 05 → 06。验收：Ntfs 98 74→0（或部分+余量原因）；
   R19/R20 既有 value-map 修复**零回退**（System D 不得新增）；红线全过；单测全绿。
6. 提交 + 报告。
**退出条件**：90min 未完成或闭环回归 → 回退本阶段全部改动，登记"schema v9 转后续"（高价值候选）。

## §P3 简报 — hex 格式族（60min，取证先行）

**目标簇**：NetBT 4311×38、VMnetAdapter 36×8、e1dexpress 32×8（classic 源，R21 后模板已命中，
剩参数格式差）。
1. 取证（≤20min）：对每簇取对齐 diff 样本（06 函数）+ raw XML（/lf:true 绝对路径）+
   `crim-probe-intype`（classic 无 CRIM 可跳过）→ 判定格式族：hex 大小写/`0x` 前缀/逗号分组/
   裸 binary dump。写出逐簇格式规则假设 + 影响面。
2. 修复（TDD）：按判定改 evtx-tools 取值/渲染层（classic Data.#text 路径或 typed 渲染臂）；
   每个规则先加失败单测。**规则必须由 ≥3 条样本归纳，禁止单样本拍脑袋**。
3. 闭环 04→05→06；目标簇清零；若规则伤及同域已修簇 → 收窄或回退登记。
4. 提交 + 报告。**退出**：60min 未收敛 → 登记格式族细节转后续。

## §P4 简报 — Resource-Exhaustion 2004×9 参数空置（45min）

**现象**：基线 "agent-win.exe (18268) 使用了 7402741760 字节；…" vs 我方 "() 使用了 字节"——模板对、
参数全空。**任务**：raw XML 取该事件 Data 结构（疑 nested/多值）→ `extract_subst_params`
（cmd_parse.rs）对该形态的取值修复（TDD）→ 04→05→06 该簇 9→0。**退出**：45min 超盒 → 登记 PARSE。

## §P5 简报 — WER 1001×2 上游 hex 保护（60min）

**背景（en 时代 recon）**：WER 报告文件名 GUID 尾段十进制串 `3003916586` 被渲染成 `0xb30c212a`；
大十进制→hex 转换有**三处调用点**（substitute_format_message 内部 pass1、final formatting、
Step 3 本体），已有 `format_for_display_protected`/`substitute_format_message_protected` 保护名单机制。
**任务**：取证（对齐样本 + 该参数 InType）→ 三处调用点一起纳入保护（或收窄 Step 3 触发条件）→
先加失败单测（`3003916586` 原样输出）→ 04→05→06 该簇 2→0、**TS35 等历史 hex 修复零回退**。
**退出**：触发任何回退 → 回退本次改动登记转后续。

## §P6 简报 — 小簇定性（60min）

EventLog 6008×2（classic；查 netevent MUI 变体是否经 R21 落库、文本差异何在）、
CodexSandboxService 1001/1002×2（新 ISV；查注册与 MUI 在盘性）、Chrome 256×8 / Edge 256×2
（疑 hex/格式差，取证后若与 P3 同族则并轨）。逐条定性 → 机械可修的修（TDD）→ 其余登记根因码。
**退出**：60min 到点即收。

## §P7 简报 — 全量验收（60min）

1. `python scripts/acceptance-check.py --prev report.prev.json`；`echo $?`。
2. G6 关豁免：`python scripts/06-compare.py --no-exemptions` 复跑后确认 diff == 默认 diff + 18；
   **跑完恢复默认 report.json**（重跑 06 默认口径）。
3. 确定性门：重跑 05→06 一次，两次 report.json 的 diff/matched/by_channel 完全一致。
4. 红线复核：Security=0、Setup=0、B/C=0、无缺失、G0=zh-CN。
5. 单测双端全绿。写 night-P7-report.md。

## §P8 简报 — 定稿（60min）

1. `skills/parity-test-skill.md` 三镜像同步（新增事实 #21+：本轮机制与坑位；基线表更新 P7 终态数字）；md5 一致。
2. `STATUS.md` §5/§6/§7/§8.1 全量更新。
3. **EXEMPTIONS 双表同一提交同步**（`06-compare.py` 与 `scripts/acceptance-check.py`）：
   视 P7 终态决定（disk/51 已脱豁免可删行；若本轮又清了 SPP/CAPI2/WPD/SAM 中某些则同步删）；
   `docs/residual-zeroing-plan-2026-09-07.md` §4 登记表同步。
4. `docs/phase-reports/overnight-2026-09-30/final-report.md` 终态锚点；写 `FINAL.md` 完成哨兵。
5. 最终 jj commit（信息含终态数字+验收记录）。

---

## 5. 心跳协议（cron 每 30 分钟，maxRuns 封顶至 08:30）

```
心跳触发 → 时间≥08:30? 退出 → FINAL.md 在? 退出 → 锁检查(90min 过期) → 领锁
  → 找第一个缺 night-P<N>-report.md 的阶段 → 派发1个子代理(§P<N> 全文+§1/§2/§3/§6)
  → 核对(报告在/单测绿/acceptance-check G0G2G3) → jj 提交 → 删锁 → 退出
```
- 限流：派发失败等 5min 重试 ≤3 次，仍败删锁退出。
- **锁**：`docs/phase-reports/overnight-2026-09-30/.lock`，不入库（提交前删）。

## 6. 阶段报告模板（`docs/phase-reports/overnight-2026-09-30/night-P<N>-report.md`）

```markdown
# Night P<N> 报告（<日期时间>）
## 结论
- 验收数字：<目标簇前→后；diff/matched/M1；红线逐项>
## 取证
- <证据命令/输出片段>
## 完成
- <任务 → 结果（改动文件）>
## 登记/转出
- <簇 → 根因码/后续专项>
## 未决与建议
- <坑位/提示>
```

## 7. 子代理派发模板（编排者用）

```text
你是 evet 夜间计划阶段 P<N> 的执行代理。严格按简报执行，时间盒 <N> 分钟。
工作目录 E:\Code\evet。先读简报，再取证，再改代码（TDD），最后闭环。

<粘贴 docs/overnight-plan-2026-09-30.md 对应 §P<N> 全文 + §1/§2/§3/§6>

铁律：
- 先探针实证，不凭记忆；未取证不得改代码。
- 任何渲染/萃取修复先加失败单测再改实现；单测全绿才算改完。
- 闭环后必须核对 tools/evtx-tools.exe mtime 已更新（坑位 1）。
- 破红线（Security/Setup>0、B/C 新增、缺失类）→ 回退改动，重做一次，仍失败登记转后续。
- 时间盒到点即收尾：宁可登记，不拖尾。

完成后必须：
1. 写 docs/phase-reports/overnight-2026-09-30/night-P<N>-report.md（模板 §6）；
2. 回报：目标簇前→后条数、红线结论、单测数字、未决项、变更文件清单。
（jj 提交由编排者统一做，子代理不提交。）
```

## 8. 相关文档
- 前置终态：`docs/phase-reports/2026-09-29-r21-classic-fixes.md`
- 勿回退事实：`skills/parity-test-skill.md`（三镜像）
- 门禁：`evtx-parity-test/scripts/acceptance-check.py`（G0–G7）
