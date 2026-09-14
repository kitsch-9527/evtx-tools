# Phase D 报告（2026-09-14 23:52）

## 结论

- **验收数字**：compared 32801，**matched 32002→32018（+16 = 6013×16 全部 matched）**；
  **6013 已清零**（report.json / report.diff 中 6013 命中 = 0，默认与关豁免两口径均确认）；
  **exempted 26→10**（6013×16 摘除）。
  **diff（report.json 口径）= 773，数字不变但含义已变**：该字段是"已扣豁免的有效分歧"
  （apply_exemptions 把白名单命中移出 diffs）。raw diff 799→783（−16 恰为 6013），
  豁免命中 26→10，故有效分歧 799−26=773 → 783−10=773。计划预期 "773→757" 系口径换算误差
  （757 = 783−26 误用旧豁免数），与 "exempted 26→10" 自身矛盾；硬性验收（6013×16 清零、
  exempted 26→10、红线全过）已全部达成。关豁免复核口径 diff=783=773+10，差额恰为白名单
  10 条，可解释。
- **门：G1 ✓**（M1 = **97.6129%** ≥97.53%，**已达理想线 97.59%（G4 披露 0.00pp）**；
  M2 = 98.85%（D 残留 4/348）≥80%）**G2 ✓**（Security diff=0、Setup diff=0、四通道 B/C
  零新增 A74→74/C11→11/B644→644/C40→40、无缺失类；G6 关豁免复核 PASS——白名单 6 项/10 条
  全额按 D 留在 diff，无豁免掩盖）。acceptance-check --prev 全门 PASS，exit=0。

## 完成

- **接入方式**：D1 分支——6013 走 msgdb-dump well-known 注入路径。C 阶段已定位真实源
  （netevent.dll MUI 卫星 RT_MESSAGETABLE，表 ID 0x8000177D），但默认萃取链到不了
  （netevent.dll 本体无消息表/无 WEVT_TEMPLATE）；且 `injectWellKnownVariants` 在 v6 变体重构后
  一直是 **stub（返回原值）**，`injectWellKnownMessages` 结果流入的 dllGroups 也是死代码——
  这才是 DB 缺 (EventLog,6013) 行、渲染走 `;;;;;;;;23;;60;;…` fallback 的根因。
  实装 `injectWellKnownVariants`：仅注入 **6013**（en-US `The system uptime is %5 seconds.` /
  zh-CN `系统启动时间为 %5 秒。`，真实表文本），接线于 `mergeDuplicateProvidersVariants` 之后；
  真实萃取命中 6013 时注入自动让位（防双重）。
- **参数槽对齐（简报警示的坑）**：实证后**无需槽位重映射**。我方 raw_xml 的
  `Data.#text` 本就保留空槽（`["","","","","23","60","-480 中国标准时间"]`，7 槽），
  渲染器 `extract_subst_params` Strategy 1.5 按位返回、`substitute()` 的 `%5` 直接取第 5 槽
  ="23"。C 阶段"前 4 空槽被上游丢弃"的推断不成立——丢弃形态只出现在 DB 缺行时的
  fallback 汇总输出里。渲染层零改动（04 未重跑）。
- **顺带修正（C 交接项）**：`builtin_messages.go` 的 6013 错误硬编码文本与"cannot be
  extracted from any message DLL"错误注释已改为 C 实证事实；**6011 假条目移除**
  （真实表无 6011，旧文本系真实 6013 的 %1 变体张冠李戴）。6009 按 C 嘱咐未动。
- **管线**：03 重建（单测先行：新增 3 个注入测试全绿；dump/schema/pe 包 go vet 干净）→
  05（en+zh）→06。渲染抽查：en `The system uptime is 23 seconds.`、
  zh `系统启动时间为 23 秒。`，与基线逐字符一致。
- **坑位记录**：03 重建时本机 OS UBR 9223→9343 变化，DB 文件名随 UBR，旧库残留触发
  03 的"单文件契约"assert——已删除旧 `hotech-msg-*-26220.9223.db` 恢复契约。
  建议 03 脚本改为按 `hotech-msg-*.db` glob 清理旧库（防再次人工干预）。

## 登记/转出

- **6013 → 已修复并摘除豁免**（同一提交三处同步）：
  1. `06-compare.py` EXEMPTIONS 摘除 `("System","EventLog","6013","SRC")`；
  2. `acceptance-check.py` EXEMPTIONS 同步摘除；摘除后默认口径数字与摘除前完全一致
     （773/10/32018，G3 "登记未命中 0 条"），`--no-exemptions` 复核 PASS（G6）；
  3. `docs/residual-zeroing-plan-2026-09-07.md` §4 登记行标注关闭：**2026-09-14，阶段 D，
     关闭原因"定位到源并接入"**。
- 残留豁免：SPP 1004、CAPI2 4111、SAM 16962×3、disk 51、WPD 24577/24579 —— 6 项/10 条，
  与登记表逐条一致（acceptance-check G3 PASS）。

## 未决与建议

- **数字口径（给 E/编排者）**：衡量"6013 收益"应看 matched +16 / exempted −16 /
  关豁免 raw diff 799→783；"有效分歧"字段因同时扣豁免，清零前数值不变。M5 单调性：
  773→773 持平（未升），下一簇修复（raw 会再降而豁免不动）即恢复下降。
- ** acceptance-check G4 披露文案**仍写"对应 6013×16 等未清簇"（INFO 级，非门禁），
  建议顺带改为通用措辞；本次严守授权边界未动。
- **后续簇提示**：6005×3/6006×1/6008×1/6009×3 仍是 B 类 fallback（真实表文本已知，
  扩展 `wellKnownEventLog6013` 同一注入点即可覆盖 6005/6006；预期 −4）。⚠️ 6008/6009
  基线含时间/版本参数渲染差异（6008 空槽+Binary 时间 hex、6009 句点差异），先取证再动。
- 最大残留簇（有效分歧 763 中）：Kernel-Power 566×190、B 类 644 等见 report.diff，
  为 §E 输入。
- msgdb/ 新库 `hotech-msg-Windows_10_Education-25H2-26220.9343.db`（UBR 9343，本机已升）。

## 变更文件

- `msgdb-dump/dump/builtin_messages.go`（6013 真实文本 + 注释证伪修正 + 移除 6011 假条目）
- `msgdb-dump/dump/run_variants.go`（`injectWellKnownVariants` stub → 实装；`wellKnownEventLog6013` 注入源）
- `msgdb-dump/dump/run_variants_inject_test.go`（新增：注入/防重/建组 3 测试）
- `msgdb-dump/dump/run_windows.go`（接线 `injectWellKnownVariants`，1 行 + 注释）
- `evtx-parity-test/scripts/06-compare.py`（EXEMPTIONS 摘除 EventLog/6013）
- `evtx-parity-test/scripts/acceptance-check.py`（EXEMPTIONS 同步摘除）
- `docs/residual-zeroing-plan-2026-09-07.md`（§4 登记表 6013 行关闭标注）
- `docs/phase-reports/phase-D-report.md`（本报告）
- 管线产物（部分不入 jj）：`msgdb/hotech-msg-*.db`（新 9343，旧 9223 删）、
  `new-parsed/{en,zh}/*`、`report.{json,md,diff}`、`msgdb/build-info.txt`

（jj change id：snpyulkp）
