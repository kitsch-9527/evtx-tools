# CONTEXT.md — evet 领域语言（Ubiquitous Language）

> 钱学森系统工程视角下的术语基线。实现细节不得渗入此处；此处只定义“是什么”，不定义“怎么做”。

## 1. 核心域：Windows 事件语义还原

本系统将 Windows ETW 事件从“二进制事实”（EVTX BinXML + EventData）还原为“人类可读消息”（Message），跨越两条路径：

- **路径 A（运行时）**：`EvtFormatMessage(EvtFormatMessageEvent, eventHandle)` — 依事件实例的 8 元组现场择优。
- **路径 B（离线）**：`msgdb-dump` 静态提取 → 生成 `provider_message_variants` → `evtx-tools` 按同等 8 元组查询期择优。

两路径在语义上等价是本项目的正确性判定基准（以 Lua `evtx2csv.exe` + `wevtapi` 的 `aevtx` 为参照）。

## 2. 术语表（Glossary）

| 术语 | 正名/英文 | 定义 | 非术语（禁用） |
|---|---|---|---|
| **Provider** | Provider | ETW 事件的逻辑发布者，以 `provider_guid` 唯一标识，隶属关系以 `WEVT` 块为准，而非物理 DLL 文件名。 | “DLL” 不可代指 Provider |
| **Message Variant** | Message Variant | 同一 `(provider, event_id)` 下，因 `(version, level, opcode, task, keywords, channel)` 不同而指向不同 `message_identifier` 的独立消息解释。互不覆盖。 | “消息模板”“消息版本” |
| **EVNT Variant** | EVNT Variant | `CRIM` 的 `WEVT` 块内 `EVNT` 段中 `event_id` 相同但其余维度不同的多条 `EventEntry`（48 字节）。是 Variant 的结构载体。 | “版本差异” |
| **Variant Identity** | Variant Identity | 判定两个 Variant 是否为同一语义所需的 8 元组：`(provider_guid, event_id, version, level, opcode, task, keywords, channel, message_identifier)`。全等方为同一 Variant，余则并存。 | “去重”“合并” 的模糊表述 |
| **Provider Attribution** | Attribution | 将物理 DLL 文件中的 `CRIM`/`RT_MESSAGETABLE` 资源按 `provider_guid → WEVT` 归属到逻辑 Provider，而非按 `dll_path → DllGroup` 批量复制。共享同一 DLL 的 46 个 Provider 各自独立。 | “DLL 分组”“共享消息” |
| **Attribution Contamination** | Contamination | 因按 DLL 分组并批量复制消息导致的跨 Provider 变体互染（例：`Kernel-General` 与 `Kernel-Boot` 在 `dll_id=163` 互染）。属结构性错误，非“系统限制”。 | “系统限制”“环境噪声” |
| **Matching Contract** | Matching Contract | 生成系统与使用系统之间的总体接口约定：以 8 元组为领料单进行精确匹配，外部 CLI/管线阶段不变，内部查询谓词必须写全。 | “解析流程不变 = 签名不变” 的误读 |
| **Fallback Ladder** | Fallback Ladder | 精确 8 元组无命中时的确定性全序降格：`去 channel → 去 keywords → 去 task → 去 opcode → 去 level → 仅 (provider, event_id)`，同层内 `version DESC`，`level=0/opcode=0/task=0` 视为通配。 | “随机回退”“高 version 胜” |
| **Dedup Key** | Dedup Key | 仅当 8 元组全等时视为冗余可去重；余则一律保留。此为 Q3=B 的复合键。 | “按 event_id 去重” |
| **单文件消息库** | Single-File MessageDB | `msgdb-dump` 单次运行的唯一产出：一个 SQLite 文件，含 v7 的 7 张实体表 + `eventlog_sources`/`param_types` 两张出处表、无视图，`provider_message_variants` 为唯一消息表（ADR 0003/0004/0005，`schema_version=8`）。 | “dual-write”“v4 兼容库”“variants 库”“双语一对库” |

## 3. 场景探针（用于锐化边界）

- `EID=11` 在 `Kernel-General` 下 **TxR**（`ExtraString/TmId/RmId`）与 **AppContainer**（`PackageFullName/ErrorCode/FailedBinary`）因 `Level/Opcode` 不同而为两个 Variant，现场按事件实例的 `Level/Opcode` 择优，互不干扰。
- `EID=20` 在 `Kernel-General`（闰秒，`UpdateReason`）与 `Kernel-Boot`（关机，`LastShutdownGood`）语义正交却同 `dll_id=163`，按 `provider_guid` 归属自然分流。
- `level=0 / opcode=0 / task=0` 在 `CRIM` 中语义为通配，不纳入强匹配，仅作排序权重。

## 4. 判定基准

- 以 `evtx_new.db` 的 `aevtx`（`evtx2csv.exe` 经 `EvtFormatMessageEvent` 产生）为参照真值，System 8/30 的“模板版本差异”误判已证伪，判定为错模板命中。
