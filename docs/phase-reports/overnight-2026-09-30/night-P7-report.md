# night-P7：全量验收（G 门 + 确定性门）

日期：2026-09-30（夜间计划 P7，全量验收，零代码改动）
基线：P6 终态（commit 1cf176d，工作树仅 docs/.lock 变更）
时间盒：60 分钟内完成，未触边界。

## 结论

**全部通过：常规门 PASS（exit=0）、G6 关豁免复核 PASS（38 = 21 + 17）、确定性门 PASS（两次全链数字逐字段一致）、红线全过、双端单测全绿。零代码改动，无 FAIL 归属事项，无需回退或登记。**

- diff = 21（System 19 + Application 2），matched = 44852/44890
- M1 = 99.9153%，M2 = 93.97%（en 基线 348，D 残留 21/348）
- exempted = 17 条 / 4 项，whitelist_diff_hits = 17
- baseline_lang = zh-CN

## 门禁明细（acceptance-check.py --prev report.prev.json，exit=0）

| 门 | 结果 | 证据 |
|---|---|---|
| G0 基线语言一致 | PASS | baseline_lang zh-CN→zh-CN |
| G1 M1 主线匹配率 | PASS | 99.9153%（44852/44890），下限 97.53% |
| G1 M2 D 闭环率（硬门） | PASS | 93.97%（D 残留 21/348），硬门 80%（en 基线 348 属已知失义，按简报只披露，当前已达标） |
| G2 红线 Security | PASS | diff=0 |
| G2 红线 Setup | PASS | diff=0 |
| G2 四通道 B/C 不新增 | PASS | Application 0→0，Setup 0→0，Security 0→0，System 0→0 |
| G2 无对齐断裂（缺失类） | PASS | 缺失类 diff=0 |
| G3 豁免命中与登记表一致 | PASS | 命中 4 项/17 条：CAPI2/4111 x1 [PARSE]、Directory-Services-SAM/16962 x8 [BIN]、WPDClassInstaller/24577 x4 [PARSE]、WPDClassInstaller/24579 x4 [PARSE]；登记未命中 2 条（条件性/待转正） |
| G3 M5 有效分歧 | PASS | diff 21 − 豁免命中 17 = 有效 ≈4 |
| G4 M1 理想线披露 | PASS（披露） | 距理想线 97.59% 还差 ≈0.00pp（对应 6013×16 等未清簇） |

总体：**PASS（必过门全过）**，`echo $?` = **0**。

## G6 关豁免复核

`python scripts/06-compare.py --no-exemptions`：

- 关豁免 diff = **38** = 默认 diff(21) + 白名单命中(17) ✓（防豁免掩盖回归通过）
- 逐通道（关豁免）：Application 3（=2 默认 + 1 豁免）、Setup 0、Security 0、System 35（=19 + 16），合计 38 ✓
- exempted = 0、whitelist_diff_hits = 17 ✓
- 复核后立即以默认口径 `python scripts/06-compare.py` 恢复 report.json：diff=21、exempted=17、whitelist_diff_hits=17（与 P6 终态一致）✓

## 确定性门（R8 先例：防 HashMap 迭代序不确定性）

重跑 `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/05-new-parse.ps1`（exit=0，en+zh 各 .db+.csv 全量重建）→ `python scripts/06-compare.py`（exit=0）。

| 指标 | 第 1 次（P6 终态，generated_local 02:34:41） | 第 2 次（generated_local 02:47:39） | 一致 |
|---|---|---|---|
| compared | 44890 | 44890 | ✓ |
| matched | 44852 | 44852 | ✓ |
| diff | 21 | 21 | ✓ |
| exempted | 17 | 17 | ✓ |
| whitelist_diff_hits | 17 | 17 | ✓ |
| baseline_lang | zh-CN | zh-CN | ✓ |
| verdict | DIFF | DIFF | ✓ |
| Application diff/matched/exempted | 2 / 10739 / 1 | 2 / 10739 / 1 | ✓ |
| Setup | 0 / 676 / 0 | 0 / 676 / 0 | ✓ |
| Security | 0 / 27366 / 0 | 0 / 27366 / 0 | ✓ |
| System | 19 / 6071 / 16 | 19 / 6071 / 16 | ✓ |
| categories（App/System 的 D 计数） | 2 / 19 | 2 / 19 | ✓ |
| exempted_detail | — | — | 逐条一致 ✓ |
| whitelist_diff_detail | — | — | 逐条一致 ✓ |

仅 generated_local 时间戳不同，其余全部字段（含逐通道与 categories 计数）**完全一致** → 确定性门 PASS。

## 红线逐项

| 红线 | 结果 | 证据 |
|---|---|---|
| Security diff=0 | ✓ | report.json by_channel.Security.diff=0 |
| Setup diff=0 | ✓ | report.json by_channel.Setup.diff=0 |
| 四通道 B/C=0 | ✓ | acceptance G2 四条 B/C 0→0；categories 全通道仅含「D-实质分歧」一类，合计 21，无非 D 类 |
| 无缺失类（对齐断裂） | ✓ | acceptance G2 缺失类 diff=0 |
| baseline_lang=zh-CN | ✓ | G0 + report.json |

## 单测双端

| 端 | 命令 | 结果 |
|---|---|---|
| Go | `cd msgdb-dump && go test ./dump/ ./schema/ ./crim/ ./pe/ -count=1` | **134 pass / 0 fail**（四包全绿；源码 Test 函数计数=134 核实）。注：P6 记录 135 的差额为四包之外 `crim-probe-intype` 包 1 例（trtables_test），本次 `./...` 复核亦全绿 → 全仓 135 与 P6 口径吻合 |
| Rust | `cd evtx-tools && cargo test --workspace` | **245 pass / 0 fail / 1 ignored**（主二进制 35 + core 207 + doc-tests 3），与 P6 记录 245 一致 |

## 未决与建议

1. **无 FAIL 归属事项**：所有门一次通过，未触发退出条件，未改任何代码，未提交（按简报由编排者统一 jj 提交）。
2. 残留 diff 21 全为 D 类（System 19 + App 2），其中豁免命中 17、有效 ≈4（6013×16 等）——维持 P6 定性与转出结论，供后续计划参考；G4 披露距理想线 ≈0.00pp。
3. G3 登记未命中 2 条（条件性/待转正）维持现状披露，不影响本阶段验收。
4. Go 用例数建议后续统一口径（四包 134 vs 全仓 135），避免跨阶段数字误读。
