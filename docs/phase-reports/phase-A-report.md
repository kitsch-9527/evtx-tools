# Phase A 报告（2026-09-08 23:23）

## 结论

- **未达**：目标簇未清零，diff 799 → 799（目标 ≤793 不成立），matched 32002 / compared 32801（M1 97.5641%）。
- **根因：简报诊断前提与当前数据不符（实测证伪），详见"未决与建议"。** 按受阻路径登记，未硬扛。
- 门：G1 ✓（M1 97.5641% ≥ 97.53%；M2 91.38% ≥ 80%硬门） G2 ✓（Security diff=0、Setup diff=0、四通道 B/C 零新增：B 74→74/0→0/0→0/644→644，C 11→11/0/0/40→40、缺失类=0；红线复核全过）
- G3 ✓（豁免命中 7 项/26 条与登记表一致；M5 有效分歧 ≈773）。acceptance-check 总体 **PASS**，exit=0。

## 完成

- WER `\\?\` 归一（canon 内字符级，`\\?\`≡`\\\` 双向，注释含根因与实测影响条数）→ **已落地，实测影响面=0**：当前数据中基线侧 `\\?\` 完整未被打掉，无可清条目。
- CAPI2 LRM 归一（canon 内剥离 U+200E/200F，注释含根因与影响条数）→ **已落地，实测清零失败（1 条仍在）**：仅剥我方 LRM 后，基线侧被管线打成 `?` 的位置（`?2026?年?8?月?25?日`，6 处与 LRM 一一对应）无法对齐；剥 LRM 只是必要条件不充分。
- `python scripts/06-compare.py` 重跑 → compared=32801 matched=32002 diff=799，两簇条数零变化，红线全过（对照改动前 dry-run 预测一致：`scripts/probe-phaseA-dryrun.py` 对 30 条 D 类逐条验证两条规则清零数为 0）。

## 登记/转出

- **WER 1001 → 转后续专项（上游渲染，非比对层）**：当前实为 **WER 1001×2**（非×5），残留根因是渲染器把文件名中 GUID 尾段十进制串 hex 化——基线（wevtapi live）`WER.c1438389-d2df-48d1-a9f7-ae3003916586.tmp.xml` vs 我方 `…ae0xb30c212a.tmp.xml`（3003916586==0xb30c212a 实证）。属 format_for_display 保护名单/取值 hex 化范畴（TS35 同域），阶段 A 授权禁改上游。App #2972、#3429 两条。
- **CAPI2 4111 → 维持既有豁免（PARSE，登记表已有）或转专项**：彻底清除需在 R5 层（`clean_baseline_artefacts`）补基线侧 `?` 手术（CJK 日期形态 `?\d+…[年月日]`，镜像既有 `?YYYY?-?MM?-?DDT` 规则），超出本阶段"canon 内剥 LRM"的授权；且清零后须同步 EXEMPTIONS 登记表与 residual plan §4（同一提交），涉及本阶段禁读文档，须编排者决策。

## 未决与建议

- **简报锚点 stale 的证据链**：runbook §6.4"WER 1001×5"写于 TS35（xtwqtmwqnkzp）之前；TS35 之后当前实测 WER 1001 仅 2 条 diff 且 `\\?\` 双侧完整——推测 TS35 前后上游已修掉 `?` 吞吃、同时暴露/遗留了 hex 化新形态。下一阶段改簇前应先用 dry-run 探针实测影响面，勿直接信计划条目。
- 坑位：report.diff 每条 Msg 截断 500 字，WER 差异在 ~1007 字处，抽样看不到真因；用 importlib 复用 06 解析函数做全消息 canon 比对定位（runbook §4 模式）。
- 两条 canon 规则虽当前零命中，仍按简报落地并留注释：LRM 剥离无损（方向标记无语义），`\\?\`≡`\\\` 双向等价防御性正确；后续上游修复后若再现该形态可自动兜底。若编排者倾向零死代码，可 `jj restore evtx-parity-test/scripts/06-compare.py` 一键摘除，不影响其他任何簇。
- CAPI2 若走清除路线，建议实施点在 `clean_baseline_artefacts`（R5 同层手术刀）而非 canon，避免 `?` 全局剥除误伤（R5 已证 `?` 在 SPP1003 `[(?)(?)]`、WER `??` 等处为真实语义）。
- WER 专项提示：先对 `3003916586` 所在参数探针（crim-probe-intype）确认 InType/OutType 与模板形态，再决定保护名单加臂还是取值层修。

## 变更文件

- `evtx-parity-test/scripts/06-compare.py`（canon_msg 新增两条归一规则 + 实测注释）
- `evtx-parity-test/scripts/probe-phaseA-dryrun.py`（新增：阶段 A dry-run 探针，只读复算两规则影响面）
- `evtx-parity-test/report.{md,json,diff}`（06 重跑再生成，数字与改前一致，仅时间戳变）
- `docs/phase-reports/phase-A-report.md`（本报告）

（jj change id：nkymvymr）

== 验收记录 · Phase A · 2026-09-08 ==
jj 变更：nkymvymr（fd8c157f）
G1: M1=97.5641%（目标 ≥97.53%）｜ M2=91.38%（硬门 ≥80%）
G2: Security=0 / Setup=0 / 四通道 B/C 不新增 / 无缺失
G3: 豁免命中 7 项/26 条（SPP1004×1, CAPI2 4111×1, SAM16962×3, disk51×1, 6013×16, WPD24577×2, 24579×2）
G5: 目标簇清零：WER 1001 ✗（实为×2，根因上游 hex 化）、CAPI2 4111 ✗（需基线侧 ? 手术，超授权）→ 登记 blocked
红线结论：全过（零簇变化，未触发回退条件；阶段目标未达按受阻登记）
