# 阶段报告：R21 经典源四修复（2026-09-29 下午）

## 结论

- **数字**：diff 1515 → **237**；matched 43356 → **44635**/44890（M1 96.58% → **99.4319%**，
  超 G1 目标线 97.53%）；**B/C 类全场清零**（en 时代 B718/C51 历史包袱闭环）；Security/Setup
  持续全绿；exempted 18 条/5 项（disk/51 脱离豁免）。
- **门**：G0 ✓；G1 M1 ✓（99.43%）；G1 M2 ✗（D237 vs en 基线 348——指标已失义，重标定待
  计划变更）；G2 ✓ 全部红线；G3 ✓（5 项/18 条与登记表一致，disk/51 未命中）。
- 单测：Go 130 绿（新增 `TestClassicVariantRowsEIDMapping`、`TestClassicVariantRowsCollisionDeterministic`）、
  Rust 228 绿（新增 `test_warmup_cross_language_fallback`、`test_lookup_win32_error0_lang_layers`）。

## 取证（四根因，全部实证）

1. **经典源 eid 映射缺失**（B1025 条总根因）：事件记录只带 MessageID 低 16 位
   （Ntfs 50 的表项是 0x80010032、6013 是 0x8000177D），classic 路径把原始全格式 ID 直接存
   event_id → 渲染端按 eid 查询全量 miss。6013 的 well-known 注入即此机制的手工前例。
2. **变体预热 lang 硬过滤**（System-Restore 8300×93）：`WHERE lang=?1` 使只有 en 行的
   provider 在 zh 轮空载；Setup 通道基线为 en 即 wevtapi MUI 回退形态（无 zh 资源 → 渲染 en）。
3. **classic eid=0 → FormatMessage(0)**（孤儿 ISV provider 177 条）：WXWorkUpgrader 注册已删
   （reg query 实证 key 不存在）而 wevtutil 仍渲染"操作成功完成。"= 系统错误 0 文本，事件
   Data（时间戳/RulesEngine）不参与渲染；§0 表 WXWork/asus/PDFShellExt/igfx 四簇基线同句。
4. **帐户/账户 译文 servicing 漂移**（SCM 7045×111）：全 System32 字节级唯一模板载体
   `zh-CN\services.exe.mui` 为"帐户"版（9KB，10 处），wevtapi 渲染"账户"版（111/111）且
   账户版文件不存在于文件系统（疑 LOS 增量缓存）→ 比对层归一（CAPI2 `?` 先例）。

## 完成

- Go：`classicVariantRows`（run_variants.go，映射+确定性去重）接入
  `provider_extractor.go` flat 路径与 `extractVariantsFallback`。
- Rust：`warmup_variants` 空 cohort 回落 en-US（`load_variant_rows_for_cohorts` 重构）；
  `lookup_win32_error0`；cmd_parse miss 路径 eid=0 臂。
- Python：06 canon_msg `帐户→账户` 归一（注释含全部取证数字）。
- 闭环陷阱记录：closure-1 的 04 因 exe 未重编（15:29 旧版）造成"修复无效"假象——重跑 04
  （exe 18:14）后数字即兑现；**跑 05 前必须核对 tools/evtx-tools.exe mtime**。

## 登记/转出（残留 237 条全 D，根因已诊断）

- value_maps 0xDn 双语化（schema v9）→ Ntfs 98×74（0xD1 Ntfs corruption-state）；
- R12 N/A 精度规则（registered-but-no-resources）→ CfgChangeMonitor×80；
- 参数 hex 格式族逐簇取证 → NetBT 4311×38 / VMnetAdapter 36×8 / e1dexpress 32×8；
- Resource-Exhaustion 2004×9（nested EventData 参数空置）；WER 1001×2（en 时代遗留）；
- EventLog 6008×2 / CodexSandboxService×2 / Chrome、Edge 256×10 逐条定性。
- M2 D 基线 348 重标定 → 用户决策；登记表 §4 待同步（disk/51 脱豁免）。

## 变更文件

`msgdb-dump/dump/{run_variants,provider_extractor}.go`、`run_variants_classic_test.go`（新）、
`evtx-tools/evtx-tools-core/src/{resolver,variant_resolver}.rs`、
`evtx-tools/evtx-tools-cli/src/cmd_parse.rs`、`06-compare.py`、skill 三镜像（md5 一致）、STATUS.md、本报告。
