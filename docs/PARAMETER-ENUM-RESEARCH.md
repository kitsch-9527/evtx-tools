# EVTX 参数枚举值解析调研

> hotech-evtx `%N` 参数枚举值（value map）解析方案预研。
> 背景：BITS event 209 `%3=isRoaming=1` → hotech 显示 `"1"`，PowerShell 显示 `"Enabled"`。
> 最后更新：2026-07-07

---

## 1. 问题定义

### 当前流水线

```
消息模板: "High performance property for BITS job \"%1\" with ID \"%2\" %3."

EVTX event_data (JSON):
  { "jobName": "Font Download", "jobId": "{...}", "isRoaming": 1 }

hotech 当前:
  %1 → "Font Download"       (string, 直接替换)
  %2 → "{...}"               (string, 直接替换)
  %3 → "1"                   (int, 原始数值 — 缺少枚举→字符串转换)

PowerShell / Event Viewer:
  %3 → "Enabled"             (通过 value map: 1 → "Enabled")
```

### 影响范围

- **21文件/2723事件的回归测试**：2,034 条有消息的事件中，~1,504 条因这类差异被标记为"不匹配"
- 涉及 BITS、Security-Auditing、Sysmon 等多个 provider 的 boolean/枚举字段

---

## 2. Windows 原生解析机制

### 2.1 三条解析路径

```
    EVTX 消息模板
         │
         ├── %1, %2, %3 ...   ──→ EventData / Strings 数组
         │         │                     │
         │         │              ┌──────┴──────┐
         │         │              │ raw value   │  (hotech 当前仅此)
         │         │              │ (1, 0, ...) │
         │         │              └──────┬──────┘
         │         │                     │
         │         │              ┌──────┴──────────┐
         │         │              │ 是否需要枚举转换?│
         │         │              └──────┬──────────┘
         │         │                     │
         │    ┌────┴────┐          ┌─────┴──────┐
         │    │ YES     │          │ NO         │
         │    │(类型有   │          │(字符串直接  │
         │    │ value   │          │ 替换)       │
         │    │ map/    │          └────────────┘
         │    │ bitmap) │
         │    └────┬────┘
         │         │
         │    ┌────┴──────────────────────────┐
         │    │ TdhGetEventMapInformation     │  ← Windows API
         │    │ 查 provider manifest 中的     │     仅线上/已知 DLL
         │    │ value map → 得到显示字符串    │
         │    └─────────────────────────────┘
         │
         ├── %%100, %%245 ... ──→ ParameterMessageFile
         │         │                    (DLL FormatMessage)
         │         └────── 已经实现（Phase 1）✅
         │
         └── %{S...}, %{...} ──→ SID / GUID 格式化
```

### 2.2 关键 API：`TdhGetEventMapInformation`

```c
TDHSTATUS TdhGetEventMapInformation(
  [in]  PEVENT_RECORD   pEvent,      // 事件记录
  [in]  PWSTR           pMapName,    // map 名称（来自 EVENT_PROPERTY_INFO.MapNameOffset）
  [out] PEVENT_MAP_INFO pBuffer,     // 接收 EVENT_MAP_INFO 结构
  [in]  PULONG          pBufferSize
);
```

**返回值：**
- `ERROR_SUCCESS` — 成功，pBuffer 包含 `EVENT_MAP_INFO`
- `ERROR_NOT_FOUND` — provider 未注册或 map 未找到
- `ERROR_FILE_NOT_FOUND` — provider 二进制文件不在注册位置

**`EVENT_MAP_INFO` 结构包含：**

| 字段 | 说明 |
|------|------|
| `NameOffset` | map 名称在 buffer 中的偏移 |
| `Flag` | 类型标志 |
| `EntryCount` | 条目数 |
| `MapEntryArray[]` | `(Value, OutputOffset)` 对数组 |

**Flag 类型：**

| 值 | 类型 | 说明 |
|----|------|------|
| `EVENTMAP_INFO_FLAG_MANIFEST_VALUEMAP` (0x1) | Value Map | 整数值→字符串映射 |
| `EVENTMAP_INFO_FLAG_MANIFEST_BITMAP` (0x2) | Bitmap | 位标志→字符串映射 |
| `EVENTMAP_INFO_FLAG_MANIFEST_PATTERNMAP` (0x4) | Pattern Map | 字符串模式映射 |
| `EVENTMAP_INFO_FLAG_WPP_VALUEMAP` (0x8) | WPP Value Map | WPP 跟踪专用 |
| `EVENTMAP_INFO_FLAG_WPP_BITMAP` (0x10) | WPP Bitmap | WPP 位图专用 |

**最低 OS：** Windows Vista / Server 2008
**库：** `tdh.dll` / `tdh.lib`

### 2.3 解析步骤

```
1. TdhGetEventInformation(pEvent) → TRACE_EVENT_INFO
     ↓
2. 遍历 EVENT_PROPERTY_INFO[]，检查 PropertyParamCount / PropertyParamBit 标志
     ↓
3. 有 value map 的字段 → 读取 MapNameOffset → 得到 map 名称字符串
     ↓
4. TdhGetEventMapInformation(pEvent, mapName, ...) → EVENT_MAP_INFO
     ↓
5. 遍历 MapEntryArray → (raw_value → display_string)
     ↓
6. %N 替换时: raw_value → 查该字段的 map → display_string
```

### 2.4 引用文档

- [TdhGetEventMapInformation (Microsoft)](https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhgeteventmapinformation)
- [TdhFormatProperty (Microsoft)](https://learn.microsoft.com/en-us/windows/win32/api/tdh/nf-tdh-tdhformatproperty)
- [Formatting Event Messages (Microsoft)](https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/WES/formatting-event-messages.md)
- [MS-EVEN: Expanding Unexpanded Event Description Strings](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-even/d76e867d-96dc-4c09-b747-b1dde1acc5a4)

---

## 3. 离线/未知 DLL 场景

### 3.1 数据来源对比

| 数据来源 | 依赖 | 未知 DLL 可用？ | 覆盖度 |
|---------|------|----------------|--------|
| `TdhGetEventMapInformation` | Provider 在本地注册 | ❌ | 完整 value map |
| `FormatMessage` + `ParameterMessageFile` | DLL 文件在本地 | ❌ | `%%N` 字符串 |
| **CRIM VMAP/BMAP** | **EVTX 文件内嵌** | **✅** | **结构定义（value→msg_id 对）** |
| **`win:*` 标准类型约定** | **无** | **✅** | boolean 等基础类型 |
| **裸数值回退** | **无** | **✅** | 无信息损失 |

### 3.2 CRIM VMAP/BMAP 格式

**evtx crate v0.11.2 已完整解析 VMAP/BMAP（仓库内部已实现）：**

```rust
pub struct MapsDefinitions<'a> {
    pub offset: u32,
    pub size: u32,
    pub maps: Vec<MapDefinition<'a>>,
}

pub enum MapDefinition<'a> {
    ValueMap(ValueMap<'a>),
    Bitmap(BitmapMap<'a>),
}

pub struct ValueMap<'a> {
    pub offset: u32,
    pub size: u32,
    pub map_string_offset: u32,
    pub entries: Vec<ValueMapEntry>,    // (identifier, message_identifier)
    pub map_string: Option<String>,     // map 名称（如 "RoamingState"）
}

pub struct ValueMapEntry {
    pub identifier: u32,                // 原始值 (0, 1, ...)
    pub message_identifier: Option<u32>,// DLL 消息 ID 或直接值
}
```

**VMAP 的限制：** `entries` 存储的是 `(value, message_id)` 对，**不是** `(value, display_string)` 对。
`message_identifier` 需要进一步通过 `FormatMessage` 解析才能得到显示字符串。
对于未知 DLL 场景，VMAP 的 `message_identifier` 无法解析。

**例外情况：** 某些 provider 的 `message_identifier` 本身就是显示字符串（`map_string` 字段包含内容）。
这取决于 provider 的实现方式。

### 3.3 `win:*` 标准类型映射

Windows 定义的标准类型自带 display mapping，**无需外部数据**：

| 类型 | 原始值 | 显示字符串 |
|------|--------|-----------|
| `win:Boolean` | `0` / `false` | `"false"` / `"Disabled"` |
| `win:Boolean` | `1` / `true` | `"true"` / `"Enabled"` |
| `win:UInt8` | 无枚举定义时 | 直接显示数值 |
| `win:UInt16` | `0xFF` | 数值或 hex |
| `win:HexInt32` | `0x80000000` | hex 格式 |

---

## 4. 其他开源项目做法

### 4.1 python-evtx

**状态：** 不支持消息解析。纯 XML/Binary 解析器，不碰消息模板。

```
python-evtx 输出: 原始 XML
  → 用户自己处理消息解析
  → 不依赖 FormatMessage
```

### 4.2 libevtx (libyal)

**状态：** 解析 CRIM 块，有 VMAP/BMAP 定义但未用于消息解析。

```
libevtx API:
  - evtx_record_get_event_xml()   → 返回原始 XML
  - 用户通过 libfwps 解析消息
  - VMAP/BMAP: 框架内定义了结构体，但下游工具未使用
```

### 4.3 hayabusa (Rust)

**状态：** 调用 Windows `EvtFormatMessage` API（仅 Windows 原生），离线时不解析消息。

```rust
// hayabusa 的消息解析路径：
// 1. Windows 上: 调用 EvtFormatMessage → 得到完整显示字符串
// 2. 跨平台: 只输出 raw event XML
// 不做自定义 %N/%%N 替换
```

### 4.4 Chainsaw (Rust)

**状态：** 同 hayabusa，依赖 `EvtFormatMessage`。

### 4.5 结论

**没有其他开源项目在跨平台场景下实现了 `%N` 参数枚举值解析。**
hotech-evtx 是唯一尝试在离线场景下做完整消息解析的项目。

---

## 5. hotech-evtx 现有数据路径

### 5.1 CRIM 解析状态（历史记录，dumper 已用 Go 重写）

> 以下描述的是旧的 Rust dumper 实现。消息提取功能已用 Go 重写。

`hotech-evtx` 中解析的 CRIM 块：

| 块 | 状态 | 存储位置 |
|----|------|---------|
| EVNT | ✅ | `event_map` |
| TASK | ✅ | `tasks` → 未存 DB |
| OPCO | ✅ | `opcodes` → 未存 DB |
| LEVL | ✅ | `levels_by_offset` → `dll_messages.level_name` |
| KEYW | ✅ | `keywords_by_offset` → `dll_messages.keywords_name` |
| TTBL | ✅ | `template_params` |
| **VMAP** | ❌ 未解析 | — |
| **BMAP** | ❌ 未解析 | — |
| CHAN | ❌ 未解析 | — |

### 5.2 evtx crate 的 VMAP 支持

**evtx v0.11.2 已完整支持 VMAP/BMAP 解析**，但 hotech-evtx 的 `extractor.rs` 中的 `parse_crim_provider_data()` 完全跳过了 VMAP/BMAP 处理。

### 5.3 `dll_messages.parameters` JSON

当前格式：`{"3":"isRoaming","1":"jobName","2":"jobId"}`

这是 `%N` → 字段名称的映射，在 `cmd_parse.rs::extract_subst_params()` 的策略 1/3 中使用。
**不包含 value map 信息。**

### 5.4 `parameter_refs` 表

当前仅用于 `%%N` 扩展。BITS provider 有 0 条 `parameter_refs` 记录。

---

## 6. 推荐方案

### 架构：三层逐级解析

```
%N 替换流水线（新增上下文管理层）:

  输入: raw_value (来自 event_data), field_name, event_id, provider_id
         ↓
  ┌────── 第 1 层：标准类型约定 ──────────────────┐
  │  条件: field 类型是 win:Boolean 或可推断的枚举   │
  │  动作: 硬编码映射 0→false/Disabled, 1→true/Enabled│
  │  适用: ✅ 所有 DLL                          │
  │  覆盖: boolean 字段                          │
  └───────────────────┬──────────────────────────┘
                      │ (未匹配 → 下推)
                      ↓
  ┌────── 第 2 层：CRIM VMAP/BMAP ────────────────┐
  │  条件: CRIM 中该 provider 有 VMAP/BMAP 定义     │
  │  动作: 查找 (event_id, field_name) → value map  │
  │  适用: ✅ 所有 EVTX（数据内嵌）                  │
  │  覆盖: provider 自定义 value map                │
  │  注意: message_id → 字符串需要 MSG DB          │
  └───────────────────┬──────────────────────────┘
                      │ (未匹配 → 下推)
                      ↓
  ┌────── 第 3 层：MSG DB value_maps ──────────────┐
  │  条件: MSG DB 中有该 provider 的 value map 条目  │
  │  动作: 查 value_map_value → display_string      │
  │  适用: ✅ 已知 DLL（dump 阶段已提取）             │
  │  覆盖: TdhGetEventMapInformation 的完整结果      │
  └───────────────────┬──────────────────────────┘
                      │ (未匹配 → 下推)
                      ↓
  ┌────── 第 4 层：裸值回退 ───────────────────────┐
  │  动作: 直接输出 raw_value "1"                    │
  │  适用: ✅ 所有场景                               │
  │  当前行为: 这就是现在的逻辑                       │
  └────────────────────────────────────────────────┘
```

### 需要实现的变更

#### Phase 1: 第 1 层 — 标准类型约定（~50 行）

- 在 `substitute.rs` 中添加 `resolve_enum(raw_value, field_type) -> String`
- 映射 `win:Boolean` 的 `0`/`1` → `"Disabled"`/`"Enabled"`
- 需要从 `parameters` JSON 获取字段名，通过字段名推断类型

#### Phase 2: 第 2 层 — CRIM VMAP 提取（~150 行）

- `extractor.rs` 的 `parse_crim_provider_data()` 增加 VMAP/BMAP 处理
- 从 evtx crate 的 `ProviderElements.maps` 读取 value map
- 构建 `event_id → field_name → value_map` 索引
- 在 `dll_messages` 表新增 `value_maps TEXT` 列（JSON）或新增 `parameter_enums` 表

#### Phase 3: 第 3 层 — MSG DB value_maps 填充（~200 行）

- **方式 A**（推荐）：dump 时调 `TdhGetEventMapInformation` + 读取 manifest → 存入 MSG DB
- **方式 B**（简化）：利用 CRIM VMAP 的 `message_identifier` → 查 `parameter_refs` →

#### 依赖关系

```
Phase 1 (类型约定)     ← 无依赖
    ↓
Phase 2 (CRIM VMAP)   ← 需要 evtx crate VMAP 类型
    ↓
Phase 3 (MSG DB)      ← 需要 Phase 2 的 schema 扩展
```

### evtx crate 现有 API 支持

hotech-evtx 目前使用的 `evtx::EvtxParser::from_path()` 返回的 `CrimProviderData` 结构中，
`parse_crim_provider_data()` 已经能访问完整 CRIM 数据（通过 `event_map` 参数）。
但 hotech-evtx 的 `CrimProviderData` 结构体没有 `maps` 字段。

**需要：** 在 `CrimProviderData` 中新增 `maps: Option<...>` 字段，
在 `parse_crim_provider_data()` 中从 evtx crate 的 `ProviderElements.maps` 读取。

---

## 7. 总结

| 维度 | 结论 |
|------|------|
| Windows 原生路径 | `TdhGetEventMapInformation` 读 manifest → value map → 显示字符串 |
| CRIM 数据 | EVTX 文件内嵌 VMAP/BMAP 块，存 `(value, message_id)` 对 |
| 离线场景 | VMAP 的 `message_id` 无法解析 → 需要 MSG DB 或 fallback |
| 标准类型 | `win:Boolean` 等可直接硬编码映射 |
| 竞品状态 | 无其他项目做跨平台 `%N` 枚举解析 |
| **推荐起点** | **Phase 1（标准类型）最快见效，Phase 2（CRIM VMAP）覆盖完整** |
