# 夜间计划终稿（2026-09-30，P0–P8 全程）

> 执行窗口：2026-09-29 23:30 → 2026-09-30 08:30（实际 P8 定稿约 03:40 完成）。
> 计划档：`docs/overnight-plan-2026-09-30.md`；逐阶段报告见本目录 `night-P1..P8-report.md`；
> 完成哨兵：`FINAL.md`。

## 1. 终态锚点（P7 验收，P8 豁免同步后复核不变）

| 指标 | 起点（09-29 18:33） | 终态 |
|---|---|---|
| compared | 44890 | 44890 |
| matched | 44635 | **44852** |
| **diff** | 237（全 D） | **21**（App 2 + System 19，全 D，全部已定性转出） |
| **M1** | 99.4319% | **99.9153%** |
| M2（D 闭环率，en 基线 348） | 31.9% | **93.97%** |
| M5 有效分歧 | 219 | **≈4** |
| exempted | 18 条/5 项 | **17 条/4 项**（SPP 1004 转绿、disk 51 脱命中后摘除） |
| baseline_lang | zh-CN | zh-CN（G0 全程一致） |
| verdict | DIFF | DIFF（残留全为转出功能缺口簇） |

- `acceptance-check.py --prev` 总体 **PASS，exit=0**（P8 豁免同步后复核仍 PASS）
- G6 关豁免：diff=**38**=21+17（防豁免掩盖回归通过）
- 确定性门：两次全链路（05→06）report.json 除 generated_local 时间戳外**逐字段全等**
- 红线全程未破：Security=0、Setup=0、四通道 B/C=0、无缺失类
- 单测：Go **134**（dump/schema/crim/pe 四包；全仓含探针包 135）+ Rust **245**（bin 35 + core 207 + doctest 3）全绿

## 2. 逐阶段战绩（diff 237→21，−216）

| 阶段 | 时间盒 | −diff | 修复与机制 | 目标簇前→后 |
|---|---|---|---|---|
| P1 | 60min | **−80** | R12b：N/A 精度 registered-but-no-resources（`providers_with_variants` + `has_any_variant`，空表=未知=保守 true） | CfgChangeMonitor×80 → 0 |
| P2 | 90min | **−75** | R21b：value_maps schema v9 双语化（lang 入 PK；per-lang stringTable=主 DLL 为底、MUI 同键全覆盖 overlay；渲染端 `load_value_maps(lang)` 双层首选+en 回退） | Ntfs 98×74 → 0 |
| P3 | 60min | **−38** | R22：classic `<Binary>` 大写 hex 追加 %n+1（仅模板命中路径；miss fallback 不动） | NetBT 4311×38 → 0 |
| P4 | 45min | **−9** | R23：UserData 嵌套展平（形态门控：深度 2 裸名/深度 3 `结构体_字段` 前缀） | Resource-Exhaustion 2004×9 → 0 |
| P5 | 60min | **−2** | R24：大十进制 hex 化收窄（GUID-span 回扫跳过转换；TS35 零回退）；SPP 1004 连带转绿（exempted 18→17） | WER 1001×2 → 0 |
| P6 | 60min | **−12** | R25：`!S!` 字符串族 specifier 消费 + `%%` verbatim（pass2 不塌缩） | Chrome/Edge 256×10 → 0、SCM 7023×2 → 0 |
| P7 | 60min | 0 | 全量验收：常规门 exit=0、G6 关豁免 38=21+17、确定性门两次全等、单测双端全绿 | — |
| P8 | 60min | 0 | 定稿：skill 三镜像（事实 #21–#23 + 基线终态）、STATUS 全量、EXEMPTIONS 摘 2 死项（验证 PASS）、终稿文档 | — |

所有修复均 TDD 落地（先红后绿），Rust 228→245、Go 130→134（四包）增量即本轮回归测试；
既有修复零回退（每阶段 System/App D 严格只减不增实证）。

## 3. 门禁记录（P7 全表，P8 复核一致）

| 门 | 结果 | 证据 |
|---|---|---|
| G0 基线语言一致 | PASS | zh-CN→zh-CN |
| G1 M1 主线匹配率 | PASS | 99.9153%（44852/44890）≥ 97.53% |
| G1 M2 D 闭环率（硬门） | PASS | 93.97%（21/348）≥ 80%（en 基线 348 属已知失义，只披露） |
| G2 Security / Setup 红线 | PASS | diff=0 / diff=0 |
| G2 四通道 B/C 不新增 | PASS | 0→0 ×4 |
| G2 无对齐断裂（缺失类） | PASS | 缺失类 diff=0 |
| G3 豁免命中与登记表一致 | PASS | 4 项/17 条：CAPI2/4111×1、SAM/16962×8、WPD/24577×4、WPD/24579×4；P8 同步后登记未命中 0 条 |
| G3 M5 有效分歧 | PASS | 21 − 17 = ≈4 |
| G4 M1 理想线披露 | 披露 | 距理想线 ≈0.00pp |
| G6 关豁免复核 | PASS | 38 = 21 + 17 |
| 确定性门 | PASS | 两次全链路逐字段全等 |

P8 豁免同步验证（`06-compare.py` → `acceptance-check.py --prev`）：
diff=21、exempted=17、命中 4 项/17 条、登记未命中 0 条、总体 PASS exit=0。
注：简报预期"exempted=0/whitelist_diff_hits=4"与"保留四项"自相矛盾（四项保留必命中 17 条），
按语义判据（diff 不变 + PASS + 白名单恰 4 项全命中 + 登记未命中归零）判定同步成功，未回滚，
已在 `night-P8-report.md` 取证节登记勘误与回滚路径。

## 4. 转出清单（残留 diff=21，逐条定性完毕，无一"未知"）

| 簇 | 条数 | 根因码 | 机理与修复路径 |
|---|---|---|---|
| e1dexpress 32（System） | 8 | MUI-WRONGLANG | DriverStore 语言中立 e1dmsg.dll 的 MUI 目录解析错选 es-ES，msgdb zh/en 键均存西语文案 → msgdb-dump MUI 解析重写（GetSystemMUILanguages/RC 常量） |
| VMnetAdapter 36（System） | 8 | EID-QUALIFIER | classic 消息 id=qualifiers<<16\|eid（0x40080024），msgdb 变体键缺 qualifiers → 链首 IoLogMsg.dll 裸 id 首中错库 → msgdb-dump qualifier 感知链式查找 |
| EventLog 6008（System） | 2 | BASE-ARTEFACT | 我方文本逐字节正确（真实 LRM U+200E），基线 iconv 把 LRM 变 `?`（斜杠形日期未被 R5 手术覆盖）→ 上游不可修，建议白名单 System/EventLog/6008/PARSE（待授权） |
| CodexSandboxService 1001/1002（App） | 2 | UNREG-FALLBACK | 源未注册，基线把 eid 当 Win32 错误码回退渲染；我方 N/A → resolver"无模板→Win32 错误表"回退（牵动 N/A-parity 谓词） |
| Wininit 23（System） | 1 | TDH-SYMBOL | 0x10000000 经 wevtutil TDH 内建符号表渲染 "STATUS_SUCCESS" → 需内建 NTSTATUS/HRESULT 符号表 |

登记真源：`docs/residual-zeroing-plan-2026-09-07.md` §4 与 skill"已知上游缺口" #4–#8。
豁免存留 4 项/17 条：CAPI2 4111（基线 `?` 手术待授权）、SAM 16962（SDDL 转换未建）、
WPD 24577/24579（取值层 Named-Data 对齐）。

## 5. 后续专项建议（按收益/管线归并）

1. **msgdb-dump 提取侧专项（−16，最高优先）**：e1dexpress MUI-WRONGLANG ×8 + VMnetAdapter
   EID-QUALIFIER ×8 同属萃取端，可合并一个阶段实施（MUI 解析走系统 MUI 语言列表而非目录枚举序；
   变体落库补 qualifiers 维度 + "链序+带 qualifier id"查找）。
2. **resolver UNREG-FALLBACK（−2）**：无模板 → eid 走 Win32 系统错误表回退；须与 R12/R12b
   N/A-parity 谓词联合设计（Codex 案例方向与 CfgChangeMonitor 相反，勿并规则）。
3. **TDH/HRESULT 内建符号表（−1）**：STATUS_ 符号解码，可与 NTSTATUS 全局组（事实 #14）扩展。
4. **6008 白名单（−2，零代码）**：三处同步（双表+登记表）走授权流程。
5. **全清上限**：diff=5（豁免 17 之外仅 5 项有效分歧）→ M1→99.9966%、M5=0。
6. **M2 D 基线重标定**：348 为 en 时代标定，zh 轮构成已完全不同，建议计划变更时重标。
7. **文档口径**：Go 用例数统一（四包 134 vs 全仓 135）；豁免摘除类验证预期模板勘误
   （见 night-P8 取证 2）。

## 6. 改动文件总账（P8 本阶段；P1–P7 见各阶段报告）

- `skills/parity-test-skill.md` + `.agents/skills/` + `.dsh/skills/`（三镜像 md5=
  7cad81a23ffe5442db929451dfc36aec）
- `STATUS.md`
- `evtx-parity-test/scripts/06-compare.py`、`evtx-parity-test/scripts/acceptance-check.py`（EXEMPTIONS 同步）
- `docs/residual-zeroing-plan-2026-09-07.md`
- `docs/phase-reports/overnight-2026-09-30/final-report.md`、`FINAL.md`、`night-P8-report.md`

未提交（编排者统一 jj commit；提交信息应含：diff=21、matched 44852/44890、M1=99.9153%、
exempted 17/4、acceptance PASS exit=0）。
