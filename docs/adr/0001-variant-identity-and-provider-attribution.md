# ADR 0001: Variant Identity 与 Provider Attribution

- **日期**: 2026-09-02
- **状态**: 已接受
- **关联决策**: Q1=A, Q2=A, Q3=B  （钱学森系统工程视角，5 问终局）

## 背景

System 频道 8/30 失配被归为“消息模板版本差异 / 系统限制”。同机取证（`Microsoft-Windows-System-Events.dll` 10.0.26100.9202，`dll_id=163` 被 46 个 Provider 共享）证明：`EID=11/12/13/20/24` 的 5 条核心失配是**错模板命中**，非版本漂移。根因为：

1. `extractor.go: bestEvents map[uint16]EventEntry` 按 `event_id` 单键去重（高 version 胜）丢弃 TxR/闰秒等 Variant。
2. `writer_sys.go: CreateSystemDB` 按 `DllGroup` 将同一 `MessagesByLang` 复制给 46 个 Provider，造成 Attribution Contamination。
3. `provider_messages` 主键 `(provider_name, lang, event_id, dll_id)` 只能存一模板，`resolver.rs: warmup_lang` 单键查询命中错误变体。

Lua 基线 `evtx_2_Sqlite.lua → evtx2csv.exe → wevtapi!EvtFormatMessage(EvtFormatMessageEvent, eventHandle)` 实证：Windows 按 `(provider_guid, event_id, version, level, opcode, task, keywords, channel)` 8 元组现场择优，同一 EID 多 Variant 并存。

候选方案：

- Q1: A. 扩展主键为 8 元组 / B. 新表 `provider_message_variants` / C. 单行 JSON 数组。
- Q2: A. 推倒 DllGroup 按 `provider_guid→WEVT` / B. 保留分组外加过滤 / C. 不修归属。
- Q3: A. 删除去重 / B. 8 元组复合键去重 / C. 查询期择优。

## 决策

- **Q1=A**：`provider_message_variants`（或扩展原表）以 `(provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)` 为身份键，单表多行。全新设计，无需兼容 `v5` 旧库。
- **Q2=A**：废除 `dllMap[per DLL] → DllGroup`，改为 `provider_guid → WEVT` 功能分组；`ExtractSingleDLL` 按 Provider 独立走 `EVNT → message_identifier → MUI RT_MESSAGETABLE` 映射后独立入库。
- **Q3=B**：去重键为 8 元组全等方视为冗余；余则一律保留。`level=0/opcode=0/task=0` 视为通配，不纳入强键区分但保留为排序信号。

## 后果

- 生成期无损：TxR 与 AppContainer、闰秒与关机各归其主。
- 查询期可被形式化验证（见 ADR 0002）。
- `msgdb-dump/dump/{extractor.go,run_windows.go,writer_sys.go,schema/v5.go}` 与 `crim/crim.go` 需重排流水；`fix_orphaned.go` 事后修补可退役。
- 代价：Schema 迁移与索引重建；收益：与 `EvtFormatMessageEvent` 同构，等价性可回归。
