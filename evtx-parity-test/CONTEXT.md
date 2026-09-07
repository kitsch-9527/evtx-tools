# CONTEXT.md — evtx-parity-test 术语基线

> 本文件只定义“是什么”，不定义“怎么做”（做法见 README.md 与 scripts/）。
> 中文名为用户侧用词，英文名为磁盘上的真实目录/文件名，两者为别名关系。

## 术语表

| 术语 | 磁盘正名 | 定义 |
|---|---|---|
| **基线文件夹** | `baseline-evtx/` | 本机四通道经 `wevtutil epl` 导出的原始 `.evtx`（Application/Setup/Security/System）。 |
| **基线解析** | `baseline-parsed/` | `msys64-Tools/evtx2text.lua` 对基线文件夹的解析输出（`*.evtx.txt` 中文文本）。 |
| **消息库** | `msgdb/` | `msgdb-dump` 在本机生成的双语单文件（`hotech-msg-*.db`，`schema_version=8`（v7 实体表 + eventlog_sources + param_types），含 1033/en-US 与 2052/zh-CN，ADR 0003/0004/0005）。 |
| **新解析** | `new-parsed/en/`、`new-parsed/zh/` | `evtx-tools` 用消息库对基线文件夹的解析输出，en-US 与 zh-CN 各跑一遍。 |
| **对比报告** | `report.md` + `report.json` + `report.diff` | 基线解析 vs 新解析（主跟 en 比，zh 作覆盖率对照）的归一化比对结论。`report.json.verdict` 为闭环口径。 |
| **应用** | Application | 事件通道 `Application.evtx`。 |
| **补丁安装** | Setup | 事件通道 `Setup.evtx`（CBS/ servicing 相关，常含 UserData 格式事件）。 |
| **安全** | Security | 事件通道 `Security.evtx`。 |
| **系统** | System | 事件通道 `System.evtx`。 |
| **顺序对齐** | — | 同一文件内按出现顺序逐条对齐：基线 `Event[N]` 序号顺序 ↔ 新解析 `sequence_number` 顺序。不按 `event_record_id` 对齐（基线 `Event[N]` 是查询序号，非记录号）。 |
