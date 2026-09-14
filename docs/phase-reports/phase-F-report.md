# Phase F 报告（2026-09-15 00:40）

## 结论

- **验收数字**（report.json 口径，与 E 阶段锚点一致，本轮零管线改动）：
  - compared 32801 / matched 32020（**M1 = 97.6190%**）/ diff 771。
  - **D 残留 2 条（M2 = 99.43%）**，逐簇归属：
    | 簇 | 条数 | 归属 |
    |---|---|---|
    | Application / Windows Error Reporting 1001（App #2972） | 1 | **后续专项**（上游渲染器 hex 化，阶段A 2026-09-08 实证） |
    | Application / Windows Error Reporting 1001（App #3429） | 1 | **后续专项**（同上） |
  - **Wininit 26×1 → 已修（非豁免）**：实测不在当前 diff。简报前提（基线 "STATUS_SUCCESS" 符号名 vs 我方 "0x0"、需离线符号名表）**已过时**——阶段E 的 NTSTATUS 文本表 code 0 的字面文本就是 `STATUS_SUCCESS`（`parameter_refs` 组 `NTSTATUS-System-Error`：0→STATUS_SUCCESS、1→STATUS_WAIT_1…，ntdll 表低段本就是符号形文本），Wininit 26 的 LocalStatus（in 0x14 / out 0x1F）经阶段E `(_,0x1F)` 渲染臂查表命中后与基线逐字符一致（双侧均为 "WNF Published with result: STATUS_SUCCESS"，2026-09-15 实测 new-parsed/en/System.csv vs baseline-parsed/System.evtx.txt）。**无需 RO 豁免，无需离线符号清单。**
  - exempted **10 条 / 6 项**（SPP1004×1 PARSE、CAPI2 4111×1 PARSE、SAM16962×3 BIN、disk51×1 SRC、WPD24577×2 PARSE、WPD24579×2 PARSE），全部豁免（根因码见括号）。
- **门：G1 ✓**（M1 97.6190% ≥97.53%） **G2 ✓**（Security/Setup 全绿、四通道 B/C 零新增、无缺失类） **G3 ✓**（命中 6 项/10 条与登记表一致；登记未命中 0；M5 = 771−10 = 761 单调不升）。
  `acceptance-check.py --prev report.prev.json` 总体 PASS，**exit=0**。

## 完成

- 逐簇核对（程序化取数：report.diff 只取簇头/计数、report.json 摘要、CSV 单行抽查，未整读 diff）：
  1. **D 残留全量 = WER 1001×2**（#2972/#3429），与阶段A 归因一致（渲染器把 WER 报告文件名 GUID 尾段十进制串 hex 化，`3003916586`→`0xb30c212a`，TS35 同域）→ 归属**后续专项**，登记入 §4。
  2. **Wininit 26×1**：全 diff 扫描（provider/消息文本双关键词）零命中 → 已修；机制探针：raw XML `LocalStatus=0x0`、msgdb `param_types` (in 20, out 31)、阶段E NTSTATUS 臂 → `STATUS_SUCCESS`。简报"转豁免（RO）"路径**不需要**，按实测登记为已修。
  3. **豁免 6 项/10 条**逐项与登记表核对一致；发现 §4 登记表**缺 CAPI2 4111 行**（EXEMPTIONS 两处有、真源表漏）——本阶段补齐。
- **EXEMPTIONS 两处（acceptance-check.py / 06-compare.py）零改动**：Wininit 已修无需入表，WER 转专项非豁免——登记表与脚本本就一致，无同步负担。

## 登记/转出

- `docs/residual-zeroing-plan-2026-09-07.md` §4（2026-09-15 阶段F，仅动相关行）：
  - **新增** WER 1001×2 行：专项（非豁免），根因=上游 hex 化（阶段A 实证），关闭条件=上游渲染器修复。
  - **新增** CAPI2 4111×1 行（PARSE）：补真源表与 EXEMPTIONS 的同步缺口。
  - **新增** Wininit 26 行（划线样式）：已修（2026-09-15 阶段F 核实），注明命中机制。
  - **注记** WPD 24577/24579：条件性→已转正（核实命中 24577×2+24579×2）。
- **无未归属簇确认**：D 残留 2 条（WER×2，后续专项）+ 豁免 10 条（有码有条件）全部有归属；G3 命中数与登记表一致。

## 未决与建议

- **给 G（全量验收）**：
  1. G6 关豁免复核（`06-compare.py --no-exemptions` 生成 report 后 `acceptance-check.py --no-exemptions`）按 ADR 0006 批次终验必跑一次；届时豁免 10 条将以 D 类回到 diff（预期 771+10=781 口径），WER×2 本就未豁免、不受影响。
  2. WER 1001×2 是唯一非豁免残留：上游修复落地后即自然清零，不动比对层；批次报告请按"后续专项"口径披露（D 闭环率 99.43%，346/348）。
  3. NTSTATUS 符号名仅低段有字面文本（0→STATUS_SUCCESS、1..63→STATUS_WAIT_n 等约十余条）；高位码（如 0xC000000E）只有英文消息文本、无符号名——若未来簇需要符号名形态仍需离线清单，当前无簇依赖，不构成阻塞。
  4. `report.prev.json` 仍是 E 前旧快照（diff 799）；G 阶段如需 M5 严格单调链，建议先 `cp report.json report.prev.json` 刷新基线。
- 风险：无。本轮未动任何代码与管线产物，红线天然全过。

## 变更文件

- `docs/residual-zeroing-plan-2026-09-07.md`（§4 登记表：WER 专项行、CAPI2 补行、Wininit 已修划线行、WPD 转正注记）
- `docs/phase-reports/phase-F-report.md`（本报告）
- `scripts/acceptance-check.py` / `scripts/06-compare.py`：**零改动**（EXEMPTIONS 无条目变化，两处天然同步）

== 验收记录 · Phase F · 2026-09-15 ==
jj 变更：qtzrlunuxvlv
G1: M1=97.6190%（目标 ≥97.53%）｜ M2=99.43%（硬门 ≥80%，D 残留 2/348）
G2: Security=0 / Setup=0 / 四通道 B/C 不新增 / 无缺失
G3: 豁免命中 6 项/10 条（SPP1004×1, CAPI2 4111×1, SAM16962×3, disk51×1, WPD24577×2, 24579×2）；登记表 §4 已与命中同步（补 CAPI2 行）
G5: D 残留逐簇归属：WER 1001×2→后续专项（阶段A 归因）；Wininit 26×1→已修核实（阶段E NTSTATUS 臂命中 code 0 字面文本 STATUS_SUCCESS）；无未归属簇
红线结论：全过（零代码改动，acceptance-check exit=0）

（jj change id：qtzrlunuxvlv）
