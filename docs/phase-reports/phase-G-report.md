# Phase G 报告（2026-09-15 00:50）

## 结论

- **验收数字**（report.json 默认口径，与 F/E 锚点一致）：
  - compared 32801 / matched 32020（**M1 = 97.6190%**，超理想线 97.59%）/ diff 771。
  - **确定性门 PASS**：重跑 05+06 一次，两次 report.json 的 diff/matched/compared/exempted/verdict 五元组完全相等（771 / 32020 / 32801 / 10 / DIFF），且 by_channel 分类计数 JSON 深比对全等（防 HashMap 序类不确定性，先例 R8）；更强证据：report.diff 字节级未变（jj 未标记修改），report.json/md 全文仅 `generated_local` 时间戳一行之差。
  - **G6 关豁免口径：diff = 781 = 771 + 10**，exempted=0；白名单 10 条全额以 D 类回到 diff，增量按通道逐项对应登记集（Application D 2→4 = SPP1004×1 + CAPI2 4111×1；System D 0→8 = SAM16962×3 + disk51×1 + WPD24577×2 + WPD24579×2）；B/C 不动（App 74/11、Sys 644/40）——**无意外掩盖**。
  - D 残留 **2 条**（WER 1001×2，后续专项已登记）；exempted **10 条 / 6 项**。
- **门：G1 ✓ G2 ✓ G3 ✓ G4 ✓ G5 ✓ G6 ✓ G7 ✓**——`acceptance-check.py --prev report.prev.json` 总体 PASS、**exit=0**；`--no-exemptions` 复核口径亦 PASS、exit=0。
- **红线复核结论：全过**（Security diff=0、Setup diff=0、四通道 B/C 不新增、无缺失类；零回退）。

## 完成

- **G1 指标门 ✓**：M1=97.6190%（32020/32801，下限 97.53%，已达/超理想线 97.59%，距理想线 ≈0.00pp）；M2=99.43%（D 残留 2/348，硬门 80%）。
- **G2 回归红线门 ✓（一票否决项）**：以现有 prev 旧快照（diff=799，E 前）为基线——Security 0、Setup 0；Application B 74→74 / C 11→11、Setup 0→0、Security 0→0、System B 644→644 / C 40→40（零新增）；缺失类 diff=0。
- **G3 豁免核对门 ✓**：命中 **6 项/10 条**与登记集逐项一致（SPP1004×1 PARSE、CAPI2 4111×1 PARSE、SAM16962×3 BIN、disk51×1 SRC、WPD24577×2 PARSE、WPD24579×2 PARSE）；登记未命中 0 条；M5 = 771−10 = 761，对 prev 口径 799−26=773 单调不升。
- **G4 理想线披露 ✓**：距理想线 97.59% 还差 ≈0.00pp（M1 已达理想线上方）；残余分歧 = WER 1001×2（D，专项）+ B 类 718 + C 类 51（6013×16 等无模板/裸参数簇），无静默。
- **G5 簇核对门 ✓**：D 残留全量恰为 **WER 1001×2**（Application #2972 / #3429，簇头+Provider 程序化核对），归属**后续专项**（上游渲染器 hex 化，阶段A 实证）；无未归属簇、无新簇。
- **G6 关豁免复核门 ✓**：`06-compare.py --no-exemptions` 全量 → 781=771+10 严格成立，逐通道增量与登记集一一对应；`acceptance-check.py --no-exemptions` 报"白名单 10 条全额计入 D、登记未命中 0"、总体 PASS（exit=0）。**复核后已重跑默认模式 06，report.{md,json,diff} 已恢复默认口径**（771/32020/exempted=10 复核无误）。
- **G7 单测门 ✓**：Rust `cargo test` **225 绿**（bin 29 + core 193 + doctest 3，与 E 阶段 225 绿记录一致，1 ignored）；Go `go test ./...` **全包绿 125 个用例**——修复 `msgdb-dump/crim-probe-566/main.go` 缺失右括号（init 提交带入的探针残留，原 `go test ./...` 编译失败，仅探针包、不在管线内，一行修复）。
- **确定性门 ✓**：见结论；第三次运行（G6 后恢复默认口径的重跑）同样复现 771/32020，三度一致。

## 登记/转出

- **无未决项**：豁免 6 项/10 条命中与登记表（`docs/residual-zeroing-plan-2026-09-07.md` §4）完全一致；D 残留 2 条归属后续专项（已登记）；本轮零登记表变更、零 EXEMPTIONS 变更。
- **`report.prev.json` 未刷新**（遵交接指令）：仍为 E 前旧快照（diff=799/matched=32002），本轮 G2 以该口径执行且全 PASS；M5 单调链以 799−26=773 → 761 记录。后续轮次如需严格 prev 链，可自行 `cp report.json report.prev.json`。

## 未决与建议

- **给 H（收尾/定稿）**：
  1. 上游 WER 1001 hex 化修复落地后，D 残留自然清零（M2 → 100%），比对层不动。
  2. `crim-probe-566/main.go` 的括号修复随本提交入库；probe 目录无测试文件，仅参与编译门。
  3. `report.prev.json` 为旧快照（799），定稿前建议刷新一次以固化最终 B/C 对比线。
  4. G6 关豁免数字（781）与默认口径（771）已双留档于本报告与 acceptance-check 输出，后续轮次勿混淆两种口径。
- 风险：无。管线代码（evtx-tools / msgdb-dump 主库 / 06-compare / acceptance-check）本轮零改动，确定性三度复现。

## 变更文件

- `msgdb-dump/crim-probe-566/main.go`（补缺失 `}`，使 `go test ./...` 可编译、全绿）
- `docs/phase-reports/phase-G-report.md`（本报告）
- `evtx-parity-test/report.{md,json,diff}`：默认口径重跑产物（数字与 F 锚点一致，确定性门产物）
- `scripts/acceptance-check.py` / `scripts/06-compare.py`：**零改动**

== 验收记录 · Phase G · 2026-09-15 ==
jj 变更：rosuotlm
G1: M1=97.6190%（32020/32801，目标 ≥97.53%，超理想线 97.59%）｜ M2=99.43%（硬门 ≥80%，D 残留 2/348）
G2: Security=0 / Setup=0 / 四通道 B/C 不新增（App 74/11、Setup 0、Security 0、Sys 644/40）/ 无缺失（基线 prev=799 旧快照）
G3: 豁免命中 6 项/10 条（SPP1004×1, CAPI2 4111×1, SAM16962×3, disk51×1, WPD24577×2, WPD24579×2）；登记未命中 0；M5=761 单调不升
G5: D 残留逐簇归属：WER 1001×2（#2972/#3429）→后续专项（上游 hex 化）；无未归属簇、无新簇
G6: 关豁免复核 diff=781=771+10，白名单 10 条全额计入 D、增量按通道逐项对应，无意外掩盖；复核后已恢复默认口径 771
G7: 单测 Rust 225 绿 + Go 125 绿（crim-probe-566 补括号后 go test ./... 全绿）
确定性门：重跑 05+06，两次 report.json diff/matched/compared/exempted/verdict + by_channel 深比对全等
红线结论：全过（acceptance-check --prev exit=0；--no-exemptions 亦 exit=0）

（jj change id：rosuotlmktwkwprzwxtptommywzpzquo）
