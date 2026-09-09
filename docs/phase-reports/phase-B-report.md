# Phase B 报告（2026-09-09 08:30）

## 结论

- **达成**：ADR 0006 豁免白名单落地为 06-compare.py 可执行机制，豁免命中判定前移进比对器，acceptance-check 改读 report.json，不再解析 report.diff。
- 验收数字：
  - 默认口径（豁免开启）：compared=32801，matched=32002（M1 97.5641%），**diff=773，exempted=26 条 / 7 项**（SPP1004×1, CAPI2 4111×1, SAM16962×3, disk51×1, 6013×16, WPD24577×2, 24579×2，与登记表逐簇一致）。
  - `--no-exemptions`（G6 复核口径）：**diff=799**（白名单 26 条全额按 D 计入），exempted=0，M1/M2 与基线一致（97.5641% / 91.38%）。
- 门：G1 ✓（M1 97.5641% ≥ 97.53%；M2 默认口径 98.85%、复核口径 91.38%，均 ≥ 80% 硬门） G2 ✓（Security diff=0、Setup diff=0、四通道 B/C 零新增：B 74→74/0→0/0→0/644→644，C 11→11/0/0/40→40、缺失类=0；红线复核全过，未触发回退）
- acceptance-check 总体 **PASS**，两模式 exit 均=0。

## 完成

- 06-compare.py：新增 `EXEMPTIONS` 表（键=通道+provider+eid，值=根因码，7 项，与 acceptance-check.py 同一提交内容一致）+ `--no-exemptions` 开关；默认口径下白名单命中的 D 类条目移出 diffs 计入 `exempted`（report.json 新增顶层 `exemptions_enabled` / `exempted` / `exempted_detail` / `whitelist_diff_hits` / `whitelist_diff_detail` 与分通道 `exempted`）；report.md 头部披露豁免开关状态与命中数。
- scripts/acceptance-check.py：G3 豁免命中数据源由 report.diff 逐簇解析改为 report.json 的 `exempted_detail`/`exempted` 分簇汇总（`parse_diff_clusters` 删除，`--diff` 参数移除）；新增 G6 关豁免复核分支（读 `whitelist_diff_detail` 核对白名单 7 项/26 条全额以 D 计入）；G3/G6 语义不变（证据型门，不新增 FAIL 条件）。
- 验证：默认模式 `python scripts/acceptance-check.py --prev report.prev.json` → 全门 PASS、exit=0、exempted=26 条/7 项、M5 有效分歧 ≈747；关豁免 `python scripts/acceptance-check.py --no-exemptions` → PASS、exit=0、diff=799。已提交的 report.{md,json,diff} 为默认口径产物（06 最后重跑恢复）。

## 登记/转出

- 无新增登记/转出。7 项豁免均为既有登记（PARSE×4 / SRC×2 / BIN×1），本阶段只做机制落地，未改登记内容；residual plan §4 无需同步变更。

## 未决与建议

- `docs/acceptance-criteria.md` M4 数据源仍写 "report.diff × 登记表"，阶段 B 后实际数据源为 report.json `exempted` 统计——文档待下次计划变更时顺手更新（本阶段按授权未动 docs 其他文件）。
- G3 目前仍是证据型门（恒 PASS、异常进证据串）；report.json 结构化后已具备收紧条件：可加"命中簇 ⊆ 登记表"与"M5 轮间单调下降"两条硬判定，建议下一阶段连同 M4 口径一起做。
- 坑位：`acceptance-check.py --no-exemptions` 必须搭配 `06-compare.py --no-exemptions` 生成的 report.json 使用（脚本 help 已注明）；验收后记得用默认模式重跑 06 恢复 report.* 产物，否则常规口径 G3 读到 exempted=0。
- `whitelist_diff_hits` 两种口径都统计（26/26），后续轮次若该数偏离 26 即白名单簇发生了新变化，可直接作为 G3 人工复核触发器。

## 变更文件

- `evtx-parity-test/scripts/06-compare.py`（EXEMPTIONS 表 + --no-exemptions + apply_exemptions + report.json/md 豁免字段）
- `evtx-parity-test/scripts/acceptance-check.py`（G3 改读 report.json、新增 G6 分支、移除 report.diff 解析）
- `evtx-parity-test/report.{md,json,diff}`（默认口径再生成：diff 799→773，exempted=26；report.diff 不再含白名单条目）
- `docs/phase-reports/phase-B-report.md`（本报告）

（jj change id：zmuvrlrp）

== 验收记录 · Phase B · 2026-09-09 ==
jj 变更：zmuvrlrp（b1077ab7）
G1: M1=97.5641%（目标 ≥97.53%）｜ M2=98.85% 默认 / 91.38% 复核（硬门 ≥80%）
G2: Security=0 / Setup=0 / 四通道 B/C 不新增 / 无缺失
G3: 豁免命中 7 项/26 条（数据源 report.json；SPP1004×1, CAPI2 4111×1, SAM16962×3, disk51×1, 6013×16, WPD24577×2, 24579×2）
G6: 关豁免复核 diff=799，白名单 26 条全额按 D 计入，无意外分歧被掩盖
红线结论：全过（两模式均未触发回退条件）
