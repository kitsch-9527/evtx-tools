# 阶段报告：语言漂移重基线 + param_refs 双语化（2026-09-29）

## 结论

- **数字**：diff 11290 → **1515**；matched 33581 → **43356**/44890（M1 74.81% → **96.5828%**）；
  exempted 19 条/6 项；**Security 0/27366 → 27366/27366 全绿**（9733 条枚举名 D 清零）；Setup 全绿。
- **门**：G0 ✓（新增，zh→zh）；G2 ✓ 全部红线（Security/Setup=0、四通道 B/C 不新增、无缺失类）；
  G3 ✓ 豁免 6 项/19 条与登记表一致；G1 ✗（M1 96.58% < 97.53%、M2 D400>348——en 时代标定线，
  zh 重标定待计划变更，见 STATUS §7）。
- 单测：Go 128 绿（新增 `TestWriteParamRefs_MultiLang`、`TestExtractParamMessagesByLang_MsobjsLive`
  活体测试）、Rust 226 绿（新增 `test_param_refs_lang_preference_and_fallback`）。

## 取证（根因链）

1. 语言漂移：`Get-WinUILanguageOverride`=zh-Hans-CN（用户显式切换）；wevtutil `/f:text` 渲染随 UI 语言
   → Lua 基线翻中文 → 06 v4 主比对（基线 vs en）全量假阳性 D（matched 32020→1840）。
   Setup 全绿的自洽机理：其 provider 无 zh 资源，EvtFormatMessage 回退 en。
2. 枚举名 en 化：Security-Auditing 4624 的 `%%1833` 走 `parameter_refs`
   （provider=Security-Auditing, param_id=1833, message='Impersonation\r\n'）——**表 PK 本就含 lang
   列但只有 en-US 12252 行**；zh 文本在 `zh-CN\msobjs.dll.mui`，萃取端从未采集。

## 完成（改动文件）

- `06-compare.py` v5：G0 `detect_baseline_lang`（zh=`'Level: 信息'`/en=`'Level: Information'`），
  主比对随基线语言、副侧覆盖率（cov_* 键），report.json 增 `baseline_lang`。
- `msgdb-dump`（v9 机制，schema 未升版）：`extractor.go` 参数文件 MUI 多语言萃取
  （`extractParamMessagesByLang`，**len>0 判断**——netevent.dll 28KB MUI-only 基础文件返回空非 nil
  map 的坑）；`run_windows.go` Win32 系统错误按 langCodes 每语言一套（langID 1033/2052）、
  ntdll zh MUI NTSTATUS 2975 条（阶段 E"MUI 不含此表"注释过时）、`writeParamRefs` 按 lang 落行。
- `evtx-tools`：`load_param_refs(lang)` 首选层+en-US 回退层；`expand_message`/`lookup_ntstatus`
  按首选→en-US 回退（镜像 EvtFormatMessage MUI 链）；CLI 传 `target_lang`。
- `acceptance-check.py`：G0 语言一致性门（未知或与 prev 不一致即 FAIL）。
- 参数引用行数：en 118767 + zh 118355（Security-Auditing 1011/1011，Dhcp 7185/7185 对称）。

## 登记/转出

- zh 新增簇 → 后续专项：System-Restore 8300-8303×93（value-map 族嫌疑）、SCM 7045×111、
  Ntfs 98×74、Resource-Exhaustion 2004×9。
- 历史簇原样复现（语言无关）：Ntfs 50×515、hcmon×266、WXWork×63、asus×46、Win32k×42、
  NetBT×38、CfgChangeMonitor×80 等（= en 时代 B718/C51 清单）。
- G1 目标线重标定（M1 97.53%/M2 基线 348）→ 用户决策。
- 豁免：6 项键语言无关自动生效（19 条，条数随新事件增长），EXEMPTIONS 未改。

## 未决与建议

- value_maps/param_value_maps 仍无 lang 列（SCM 7045/System-Restore 疑似此族）——下一专项首选。
- `wevtutil epl` 不覆盖已存在文件：01-export.ps1 建议加重导前删除（本次手工先删）。
- 管线坑位已固化到 skill 三镜像事实 #17-#19。

## 变更文件

`06-compare.py`、`acceptance-check.py`、`msgdb-dump/dump/{types,extractor,run_windows,ntstatus}.go`、
`msgdb-dump/dump/param_refs_lang_test.go`（新）、`evtx-tools/evtx-tools-core/src/resolver.rs`、
`evtx-tools/evtx-tools-cli/src/cmd_parse.rs`、`skills/parity-test-skill.md`（三镜像 md5 一致）、
`STATUS.md`、`scripts/_01-security-elevated.ps1`（新）、`scripts/_01-elevated-run.ps1`（新）、
本报告 + `docs/phase-reports/2026-09-29-language-drift-verification.md`（校验报告）。
