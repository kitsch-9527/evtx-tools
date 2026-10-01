# evet 项目现状说明

> 生成时间：2026-09-07，**2026-09-30 夜间计划定稿更新**（P1–P7 把 diff 237→21；数字口径以
> `evtx-parity-test/report.json` 为准，术语见 `CONTEXT.md`）。

## 1. 项目是什么

`evet` 是 Windows 事件日志（EVTX）**离线语义还原**工程：把 EVTX 二进制事件还原成
与 Windows 运行时（`wevtapi` / `wevtutil`）一致的人类可读消息，并用双链路比对闭环
证明一致性。判定基准：本机四通道（Application / Setup / Security / System）事件
经两条路径解析后逐条严格一致（五字段：EventID / Provider / Channel / TimeCreated /
Message）。

- **路径 A（真值）**：`msys64-Tools/evtx2text.lua`（经 wevtutil/wevtapi 渲染）→ `baseline-parsed/`
- **路径 B（被测）**：`msgdb-dump`（静态萃取消息库）→ `evtx-tools`（离线渲染）→ `new-parsed/`
- **闭环**：`06-compare.py` 逐条对齐比对 → `report.json`（verdict 即闭环口径）

## 2. 顶层结构

| 目录 | 内容 | 说明 |
|---|---|---|
| `msgdb-dump/` | Go 萃取器 | PE/CRIM/MUI/注册表 → 单文件 SQLite 消息库（schema v9） |
| `evtx-tools/` | Rust 渲染器 | EVTX 解析 + 消息库查询 + 模板代入（parity 被测端） |
| `evtx-parity-test/` | 比对闭环台 | 01-export … 06-compare 管线脚本 + 报告产物 |
| `msys64-Tools/` | 基线解析器 | `evtx2text.lua`（路径 A，只跟踪顶层脚本，工具链本体不入库） |
| `docs/` | ADR + 研究文档 | ADR 0001-0005 及历史分析报告 |
| `skills/`（+`.agents/`、`.dsh/` 三镜像） | 工程技能基线 | `parity-test-skill.md` 含"勿回退"事实清单 |
| `.agent-teams/` | 历史 agent 协作档案 | 只读归档 |

## 3. 消息库（Single-File MessageDB，schema_version = 9）

`msgdb-dump` 一次运行产出唯一 SQLite 文件（`hotech-msg-<系统>-<build>.db`，约 300MB，
不入版本库——由管线 03 随时重建）。v9 = v7 的 7 张实体表 + 2 张出处表（ADR 0004/0005）
+ value_maps 双语化（lang 入 PK，2026-09-30 P2，不做新旧库兼容：读端 `MessageDb::open`
硬校验版本，缺表即错）：

| 表 | 职责 |
|---|---|
| `provider_message_variants` | 唯一消息表：8 元组 Variant Identity（ADR 0001）精确匹配，约 158 万行 |
| `providers` / `dll_entries` | Provider 归属与来源 DLL |
| `parameter_refs` | `%%N` 参数引用（含 `Win32-System-Error` 回退层，9813 行） |
| `value_maps` / `param_value_maps` | 枚举/位图值映射及其与事件参数的链接（v9 起 `value_maps` 带 lang 维度，PK 含 lang） |
| `eventlog_sources`（0004） | 注册源快照（1272 行）：classic EventLog ∪ WINEVT Publishers |
| `param_types`（0005） | 参数 TDH InType/OutType 字节（71598 行）：类型化渲染依据 |

## 4. 渲染管线（evtx-tools，逐层）

1. miss-cohort 判定：库无命中时，源未注册（不在 `eventlog_sources`）且 EID≠0 → `N/A`（R12）。
2. `%%N` 展开（R15/R17：provider 私有优先，Win32 系统错误回退）。
3. TDH 类型化渲染（R10，`apply_param_types`，规则表实证于 skill 基线 #8）。
4. 枚举启发式（R11）→ CRIM 值映射（Layer 2/2b）→ LUID 大写 hex（R14）。
5. `%1-%99` 位置代入 → 展示格式化。取值层含 JSON 原生 Bool 修复（R18）。

## 5. 当前基线（2026-09-30 终态，**verdict=PASS，绝对 100%**：45166/45166 一致，diff=0，exempted=0）

> 经 v5 G0 主比对随基线、v9 双语化、R21/R26–R29/R31–R34 三轮修复，**2026-09-30 达成
> 比对集内绝对 100%**：diff=0 且受控豁免清零（EXEMPTIONS 双表已清空，登记表 §4 全部关闭）。

| 通道 | 比对 | 一致 | 差异 |
|---|---|---|---|
| Application | 11012 | 11012 | **0** |
| Setup | 676 | 676 | **0** |
| Security | 27253 | 27253 | **0** |
| System | 6225 | 6225 | **0** |

- 全量：45166 条比对全部一致；**M1 = 100.0000%，M2 = 100.00%**。
- 门禁：G0–G4 全过 exit=0；G6 关豁免 diff=0=0+0；确定性门两次全等；单测 Go 137 / Rust 249 绿。
- 本轮（R31–R34，全部 TDD 红→绿 + 全量回归守护）：R31 CJK 日期伪影手术（CAPI2 4111×1）、
  R32 classic 零参占位符字面量透传（Qualifiers 金标门控；WPD×8）、R33 Strategy 2 跳过 `#`
  伪键（SAM×8）、R33b canon 相等性前置（B 类判据误判修复）、R34 空串 #text 不算参数
  （有 Binary 除外）。
- 历史：09-05 起点 diff=1127 → en 终态 771/97.6190% → 09-29 语言翻转事故（1840）→
  v5+v9 96.58% → R21 99.4319% → 夜间 P1–P7 99.9153% → R26–R29 99.9621%（豁免 17）→
  **R31–R34 绝对 100%**。

## 6. 已落地修复（全部 TDD 回归测试保护，勿回退）

| 修复 | 日期 | 内容 |
|---|---|---|
| R1-R7 | 09-03/05 | 比对口径与管线修正（hex/花括号/SDDL 严格化、`?` 手术刀、分号对称化、尾冒号剥除） |
| R8 | 09-04 | Fallback Ladder 平分按 message_identifier 最小仲裁（确定性） |
| R9/R9b | 09-04 | ParamMapRefs 主分支补采 + warmup 挂载 |
| R10 | 09-07 | **TDH 类型化渲染**：`param_types` 采集（双路径）+ `apply_param_types`（ADR 0005） |
| R11 | 09-04 | 有真 map 字段跳过布尔启发式 |
| R12 | 09-07 | **源注册快照 + miss N/A parity**（ADR 0004） |
| R14/R15/R15b/R16/R17 | 09-04/05 | LUID 大写、Win32 系统错误 %%N、provider 维度隔离、MUI companion、参数引用按 provider 分组 |
| R18 | 09-07 | JSON 原生布尔参数取值（此前被吃成空串） |
| R19/R20 | 09-08 | CRIM VMAP 0xDn 字符串表家族泛化（value_maps 8540→29718）、param_name 校验挂载、bool/(8,8) 接管臂 |
| R21 | 09-29 | 经典源四修复：eid 映射（rawID & 0xFFFF）、预热跨语言回落、eid=0 FormatMessage(0)、06 canon 帐户≡账户 |
| R12b | 09-30 | N/A 精度：registered-but-no-resources（`providers_with_variants`/`has_any_variant`，空表=未知=保守 true）——CfgChangeMonitor×80 清零 |
| R21b（v9） | 09-30 | value_maps 双语化 schema v9：lang 入 PK、per-lang stringTable 主 DLL 为底 + MUI 同键全覆盖 overlay、渲染端 `load_value_maps(lang)` 双层首选+en 回退——Ntfs 98×74 清零 |
| R22 | 09-30 | classic `<Binary>` 大写 hex 追加 `%n+1`（仅模板命中路径，miss fallback 不动）——NetBT 4311×38 清零 |
| R23 | 09-30 | UserData 嵌套展平（形态门控；深度 2 裸名/深度 3 `结构体_字段` 前缀）——Resource-Exhaustion 2004×9 清零 |
| R24 | 09-30 | 大十进制 hex 化收窄：GUID-span 回扫跳过转换（WER 1001×2 清零；TS35 孤立十进制零回退） |
| R25 | 09-30 | `!S!` 字符串族 specifier 消费 + `%%` verbatim（pass2 不塌缩）——Chrome/Edge 256×10、SCM 7023×2 清零 |
| 批次 C–G | 09-14/15 | 豁免白名单+`--no-exemptions`；6013 消息表接入（netevent MUI RT_MESSAGETABLE 0x8000177D，well-known 注入）；NTSTATUS `NTSTATUS-System-Error` 全局组（ntdll 2975 条）+ (_,0x1F) hit-only 渲染臂；残余归属收口（Wininit 26 闭合） |

## 7. 残留问题（2026-09-30 终态：diff=0，唯一登记分歧=受控豁免 17 条/4 项）

- **verdict=PASS**：21 条真差异经 R26–R29 五修复全部清零（e1dexpress×8、VMnetAdapter×8、
  EventLog 6008×2、CodexSandbox×2、Wininit 23×1），修复机理见 skill 三镜像事实 #21–#23。
- **受控豁免 17 条/4 项**（ADR 0006，登记表真源 `docs/residual-zeroing-plan-2026-09-07.md` §4）：
  CAPI2 4111×1（PARSE，基线侧 CJK 日期 `?` 手术可选）、SAM 16962×8（BIN，二进制语义）、
  WPD 24577×4 / 24579×4（PARSE）。SPP 1004 与 disk 51 已于 2026-09-30 关闭出豁免。
- **后续可选项**：CAPI2 4111 的基线侧 `?` 手术（同类 R5/R28 形态扩展）可再减 1 条豁免。

## 8. 验证与测试循环（使用 `evtx-parity-test/`）

> 完整逐步手册（含每步验证点、探针速查、剩余工作项执行序列）见
> **`docs/execution-runbook.md`**。本节为摘要。

两层验证，改完代码两层都要过；迭代节奏以第二层为闭环口径。

### 8.1 层次一：代码单元测试（秒级，改上游后立即跑）

```bash
cd msgdb-dump   && go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1   # Go 侧（2026-09-30：137 用例四包全绿）
cd evtx-tools   && cargo test --workspace                       # Rust 侧（2026-09-30：246 绿）
```

覆盖：v8 schema 契约（旧 v7 拒开、缺表硬错、空表特性关）、渲染规则
（TDH 类型化、N/A parity、`%%N` 展开、LUID、值映射）、取值层（JSON Bool）。
**任何渲染修复必须先在这里加回归测试再改实现**（本仓库全部修复均按此落地）。

### 8.2 层次二：全链路闭环（`evtx-parity-test/`，分钟级）

前置一次性（基线不变则永不重跑）：`01-export`（管理员，wevtutil 导出四通道）
→ `02-baseline-parse`（evtx2text.lua 解析基线）。

每轮必跑（改了哪个上游就重跑到哪一步，比对永远最后）：

| 步 | 耗时 | 干什么 | 何时需要重跑 |
|---|---|---|---|
| `03-build-msgdb.ps1` | ~3min | go build + 重建消息库 | 改 msgdb-dump/** |
| `04-build-evtx-tools.ps1` | ~1min | cargo build --release | 改 evtx-tools/** |
| `05-new-parse.ps1` | ~4min | en-US/zh-CN 各解析一遍 → new-parsed/ | 03 或 04 之后 |
| `scripts/06-compare.py` | ~1min | 顺序对齐比对 → report.md/json/diff | 每轮必跑 |

一键等价：`.\run-all.ps1`（含 01/02）。机械细节见 `evtx-parity-test/README.md`。

### 8.3 迭代循环（一轮 = 取证 → 修复 → 闭环验证）

1. **选题**：`report.diff` / `report-exemplars.md`（分组样本，每组一条）按条数挑最大 D 簇。
2. **取证**：wevtutil `/f:xml` 看 raw 值形态；`msgdb-dump/crim-probe-intype.exe <dll> <guid> <eid>`
   读 CRIM 参数 InType/OutType；`python scripts/probe-r12-na-model.py` 式全量模型验证
   （复用 06 的解析函数，改假设参数即可复用）。**先实证再写规则，不凭记忆**。
3. **修复**：TDD——先在 8.1 层加失败测试，再改实现到绿。
4. **闭环**：重跑 03→04→05→06，对比本轮 `report.json` 与上轮：
   - 目标簇条数应清零（`by_channel.*.categories` 与 report.diff 分簇核对）；
   - **回归红线：Security 26031 全绿、Setup 全绿、B/C 类不得新增**——任何一条破线立即回退；
   - `report.json` 即下一轮的对比基线。
5. **闭合判定**：`verdict=PASS` 即闭环完成；`DIFF` 则回到第 1 步。
6. **定稿**：更新 `skills/parity-test-skill.md`（三镜像同步）的"勿回退事实"与基线表，
   ADR 层面的决策先写 `docs/adr/`。

## 9. 版本控制

- 2026-09-07 起用 **jj（Jujutsu，git 后端）monorepo** 管理整个 `E:\Code\evet`。
- 此前 `msgdb-dump/`、`evtx-tools/` 各自有独立 git/jj 仓库：吸收前已在各自仓库内
  提交最终快照，历史备份于 **`E:\Code\evet-vcs-backup\`**（仓库外，不入库）。
- 不入库（`.gitignore`）：管线大数据（msgdb/new-parsed/baseline-*/tools，约 2.3GB）、
  cargo `target/`（4.2GB）、全部 `*.exe` 构建产物、msys64 工具链本体、`.claude/` 本地配置。

## 10. 文档索引

- 术语基线：`CONTEXT.md`、`evtx-parity-test/CONTEXT.md`
- 架构决策：`docs/adr/0001`（Variant Identity）→ `0005`（TDH 类型化渲染）
- 操作手册：`evtx-parity-test/README.md`、`skills/parity-test-skill.md`（勿回退事实 #1-#23）
- 下轮工作清单：`C:\msys64\tmp\evet-handoff-2026-09-07.md`（会话交接文档）

## 9. 夜间值守（2026-10-01 晚 → 10-02 08:30，automation-bbe62e97 每 30 分钟一轮）

- R39 已提交（1be9812b）：CLI 默认 --threads 0→4（co-pool 过订阅右移）。
- Round 1（00:08）：B 项多输入摊销探针后**不做**——4 文件热跑 20.7s 中固定开销仅占
  13.5%，收益 ~10-12% < 30% 门槛（amortization 不适用本库负载形态）。
- C 项 chunk 分片进行中：C① shard.rs 遍历器已提交（d2785fb6，等价性测试实证分片流
  ==全流）；下轮 C② 有界通道并行驱动+保序归并 → C③ --shard N 开关 → C④ 基准
  vs 58.9s → C⑤ 全门禁。
- 交接锚点：evtx-parity-test/perf-run/overnight-log.md（每轮一节）；
  08:30 后值守自动写总结并自删 automation。
- Round 2（00:50）：C② 并行驱动+保序归并（074c1ff7）与 C③ --shard 开关（9acda652）已提交，
  端到端 CSV 字节级等价实证、264 测试绿、05+06 全门 PASS；**C④ 首测回归**：Security
  --shard 4 = 85.7s vs 基线 58.9s（+45%）——不设默认，下轮三连探针归因（worker 吞吐
  曲线/ParserSettings 对齐/池拓扑）。
