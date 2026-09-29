# Night P8 报告 — 定稿与文档同步（2026-09-30 03:14→03:40 前后）

- 基线：P7 终态（diff=21、matched=44852/44890、M1=99.9153%、baseline_lang=zh-CN、acceptance PASS exit=0）
- 时间盒：60 分钟内完成全部四步，未触边界
- 性质：纯文档改动 + EXEMPTIONS 双表同步（本轮唯一授权动豁免表处）；**零渲染/萃取代码改动、未提交（由编排者 jj 统一提交）**

## 结论

**验收全过：skill 三镜像 md5 一致；EXEMPTIONS 摘除 2 条零命中项后全链路复核 diff=21 不变、总体 PASS exit=0、G3 命中 4 项/17 条且登记未命中 0 条（原 2 条）→ 不回滚，豁免表定稿为 4 项。**

- 终态锚点维持：diff=21（App 2 + System 19）、matched=44852/44890、M1=99.9153%、M2=93.97%、
  M5≈4、exempted=17 条/4 项、baseline_lang=zh-CN、B/C 全场 0
- 单测：Go 134（四包；全仓 135）+ Rust 245 绿（P7 已实测，本阶段无代码改动不重跑）

## 取证

1. P8 前核对 `report.json`：exempted_detail/whitelist_diff_detail 各 17 条 = CAPI2 4111×1 +
   SAM 16962×8 + WPD 24577×4/24579×4——**SPP 1004 与 disk/51 均零命中**（摘除前置条件成立）。
2. 简报预期口径勘误（登记不回滚）：简报写"预期 exempted=0、whitelist_diff_hits=4"，与同句
   "保留 CAPI2/SAM/WPD 四项"矛盾——四项保留则必然命中 17 条（CAPI2×1+SAM×8+WPD×8），
   exempted=0 只在 G6 `--no-exemptions` 口径下成立。实际验证以**语义一致判据**执行：
   diff=21 不变 + 总体 PASS + 白名单恰 4 项且全部命中 + 登记未命中 0 条 + 21 条转出簇零命中
   白名单——全部满足，故保留改动。若编排者坚持字面回滚条件，`jj restore` 本阶段
   `scripts/06-compare.py`、`scripts/acceptance-check.py`、`docs/residual-zeroing-plan-2026-09-07.md`
   三个文件即可（其余文档不受影响），并在报告登记"维持原豁免表"。

## 完成（改动文件清单）

**Step 1 — skill 三镜像（唯一真源 `skills/parity-test-skill.md` → cp 两镜像）**
- 事实 #21：R12b registered-but-no-resources（`providers_with_variants`/`has_any_variant`，空表=未知=保守 true）
- 事实 #22：value_maps schema v9（lang 入 PK；per-lang stringTable 主 DLL 为底、MUI 同键全覆盖 overlay；渲染端 `load_value_maps(lang)` 双层首选+en 回退；schema_version=9 硬校验）
- 事实 #23：渲染层四修复（Binary→大写 hex 追加 %n+1 仅模板命中路径；UserData 嵌套展平形态门控；GUID-span 回扫跳过 hex 化；`!S!` specifier 消费 + `%%` verbatim）
- 事实 #16 同步：豁免终态改 4 项（SPP/disk 摘除注记）；概述 v8→v9
- "已知上游缺口" #4–#8 转出登记（6008/Codex/e1d/VMnet/Wininit 23）；基线表与叙事整体更新为 P7 终态（44852/99.9153%、逐阶段战绩、G6/确定性门/单测数字）
- 三镜像 md5：**7cad81a23ffe5442db929451dfc36aec ×3（一致）**

**Step 2 — `STATUS.md`**
- §5 基线表与全量数字（P7 终态口径 + 逐阶段战绩）；§6 修复表补 R21 及新增 R12b/R21b(v9)/R22/R23/R24/R25 六行；§7 残留重写（21 条=5 簇转出明细 + 豁免 4 项/17 条）；§8.1 单测数字（Go 134 四包/全仓 135、Rust 245）；§2/§3 schema v8→v9；§10 事实编号 #1-#23

**Step 3 — EXEMPTIONS 双表同一提交同步（本轮唯一豁免改动点，两文件一起改）**
- `evtx-parity-test/scripts/06-compare.py`、`evtx-parity-test/scripts/acceptance-check.py`：
  删 `("Application","Microsoft-Windows-Security-SPP","1004","PARSE")` 与
  `("System","disk","51","SRC")`；保留 CAPI2 4111、SAM 16962、WPD 24577/24579 四项（两文件逐行一致）
- `docs/residual-zeroing-plan-2026-09-07.md` §4：SPP 1004、disk 51 两行标记关闭（2026-09-30，
  关闭条件写明 P5/P7 稳定转绿与 R21 脱命中）；WER 1001 行补记 P5 已闭环；SAM 16962 条数 3→8
  （zh 轮实际命中）；新增转出 5 簇登记行（根因码 BASE-ARTEFACT/UNREG-FALLBACK/MUI-WRONGLANG/
  EID-QUALIFIER/TDH-SYMBOL）+ 段尾终态注记

**Step 4 — 终稿文档**
- `docs/phase-reports/overnight-2026-09-30/final-report.md`（终态锚点/逐阶段战绩/门禁记录/转出清单/后续专项建议）
- `docs/phase-reports/overnight-2026-09-30/FINAL.md`（完成哨兵）
- 本报告

## EXEMPTIONS 同步验证输出（摘录）

```
$ python scripts/06-compare.py
compared=44890 matched=44852 diff=21 exempted=17
$ python scripts/acceptance-check.py --prev report.prev.json
[G0] ✓ baseline_lang zh-CN→zh-CN
[G1] ✓ M1 99.9153% (44852/44890) ≥ 97.53%；M2 93.97%（21/348）≥ 80% 硬门
[G2] ✓ Security diff=0；Setup diff=0；四通道 B/C 0→0；缺失类 diff=0
[G3] ✓ 豁免命中 4 项 / 17 条：CAPI2/4111 x1 [PARSE]; SAM/16962 x8 [BIN];
        WPD/24577 x4 [PARSE]; WPD/24579 x4 [PARSE]；登记未命中 0 条（原 2 条）
[G3] ✓ M5 有效分歧: 21 − 17 = ≈4
总体：PASS（必过门全过）  EXIT=0
```

判据核对：diff=21 ✓；白名单恰 4 项 ✓；21 条转出簇不命中白名单 ✓（命中恰为豁免的 17 条，
与 diff 21 不相交）；总体 PASS ✓。**唯一与简报字面不符的是 exempted=17（非 0）、
whitelist_diff_hits=17（非 4）**——系简报预期内部矛盾（见"取证 2"），已按语义判据保留改动并在此登记。

## 登记/转出

- 无新增转出。6008×2 建议白名单（System/EventLog/6008/PARSE）仍待授权——动豁免表须双表+登记表
  三处同步，超本阶段授权未动（skill 缺口 #4 已登记）。

## 未决与建议

1. 后续专项优先级（按条数/管线归并）：**msgdb-dump 提取侧专项**（e1dexpress MUI-WRONGLANG ×8 +
   VMnetAdapter EID-QUALIFIER ×8，同管线可合并实施，−16）；resolver UNREG-FALLBACK ×2；
   TDH 符号表 ×1；6008 白名单 ×2（零代码）。全清则 diff=5 豁免 + M1→99.9966%。
2. M2 D 基线 348（en 时代标定）建议重标定，当前 21/348=93.97% 已失分母意义。
3. Go 用例数口径统一（四包 134 vs 全仓 135）沿用 P7 建议。
4. 简报模板勘误：豁免摘除类验证预期应写"exempted 不变 + 命中项数=保留项数 + 登记未命中归零"，
   避免"exempted=0"与"保留 N 项"同句矛盾（本次以语义判据处理，见取证 2）。
