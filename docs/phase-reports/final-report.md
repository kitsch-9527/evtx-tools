# 六小时持续执行计划 · 最终执行报告

> **✅ 全部 8 阶段已完成（2026-09-15，C–H 于夜间心跳续跑完成）**

- 执行日期：2026-09-08 23:04:31 启动（编排者会话，模型 glm-5.3-flash）
- 实际执行窗口：23:04–23:32 + 次日 08:15–08:55（夜间遭平台限流与会话中断，跨夜续跑；
  硬停止 2026-09-09 08:55）+ **2026-09-14/15 夜间心跳续跑（C–H）**。
- 阶段完成度：**A/B 于 09-08 完成；C–H 于 09-14/15 夜间心跳续跑完成——8/8 全部完成**。

---

## 1. 各阶段验收数字

### 阶段 A — WER×5 + CAPI2×1 比对层归一（✗ 目标未达，按退出条件登记 blocked）

- 两条 canon 归一规则已落地（`\\?\`≡`\\\` 双向、剥离 U+200E/200F），但 **dry-run 探针与
  全量重跑均实测影响面=0**：diff 799→799，两簇未清零（目标 ≤793 不成立）。
- 根因：**简报诊断前提与当前数据不符（实测证伪）**——
  - WER 1001 当前实为 **×2（非×5）**，`\\?\` 双侧完整；真因是上游渲染器把文件名 GUID
    尾段十进制串 hex 化：`3003916586` → `0xb30c212a`（实证相等）。属上游取值层
    （TS35 同域），超出阶段 A"只动比对层"授权 → **转后续专项**（App #2972/#3429 两条）。
  - CAPI2 4111 剥 LRM 只是必要条件：基线侧 LRM 已被管线打成 `?`
    （`?2026?年?8?月?25?日`，6 处与 LRM 一一对应），彻底清除需 R5 层
    `clean_baseline_artefacts` 补基线侧 CJK 日期 `?` 手术，超本阶段授权 → 维持既有
    豁免（PARSE）或转专项。
- 门：G1 ✓（M1 97.5641%、M2 91.38%）G2 ✓（红线全过）G3 ✓（豁免 7 项/26 条一致）。
- 两条零命中规则按简报保留（防御性正确、注释含根因；可一键摘除，不影响其他簇）。
- jj：`nkymvymr`（归一落地+报告）+ `opwvsqrm`（报告收尾）。

### 阶段 B — 豁免白名单与开关（✓ 全部达成）

- `06-compare.py` 内置 EXEMPTIONS 表（与 acceptance-check.py 同内容、同一提交）+
  `--no-exemptions` 开关 + `apply_exemptions()` 命中判定前移；report.json 新增
  `exempted`/`exempted_detail`/`whitelist_diff_hits` 等字段与分通道 exempted。
- `acceptance-check.py`：G3 数据源改读 report.json（替换 report.diff 解析）、
  新增 G6 关豁免复核分支；G3/G6 语义不变。
- 验收数字：
  - 默认口径：compared=32801, matched=32002, M1=97.5641%，**diff=773，
    exempted=26 条/7 项**（与登记表逐簇一致、未命中 0、白名单外命中 0），
    **M2=98.85%（D 残留 4/348）**，M5 有效分歧 ≈747。
  - `--no-exemptions`：**diff=799**、exempted=0，白名单 26 条全额按 D 计入
    （M2=91.38%），G6 ✓。
- 门：两模式 acceptance-check 均 **PASS，exit=0**；红线零 FAIL。
- jj：`zmuvrlrp`。

### 阶段 C–H（✅ 已于 2026-09-14/15 夜间心跳续跑完成，逐阶段结论）

| 阶段 | 结论 | 报告 |
|---|---|---|
| C | **定位到源**：6013 消息在 netevent.dll MUI RT_MESSAGETABLE（type 11），ID 0x8000177D（DUMP_STRINGS 只扫 type 6，此前 0 命中系假证据） | phase-C-report.md |
| D | **6013 清零**：`injectWellKnownVariants` 实装接入真实表文本，16 条全 matched；6011 假条目移除 | phase-D-report.md |
| E | **Ntfs 140 清零 + 超收**：ntdll.dll RT_MESSAGETABLE 2975 条入 `NTSTATUS-System-Error` 全局组，渲染臂 (_,0x1F) hit-only | phase-E-report.md |
| F | **归属收口**：Wininit 26 闭合（STATUS_SUCCESS 经 NTSTATUS 臂），豁免收口 6 项/10 条，登记表更新 | phase-F-report.md |
| G | **全量验收 G1–G7 全过**：M1=97.6190%、M2=99.43%、确定性门三度全等、关豁免 781=771+10 无掩盖、exit=0 | phase-G-report.md |
| H | **定稿**：skill 三镜像勿回退事实、STATUS 终态数字、计划勾销、handoff/final-report 同步、最终提交 | phase-H-report.md |

## 2. 终态锚点（2026-09-15 阶段 G/H 实测，后续工作以此为基线）

- **默认口径（豁免开启）**：diff=771、matched=32020/32801（**M1=97.6190%**，超理想线 97.59%）、
  D 残留 2/348（**M2 99.43%**）、exempted=10 条/6 项、M5=761。
- **关豁免口径**：diff=781=771+10（白名单全额按 D 计入，exempted=0），acceptance-check 亦 PASS。
- 红线：Security/Setup diff=0、四通道 B/C 零新增（B 74/0/0/644，C 11/0/0/40）、
  缺失类=0；单测 Rust 225 绿 + Go 125 绿。
- D 残留归属：WER 1001×2（App #2972/#3429，上游 hex 化专项）；无未归属簇。
- `report.prev.json` 为 E 前旧快照（diff=799）——G2 红线即以该口径执行且全过；
  如需严格 prev 链可 `cp report.json report.prev.json` 刷新。
- EXEMPTIONS 双表（06-compare.py / acceptance-check.py）自 B 起同提交同步；
  登记表唯一真源 residual plan §4（F 已核对一致）。

## 3. 红线复核结论

**全过。** 每阶段提交前均过 acceptance-check（A/B 两阶段共 4 次运行，含关豁免口径），
编排者终验复跑 `acceptance-check.py --prev report.prev.json` 全门 PASS（exit=0）：
Security=0、Setup=0、B/C 不新增、无缺失类 diff、豁免命中与登记表一致。无破线回退。

## 4. 未决与后续专项清单

1. **WER 1001×2 上游 hex 化专项**（阶段 A 转出）：实施点在 format_for_display
   保护名单/取值层（TS35 同域）；先 crim-probe 探 `3003916586` 所在参数的
   InType/OutType 与模板形态再定方案。涉及 App #2972/#3429。
2. **CAPI2 4111×1 基线侧 `?` 手术**：`clean_baseline_artefacts` 增加 CJK 日期形态
   `?\d+…[年月日]` 规则（镜像既有 `?YYYY?-?MM?-?DDT` 先例）；注意 `?` 在
   SPP1003 `[(?)(?)]`、WER `??` 等处为真实语义，勿全局剥除；清零后须同一提交同步
   EXEMPTIONS 双表与登记表。
3. **6013×16 research（C→D 链）**：netevent.dll MUI 已实证排除；按 §C 三步法执行，
   预算 2.5h；未定位则登记豁免（SRC，关闭条件"定位到源"）。
4. **Ntfs 140×1 NTSTATUS（E）**：预期 out=0x1F；`A device which does not exist was
   specified.` = 0xC000000E；新表 `NTSTATUS-System-Error` 照 R15 模式萃取。
5. **Wininit 26×1（F）**：需 NTSTATUS 符号名表（"STATUS_SUCCESS" vs "0x0"）；
   若 §E 建表含符号名可顺手接入，否则转豁免（RO）。
6. **确定性门（G）**：重跑 05+06 两次，比对 report.json diff/matched 完全一致
   （防 HashMap 序不确定性，R8 先例）。
7. **文档同步（H）**：skill 三镜像新"勿回退"事实（本日批次：canon 双规则零命中保留、
   豁免机制落地）；STATUS.md 终态数字；六小时计划与 residual plan §4 勾销/登记核对。
8. **文档 stale 修正**：acceptance-criteria.md M4 数据源描述（实际已改读
   report.json）；execution-runbook.md §6.4 "WER 1001×5" 锚点（当前实为×2，且根因
   已变为上游 hex 化）。**经验教训：改簇前先跑 dry-run 探针实测影响面，勿直接信
   计划条目**（本次 A 阶段简报前提即被证伪）。

## 5. 续跑指引

- 入口：`docs/orchestrator-prompt.md`（原样粘贴）→ 启动核对会发现第一个无报告阶段为
  **C**（phase-A/phase-B 报告已在 `docs/phase-reports/`）。
- 派发 C 时必读清单加：`docs/phase-reports/phase-B-report.md`（豁免机制使用方式）。
- 已知坑位：Runbook §3.3（03 重建期间禁止并发 05；03 输出 GBK）；report.diff 每条
  Msg 截断 500 字（深差异用 importlib 复用 06 解析函数做全消息比对，勿抽样下结论）。
