# ADR 0004: 注册表事件源出处快照（R12）

- **日期**: 2026-09-07
- **状态**: 已接受
- **关联决策**: 追加 ADR 0003（schema_version 仍为 7，附表可选）

## 背景

离线路径 B 对 `provider_message_variants` 无命中的事件（miss cohort）渲染裸参数或 cannot-be-found 文本。wevtutil 基线对同一批 miss 事件的渲染由**源注册状态**决定（2026-09-07 全量取证，887 条 miss 事件）：

- 源未注册（不在经典 `HKLM\SYSTEM\CurrentControlSet\Services\EventLog\<channel>\<name>`，也不在 `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\WINEVT\Publishers`）且 `EventID != 0` → 基线渲染 `N/A`（101/101 成立，含 AppXDeploymentServer/Operational 2562×86）。
- 源已注册（classic 或 publisher）→ 基线渲染真实消息（675 条成立；winevt publisher EventSystem 4625 亦有消息，属采集缺口非 N/A 语义）。

该判定需要注册表出处（provenance）信息，而现有 7 张实体表均为 CRIM/PE 萃取物，无注册表维度。

## 决策

- msgdb-dump 新增附表 `eventlog_sources(source_name TEXT PRIMARY KEY, source_type TEXT NOT NULL)`：经典 `EventLog` 键全部通道子键源名 ∪ WINEVT Publishers 默认值名（`source_type` ∈ `classic` / `winevt`）的一次性快照。经典侧取全部通道（而非仅四通道）是防御性选择：多注册只会让渲染器欠射（保持旧行为），不会过射（误 N/A 回归）。
- **schema_version 升 8**（2026-09-07 修订：原决策"保持 7 + 可选附表"经用户裁决废弃——不做新旧库兼容）：`eventlog_sources` 为 v8 必需表，读端缺表即硬错（提示重跑 msgdb-dump）；空表（采集失败）仍视为"注册状态未知"，特性关闭，防全量误 N/A。
- evtx-tools 渲染判定（仅 miss cohort 生效）：注册快照已知 且 源不在表内 且 `event_id != 0` → 渲染 `N/A`；其余（含 EID=0、classic/winevt miss）保持既有回退路径。

## 后果

- AppXDeploymentServer 2562×86 等未注册源 miss 事件与基线 `N/A` 对齐（Application D -87，B -13，System B -2）。
- 经典已注册但消息文件缺失/萃取不到的 miss（如 Configuration-Change-Monitor×40）不在本 ADR 范围，仍走既有路径。
- 非平台（non-windows）dump 产出的库该表为空，读端特性自动关闭，行为与 R11 版一致。
