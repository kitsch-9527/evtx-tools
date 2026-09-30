# Windows 事件消息渲染机制调研——公开知识与 evet 实证的系统性对齐

- **调研日期**：2026-09-30
- **调研对象**：Windows 事件日志（EVTX/EVTX 前身 classic EVT）离线语义还原中"把二进制事件渲染成与 wevtutil/wevtapi 完全一致的人类可读消息"所需的全部机制
- **对齐基准**：evet 项目（本仓库）双链路比对闭环已于 2026-09-30 达成 `verdict=PASS`（diff=0，matched 44873/44890，M1=99.9621%），实证事实清单见 `skills/parity-test-skill.md`"已核实的关键事实（勿回退）"#1–#23 与 `docs/phase-reports/2026-09-30-r26-r29-pass.md`
- **方法**：Microsoft Learn 官方文档 + PE/COFF 规范 + 各开源仓库源码/README/spec 逐项核对，与 evet 黑盒试探 + 活体取证得到的实证逐条对照；查不到公开资料的如实标注"未见于公开资料"

## 结论摘要

1. **消息渲染的"骨架"有完整官方文档**：classic 源注册模型（`EventMessageFile` 分号分隔多 DLL）、`MESSAGE_RESOURCE_DATA` 消息表二进制格式、消息占位符语法（`%1`、`%b`、`%!s!`、`%%`）、MUI 回退链、`EvtFormatMessage` 各 flag 与查找顺序——均可与 evet 实证一一对齐。
2. **编译态清单（CRIM/WEVT_TEMPLATE）无任何官方文档**；2026-06 出现的首篇系统性公开拆解（kernullist）确认了 `CRIM` magic、`EVNT` 48 字节条目、`CHAN/LEVL/TASK/OPCO/KEYW/MAPS/BMAP/VMAP/TTBL/TEMP` 等块家族的**存在与骨架**，但 evet 实证中最关键的黑盒语义——**CRIM 8 元组 variant 选择阶梯、`TEMP` 描述符 +4 处 `(InType,OutType)` 字节、`0xDn` 字符串表按完整 message ID 键控与 MUI-only 归属、双语 overlay 规则**——均超出公开资料，属 evet 独立逆向。
3. **`(Qualifiers << 16) | code` 全键**已被公开侧印证：libevtx 格式规范明文给出该公式；但"精确探针压过通配梯子"的 wevtapi 查找优先级语义仍只有 evet 有实证。
4. **开源生态没有 evet 的同类物**。三条路线各有天花板：A) 调 wevtapi（准确但绑机器注册状态与 UI 语言）；B) `LoadLibraryEx+FormatMessage`（官方背书，但仅覆盖 classic 表）；C) 全静态复刻（evet 独有，已以 diff=0 闭环）。现有工具最远只走到"EvtxECmd 人工 Maps 模板库""EventLogExpert 跨机捕获 Provider DB""plaso/Hayabusa classic 消息表萃取"这一层。
5. 个别指定调研对象无法核实：**megatron（matthewdunwoody）仓库当前 404、Wayback 无存档、搜索无收录**，本文按"不可验证"如实记录，其代表的路线 B 机制本身由官方文档背书。

---

# 第一部分：Windows 事件消息渲染的内部实现（逆向视角）

按一条事件从"注册"到"渲染"的数据流顺序展开。每节先给"官方文档怎么说"，再给"实现细节/evet 实证补充"。

## 1.1 Publisher 注册模型

### classic（XP/2003 时代模型，Vista 后仍服务大量存量源）

**官方文档**：classic 模型把"哪个日志里有哪个源、源用什么消息文件"全部放在注册表
`HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\Eventlog\<log>\<source>`
（[Eventlog Key](https://learn.microsoft.com/en-us/windows/win32/eventlog/eventlog-key)、[Event Sources](https://learn.microsoft.com/en-us/windows/win32/eventlog/event-sources)）。每个源可含：

| 注册表值 | 官方描述 |
|---|---|
| `EventMessageFile` | 事件消息文件路径，**可用分号分隔多个文件**（REG_SZ / REG_EXPAND_SZ） |
| `ParameterMessageFile` | 参数消息文件路径（承载 `%%n` 参数串） |
| `CategoryMessageFile` / `CategoryCount` | 类别消息文件与类别数 |
| `TypesSupported` | 现行文档标注为 **Unused**（历史上为事件类型 bitmask——文档语义漂移的一例） |

源数量上限 16384；源名不能与日志名相同、不能含反斜杠。

**渲染流程官方口径**（[Message Files](https://learn.microsoft.com/en-us/windows/win32/eventlog/message-files)）：`RegOpenKey` 源键 → `RegQueryValueEx` 读 `EventMessageFile` → `LoadLibraryEx` 装载 → `FormatMessage` 按 message identifier 取文。官方同时明确两个"渲染器须自己兜底"的怪癖：

- `FormatMessage` 只替换 `%n` 插入串，**不展开 `%%n` 参数串**——调用方须解析结果、加载 `ParameterMessageFile`、二次 `FormatMessage` 后回填（evet `parameter_refs` 双遍展开的官方出处）；
- 分号分隔的多 DLL 链要求渲染器逐个文件查找 message ID（evet 链序查找与 R26"链尾真身"问题的根源）。

**evet 实证补充**：未注册源与"已注册但零消息资源"源的渲染分支（N/A vs Win32 错误码回退）官方文档只写到"没有消息文件时 Event Viewer 无法映射 identifier 并显示错误"这一句；**N/A 的精确谓词、eid=0 与未注册 classic 的 `FormatMessage(eid)` 回退，均无公开文档**（事实 #7/#20/#23）。

### manifest（Vista+ 模型）

**官方文档**：编译态清单随 `MessageFileName` / `ResourceFileName` / `ParameterFileName` 三个属性注册到
`HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\WINEVT\Publishers\<provider-guid>`（channel 另行注册于 `...\WINEVT\Channels`）。API 侧由
[EvtOpenPublisherMetadata](https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtopenpublishermetadata)（参数 `PublisherId` + 可选 `Locale` + 可选 `LogFilePath`）与
[EvtGetPublisherMetadataProperty](https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtgetpublishermetadataproperty) 暴露；`LogFilePath` 参数允许从**归档日志内嵌的 provider metadata** 取元数据——这是官方给"离线"开的唯一口子。清单的**编写侧 XML schema**（publisher 元素三属性、channels/levels/opcodes/tasks/keywords/maps 子元素）有公开文档（WES EventManifest schema，见 [EvtFormatMessage 页引用的 schema 链接](https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtformatmessage)），但其**编译态二进制（CRIM）无官方文档**（见 1.4）。

**evet 实证补充**：`eventlog_sources` 快照（1272 行）+ `provider_message_variants` 的"已注册但零资源"判别（事实 #21/R12b）在官方文档中无对应命题——`EvtOpenPublisherMetadata` 打开成功与否并不等于"该 provider 有可渲染资源"。

## 1.2 PE 消息资源

### 资源目录三级结构

**官方文档**：PE/COFF 规范的 `.rsrc` 节把资源组织为 **Type → Name → Language** 三级树
（[PE Format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format)）：顶层 `IMAGE_RESOURCE_DIRECTORY`（`NumberOfNamedEntries`/`NumberOfIdEntries`）→ 目录项（高位标志名字符串 vs 整数 ID、偏移指向子目录或数据项）→ 叶子 `IMAGE_RESOURCE_DATA_ENTRY`（`DataRVA`/`DataSize`/`CodePage`/`Reserved`）。规范明确：**MUI 相关文件把 Language 字段置 0**，语言专属资源拆到 `.mui`。消息相关的资源类型：`RT_MESSAGETABLE`（type 11）、`RT_STRING`（type 6）、`RT_RCDATA` 等均在 winnt.h 常量表中。**evet 实证补充**：`RT_STRING` 的 RC 编译器按 ID 高位每 **16 条**一块（`(id>>4)-1` 定位块，块内 16 条）是公开的实现惯例；而 **R27 发现的"单文件内嵌多语言 RT_MESSAGETABLE 块"**（DriverStore e1dmsg.dll 10 块：en=37/zh=36/es=36）正是三级树 Language 层的直接利用——公开资料只讲结构，不讲"last-write-wins 合并会错语言"这类工程陷阱（事实 #22）。

### RT_MESSAGETABLE（MESSAGE_RESOURCE_DATA）

**官方文档**：winnt.h 定义三结构，Learn 有结构页
（[MESSAGE_RESOURCE_DATA](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-message_resource_data)、[MESSAGE_RESOURCE_BLOCK](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-message_resource_block)、[MESSAGE_RESOURCE_ENTRY](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-message_resource_entry)）：

```c
typedef struct _MESSAGE_RESOURCE_DATA {
    DWORD NumberOfBlocks;
    MESSAGE_RESOURCE_BLOCK Blocks[1];   // 实长 = NumberOfBlocks
} MESSAGE_RESOURCE_DATA;

typedef struct _MESSAGE_RESOURCE_BLOCK {
    DWORD LowId;            // 块内最小 message ID（含）
    DWORD HighId;           // 块内最大 message ID（含）
    DWORD OffsetToEntries;  // 相对 MESSAGE_RESOURCE_DATA 起始的偏移
} MESSAGE_RESOURCE_BLOCK;

typedef struct _MESSAGE_RESOURCE_ENTRY {
    WORD  Length;           // 本条目总长（含头）
    WORD  Flags;            // 0 = ANSI，1 = UTF-16
    UCHAR Text[1];          // 以 NUL 结尾的消息文本
} MESSAGE_RESOURCE_ENTRY;
```

查找算法：块按 `LowId` 升序、二分/线性命中 `LowId <= id <= HighId` 后跳 `OffsetToEntries`，条目按 `Length` 链行走、message ID 自 `LowId` 顺序递增。**4 字节边界对齐**由结构的 DWORD 基调隐含；官方无文字明说条目级对齐，evet 实证条目行走按 4 字节对齐处理（未见于公开文字，属结构定义隐含 + 实证）。**evet 排查教训（事实 #12）**：EventLog 6013 的消息在 netevent.dll MUI 的 RT_MESSAGETABLE（type 11），ID `0x8000177D`——只扫 `RT_STRING` 的抽取器会以"0 命中"误判"type 11 消息不存在"，此类排查必须走消息表抽取路径。

### 消息占位符语法

**官方文档**（[Message Text Files](https://learn.microsoft.com/en-us/windows/win32/eventlog/message-text-files)，配合 [wsprintf](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-wsprintfa) 的 printf 类型）：

- `%n[!format_specifier!]`：插入符，n=1–99，缺省 `!s!`；format 可含 `*`（宽度/精度），此时**消耗第 n+1、n+2 号插入**；
- `%0` 行尾无换行收束、`.` 生成句点、`!` 生成叹号、`%%` 生成百分号、`%n` 硬换行、`%b` 生成空格、`%r` 硬回车；
- 消息 ID = `Severity(2bit)<<30 | Facility(12bit)<<16 | Code(16bit)`（[Event Identifiers](https://learn.microsoft.com/en-us/windows/win32/eventlog/event-identifiers) 位图）。

**evet 实证补充（事实 #23④）**：`EvtFormatMessage` 的**两遍语义**——pass1 模板侧 `%%`→`%` 塌缩、**pass2 参数值侧不塌缩**（`%%2149884192` verbatim，SCM 7023 实证）；`%N` 后 `!S!` 字符串族 specifier（s/S/hs/ws/ls）被消费而 `!x!`/`!d!` 不被消费。官方文档只说"format 见 wsprintf"，**两遍差异与 specifier 消费集合无公开文字**。其余格式怪癖（`<Binary>` hex blob 大写无 0x 追加为末位参数、大十进制 hex 化的 GUID-span 回扫 guard）均为 evet 独立实证（事实 #23①③）。

## 1.3 MUI 本地化模型

**官方文档**：

- **LN + 语言卫星**：MUI 技术把语言中立（LN）主文件与语言专属资源分离，卫星文件命名为 **LN 全名 + `.mui`**（`netevent.dll` → `en-US\netevent.dll.mui`），存放于 **LN 所在目录下以语言命名的子目录**；LN 与 `.mui` 以 RC Config 中的 checksum（主版本号 + 文件名大小写敏感散列）绑定（[MUI Resource Management](https://learn.microsoft.com/en-us/windows/win32/intl/mui-resource-management)、[Resource Utilities](https://learn.microsoft.com/en-us/windows/win32/intl/resource-utilities)）。
- **回退链**：资源加载器按 **线程首选 UI 语言（含中立形）→ 进程首选 → 用户 UI 语言 → 系统 UI 语言（用户未设时）→ 系统默认 UI 语言**合并去重成有序回退表；找不到时用 RC Config 指定的"guaranteed good"回退语言（[User Interface Language Management](https://learn.microsoft.com/en-us/windows/win32/intl/user-interface-language-management)）。evet 的 G0 门（事实 #17）与 `load_param_refs/load_value_maps` 的"首选层→en-US 回退层"（事实 #18/#22）正是对该链的镜像实现。
- **SystemResources（.mun）**：Windows 10 1903 起系统二进制的资源外置到 `C:\Windows\SystemResources\<name>.mun`，逐步替代部分 `.mui` 卫星。**官方无 .mun 布局文档**，现有公开认知来自社区拆解（如 [NTCore: MUI files under the hood](https://ntcore.com/mui-files-under-the-hood/)、winclassic.net 与 Microsoft Q&A 的 1903 行为讨论）——属"结构仍是 PE 资源容器、加载归资源重定向器管"的层面。

**evet 实证补充**：任务书提到的 "MUIR.dll" 未见于任何公开资料与本项目语料（疑为 `.mui` 卫星家族之泛称）；实证中真正棘手的是 **MUI-only 布局**——基础文件小到不含消息表（netevent.dll 28KB 空表），表只在语言卫星里，且 **ntdll 的 zh-CN MUI 确有 NTSTATUS 表（2975 条）**（事实 #14/#18）；以及 **多语言单文件**（e1dmsg.dll）这种既非 LN+卫星也非 .mun 的第三形态（事实 #22）。任务书中提及的"帐户/账户 servicing 漂移"（事实 #20④：磁盘上 zh-CN services.exe.mui 为"帐户"版、wevtapi 渲染"账户"版且文件不在盘上不可萃取）属 **本地化 servicing 与在盘资源不同步**的活体现象，无公开资料记载。

## 1.4 事件清单与 CRIM（编译态 manifest）

**官方文档口径**：清单编写侧 XML schema 公开；注册后 TDH 能按 provider GUID 找到 `WEVT_TEMPLATE` 资源（见 [Instrumenting via TDH 相关文档](https://learn.microsoft.com/en-us/windows/win32/etw/retrieving-event-data-using-tdh)）。**WEVT_TEMPLATE 的二进制布局无任何官方文档**——Palantir 2018-12 的《Tampering with Windows Event Tracing》（blog.palantir.com，Medium 托管、拒绝机器人抓取，检索摘要确认其存在与该论断）即已直言其 under-documented。

**公开拆解现状（2026-06 起）**：kernullist 的 ETW internals 长文（[About ETW Internals](https://kernullist.github.io/kernullist-blog/posts/etw-internals-deep-dive/)，目标 build Windows 11 25H2）§14.2 首次系统公开了 CRIM 骨架，与 evet 实证吻合的**公开部分**：

- `WEVT_TEMPLATE` 资源以 **`CRIM` magic** 开头；头为 `{magic, Size, MajorVer, MinorVer, NumberOfProviders, [GUID, ProviderDataOffset] 数组}`；
- Provider 数据以 `WEVT_PROVIDER`（magic `WEVT`）描述，内含按偏移索引的元素家族：`CHAN`/`LEVL`/`TASK`/`OPCO`/`KEYW`（各 16 字节，KEYW 携 64-bit 关键字位掩码）、**`EVNT`（每条 48 字节）**、`MAPS` 容器（下挂 **`BMAP` 位图映射与 `VMAP` 值映射**）、**`TTBL` 模板表容器（下挂 `TEMP`，BinXml 模板 + 属性描述符）**、`PRVA`（属性值数组）；
- 公开工具生态：zodiacon/EtwExplorer（GUI 查看 provider metadata）、williballenthin/wevt_template（Rust 抽取+解析 WEVT_TEMPLATE，README 引用 forensikblog.de 资源 DLL 链接分析、TZWorks elmo、libyal/libfwevt、Palantir 文）、libyal/libfwevt（experimental，Windows Event manifest 二进制格式，见 [libfwevt](https://github.com/libyal/libfwevt)）、Microsoft PerfView。

**未见于公开资料、evet 为独立逆向的部分**（写文档时不得编造出处）：

- `EVNT` 48 字节 EventEntry 内部的 **8 元组字段语义**（event_id/version/level/opcode/task/channel/keywords/template 引用）的具体偏移与解析；
- **variant 选择的黑盒语义**：同一 (provider, eid) 多版本/多模板时 wevtapi 如何按事件实例的 (version, level, opcode, task, channel, keywords) 走"阶梯"并在缺项时降格——evet 有正反两例（Security 全绿 vs skill 缺口 #2 的 SPP 1016 opcode=0 降格分歧）；kernullist 仅在 TDH 侧提到元数据缓存键为 `(ProviderId, EventId, Version, Opcode, Task, Channel)` 元组，与 evet 的 8 元组族同构但**无选择语义**；
- **`TEMP` 描述符 +4 处的 TDH `(InType,OutType)` 字节对**的布局与取值表（事实 #8）；
- **`0xDn` 字符串表家族**：kernullist 仅一句带过"被引用字符串存在 0xDxxx 范围的 string table"；**按完整 message ID 键控、`resolves` 按 `(ref>>24)&0xF0==0xD0` 泛化、表可能只在 MUI 中（KPP 主 DLL 0 条/MUI 36 条）**均为 evet 独立逆向（事实 #10/#22）；
- **双语 overlay 语义**：per-lang stringTable"主 DLL 为底 + 该语言 MUI 同键全覆盖（非补缺）"（事实 #22/v9）——公开资料连该问题的存在都未记载。

## 1.5 EvtFormatMessage 渲染语义

**官方文档**（[EvtFormatMessage](https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtformatmessage)、[EVT_FORMAT_MESSAGE_FLAGS](https://learn.microsoft.com/en-us/windows/win32/api/winevt/ne-winevt-evt_format_message_flags)）：

- flag 族（数值即枚举序）：`EvtFormatMessageEvent=1`、`Level`、`Task`、`Opcode`、`Keyword`（多关键字返回 NUL 分隔串列表）、`Channel`、`Provider`、`Id`（按资源 ID 格式化，ID 来自 publisher metadata）、`Xml`（整事件 + RenderingInfo）；
- `PublisherMetadata=NULL` 的两种合法场景：转发事件自带 RenderingInfo；或仅引用 Winmeta.xml 定义（level=win:Error 等）——此时以 Winmeta provider 为格式化上下文；
- **查找顺序（官方 Remarks 明文）**：事件消息 → `messageFileName` → 找不到再查系统消息；**Level/Opcode/Keyword → 先 Winmeta 再 messageFileName**；**Task → 先 messageFileName 再 Winmeta**；**`%%n` 本地化参数 → `parameterFileName` 从左到右 → 再系统消息**；
- 语言由 `EvtOpenPublisherMetadata` 的 `Locale` 参数决定（0 = 调用线程 locale），实际命中走 1.3 的 MUI 回退链。

**evet 实证补充**：

- **classic 事件的 `Qualifiers` 参与**：libevtx 格式规范（[Windows XML Event Log (EVTX).asciidoc](https://github.com/libyal/libevtx/blob/main/documentation/Windows%20XML%20Event%20Log%20%28EVTX%29.asciidoc)）明文公开两种 message string identifier 映射法：①`<EventID Qualifiers="16384">7036</EventID>` 时 **`message id = (Qualifiers << 16) | eventID`**（0x4000<<16 | 0x1b7c = 0x40001b7c）；②Provider 带 `@Guid` 时进 `WEVT_TEMPLATE` 查 provider。evet R26 的 **"(q<<16)|eid 精确探针压过通配梯子"**（wevtapi 全键查找优先级语义、且链/表内姊妹条目不得因同 eid 去重而相撞）是该公开公式的运行时语义细化（事实 #21，亦即任务书所称 `(Qualifiers<<16)|code` 全键）。
- **variant 阶梯/降格**：manifest 事件多 variant 时按事件实例字段选 variant 的完整语义（含 opcode=0 降格、版本梯子、klass 通配）**无公开文档**——evet 以双链路比对把它逼近到与 wevtapi 逐条一致。
- **语言漂移**：`wevtutil /f:text` 渲染语言跟随**本机 UI 语言**、provider 无目标语言资源时回退 en（Setup 通道全绿机理）——MUI 回退链机制有官方文档，wevtutil 的具体行为无文档，为 evet G0 实证（事实 #17）。

## 1.6 TDH 类型化渲染

**官方文档**：

- [TdhGetEventInformation](https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhgeteventinformation)：从 TraceLogging/manifest/MOF/WPP 四类元数据源还原 `TRACE_EVENT_INFO`（源优先级与缓存键为社区/逆向侧补充，见 kernullist §8）。
- [TdhFormatProperty](https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhformatproperty)：按 `PropertyInType`/`PropertyOutType` 格式化属性值；`MapInfo` 可选——**不提供映射时按整数/位值原样输出**；IPv6 属性要求 `Length==sizeof(IN6_ADDR)` 且 `(InType,OutType)==(TDH_INTYPE_BINARY, TDH_OUTTYPE_IPV6)`；数据与清单不符返回 `ERROR_EVT_INVALID_EVENT_DATA`。
- [TDH_IN_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/tdh/ne-tdh-_tdh_in_type)（37 值：NULL…HEXDUMP、WBEMSID）、[TDH_OUT_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/tdh/ne-tdh-_tdh_out_type)（41 值：含 `HEXBINARY/HEXINT8/16/32/64`、`IPV4/IPV6/PORT/SOCKETADDRESS`、`ERRORCODE/WIN32ERROR/NTSTATUS/HRESULT`、`ETWTIME/CULTURE_INSENSITIVE_DATETIME`、`XML/JSON/UTF8`、`NOPRINT`）。**注意：现行 Learn 枚举页只列值名、无逐值描述**——逐值语义（hex 大小写、HRESULT 分支等）不在官方文字里。
- [TdhGetEventMapInformation](https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhgeteventmapinformation)：返回 value map / bitmap / pattern map；官方怪癖：**manifest 定义的 map 字符串尾部带一个空格**（"Monday "）。

**evet 实证补充**：evet 的规则表把 `(InType,OutType)` 字节对钉到具体输出格式——`(14,12)/(08,12)→0x%X` 大写、`(15,13)→0x%X`、`(0A,13)→0x%x` 小写、`(07,20)/(14,20)→HRESULT FormatMessage`（0x8007xxxx 取低字查 Win32 表、回退 `0x%08X`）、`(11,02)→9 位纳秒小数`、`(08,08)→十进制且 map 照常`；且 **hex/bool/HRESULT InType 一律跳过 value map**（TDH 语义）。官方只提供"有这两个枚举 + 有这些错误码输出类型"的骨架，**上述字节对→格式映射与"跳 map"规则全部来自 evet 探针实证**（事实 #8/#11）。同样：**Wininit 23 的 facility-only success HRESULT `0x10000000` → wevtapi 经 TDH 内建符号表渲染 `STATUS_SUCCESS`** 的内建符号表行为，官方枚举值（`TDH_OUTTYPE_NTSTATUS`）可佐证方向，具体命中臂为 evet 实证（事实 #23③、R28/R29）。**classic 与 manifest 两套元数据的合并规则**（WELM import、CRIM+MUI、provider 中心路径如何叠加 classic 消息表）无公开文档，属 evet 独立逆向。

---

# 第二部分：现有开源/工具方案对比

## 2.1 逐项调研

### omerbenamram/evtx（Rust）

高性能 EVTX 解析器，`evtx_dump` 输出 XML/JSON（[README](https://github.com/omerbenamram/evtx)）。BinXML 模板（含外置 `WEVT_TEMPLATE`，可经提取的 `.wevtcache` 恢复缺失模板）由共享 IR 构造，"no xml2json conversion"。**不做消息渲染**——输出的是事件结构本身，无 human-readable Message。evet 的 Rust 侧（`evtx-tools`）即构建于该 crate 之上，并补齐了它没有的渲染层。

### libevtx（Joachim Metz / libyal）与 python-evtx（Willi Ballenthin）

[libevtx](https://github.com/libyal/libevtx)：C 库访问 EVTX（LGPL-3.0+，alpha）；README 无消息字符串能力。[python-evtx](https://github.com/williballenthin/python-evtx)：纯 Python，`evtx_dump.py` 出 XML/JSON，`evtx_templates.py` 展示模板（Apache-2.0），明确不做"把模板替换成可读消息"。**两者的价值在格式规范**：libevtx 的 EVTX asciidoc 规范公开记载了 `Qualifiers` 映射公式与 WEVT_TEMPLATE 查找法（见 1.5），是 evet R26 结论的公开侧印证。python-evtx 系谱上溯 Andreas Schuster 2007 年对 EVTX 格式的首次公开拆解。

### megatron（matthewdunwoody）

**本次调研无法核实**：GitHub 仓库 404、Wayback Machine 无存档、搜索引擎无收录（多轮检索 + `archive.org/wayback/available` 均空）。按任务书描述其路线为"解析注册表 `EventMessageFile`/`ParameterMessageFile` → `LoadLibraryEx`（数据文件方式）→ `FormatMessage` 离线渲染"。**该路线的每一步正是微软官方 [Message Files](https://learn.microsoft.com/en-us/windows/win32/eventlog/message-files) 页所载标准流程**，且有其局限的官方明文（`%%n` 参数串须自行二段展开）。同一"离线萃取消息表"路线可验证的公开实例：

- **plaso / log2timeline**：`WinevtResourcesHelper` + PE 解析器 `extract_winevt_resources` 把 provider DLL 的第一条消息表抽进 `winevt-rc.db`（SQLite），psort 时回填 message string；`libyal/winevt-kb` 为其知识库工具，且已有 issue 追踪 **WEVT_TEMPLATE mapped event identifiers** 的消息匹配（[plaso](https://github.com/log2timeline/plaso)、[winevt-kb issue #18](https://github.com/libyal/winevt-kb)）。
- **TZWorks elmo**：商业 EVTX 消息还原原型（wevt_template README 引用）。
- **Hayabusa（Yamato Security）**：Rust Sigma 时间线工具，内置默认 Windows 事件消息库并支持从 EVTX 样本自建消息库、以实际字段回填渲染（[README](https://github.com/Yamato-Security/hayabusa)；该节位于旧版 README 后半段，本次抓取截断，结论置信度中）。

### Eric Zimmerman 系（EvtxECmd 等）

[EvtxECmd](https://github.com/EricZimmerman/evtx)：C# 自研 EVTX 解析 + **Maps 静态模板库**——按 `!Channel-Name_Provider-Name_EventID` 命名的 CSV，把事件字段占位映射成可读字段/消息；免注册表、离线可用，`--sync` 从 GitHub Maps 仓库拉更新；输出 CSV/JSON/XML（MIT）。**不查消息 DLL、不做运行时格式化**——本质是"人工维护的消息模板语料"。LogParser 2.2 一脉（2005 年代）面向 classic EVT/注册表栈，现代链路已被 EvtxECmd 类工具取代。

### Microsoft 系（krabsetw、EventLogExpert；"ManifestFromETW"）

- [krabsetw](https://github.com/microsoft/krabsetw)：Microsoft 官方 C++/C# ETW **实时消费**库（含 schema 解析），面向在线 trace，不做离线消息渲染。**"ManifestFromETW" 未检索到公开仓库**（GitHub 无此名仓库、krabsetw README 无此工具）——如实标注；任务书所指"Microsoft 系 manifest 解析路线"的可验证代表是下述两支：
- [zodiacon/EtwExplorer](https://github.com/zodiacon/EtwExplorer)（Pavel Yosifovich，MIT）：GUI 展示 ETW provider metadata，配合解析二进制 WEVT_TEMPLATE 资源——kernullist 公开拆解引用的工具之一；
- [microsoft/EventLogExpert](https://github.com/microsoft/EventLogExpert)（MIT）：官方开源 EVTX 查看器，**用"Provider Databases"离线渲染**——可在 A 机器捕获 provider 消息 `.db` 后到 B 机器解析 `.evtx` 描述与任务类别。这是"wevtapi 捕获态 + 静态库消费"的官方混合实现，与 evet 路线最接近的官方先例，但它**不做全静态复刻、也无一致性闭环证明**。

### Chainsaw / Sigma 等检测框架

[Chainsaw](https://github.com/WithSecureLabs/chainsaw)（Rust，GPL-3.0，WithSecureLabs）：Sigma + TAU 规则直接匹配 EVTX 的 EventID/字段值，包装 omerbenamram 的 evtx-rs；**只消费不渲染**（输出中无 vendor 消息替换）。Sigma 生态（EVTX-ATTACK-SAMPLES 等）同理：检测规则以 `EventID + 命名字段` 为键，渲染层的全部难点（variant、CRIM、MUI、TDH 类型化）被整层跳过。

## 2.2 对比表

| 方案 | EVTX 解析 | 消息渲染方式 | 本地化 | 关键局限 |
|---|---|---|---|---|
| omerbenamram/evtx | 自研 BinXML（Rust，IR） | **无** | 无 | 只出结构，消息层空缺 |
| libevtx / python-evtx | 自研（C / 纯 Python） | **无**（规范侧贡献 Qualifiers 公式） | 无 | 同上 |
| megatron（不可验证，404/无存档） | （描述）依赖外部解析 | LoadLibraryEx+FormatMessage（路线 B） | 依赖消息 DLL 内语言 | 仅 classic 表；参数串/回退链须自实现 |
| plaso winevt_rc + winevt-kb | plaso 自有 | 消息表萃取进 `winevt-rc.db` 静态回填 | 有限（首条消息表、语言处理弱） | WEVT_TEMPLATE 映射支持仍在 issue 阶段 |
| Hayabusa | 内置 Rust 解析 | 内置默认消息库 + 样本自建消息库回填 | 含日/英 | 覆盖度驱动，无逐条一致性证明 |
| EvtxECmd（Eric Zimmerman） | 自研（C#） | **人工 Maps CSV 模板**（channel_provider_eid） | 无（模板即英文为主） | 覆盖度靠人肉维护，漂移即错 |
| EventLogExpert（Microsoft） | 自研 | **Provider DB 跨机捕获**（wevtapi 捕获态静态化） | 捕获机语言 | 无全静态复刻、无一致性闭环 |
| krabsetw / ManifestFromETW（后者未检索到） | —（在线 ETW） | 无（在线 schema 解析） | — | 非离线渲染工具 |
| EtwExplorer / wevt_template / libfwevt | — | 无（解析 WEVT_TEMPLATE 元数据） | 无 | 只到"清单可见"，不到"消息可渲染" |
| Chainsaw / Sigma | evtx-rs | **无**（按 EventID/字段匹配） | 无 | 检测键绕过整个渲染层 |
| **evet（本项目）** | evtx crate + 自研 msgdb/CRIM/MUI 萃取 | **全静态复刻**：CRIM+RT_MESSAGETABLE/RT_STRING+0xDn+TDH 字节对+MUI 双语 overlay | en-US/zh-CN 双语单文件 | 需自持逆向资产；servicing 漂移须持续追踪 |

## 2.3 三条技术路线

| 路线 | 代表 | 适用面 | 天花板 |
|---|---|---|---|
| **A. 调用 wevtapi**（EvtOpenPublisherMetadata/EvtFormatMessage、wevtutil） | wevtutil 基线链、EventLogExpert 的捕获机 | 要求与系统行为逐位一致的场景；有活体机器 | 绑定本机注册状态与 UI 语言（evet R12/R12b 的 N/A 谓词、G0 语言漂移均为该绑定的实证）；离线语料不可迁移；归档场景仅 `LogFilePath` 一个官方口子 |
| **B. LoadLibraryEx + FormatMessage**（官方 Message Files 流程） | megatron（不可验证）、plaso winevt_rc、TZWorks elmo、Hayabusa 消息库 | classic 源、注册表可读、对 manifest 族无要求的工具链 | 仅覆盖 classic RT_MESSAGETABLE/RT_STRING；`%%n` 二段展开、MUI 回退链、多语言单文件分取须全部自实现；CRIM/variant/TDH 类型化整层缺失 |
| **C. 全静态复刻** | **evet（独有）** | 离线语料的逐字节语义还原、跨语言可重复比对 | 需自行逆向 CRIM 8 元组/0xDn/TDH 字节对等无文档语义（evet 已以 2026-09-30 verdict=PASS diff=0 闭合）；维护成本是系统 servicing 与本地化漂移（帐户/账户案） |

结论：公开生态在 A 的"捕获态静态化"（EventLogExpert）与 B 的"classic 表萃取"（plaso/Hayabusa）处停步；**没有任何公开项目尝试 C 路线的完整语义复刻与双链路一致性证明**。

---

# 第三部分：与 evet 项目实证事实对照

对照 `skills/parity-test-skill.md`"已核实的关键事实（勿回退）"#1–#23 与 `docs/phase-reports/2026-09-30-r26-r29-pass.md`。状态四档：**①已被公开知识印证**（有官方文档或开源实现明文对应）；**②部分印证**（机制/枚举公开，黑盒语义超出公开资料）；**③evet 独立逆向**（未检索到任何公开资料）；**④工程管线事实**（基线工具/自研管线行为，公开世界无对应命题，无需印证）。

| # | 事实摘要 | 本次调研的公开佐证 | 状态 |
|---|---|---|---|
| 1 | `evtx2text.lua` 基线脚本特性（GBK、lfs+wevtutil 链、stdin=NUL 跳过） | 无对应命题（自研基线工具） | ④ |
| 2 | 02 脚本须用 System32 `cmd.exe` 全路径（msys64 垫片遮蔽） | 无对应命题（Windows PATH 解析常识，无文档化案例） | ④ |
| 3 | wevtutil 文本基线 Date 为本地时间但保留 Z 后缀、小数秒截断 | wevtutil CLI 文档不描述 text 输出时间语义；无公开资料 | ③ |
| 4 | `--format both` 与 `-o` 同路径抢写 | evtx crate/evtx-tools 行为，无公开记载 | ④ |
| 5 | CSV 每 batch 夹带 BOM+表头重复行 | 同上 | ④ |
| 6 | 基线块尾 TRAIL 剥除须含分号冒号空白 | 同上 | ④ |
| 7 | R12：miss 臂渲染由**源注册状态**决定（未注册∧EID≠0→N/A；winevt 注册源 miss 仍有消息）；`eventlog_sources` 快照 1272 行 | 官方 [Event Sources](https://learn.microsoft.com/en-us/windows/win32/eventlog/event-sources)/[Message Files](https://learn.microsoft.com/en-us/windows/win32/eventlog/message-files) 印证机制方向（无消息文件→无法映射 identifier）；**N/A 精确谓词**无文档 | ② |
| 8 | R10/R18：wevtapi 按 CRIM TEMP 描述符 +4 的 TDH `(InType,OutType)` 字节类型化渲染；具体规则表（0x%X 大小写、HRESULT 低字、9 位纳秒）；hex/bool/HRESULT 跳 value map；`param_types` 双采集路径 | 枚举本身公开：[TDH_IN_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/tdh/ne-tdh-_tdh_in_type)/[TDH_OUT_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/tdh/ne-tdh-_tdh_out_type)（含 WIN32ERROR/NTSTATUS/HRESULT 值名）；[TdhFormatProperty](https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhformatproperty) 语义公开。**+4 偏移、字节对→格式映射、跳 map 规则均不在官方文字**（Learn 枚举页无逐值描述） | ② |
| 9 | evtx crate 对 manifest 布尔参数输出 JSON 原生 `true/false` | 开源实现行为，可由 [omerbenamram/evtx](https://github.com/omerbenamram/evtx) 源码复核 | ① |
| 10 | R19：CRIM VMAP 条目字符串引用是 **0xDn 字符串表家族**（0xD2/0xD1/0xD0/0xDC，按完整 message ID 键）；`(ref>>24)&0xF0==0xD0` 泛化；表可能只在 MUI | [kernullist ETW internals](https://kernullist.github.io/kernullist-blog/posts/etw-internals-deep-dive/) §14.2 公开提及"被引用字符串存在于 0xDxxx 范围 string table"（一句级）；**键控方式、泛化判据、MUI-only 归属**无公开资料 | ② |
| 11 | R20：param_types/param_value_maps 为 (provider,eid,index) 键但同 index 可跨 variant 异参（KGA 1 v3/v4 index3 漂移）；bool(13,*)/(8,8) 接管臂；DB 重建须删旧文件 | variant 模板间参数语义漂移：无公开资料 | ③ |
| 12 | EventLog 6013 消息在 netevent.dll MUI 的 RT_MESSAGETABLE（type 11）ID 0x8000177D；只扫 RT_STRING 的抽取会误判 | `RT_MESSAGETABLE` 资源类型与结构公开（[MESSAGE_RESOURCE_DATA](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-message_resource_data) 族）；netevent.dll 承载 classic 网络消息表为社区共识；6013 具体 ID 为活体实证 | ② |
| 13 | injectWellKnownVariants 已实装（6013 硬编码移除） | 自研机制，无对应命题 | ④ |
| 14 | NTSTATUS 渲染臂：ntdll.dll RT_MESSAGETABLE 2975 条入全局组；`(_,0x1F)` hit-only | [TDH_OUT_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/tdh/ne-tdh-_tdh_out_type) 之 `NTSTATUS` 值印证方向；ntdll 表容量与 hit-only 臂设计为 evet 实证 | ② |
| 15 | canon 双归一（`\\?\`≡`\\`、U+200E/200F 剥离）防御性保留 | 无对应命题（比对层工程事实） | ④ |
| 16 | EXEMPTIONS 双表同步机制（4 项终态） | 无对应命题 | ④ |
| 17 | G0 语言漂移：`wevtutil /f:text` 跟随本机 UI 语言；provider 无 zh 资源→EvtFormatMessage 回退 en（Setup 通道机理） | MUI 回退链机制公开（[User Interface Language Management](https://learn.microsoft.com/en-us/windows/win32/intl/user-interface-language-management)、[EvtOpenPublisherMetadata](https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtopenpublishermetadata) Locale）；**wevtutil 行为本身无文档** | ② |
| 18 | parameter_refs 语言层（v9）：参数文件按语言萃取（基础文件 en-US 层/MUI companion/en-US MUI 回退，`len>0` 非 nil 判断）；渲染端首选→en-US 回退镜像 EvtFormatMessage MUI 链；Win32-System-Error 每语言一套；ntdll zh-CN MUI 有 NTSTATUS 表 | MUI 卫星与回退链公开印证分层方向；**基础文件无表回落 en-US MUI、MUI-only 布局、per-lang FormatMessage langID** 无公开资料 | ② |
| 19 | 管线运维坑（epl 不覆盖、Security 提权、纯 ASCII ps1、`/lf:true`、重定向 ANSI/GBK） | 无对应命题（CLI 行为，无文档） | ④ |
| 20 | R21 classic 四修复：event_id=原始 ID&0xFFFF；warmup 空 cohort 回落 en-US；eid=0 miss→FormatMessage(0) Win32 文本（177 条实证）；canon `帐户`→`账户`（services.exe.mui servicing 漂移） | 低 16 位截取被 [libevtx EVTX 规范](https://github.com/libyal/libevtx/blob/main/documentation/Windows%20XML%20Event%20Log%20%28EVTX%29.asciidoc)印证；FormatMessage(0) 回退与 **帐户/账户本地化 servicing 漂移**无公开资料 | ②（其中漂移为 ③） |
| 21 | R26：classic 保留原始全格式 msgid（>0xFFFF 不去重，链尾真身防 PK 相撞）；`(q<<16)|eid` 精确探针压过通配梯子；resolved 闭包禁二次 db.lock | **公式本身已公开印证**：libevtx 规范明文 `( 0x4000 << 16 ) | 0x1b7c`；**wevtapi 探针优先于通配梯子的查找语义**为 evet 独立实证 | ①（查找语义为 ③） |
| 22 | R27：DriverStore 单文件内嵌多语言 RT_MESSAGETABLE 块（e1dmsg 10 块 en37/zh36/es36）；`MessagesPerLang` 按资源语言 ID 分桶 + `SelectMessagesForLangLCID`（精确→0x409→merged）；value_maps v9 双语 overlay（主 DLL 为底、该语言 MUI 同键全覆盖） | 资源三级树 Language 层公开（[PE/COFF](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format)、消息 .mc 的 LanguageNames=langid 机制）；**多语言块 last-write-wins 陷阱与三层选择算法、双语 overlay 语义**无公开资料 | ② |
| 23 | R28/R29：比对层斜杠日期伪影手术；未注册纯 classic eid≠0→Win32 错误文本回退（Codex 1001/1002，eid 1000 无文本两侧同 N/A）；`format_hresult` else 支 NTSTATUS 表探针(0)=STATUS_SUCCESS hit-only（Wininit 23 0x10000000） | iconv 伪影为基线管线工程事实（④）；未注册 classic 回退策略：无公开资料（③）；TDH 内建符号表方向被 `TDH_OUTTYPE_NTSTATUS`/STATUS_SUCCESS=0 公开常量佐证，hit-only 臂为 evet 实证（②） | 混合④③② |

## 3.1 重点标注：evet 超出公开知识的五项发现

1. **CRIM 8 元组 variant 选择的黑盒语义**（缺口 #2 SPP 1016 降格分歧、Security 全绿实证链）：公开侧只有 kernullist 的"TDH 缓存键元组"一句；**"按事件实例字段走阶梯、缺项降格"的完整选择算法未见于公开资料**，为 evet 独立逆向的核心资产。
2. **0xDn 字符串表家族**（事实 #10/#22）：公开侧仅 kernullist 一句"0xDxxx range string tables"；按完整 message ID 键控、`(ref>>24)&0xF0==0xD0` 泛化、MUI-only 归属、双语 overlay 全为 evet 原创实证。
3. **`(Qualifiers<<16)|code` 全键**（事实 #21）：映射公式已被 libevtx 规范公开印证；**"精确探针压过通配梯子"的运行时查找优先级**（及链内姊妹条目不得同 eid 去重的工程教训）为 evet 实证细化。
4. **UserData 嵌套展平**（事实 #23②：形态门控、深度 2 丢容器名、深度 3 `结构体_字段` 前缀、与 msgdb 命名规则同构）：`TDH_OUTTYPE_XML/JSON` 的存在是公开的，但**具体展平形态规则未见于公开资料**，为 evet 独立逆向。
5. **帐户/账户 servicing 漂移**（事实 #20④）：磁盘上 zh-CN `services.exe.mui` 为"帐户"版而 wevtapi 渲染"账户"版且文件不在盘上——**本地化 servicing 与在盘资源不同步导致渲染真值不可萃取**，无任何公开资料记载同类现象。

## 3.2 对照总结

- #1–#6、#13、#15–#16、#19（9 项）为工程管线事实，公开世界无对应命题，不构成印证对象。
- 机制级印证（①/②）覆盖 #7–#12、#14、#17–#18、#20–#23（14 项）：**官方文档给出的骨架（注册模型、消息表格式、占位符语法、MUI 链、EvtFormatMessage/TDH 语义、Qualifiers 公式）与 evet 实证零冲突**，多处（`%%n` 二段展开、manifest map 尾空格、Level/Opcode 先查 Winmeta 的查找序）直接成为 evet 实现的出处。
- 真正的超越集中在 CRIM 内部语义层（#8 的 +4 字节对、#10 的 0xDn 键控、#11/#22 的 variant 异参与 overlay、缺口 #2 的 variant 阶梯）与活体怪癖层（#3 时间格式、#17 语言漂移、#20④ servicing 漂移、#23 的回退臂）——**这两层合计 10+ 项在公开资料中查无可查，构成 evet 相对全部已知公开方案的独有贡献**。

---

## 参考链接汇总

**Microsoft Learn 官方文档**

- EvtFormatMessage：https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtformatmessage
- EVT_FORMAT_MESSAGE_FLAGS：https://learn.microsoft.com/en-us/windows/win32/api/winevt/ne-winevt-evt_format_message_flags
- EvtOpenPublisherMetadata：https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtopenpublishermetadata
- EvtGetPublisherMetadataProperty：https://learn.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtgetpublishermetadataproperty
- TDH_IN_TYPE / TDH_OUT_TYPE：https://learn.microsoft.com/en-us/windows/win32/api/tdh/ne-tdh-_tdh_in_type 、https://learn.microsoft.com/en-us/windows/win32/api/tdh/ne-tdh-_tdh_out_type
- TdhFormatProperty：https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhformatproperty
- TdhGetEventMapInformation：https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhgeteventmapinformation
- TdhGetEventInformation：https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhgeteventinformation
- MESSAGE_RESOURCE_DATA / BLOCK / ENTRY：https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-message_resource_data
- PE/COFF 规范（.rsrc 三级树）：https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
- Message Text Files（占位符语法）：https://learn.microsoft.com/en-us/windows/win32/eventlog/message-text-files
- Message Files（LoadLibraryEx+FormatMessage 流程、%%n 自行展开）：https://learn.microsoft.com/en-us/windows/win32/eventlog/message-files
- Event Sources / Eventlog Key / Event Identifiers：https://learn.microsoft.com/en-us/windows/win32/eventlog/event-sources 、https://learn.microsoft.com/en-us/windows/win32/eventlog/eventlog-key 、https://learn.microsoft.com/en-us/windows/win32/eventlog/event-identifiers
- MUI Resource Management / User Interface Language Management / Resource Utilities：https://learn.microsoft.com/en-us/windows/win32/intl/mui-resource-management 、https://learn.microsoft.com/en-us/windows/win32/intl/user-interface-language-management 、https://learn.microsoft.com/en-us/windows/win32/intl/resource-utilities
- wsprintf：https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-wsprintfa

**社区/逆向公开资料**

- kernullist《About ETW Internals》（§14.2 CRIM/WEVT_TEMPLATE，2026-06）：https://kernullist.github.io/kernullist-blog/posts/etw-internals-deep-dive/
- Palantir《Tampering with Windows Event Tracing》（2018-12，blog.palantir.com；Medium 拒绝抓取，未附直链，论断经检索摘要确认）
- libevtx EVTX 格式规范（Qualifiers 公式、WEVT_TEMPLATE 查找法）：https://github.com/libyal/libevtx/blob/main/documentation/Windows%20XML%20Event%20Log%20%28EVTX%29.asciidoc
- NTCore《MUI files under the hood》：https://ntcore.com/mui-files-under-the-hood/
- ntdll.xyz《CRIM Format Internals》（经 kernullist 中文机翻版转引；原页为 SPA，本次未能抓取正文，收录存疑）：https://ntdll.xyz/posts/crim-format

**开源仓库**

- omerbenamram/evtx：https://github.com/omerbenamram/evtx
- libyal/libevtx：https://github.com/libyal/libevtx ；libyal/libfwevt：https://github.com/libyal/libfwevt
- williballenthin/python-evtx：https://github.com/williballenthin/python-evtx ；williballenthin/wevt_template：https://github.com/williballenthin/wevt_template
- EricZimmerman/evtx（EvtxECmd）：https://github.com/EricZimmerman/evtx
- microsoft/krabsetw：https://github.com/microsoft/krabsetw ；microsoft/EventLogExpert：https://github.com/microsoft/EventLogExpert
- zodiacon/EtwExplorer：https://github.com/zodiacon/EtwExplorer
- WithSecureLabs/chainsaw：https://github.com/WithSecureLabs/chainsaw ；Yamato-Security/hayabusa：https://github.com/Yamato-Security/hayabusa
- log2timeline/plaso：https://github.com/log2timeline/plaso ；libyal/winevt-kb：https://github.com/libyal/winevt-kb

**本仓库内部对照材料**

- `skills/parity-test-skill.md`（已核实的关键事实 #1–#23、终态基线 verdict=PASS）
- `docs/phase-reports/2026-09-30-r26-r29-pass.md`（R26–R29 五修复与首次 PASS）
