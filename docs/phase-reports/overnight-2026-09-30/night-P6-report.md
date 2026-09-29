# night-P6 报告：残留逐簇定性 + Chrome/Edge `!S!` 与 SCM 7023 `%%` 修复（2026-09-30 02:00→02:45）

## 结论

**验收通过。33→21（只减不增，−12），acceptance 总体 PASS，红线全绿。**

| 指标 | 前（P5 后锚点） | 后（本阶段） |
|---|---|---|
| diff 总数 | 33（App 12 + System 21） | **21（App 2 + System 19）** |
| matched | 44840/44890 | **44852/44890**（+12：Chrome×8 + Edge×2 + SCM7023×2） |
| M1 主线匹配率 | 99.8886% | **99.9153%** |
| M2 D 闭环率 | 90.52% | **93.97%**（D 残留 21/348） |
| exempted | 17 | 17（不动） |
| Chrome/Edge 256（App）簇 | 10 | **0** |
| SCM 7023（System）簇 | 2 | **0** |
| SPP 1004（App） | 已转绿 | 复跑两次均稳定绿（登记，未动代码） |

红线逐项（acceptance-check.py --prev report.prev.json，总体 **PASS**）：

- [G0] baseline_lang zh-CN→zh-CN ✓
- [G1] M1=99.9153% ≥ 97.53% ✓；M2=93.97% ≥ 80% 硬门 ✓
- [G2] Security diff=0 ✓；Setup diff=0 ✓
- [G2] 四通道 B/C 不新增：App 0→0、Setup 0→0、Security 0→0、System 0→0 ✓
- [G2] 无对齐断裂（缺失类 diff=0）✓
- [G3] 豁免命中 4 项/17 条与登记表一致 ✓
- D 只减不增：33→21 ✓
- 单测：Rust **245 全绿**（core **207**=205+2 条本阶段红绿新测、cli 35、other 3）；Go **135 绿**（msgdb-dump `-count=1` 实跑，本阶段无 Go 改动）。exe mtime **01:55 → 02:32 已核对**；06-compare 复跑两次 diff=21 稳定。

## 取证（逐簇定性表）

importlib 复用 06-compare.py 全量重分簇确认 33 条 6 簇（SPP 1004 已绿不在清单），逐簇 registry/msgdb/raw-XML 探针定性：

| 簇 | 根因码 | 取证要点 | 可修性 | 处置 |
|---|---|---|---|---|
| Chrome 256×8 + Edge 256×2（App） | **FMT-SPEC** | 两侧均注册（EventMessageFile=eventlog_provider.dll 版本化路径）；msgdb 模板=`%1!S!\r\n`（双语同文，Chrome 256/257/258+Edge）；wevtapi 消费 FormatMessage 字符串型格式说明符 `!S!`，我方 substitute 不识别 → 字面 `!S!` 尾缀 | 机械，单点 | **本阶段修复**（10→0） |
| SCM 7023×2（System） | **PCT-ESCAPE** | raw param2=`%%2149884192`（=0x8024A120 数值实证）；wevtutil verbatim；我方两处：pass2 `%%`→`%` 塌缩 + Step3 大十进制 hex 化 → `%0x8024a120` | 机械，单点 | **本阶段修复**（2→0） |
| SPP 1004×1（App） | —（贪心配对噪声） | P5 后连带转绿；本阶段 06 两次复跑均不在 diff | — | 登记稳定，不动作 |
| EventLog 6008×2（System） | **BASE-ARTEFACT**（PARSE 族，CAPI2 4111 同构） | netevent MUI zh 模板已落库且正确；**我方文本逐字节正确**（真实 LRM U+200E：`‎2026/‎8/‎21`），基线管线 iconv 把 LRM 变 `?`（`?2026/?8/?21`）；R5 手术只清 `-` 形日期未覆盖斜杠形 | 上游不可修（我方无错）；基线侧手术/白名单超授权 | 登记转出（建议白名单 System/EventLog/6008 PARSE） |
| CodexSandboxService 1001/1002×2（App） | **UNREG-FALLBACK** | 注册态实证：`EventLog\Application\CodexSandboxService.OpenAI.Codex` 键**不存在**（reg query 报错）、`wevtutil ep` 无 ETW manifest、eventlog_sources 无此源；基线渲染=eid 当 Win32 错误码回退（1001→"递归太深；堆栈溢出。"、1002→"窗口无法在已发送的消息上操作。"，与事件原始 Data 英文串无关）；我方 N/A | 需 resolver 加"无模板→eid 走 Win32 系统错误表"回退，牵动 N/A-parity 谓词（resolver.rs 73/203/261 行机制），非机械 | 登记转出 |
| e1dexpress 32×8（System） | **MUI-WRONGLANG** | 链=netevent.dll;DriverStore\...\e1dmsg.dll；msgdb `e1dexpress` 全部 14 条变体（dll_id=717=e1dmsg.dll）**zh-CN 与 en-US 键下存的都是西语文案**（"Se ha establecido el enlace…"）→ DriverStore 语言中立 dll 的 MUI 目录解析错选 es-ES 且双语同文错存 | msgdb-dump（Go）MUI 解析重写，修复面大 | 登记转出（机理已一层深挖） |
| VMnetAdapter 36×8（System） | **EID-QUALIFIER** | 链=IoLogMsg.dll;vmnetadapter.sys；raw XML `EventID Qualifiers='16392'` → classic 实查 id=qualifiers<<16\|eid=**0x40080024**，IoLogMsg.dll 无此 id、vmnetadapter.sys 有 → OS 渲染驱动文案；msgdb 按**裸 eid 36** 命中链首 IoLogMsg.dll 配额消息（dll_id=357）→ 错库机理=消息 id 丢 qualifiers+链序按裸 id 首中 | msgdb-dump（Go）qualifier 感知链式查找，修复面大 | 登记转出（机理已一层深挖，修正 P3"错库"表述：非取错 DLL 文件，是查找键缺 qualifiers） |
| Wininit 23×1（System） | **TDH-SYMBOL** | raw HRESULT=268435456（0x10000000，XML 实证）；基线 wevtutil/TDH 内建符号表渲染 "STATUS_SUCCESS"；我方 typed HRESULT 渲 0x10000000 | 需内建 NTSTATUS/HRESULT 符号表，非机械 | 登记转出 |

## 完成（改动文件）

- `evtx-tools/evtx-tools-core/src/substitute.rs`（唯一代码改动，红→绿各 1 测试先行）：
  - 新增测试 `substitute_consumes_string_format_specifier_after_placeholder`：真实 Chrome 样本（`%1!S!\r\n` 模板端到端 substitute_format_message）断言无 `!S!` 尾缀；字符串族 s/S/hs/ws/ls 消费；`!x!`/`!d!` **不**消费（窄条件，无实证不改渲染）。
  - 新增测试 `format_for_display_keeps_double_percent_error_code_verbatim`：真实 SCM 7023 样本端到端断言 `%%2149884192` 原样、无 `%0x`；红线断言模板级 `%%`→`%`（pass1）与孤立大十进制 hex 化不回退。
  - 实现 3 处：① substitute 在 `%N` 后消费 `!字符串族!` 说明符（≤15 字符、字母数字+`.`、需闭合；不合规则原样吐回）；② Step3 hex 化新 guard `is_percent_context`（前随 `%` 不转换）；③ substitute_opts(..., unescape_double_percent)：pass2（参数值侧）不做 `%%` 塌缩，pass1（模板侧）保留——与 EvtFormatMessage 两遍语义对齐（SCM 实证 wevtutil 保留 `%%`）。
- 数据产物（管线再生）：new-parsed/{zh,en}/*、report.json/report.md/report.diff。
- 环节：04-build（mtime 01:55→02:32 核对）→ 05-new-parse → 06-compare（21，复跑两次一致）→ acceptance-check PASS。

## 登记-转出

1. EventLog 6008×2：BASE-ARTEFACT（基线 LRM→`?`，斜杠形日期未被 R5 手术覆盖）。建议后续以白名单（System/EventLog/6008/PARSE）登记——需同步 residual-zeroing-plan §4 与 acceptance-check EXEMPTIONS 两处，超本阶段授权未动。
2. CodexSandboxService 1001/1002×2：UNREG-FALLBACK（未注册源 eid→Win32 错误文案回退缺失）。
3. e1dexpress 32×8：MUI-WRONGLANG（msgdb-dump DriverStore e1dmsg.dll MUI 错选 es-ES，双语键同文错存）。
4. VMnetAdapter 36×8：EID-QUALIFIER（classic 消息查找须用 qualifiers<<16|eid，msgdb 变体键缺 qualifiers 致链首 IoLogMsg.dll 裸 id 首中）。3/4 同属 msgdb-dump（Go）提取侧，可并一个后续阶段。
5. Wininit 23×1：TDH-SYMBOL（wevtutil 内建 HRESULT/NTSTATUS 符号解码）。

## 未决与建议

1. 残余 21 条全部定性完毕、无一属"未知"：2 条我方上游无错（6008）、19 条为功能缺口（msgdb-dump 提取侧 16、resolver 回退 2、TDH 符号表 1）。6013×16 等未清簇在 G4 披露口径下距理想线 ≈0.00pp。
2. e1d/VMnet 修复建议路径：msgdb-dump 在变体落库时补 `qualifiers` 维度（EventRecord 的 EventID Qualifiers 属性）并按"链序+带 qualifier id"查找；e1d 另需 MUI 解析走 GetSystemMUILanguages/RC 常量而非目录枚举序。两簇同管线可合并实施。
3. `!S!` 消费仅覆盖字符串族：msgdb 现存 `%1!d!`/`%1!ws!`（CredUI 2505+ 等）事件未在本基线出现，若未来捕获需复核数值型说明符渲染。
