# parity-test Skill

## 概述
双链路可重复闭环验证台：本机四通道事件查看器经两条解析路径
（`evtx2text.lua` 基线 vs `evtx-tools` 新解析）归一化比对后生成对比报告。
每次改动上游后重跑 `run-all.ps1` 即得同一口径的 `report.md`（人类结论）+
`report.json`（机器可读，下轮迭代的对比基线）+ `report.diff`（差异明细）。
闭环语义：`report.json.verdict` 为 `PASS` 即闭合；为 `DIFF` 则按
`by_channel.*.categories` 定位回归分类，修上游后再跑下一轮。

项目位置：`evtx-parity-test/`（术语别名见其 `CONTEXT.md`）。

## 核心能力
- 四通道本机导出（应用=Application / 补丁安装=Setup / 安全=Security / 系统=System）
- 原脚本中转解析（`evtx2text.lua` 不动，复制到 `msys64-Tools/evtx/` 跑完搬回）
- 双源码重编（`msgdb-dump` 用 `go build`，`evtx-tools` 用 `cargo build --release`）
- 同一双语单文件 en-US / zh-CN 各跑一遍（`--msg-db` 显式指向 `hotech-msg-*.db`，
  v9 = v7 + eventlog_sources + param_types + value_maps.lang 双语化，ADR 0003/0004/0005）
- 顺序对齐 + 五字段归一化比对（标准库 Python，uv-ready）
- 机器可读汇总 `report.json`（verdict + 分通道 categories，供下轮回归对比）

## 快速开始

前置条件：管理员 PowerShell（步骤 01 需导出 Security 通道）、Go 1.20+、cargo、Python 3.12+。

一键全链路（`evtx-parity-test/` 下执行）：
```powershell
.\run-all.ps1
```
单步（可单独重跑）：`01-export` / `02-baseline-parse` / `03-build-msgdb` /
`04-build-evtx-tools` / `05-new-parse` / `06-compare.py`，详见测试项目 README。

## 比对口径（06-compare.py）

- 对齐：同一文件按出现顺序逐条对齐，基线 `Event[N]` 序号顺序对应新解析 `sequence_number` 顺序。
  基线 `Event[N]` 是 wevtutil 查询序号（从 0 起），不是 `event_record_id`，不可按记录号对齐。
- 主链：wevtutil 文本基线 vs `new-parsed/` 同语言侧（06 v5 从基线文本 G0 探测 `baseline_lang`
  ——zh 判据 `'Level: 信息'`、en 判据 `'Level: Information'`——主比对随基线语言走；
  严格五字段：EventID / Provider / Channel / TimeCreated 两边本地时间容差 2s / Message 空白归一）。
- 对照：另一语言侧只计覆盖率（条数 / EventID 顺序一致率 / 空消息率），不逐条比 Message；
  report.json `baseline_lang` 供 acceptance-check G0 门核对与上轮一致。
- Message 分类：A-渲染格式差（已豁免）/ B-裸参数未渲染 / C-无模板fallback / D-实质分歧。
<!--P2-->
## 已核实的关键事实（勿回退）

1. `evtx2text.lua` 无参数、输入写死 `msys64-Tools` 下 `evtx` 子目录、GBK 编码、
   依赖 `lfs` + `wevtutil/tr/sed/perl/iconv`，结尾有 `PAUSE`（02 脚本以 stdin=NUL 跳过）。
2. 02 脚本必须用 `System32` 下 `cmd.exe` 全路径调用，裸 `cmd` 会被 PATH 上某 msys64 的
   `cmd` bash 垫片遮蔽，导致 lua 根本没跑（2026-09-03 实测 0 产出根因）。
3. 基线 Date 看似 UTC（带 Z）实为本地时间：wevtutil 文本渲染本地时间但保留 Z 后缀
   （与 raw XML SystemTime 差 +8h，Setup 首条 EventRecordID=1 实测）；`evtx-tools` 的时间
   转换输出与 raw UTC+8 一致，无上游 bug。基线小数秒被截断补零，容差覆盖。
4. `evtx-tools --format both` 与 `-o` 单路径会让 db 与 csv 抢写同一文件，05 脚本按 db/csv 分两次跑。
5. 新解析 CSV 内每 batch 夹带一行 BOM+表头重复，06 按 `event_id` 非数字过滤；建议上游修存储层。
6. 基线块尾 TRAIL 剥除必须含分号冒号与空白：块尾冒号来自原文末尾 NUL 经 sed 转换，非消息语义。
   曾尝试保留冒号导致全量失配（matched=0），已回退。
7. **R12（2026-09-07，ADR 0004）**：miss cohort 的基线渲染由**源注册状态**决定——未注册源
   EID≠0 → `N/A`（101/101 全量实证）；classic/winevt 注册源 miss → 真消息（EventSystem 即
   winevt miss 也有消息，不得按 winevt 判 N/A）。实现：`eventlog_sources` 附表
   （1272 行快照，v8 必需表），缺表=坏库即错；空表=采集失败=状态未知=特性关。采集器开子键必须用全路径（关闭的父句柄
   开子键会静默失败，2026-09-07 bug）。
8. **R10/R18（2026-09-07，ADR 0005）**：wevtapi 按 CRIM TEMP 描述符 +4 的 TDH
   `(InType,OutType)` 字节类型化渲染，规则表（探针实证，勿凭记忆扩充）：
   `(14,12)/(08,12)→0x%X` 大写、`(15,13)→0x%X` 大写、`(0A,13)→0x%x` 小写、
   `(07,20)/(14,20)→HRESULT FormatMessage`（0x8007xxxx 取低字，查 R15 表回退 0x%08X）、
   `(11,02)→9 位纳秒小数`、`(08,08)→十进制且不接管（map 照常）`。
   hex/bool/HRESULT InType 一律**跳过 value map**（TDH 语义，over-mapping 根因）。
   `param_types` 附表（v8 必需，缺表即错），双采集路径（enrichFromCRIM + ExtractProviderVariants）都挂——
   只挂主分支会漏掉 CRIM+MUI/provider 中心路径的 WU/Kernel-Boot/Wininit 等大簇（实测教训）。
9. **R18**：evtx crate 对 manifest 布尔参数输出 JSON 原生 `true/false`，取值层必须
   `Value::Bool → "true"/"false"`（Kernel-Boot 20「was .」根因）。
10. **R19（2026-09-08，批次 1）**：CRIM VMAP 条目字符串引用是 **0xDn 字符串表家族**
    （0xD2/0xD1/0xD0/0xDC…，按完整 message ID 键），resolves 须按 `(ref>>24)&0xF0==0xD0`
    泛化——只认 0xD200 会漏掉 Ntfs(0xD1)/KPP(0xD0)/System-Events(0xDC) 的全部条目
    （value_maps 8540→29718）。表可能只在 MUI 中（KPP 主 DLL 0 条、MUI 36 条）。
11. **R20（2026-09-08，批次 2）**：①param_types/param_value_maps 是 (provider,eid,index)
    键，但同事件不同 variant 模板的同名 index 可能是**不同参数**（KGA 1 v3 index3=Reason
    vs v4 index3=TimeDeltaInMs）——链接/类型带 param_name 落库，读端按 variant 自身参数名
    校验后挂载（空名=未知=信任）；②bool (13,*) 需接管臂（否则被 enum/value-map 层改写成
    Disabled/Enabled）；③(8,8) 无 map 链接时接管（否则 Layer-1 hex 启发式再 hex 化）；
    ④format_for_display Step 2.5（9 位小数剥 000）已移除——它与 (17,2) 纳秒补位冲突，
    小数位差异由 06 norm_msg_time_frac 在比对层归一；⑤03 重建前必须删旧 DB 文件
    （CREATE TABLE IF NOT EXISTS 会保留旧表结构，param_name 列曾静默缺失）。
12. **消息表排查路径（2026-09-14，阶段 C/D）**：EventLog 6013 的消息在 netevent.dll MUI 的
    **RT_MESSAGETABLE（type 11）**，ID 0x8000177D。msgdb-dump 的 DUMP_STRINGS 只扫
    RT_STRING（type 6）——**勿再以其 0 命中作为"type 11 消息不存在"的证据**；
    同类排查一律走消息表抽取路径（阶段 D 已以 well-known 注入接入，6013×16 全 matched）。
13. **injectWellKnownVariants 已实装（2026-09-14，阶段 D，勿回退为 stub）**；
    `builtin_messages.go` 的 6013 硬编码已改正、6011 假条目已移除（勿恢复）。
14. **NTSTATUS 渲染臂（2026-09-14，阶段 E）**：ntdll.dll RT_MESSAGETABLE 2975 条已入
    `NTSTATUS-System-Error` 全局组（`parameter_refs`）；渲染臂 `(_, 0x1F)` 为
    **hit-only（未命中 passthrough）**——勿改为全接管，否则殃及其它 0x1F 参数
    （Wininit 26 `STATUS_SUCCESS` 即经此臂闭合）。
15. **canon 双归一规则（2026-09-08，阶段 A）**：06-compare.py 的 `\\?\`≡`\\\` 双向与
    U+200E/200F 剥离当前零命中仍保留——防御性正确（注释含根因，可一键摘除）；
    勿以"零命中"为由删除。
16. **EXEMPTIONS 双表（2026-09-08 起，阶段 B）**：`06-compare.py` 与
    `scripts/acceptance-check.py` 的豁免表必须**同一提交同步改动**；登记表唯一真源为
    `docs/residual-zeroing-plan-2026-09-07.md` §4。终态（2026-09-30 P8 同步后）**4 项**：
    CAPI2 4111、SAM 16962、WPD 24577/24579；SPP 1004（P5 起稳定转绿）与 disk 51
    （R21 经典映射后不再命中）已同步摘除。豁免键（channel, provider, eid）**语言无关**，
    语言翻转后白名单自动继续生效（zh 轮实测命中 17 条）。
17. **G0 语言漂移（2026-09-29，勿回退）**：wevtutil `/f:text` 渲染语言跟随**本机 UI 语言**，
    本机 2026-09 下旬由 en-US 切为 zh-CN（`Get-WinUILanguageOverride`=zh-Hans-CN），曾致
    基线 vs en 主比对全量假阳性 D（matched 32020→1840）。铁律：任何重跑前先看基线渲染语言；
    06 v5 主比对随 `baseline_lang` 走，acceptance-check 新增 **G0 门**（语言未知或与 prev
    不一致即 FAIL）。Setup 通道全绿不受语言翻转影响的机理：其 provider 无 zh 资源，
    EvtFormatMessage 回退 en。
18. **parameter_refs 语言层（2026-09-29，v9 机制，勿回退）**：表 PK 本就含 lang
    （schema 未升版）；萃取端参数文件（ParameterMessageFile）必须按语言萃取——基础文件为
    en-US 层、非 en 语言走 MUI companion、基础文件无表时回落 en-US MUI（**必须用
    `len(msgs)>0` 判断而非 nil**：MUI-only 布局的基础文件如 netevent.dll(28KB) 返回空非 nil
    map，nil 判断会挡住 en-US 回退臂造成 en/zh 失衡 12252 vs 118355）；渲染端
    `load_param_refs(lang)` 语言行进首选层、en-US 行进回退层，expand/lookup 按
    首选→en-US 回退（镜像 EvtFormatMessage MUI 链）。Win32-System-Error 按 langCodes
    每语言一套（`FormatMessage` langID 显式 1033/2052…）；**ntdll 的 zh-CN MUI 确有
    NTSTATUS 表（2975 条）**——阶段 E 注释"MUI 不含此表"已过时（勿据旧注释跳过）。
19. **管线运维坑（2026-09-29）**：①`wevtutil epl` **不覆盖已存在文件**（报"文件存在"），
    重导前必须先删目标文件；②Security 通道导出需管理员，本会话用 UAC 提权包装
    （`scripts/_01-security-elevated.ps1`）；③提权运行的 .ps1 必须**纯 ASCII**——无 BOM
    UTF-8 的中文注释被 PS5.1 按 GBK 读入会吞换行（与坑位 GBK 控制台同源）；④`wevtutil qe`
    查询导出的 .evtx 文件必须加 **`/lf:true`** 且用绝对路径（相对路径/缺 /lf 报"指定的通道
    路径无效"——多轮空结果假象的根因）；⑤wevtutil 输出**重定向到文件时是 ANSI/GBK**
    （控制台管道才是 UTF-16），python 侧解码先试 GBK。
20. **R21 经典源四修复（2026-09-29，勿回退）**：①`classicVariantRows`：classic provider 的
    event_id = 消息表原始 ID & 0xFFFF（事件记录只带低 16 位；6013 的 well-known 注入即此
    机制的手工前例，勿回退注入本身）；②`warmup_variants` 空 cohort 回落 en-US 行（wevtapi
    MUI 回退链；Setup 通道基线为 en 即此形态）；③classic eid=0 miss → FormatMessage(0)
    文本（`lookup_win32_error0`，孤儿 ISV provider 177 条 100% 实证；事件 Data 参数不参与
    渲染）；④06 canon `帐户`→`账户` 归一（SCM 7045 译文 servicing 漂移：磁盘唯一载体
    zh-CN\services.exe.mui 是"帐户"版，wevtapi 渲染"账户"版且文件不在盘上，不可萃取）。
21. **R12b N/A 精度：registered-but-no-resources（2026-09-30 夜间 P1，勿回退）**：miss 臂
    N/A 判定在既有"未注册 ∧ EID≠0"之外追加 **"已注册但零消息资源"** 同判 N/A
    （CfgChangeMonitor×80 实证：`eventlog_sources` 有行、`provider_message_variants` 0 行，
    wevtapi 同样渲染 N/A，fallback 长文全错）。实现：open 时加载
    `providers_with_variants: HashSet`（`SELECT DISTINCT provider_name`，**空表=未知=保守
    true**）+ `pub fn has_any_variant(name)`；miss 臂追加 `&& has_any_variant(prov)`；
    既有"未注册→N/A"语义原样保留，全量 dry-run 实证该规则仅命中 CfgChangeMonitor 一簇。
22. **value_maps schema v9 双语化（2026-09-30 夜间 P2，勿回退）**：`value_maps` 加
    `lang TEXT NOT NULL DEFAULT 'en-US'` 并**列入 PK**；metadata `schema_version=9`
    （Rust `open()` 硬校验，旧库拒开）。萃取端 per-lang stringTable：主 DLL
    RTStrings+D2Strings 为底、该语言 MUI D2Strings **同键全覆盖 overlay**（非补缺——镜像
    MUI 整族本地化语义，旧"主 DLL 先填、MUI 只补缺"即 en 偏置根因）；两条写入路径
    （CRIM `InsertValueMap`/`writeValueMaps` 与 WELM import Step 6）都带 lang。
    渲染端 `load_value_maps(lang)` 目标 lang 行进首选层 + en-US 行进回退层（镜像
    param_refs v9 双层），`get_value_map`/`resolve_value`/param_value_maps merge 一律
    首选→回退。行为红线：R19/R20 既有 value-map 修复零回退（System D 只减不增实证）。
23. **渲染层四修复（2026-09-30 夜间 P3–P6，勿回退）**：①classic `<Binary>` hex blob 以
    **大写、无 0x、无分隔**追加为末位参数 `%n+1`——**仅模板命中路径**（include_binary=true），
    miss fallback 不追加（NetBT 4311 模板 `%2` 引 Binary、Data 值占 %1 实证；全 corpus
    50 簇排查仅 2 簇模板引用 %n+1，追加对未引用模板无输出差）；②UserData 嵌套展平带
    **形态门控**：仅根子节点=无 `#text`/`$` 叶子的 Object 触发，深度 2 丢容器名、
    深度 3 用 `结构体_字段` 前缀（`ProcessInfo.Process_1.Name`→`Process_1_Name`，msgdb
    命名规则同构；RE-Detector 2004×9 实证）；③大十进制 hex 化（Step 3，≥0x80000000）新增
    **GUID-span 回扫 guard**：数字串被 `scan_guid` 识别为 GUID 形 token 尾段时跳过转换
    （WER 1001 文件名 `3003916586` 实证；孤立十进制 TS35 历史修复不回退）；④substitute
    消费 `%N` 后的 **`!S!` 字符串族 specifier**（s/S/hs/ws/ls；`!x!`/`!d!` 不消费）且
    **pass2（参数值侧）不做 `%%`→`%` 塌缩**——`%%2149884192` verbatim（SCM 7023 实证，
    与 EvtFormatMessage 两遍语义对齐；pass1 模板侧塌缩保留）。

## 已知上游缺口（首轮闭环发现；ADR 0003 单文件迁移已修 #1）

1. [已修，已验证] msgdb-dump v6 双写漏映射表：单文件 v7 写三表（5732/8540/13352，2026-09-04 实测），
   v7 首轮 Security D 类 8078→59，`%1843` 缺口已闭合；剩余 59 为长串 GUID/时间格式残留（0.2%）。
2. Variant 降格分歧实例（Application#309，SPP 1016）：事件实例 opcode=0 时阶梯选了 Grace-timer，
   而 wevtutil 现场拿到 Proof-of-Purchase，降格策略与运行时语义的真实分歧点。
3. CSV 表头重复、`--format both` 同路径抢写（见上）。
4. [2026-09-30 夜间转出] **EventLog 6008×2（BASE-ARTEFACT）**：我方文本逐字节正确（真实 LRM
   U+200E `‎2026/‎8/‎21`），基线管线 iconv 把 LRM 变 `?`（`?2026/?8/?21`），R5 手术只清 `-` 形
   日期未覆盖斜杠形——上游不可修（我方无错），**建议白名单 System/EventLog/6008/PARSE**
   （CAPI2 4111 同构；动豁免表须双表同步+登记表同步，超夜间阶段授权未动）。
5. [2026-09-30 夜间转出] **CodexSandboxService 1001/1002×2（UNREG-FALLBACK）**：源未注册
   （注册表无键、无 ETW manifest），基线把 eid 当 Win32 错误码回退渲染（1001→"递归太深；
   堆栈溢出。"）；我方 N/A。修复需 resolver 加"无模板→eid 走 Win32 系统错误表"回退，
   牵动 N/A-parity 谓词，非机械。
6. [2026-09-30 夜间转出] **e1dexpress 32×8（MUI-WRONGLANG）**：链接 netevent.dll;DriverStore\…
   \e1dmsg.dll（语言中立）；msgdb 该 provider 全部 14 条变体 zh-CN/en-US 键下存的都是西语文案
   → DriverStore MUI 目录解析错选 es-ES 且双语同文错存。msgdb-dump（Go）MUI 解析重写
   （GetSystemMUILanguages/RC 常量，非目录枚举序）。
7. [2026-09-30 夜间转出] **VMnetAdapter 36×8（EID-QUALIFIER）**：classic 消息 id =
   **qualifiers<<16 | eid**（raw `EventID Qualifiers='16392'`→0x40080024），msgdb 变体键缺
   qualifiers 维度 → 按裸 eid 36 首中链首 IoLogMsg.dll 配额文案（错库）。msgdb-dump 补
   qualifier 维度 + "链序+带 qualifier id"查找（与 #6 同管线可合并实施）。
8. [2026-09-30 夜间转出] **Wininit 23×1（TDH-SYMBOL）**：HRESULT 0x10000000（XML 实证
   268435456），wevtutil 经 TDH 内建符号表渲染 "STATUS_SUCCESS"；我方 typed 渲染 0x10000000。
   需内建 NTSTATUS/HRESULT 符号表，非机械。
21. **R26 classic 全格式 msgid + Qualifier 全键探针（2026-09-30，勿回退）**：
    ①`classicVariantRows` 保留**原始全格式 message_identifier**（raw>0xFFFF 时）且不再做
    同 eid 去重——EventMessageFile 链/同表内的姊妹条目（IoLogMsg 0x40000024 配额 vs
    vmnetadapter 0x40080024 启动，低 16 位同为 36）此前因 (eid, msgid=0, dll_id) PK 相撞
    被 INSERT OR IGNORE 静默吞掉链尾真身；②渲染端 `resolve_variant_full_id(key, full)`：
    事件 EventID 带 Qualifiers 属性时以 (q<<16)|eid 精确探针压过通配梯子（wevtapi 全键
    查找语义）；**该探针位于 resolved 闭包内，外层 guard 已持 db 锁——严禁二次
    db.lock()（std Mutex 不可重入，重入即死锁，09-30 实测 05 全量挂死 20 分钟根因）**。
22. **R27 多语言单文件消息表分取（2026-09-30，勿回退）**：DriverStore e1dmsg.dll 型文件
    单文件内嵌多语言 RT_MESSAGETABLE 块（e1dmsg 10 块：en=37/zh=36/es=36），merged
    Messages last-write-wins 会让随机语言占据所有 lang 键（e1dexpress 西语入库根因）。
    `CombinedPEData.MessagesPerLang` 按资源语言 ID 分桶 + `SelectMessagesForLangLCID`
    （精确→0x0409→merged）三层选择；flat classic 路径与 extractor step-3 均已接
    `lcidForLangCode`。
23. **R28/R29 比对与回退收尾（2026-09-30，勿回退）**：①06 `clean_baseline_artefacts`
    增斜杆日期形态 `?YYYY/?M/?D`（EventLog 6008 的 LRM 被 iconv 吞成 '?'——与 R5 短横线
    形态同源，只删该形态）；②R12 未注册 N/A 精化：**纯 classic（事件无 Provider @Guid）
    未注册源 eid≠0 → 先查 Win32 错误文本（`lookup_win32_error_fallback`，命中即渲染——
    Codex 1001/1002 live 实证；eid 1000 无文本两侧同为 N/A），带 @Guid 的未注册仍 N/A**
    （事实 #7 的精化而非回退）；③`format_hresult` else 支（非 0x8007 facility）：
    success 严重级 ∧ code=0 → NTSTATUS 表探针(0)=STATUS_SUCCESS hit-only（Wininit 23
    0x10000000 live 实证；value=0 不走此臂）。
24. **R31–R34 豁免全清四修复（2026-09-30，勿回退）**：①R31 `clean_baseline_artefacts`
    CJK 日期形态 `?YYYY?年?M?月?D?日`（R5/R28 家族第三变体，CAPI2 4111）；②R32
    占位符字面量透传**必须以 `event.qualifiers.is_some()` 门控 classic**
    （`substitute_format_message_protected_classic`）——wevtapi 双语义：classic 零参
    字面量（WPD）/manifest 缺参补空（VSS 等 233 条），未门控即 0→233 全量回归；
    ③R33 Strategy 2 跳过 `#` 前缀伪键（SAM 16962 `#attributes` 挤位）；④R34 空串
    `#text` 不算参数**但有 Binary 时除外**（NetBT `<Data/>+<Binary>` 的空 Data 是
    真实 %1）；⑤R33b `classify_msg_diff` canon 相等性前置（SDDL 分号触发 B 类误判）；
    ⑥`info` 子命令版本校验须与 parse 同步跟进 schema 版本（R30 曾停在 v7）。

## 基线（严格口径，2026-09-30 终态，**verdict=PASS，绝对 100%**：45166/45166 一致，diff=0，exempted=0）

> 主比对 zh-CN（G0）。战绩链：09-05 起点 diff=1127 → en 终态 771/97.6190% → 09-29 语言
> 翻转事故（matched 1840）→ v5+param_refs 双语化 96.58% → R21 四修复 99.4319% → 夜间
> P1–P7 99.9153% → R26–R29 五修复 99.9621%（豁免 17 条）→ **R31–R34 五修复：豁免全部
> 闭合，diff=0 且 exempted=0——比对集内无双路径分歧**。
> 门禁：G0–G4 全过 exit=0（M1=100.0000%、M2=100.00%）；G6 关豁免 diff=0=0+0；确定性门
> 两次全等；单测 Go 137 / Rust 249 绿。EXEMPTIONS 双表 2026-09-30 清空（全部闭合，
> 登记表 §4 同步关闭）。

| 通道 | 基线 | 新解析 | 一致 | 差异 |
|---|---|---|---|---|
| Application | 11012 | 11012 | 11012 | **0** |
| Setup | 676 | 676 | 676 | **0** |
| Security | 27253 | 27253 | 27253 | **0** |
| System | 6225 | 6225 | 6225 | **0** |

最后 17 条豁免的闭合机理（R31–R34，全部 TDD 红→绿 + 全量回归守护）：
CAPI2 4111×1 = R31 `clean_baseline_artefacts` CJK 日期伪影手术（R5/R28 家族第三变体）；
SAM 16962×8 = R33 Strategy 2 跳过 `#` 序列化伪键（`#attributes` 对象值挤占 %1）；
WPD 24577×4/24579×4 = R32 classic 零参占位符字面量透传（**Qualifiers 属性 = classic 金标**，
`substitute_format_message_protected_classic`）+ R34 空串 `#text` 不算参数（有 Binary 时除外）。
**wevtapi 双语义实证（R32 教训， 勿改）**：classic 零参 → 字面量透传；manifest/CRIM 缺参
→ 补空（VSS 8224 等 233 条全量回归实证——R32 未门控时 0→233）。

历史（en 口径，2026-09-15 终态）：diff=771（matched 32020/32801，M1=97.6190%）；
2026-09-05 起点 diff=1127。

严格口径（2026-09-04 起）：hex 大小写 / GUID 花括号 / SDDL 分号 / 基线 `?` 不再豁免，
上游渲染必须完全一致。Security 全绿实证链：`%1843` 映射修复 + 零 GUID 加花括号 +
`*LogonId/Status/SubStatus/EventSourceId` 大写 + SDDL `S:ARAI(` 扩展。R1（hex 统一小写）证伪回退：
基线同消息内大小写混杂（`0x3E7` vs `0xC20`），大小写来自参数值本身。R2（GUID 去花括号）证伪回退：
花括号 GUID 仅零 GUID 特殊，其余 passthrough（路径 `{A73E…}` 大写 vs SDDL `{cb4…}` 小写为两源）。
B（裸参数）/ C（无模板）为已知未覆盖场景，当前全场 0。历史：v6 双写轮 22692/32801；v7 豁免轮 30387/32801。

## 循环闭合用法

1. 改上游（建议先修缺口 1：v6 映射表双写）。
2. 重跑全链路 `run-all.ps1`。
3. 对比新 `report.json` 与本轮 `by_channel.categories` 分布：D 类归零即闭合；新增分类即回归。
4. `report.json` 即下轮的对比基线。

## 相关文档

- 测试项目 README（`evtx-parity-test/README.md`）与术语（`evtx-parity-test/CONTEXT.md`）
- 比对脚本（`evtx-parity-test/scripts/06-compare.py`）
- 集成指南（`integration-skill.md`）、`evtx-tools` 技能、`msgdb-dump` 技能
