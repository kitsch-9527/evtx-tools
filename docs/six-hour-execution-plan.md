# 六小时持续执行计划（子代理分阶段上下文管理版）

> 版本：2026-09-08。上游：`docs/residual-zeroing-plan-2026-09-07.md`（v2 计划）、
> `docs/execution-runbook.md`（Runbook）、`docs/acceptance-criteria.md`（验收规格）。
> 起点：diff=799（97.53%）、D=30/12 簇、Security/Setup 全绿。

---

## 1. 编排模型（总则）

- **编排者（主会话）** 只做四件事：读阶段报告 → 生成了阶段简报提示词 → 派发子代理 → 核对阶段验收。**不亲自执行阶段任务**（避免主上下文膨胀）。
- **子代理（子模型）** 每阶段一个、**全新上下文**，**模型固定为 glm-5.3-flash**（派发时显式指定）。提示词 = 阶段简报（本文档对应小节全文）+ 三个必读文件路径。禁止读 handoff 全文、report.diff 全文、DB 全量等大文件。
- **为 flash 模型的设计补偿**：简报自包含、步骤原子化、验收数字明确——子代理只执行"步骤→命令→核对数字→写报告"；开放式决策（归因分叉、豁免取舍）一律按简报写死的退出条件走，或升级回编排者。
- **状态全部落盘**：阶段结束必须写 `docs/phase-reports/phase-X-report.md`（模板见 §4）——下一阶段只依赖磁盘文件，不依赖任何会话记忆。
- **失败不阻塞**：阶段内任一任务失败/超时 → 按该阶段"退出条件"登记（转豁免/转后续）→ 写报告 → 正常结束。**禁止为单簇突破时间盒**。
- **提交纪律**：每阶段一个 jj commit（可回滚、可审计）；提交前必须过该阶段的门（G1/G2 每阶段必跑）。

## 2. 上下文管理六原则

1. **必读清单制**：每阶段列出必读文件（总量 ≤300 行）与禁读清单。
2. **取数工具化**：report.diff（300KB+）只允许经脚本/SQL 取簇摘要
   （`acceptance-check.py` / 分簇脚本 / SQL 探针），禁止整读。
3. **数字落盘**：所有验收数字写入阶段报告，跨阶段传递只认报告不认记忆。
4. **单文件聚焦**：子代理的代码改动限定在简报列出的文件集；越界即停。
5. **时间盒硬性**：超时立即走退出条件，宁可登记豁免/后续，不拖尾。
6. **报告模板强制**：§4 模板字段缺一不可（编排者按字段核对验收）。

## 3. 阶段总表（6h = 8 阶段）

| 阶段 | 时间盒 | 目标 | 数字验收（硬性） | 必读 |
|---|---|---|---|---|
| A | 0:00–0:45 | WER×5 + CAPI2×1 比对层归一 | diff 799→≤793；两簇清零；红线全过 | 本档 §A；runbook §3/§5 |
| B | 0:45–1:30 | 豁免白名单进 06 + --no-exemptions + acceptance-check 集成 | 白名单命中=登记表；关豁免模式可跑 | 本档 §B；ADR 0006 |
| C | 1:30–2:30 | 6013×16 有界 research | 结论二选一：定位到源 / 转豁免登记 | 本档 §C；探针速查 |
| D | 2:30–3:30 | C 的执行（找到源→接入萃取；未找到→WPD×4 取值层取证修复） | 6013 清零 或 WPD×4 清零 | 本档 §D；C 阶段报告 |
| E | 3:30–4:15 | Ntfs 140×1 NTSTATUS 源定位（有界 30min）+ 落地或转豁免 | -1 或登记 | 本档 §E |
| F | 4:15–5:00 | 残余核对：Wininit 26 / SAM / disk / SPP 豁免登记核对 + 顺手修复 | 全部有归属（修复/豁免/后续） | 本档 §F |
| G | 5:00–5:30 | 全量验收：G1–G7 七门（含关豁免复核 + 确定性门） | acceptance-check 全 PASS ×2 | 本档 §G；验收规格 |
| H | 5:30–6:00 | 定稿：skill 三镜像 / STATUS / 计划勾销 / 最终提交 | 文档同步完成 | 本档 §H |

**缓冲**：各阶段 15% 超时余量内自行消化；累计超时 30min 由编排者裁剪 D/E 的"落地"转"登记"。

---

## §A 阶段 A 简报 — WER×5 + CAPI2×1 比对层归一

**任务**：06-compare.py 的比对归一层新增两条规则（不动上游渲染）。

1. WER `\\?\` 归一：canon 阶段把 `\\?\` 与 `\\\` 双向视为等价。
   依据：R5 live wevtapi 字节取证（skill 基线 #4/#5）——渲染器输出正确，
   基线管线吃掉了 `?`。影响面：WER 1001×5。
2. CAPI2 LRM 归一：剥离 U+200E/200F（方向标记）后再比对。
   依据：基线管线把 LRM 转成 `?`，我们保留真实 LRM。影响面：CAPI2 4111×1。
3. 两处归一必须：注释标注根因与影响条数；实现为 canon 内的字符级归一
   （与既有 `;`→空格同层）；不得影响其他簇。

**步骤**：06-compare.py 对应函数改 2 处 → `python scripts/06-compare.py` →
核对两簇清零、diff 799→≤793、红线全过 → jj commit。

**退出条件**：归一后出现意外簇变化 → 回退本次改动，登记 blocked。

---

## §B 阶段 B 简报 — 豁免白名单与开关

**任务**：把 ADR 0006 的豁免清单变成 06-compare.py 的可执行机制。

1. 06-compare.py 增加 `EXEMPTIONS`（键：通道+provider+eid）与
   `--no-exemptions` 开关（argparse 已有雏形可扩展）。
2. 开启豁免（默认）：白名单簇的 diff 条目计入 `exempted` 而非 `diffs`
   （report.json 新增 `exempted` 字段与逐条明细）。
3. `--no-exemptions`：白名单仍按 D 计入（复核口径）。
4. `scripts/acceptance-check.py` 对接：豁免命中数改读 report.json 的
   exempted 统计（替换 report.diff 解析）；G3/G6 语义不变。
5. 验收：`acceptance-check.py` 输出 exempted=26 条（7 项）；
   `--no-exemptions` 模式 diff 回到 799−前序批次收益。

**退出条件**：report.json 结构改动导致 acceptance-check 不可用 → 修脚本优先。

---

## §C 阶段 C 简报 — 6013×16 有界 research（时间盒 60min）

**背景**：EventLog 6013（系统开机时长）基线 "The system uptime is N seconds."；
我们渲染裸参数+二进制 hex。netevent.dll MUI 已实证排除。
raw 参数：`23 / 60 / -480 中国标准时间 / <UTF-16 hex 块>`。

**研究步骤**（按序，每步 ~15min，命中即停）：
1. **注册表静态源**：枚举 `HKLM\SYSTEM\CCS\Services\EventLog\System` 全部源的
   EventMessageFile，对每个 DLL/MUI 用 crim-probe（DUMP_STRINGS）grep
   "system uptime"（大小写不敏感）。
2. **全盘 message table 扫描**：对 `C:\Windows\System32` + `SysWOW64` +
   `drivers` + `en-US`/`zh-CN` 全部 PE 跑 message-table 抽取（可写一次性 Go
   脚本复用 pe 包），搜 "The system uptime is"。
3. **在线对照**：research/WebSearch "EventLog 6013 The system uptime is
   seconds message source DLL"（微软文档/常见已知源为 netevent.dll 0x4000001E
   —— 若确认 6013 走 netevent 但 MUI 无文本，记录证据链）。

**退出条件**：定位到源 DLL → 转 §D 执行接入；2.5h 预算内未定位 → 登记豁免
（根因码 SRC，关闭条件"定位到源"）→ 写报告结束。
**禁读**：report.diff 全文、全部 docs/。

---

## §D 阶段 D 简报 — 依 C 结果二选一

**D1（C 定位到源）**：接入萃取。参照 Ntfs（D1 表）/KPP（D0 表）先例：
crim-probe 确认该源的事件模板与 0xDn 表 → 6013 走既有 msgdb-dump 路径
（若源 DLL 无 WEVT_TEMPLATE，用 `--import-dll`/well-known 注入模式，
参照 `injectWellKnownMessages` 先例）→ 03→04→05→06 → 6013×16 清零。

**D2（C 未定位）**：WPD 24577/24579×4 取值层取证修复：
1. raw XML 取 WPD 事件（UserData 形态）。
2. 对照 extract_subst_params 策略命中（大概率 UserData 嵌套未覆盖）。
3. 修取值策略 + 单测 → 04→05→06 → WPD×4 清零（基线消息内含字面 `%1`，
   注意对齐基线的字面量形态）。
4. WPD 修不动 → 登记豁免（PARSE）。

**验收**：二选一有产出（清零或登记）；03-06 全链路过、红线全过。

---

## §E 阶段 E 简报 — Ntfs 140×1 NTSTATUS（时间盒 30min）

1. 探针取 Ntfs 140 `Failure status` 参数 InType/OutType（预期 out=0x1F）。
2. 基线文本 "A device which does not exist was specified." = NTSTATUS
   0xC000000E 的消息。检索本地：`C:\Windows\System32\ntdll.dll` 的消息表/
   `C:\Windows\System32\ntoskrnl.exe` MUI 中 0xC000000E。
3. 找到 → 扩展 param_types 渲染臂 (x, 0x1F)→NTSTATUS 查表（新表
   `NTSTATUS-System-Error` 照 R15 模式萃取）→ 03→04→05→06。
4. 30min 未找到 → 登记豁免（RO，"单条 ROI"）。

---

## §F 阶段 F 简报 — 残余归属核对

1. 逐簇核对剩余 D（应为：6013/WPD/Ntfs140 之外 + Wininit 26×1 等），
   每簇标注归属：已修 / 豁免（根因码）/ 后续专项。
2. Wininit 26×1（"STATUS_SUCCESS" 符号名 vs "0x0"）：需要 NTSTATUS 符号名
   表——大概率转豁免（RO）；若 §E 已建 NTSTATUS 表且含符号名，顺手接入。
3. 更新计划文档 §4 登记表与 acceptance-check.py EXEMPTIONS 表（保持同步）。
4. 预期终态：D = 登记豁免条目集（≈17-30 条），无未归属簇。

---

## §G 阶段 G 简报 — 全量验收（G1–G7）

1. G6 关豁免复核：`06-compare.py --no-exemptions` 全量 → 确认豁免条目
   恰好等于登记集，无额外掩盖。
2. 确定性门：重跑 05+06 一次，两次 report.json 的 diff/matched 完全一致
   （防 HashMap 序类不确定性，先例 R8）。
3. `python scripts/acceptance-check.py --prev report.prev.json` → 全 PASS。
4. 红线复核结论写入提交信息。

---

## §H 阶段 H 简报 — 定稿

1. `skills/parity-test-skill.md` 三镜像：新"勿回退"事实（本日批次 3/4）。
2. `STATUS.md` 数字更新；计划文档勾销完成项。
3. handoff 文档更新（残留/豁免/后续专项）。
4. 最终 jj commit（含验收记录模板填写的 G1–G7 结果）。

---

## 4. 阶段报告模板（`docs/phase-reports/phase-X-report.md`）

```markdown
# Phase <X> 报告（<日期时间>）
## 结论
- 验收数字：<diff/matched/簇清零清单>
- 门：G1 ✓/✗  G2 ✓/✗（红线复核结论）
## 完成
- <任务 → 结果>
## 登记/转出
- <簇 → 豁免(根因码)/后续专项>
## 未决与建议
- <发现、坑位、给下一阶段的提示>
## 变更文件
- <清单>（jj change id：<id>）
```

## 5. 子代理派发提示词模板（编排者用）

> 整个计划的**会话入口提示词**（编排者角色、启动核对、派发循环、硬规则、收尾）
> 已独立成文：`docs/orchestrator-prompt.md`——新会话原样粘贴该文件分隔线内内容即可。
> 本节是**单阶段**派发模板（编排者在派发每个子代理时使用）。

```
你是阶段 <X> 的执行代理。严格按以下简报执行，时间盒 <N> 分钟：
<粘贴本文档 §X 全文>
必读文件（只读这些）：
- docs/execution-runbook.md §3、§5
- docs/acceptance-criteria.md
- docs/phase-reports/<上一阶段>-report.md
禁读：handoff 全文、report.diff 全文、docs/ 其他文件。
完成后：写 docs/phase-reports/phase-X-report.md（按模板），jj commit，
报告 jj change id 与验收数字。超时或受阻：按退出条件登记并写报告。
```
