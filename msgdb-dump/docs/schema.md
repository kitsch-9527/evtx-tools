# 数据库 Schema（hotech-msg v4 + v6 变体精准匹配）

> ADR 0003（2026-09-04）：本文档 v4–v6 历史章节保留作考古，现行以 schema/schema.go（v7 单文件 7 表）为准。

## 概述

msgdb-dump 输出两类 SQLite 数据库：

- **系统级数据库** — 包含所有 DLL 和提供程序的完整消息库。双写：
  - **v4 兼容库**（`hotech-msg-*.db`，`dll_messages` 单模板，7 张表）— 保留兼容；
  - **v6 变体库**（`hotech-msg-*-variants.db`，`provider_message_variants` 8 元组 `VariantKey`，1,495,630 行）— 与 `EvtFormatMessageEvent` 的 `(provider_guid, event_id, version, level, opcode, task, keywords, channel)` 精准匹配对应（见 `CONTEXT.md` Variant Identity / ADR 0001/0002）。
- **每 DLL 数据库**（可选，`--split-dlls`）— 单 DLL 的轻量级消息库（schema v2，2 张表）

---

## 系统数据库（schema v4 兼容 + v6 变体库现行）

> v6 为现行真源，v4 为兼容视图。以下 DDL 同时展示两版本；新库以 `schema/v6.go` 的 `provider_message_variants` 为准，`evtx-tools --msg-db` 应指向 `-variants.db`。

### 系统数据库 DDL（v4 兼容，仍写入）

### DDL

```sql
CREATE TABLE IF NOT EXISTS dll_entries (
    dll_id      INTEGER PRIMARY KEY AUTOINCREMENT,
    dll_name    TEXT NOT NULL,
    dll_path    TEXT NOT NULL UNIQUE,
    pe_version  TEXT NOT NULL DEFAULT '0.0.0.0'
);

CREATE TABLE IF NOT EXISTS providers (
    provider_name TEXT NOT NULL PRIMARY KEY,
    guid          TEXT,
    channel       TEXT,
    dll_id        INTEGER NOT NULL REFERENCES dll_entries(dll_id)
);

CREATE TABLE IF NOT EXISTS dll_messages (
    dll_id        INTEGER NOT NULL REFERENCES dll_entries(dll_id),
    lang          TEXT NOT NULL DEFAULT 'en-US',
    event_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    level         INTEGER,
    task          TEXT,
    opcode        TEXT,
    keywords      TEXT,
    parameters    TEXT,
    level_name    TEXT,
    keywords_name TEXT,
    PRIMARY KEY (dll_id, lang, event_id)
);

CREATE TABLE IF NOT EXISTS provider_aliases (
    provider_name TEXT NOT NULL,
    dll_id        INTEGER NOT NULL REFERENCES dll_entries(dll_id),
    PRIMARY KEY (provider_name, dll_id)
);

CREATE TABLE IF NOT EXISTS metadata (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS parameter_refs (
    provider_name TEXT NOT NULL,
    param_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    lang          TEXT NOT NULL DEFAULT 'en-US',
    PRIMARY KEY (provider_name, param_id, lang)
);

CREATE TABLE IF NOT EXISTS value_maps (
    provider_name TEXT NOT NULL,
    map_name      TEXT NOT NULL,
    entry_value   TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    is_bitmap     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (provider_name, map_name, entry_value)
);
```

### 索引

```sql
CREATE INDEX IF NOT EXISTS idx_dll_messages_eid ON dll_messages(event_id);
CREATE INDEX IF NOT EXISTS idx_dll_messages_dll ON dll_messages(dll_id);
CREATE INDEX IF NOT EXISTS idx_providers_dll ON providers(dll_id);
```

### 系统数据库 DDL（v6 变体精准匹配，现行）

```sql
-- provider_message_variants: 8 元组 Variant Identity，Provider 归属隔离（ADR 0001 Q2=A）
CREATE TABLE IF NOT EXISTS provider_message_variants (
    provider_name      TEXT NOT NULL,
    provider_guid      TEXT NOT NULL,
    lang               TEXT NOT NULL DEFAULT 'en-US',
    event_id           INTEGER NOT NULL,
    version            INTEGER NOT NULL DEFAULT 0,
    level              INTEGER NOT NULL DEFAULT 0,
    opcode             INTEGER NOT NULL DEFAULT 0,
    task               INTEGER NOT NULL DEFAULT 0,
    keywords           TEXT NOT NULL DEFAULT '0',
    channel            INTEGER NOT NULL DEFAULT 0,
    message_identifier INTEGER NOT NULL,
    dll_id             INTEGER NOT NULL REFERENCES dll_entries(dll_id),
    message            TEXT NOT NULL,
    parameters         TEXT,
    level_name         TEXT,
    keywords_name      TEXT,
    PRIMARY KEY (provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)
);
CREATE INDEX IF NOT EXISTS idx_pmv_lookup ON provider_message_variants(provider_name, lang, event_id);
CREATE INDEX IF NOT EXISTS idx_pmv_guid ON provider_message_variants(provider_guid, lang, event_id);
-- 兼容视图（v4 读路径仍可用）
CREATE VIEW IF NOT EXISTS dll_messages AS
SELECT pm.dll_id, pm.lang, pm.event_id, pm.message, pm.level, pm.task, pm.opcode, pm.keywords, pm.parameters, pm.level_name, pm.keywords_name
FROM provider_message_variants pm;
```

### 表详情

#### dll_entries

消息来源 DLL 的清单。`dll_path` 是唯一标识，确保同一 DLL 只被索引一次。

| 列 | 类型 | 说明 |
|---|------|------|
| `dll_id` | INTEGER PK | 自动递增 ID |
| `dll_name` | TEXT | DLL 文件名（如 `adtschema.dll`） |
| `dll_path` | TEXT UNIQUE | 完整路径（如 `C:\Windows\System32\adtschema.dll`） |
| `pe_version` | TEXT | PE 版本号（如 `10.0.22621.1`） |

#### providers

子系统（EventLog channels 和 WINEVT Publishers）中注册的 ETW 提供程序。

| 列 | 类型 | 说明 |
|---|------|------|
| `provider_name` | TEXT PK | 提供程序显示名（如 `Microsoft-Windows-Security-Auditing`） |
| `guid` | TEXT | 提供程序 GUID（`{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}` 格式） |
| `channel` | TEXT | ETW 频道名（如 `Security`、`System`、`Application`） |
| `dll_id` | INTEGER FK | 关联的 DLL 条目 ID |

#### dll_messages

按 DLL、语言和事件 ID 索引的消息模板。每条记录对应一个事件的消息文本。

| 列 | 类型 | 说明 |
|---|------|------|
| `dll_id` | INTEGER FK | 关联的 DLL 条目 ID |
| `lang` | TEXT | 语言代码（如 `en-US`、`zh-CN`） |
| `event_id` | INTEGER | 事件 ID |
| `message` | TEXT | 消息模板文本（含 `%1`、`%2` 等占位符） |
| `level` | INTEGER | 事件级别（0=未知, 1=CRITICAL, 2=ERROR, 3=WARNING, 4=INFO, 5=VERBOSE） |
| `task` | TEXT | 任务分类名 |
| `opcode` | TEXT | 操作码名 |
| `keywords` | TEXT | 关键字名（多个以逗号分隔） |
| `parameters` | TEXT | JSON 格式的参数名列表（如 `{"param1":"FileName","param2":"ProcessId"}`） |
| `level_name` | TEXT | 级别的显示名（如 `Error`、`Warning`） |
| `keywords_name` | TEXT | 关键字的显示名 |

> **注意：** 一个 `(dll_id, lang, event_id)` 组合可能会对应多个提供程序（通过 `providers` 表关联）。消息解析时的查询流程为：`provider_name` → `providers.dll_id` → `dll_messages.event_id`。

#### provider_aliases

将 EVTX 文件中的提供程序名称映射到实际包含消息的 DLL。当目标机安装了构建机上没有的第三方软件时，通过 `--import-dll` + `--alias` 机制填充此表。

| 列 | 类型 | 说明 |
|---|------|------|
| `provider_name` | TEXT | EVTX 中的提供程序名（如 `MySQL`） |
| `dll_id` | INTEGER FK | 实际包含消息的 DLL 条目 ID |

#### metadata

数据库元数据键值对。

| key | 值示例 | 说明 |
|-----|--------|------|
| `product_name` | `Windows 10 Education` | 操作系统产品名 |
| `display_version` | `25H2` | 操作系统显示版本 |
| `build_number` | `26220.7052` | 操作系统内部版本号 |
| `architecture` | `amd64` | 处理器架构 |
| `created_at` | `2026-07-09T05:56:54Z` | 数据库创建时间（RFC 3339） |
| `schema_version` | `4` | 数据库 schema 版本号 |
| `langs` | `en-US,zh-CN` | 数据库包含的语言列表 |

#### parameter_refs

提供程序的 `%%N` 参数引用表。某些事件消息使用 `%%N` 语法引用参数消息（与 `%N` 占位符不同），此表存储这些参数消息的解析结果。

| 列 | 类型 | 说明 |
|---|------|------|
| `provider_name` | TEXT | 提供程序名 |
| `param_id` | INTEGER | 参数编号（对应于 `%%N` 中的 N） |
| `message` | TEXT | 参数解析后的消息文本 |
| `lang` | TEXT | 语言代码 |

#### value_maps

提供程序的 VMAP（值映射）和 BMAP（位图）枚举值。用于将事件数据中的整数值解析为可读的显示名。

| 列 | 类型 | 说明 |
|---|------|------|
| `provider_name` | TEXT | 提供程序名 |
| `map_name` | TEXT | 映射名（位图以 `_bits` 后缀标识） |
| `entry_value` | TEXT | 数值（字符串形式） |
| `display_name` | TEXT | 显示名 |
| `is_bitmap` | INTEGER | 0=VMAP 值映射，1=BMAP 位图 |

---

## 每 DLL 数据库（schema v2）

`--split-dlls` 选项为每个 DLL 生成独立的轻量级数据库，适用于按 DLL 分发消息的场景。

```sql
CREATE TABLE IF NOT EXISTS messages (
    lang       TEXT NOT NULL DEFAULT 'en-US',
    event_id   INTEGER NOT NULL,
    message    TEXT NOT NULL,
    level      INTEGER,
    task       TEXT,
    opcode     TEXT,
    keywords   TEXT,
    parameters TEXT,
    PRIMARY KEY (lang, event_id)
);

CREATE TABLE IF NOT EXISTS metadata (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);
```

### metadata 键值

| key | 说明 |
|-----|------|
| `dll_name` | DLL 文件名 |
| `lang` | LCID（如 `1033`） |
| `pe_version` | PE 版本号 |
| `msg_lang` | 消息语言代码（如 `en-US`） |
| `created_at` | 创建时间 |
| `schema_version` | `2` |

---

## 版本历史

| Schema 版本 | 变更 | 备注 |
|-------------|------|------|
| 2 | 初始 DLL 级数据库 | — |
| 4 | 系统级数据库：新增 `provider_aliases`、`parameter_refs`、`value_maps` 表 | 兼容保留 |
| 5 | 单模板 `dll_messages` 定型，7 张表 | 兼容保留 |
| 6 | **变体精准匹配**：新增 `provider_message_variants`（`PRIMARY KEY(provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)`，`keywords TEXT` 防溢出）；`provider_guid` 归属；双写 `v5+v6`；兼容视图 `dll_messages` 投影 | 本仓现行，`evtx-tools` 指向 `-variants.db` |

> v6 的 `provider_message_variants` 与 `evtx-tools` 的 `VariantKey {provider_name, event_id, version, level, opcode, task, keywords, channel, message_id}` + `Fallback Ladder` 一一对应；`evtx-rates` 以 `-variants.db` 的 `Setup 100% / System Kernel-General 90% / Security 100%` 为门禁。
