# 缺失 CRIM 块的数据影响分析

> hotech-evtx 当前不解析的 CRIM 块类型，以及它们实际丢失的数据和信息。
> 逐块分析"不解析这个块 = 损失了什么"。
> 最后更新：2026-07-07

---

## 总览

```
CrimProviderData {
    events:      Vec<CrimEventEntry>,     ← EVNT 块 ✅
    tasks:       HashMap<u32, String>,    ← TASK 块 ✅
    tasks_by_offset:  ..                  ← TASK 块 ✅
    opcodes:     HashMap<u32, String>,    ← OPCO 块 ✅
    opcodes_by_offset: ..                 ← OPCO 块 ✅
    template_params:  ..                  ← TTBL 块 ✅

    levels:      ❌ 缺失                   ← LEVL 块
    levels_by_offset: ❌ 缺失              ← LEVL 块
    keywords_map:  ❌ 缺失                 ← KEYW 块
    keywords_by_offset: ❌ 缺失            ← KEYW 块
}
```

---

## 逐块分析

### 块 1: LEVL（Level Definition）— **直接影响消息质量**

**当前行为：** level 数值 → `severity_from_level()` 硬编码中文映射 → severity 列

**不解析 LEVL 的具体影响：**

| 维度 | 当前 | 解析后 |
|------|------|--------|
| DB messages.level 列 | `0`（数值） | `"Information"`（可读名称） |
| DB messages.severity 列 | `"信息"`（中文硬编码） | 保留中文 severity + 新增 `level_name` |
| 解析输出 level | `0` | `0`（不变，保留数值） |
| 解析输出 severity | `"信息"` | `"信息"`（不变） |
| **新增字段 level_name** | **NULL** | **`"Information"`** |
| English 用户使用 | 只能看到中文 | 可选择输出英文 level name |

**具体数据损失：**

| level 值 | 硬编码中文 | LEVL 块应提供的英文 | 损失 |
|----------|-----------|-------------------|------|
| 1 | 关键 | `"Critical"` | 丢失英文名称 |
| 2 | 错误 | `"Error"` | 丢失英文名称 |
| 3 | 警告 | `"Warning"` | 丢失英文名称 |
| 4 | 信息 | `"Information"` | 丢失英文名称 |
| 5 | 详细 | `"Verbose"` | 丢失英文名称 |

**实际丢失的信息量：** 每个 level 值对应一个英文名称。不解析 LEVL = 丢失 5 个字符串。
影响中等。

**修复成本：** ~30 行代码（与 OPCO 完全相同的解析模式）

**消息流中的位置：**
```
EVTX JSON: System.Level = "2"
    ↓
extract_fields(): level = Some(2)
    ↓
severity_from_level(2) → "错误"  ✅ 已有，保留
    ↓
(当前)  → column: level=2, severity="错误"
(修复后) → column: level=2, severity="错误", level_name="Error"
```

---

### 块 2: KEYW（Keyword Definition）— **直接影响消息质量**

**当前行为：** EVTX JSON 的 `System.Keywords` 字段原样输出 hex 字符串。

**不解析 KEYW 的具体影响：**

| 维度 | 当前 | 解析后 |
|------|------|--------|
| DB messages.keywords 列 | `"0x8020000000000000"` | `"Audit Success, System"`（可读） |
| 解析输出 keywords | `"0x8020000000000000"` | `"0x8020000000000000"`（保留 hex） |
| **新增字段 keywords_name** | **NULL** | **`"Audit Success, System"`** |

**具体数据损失：**

常见的 Keywords 64-bit bitmask：

| Bitmask | KEYW 块定义名称 | 当前输出 | 解析后输出 |
|---------|----------------|---------|-----------|
| `0x8020000000000000` | `Audit Success` + `System` | `"0x8020000000000000"` | `"Audit Success, System"` |
| `0x8010000000000000` | `Audit Failure` + `System` | `"0x8010000000000000"` | `"Audit Failure, System"` |
| `0x8000000000000000` | `System` | `"0x8000000000000000"` | `"System"` |
| `0x8000000000000001` | `Correlation Hint` + `System` | `"0x8000000000000001"` | `"Correlation Hint, System"` |
| `0x4000000000000000` | `Sqm` | `"0x4000000000000000"` | `"Sqm"` |
| `0x2000000000000000` | `Diag` | `"0x2000000000000000"` | `"Diag"` |
| `0x1000000000000000` | `Perf` | `"0x1000000000000000"` | `"Perf"` |

**实际丢失的信息量：** 每个 provider 在 KEYW 块中定义了 2-10 个 keyword 名称。
不解析 KEYW = 丢失所有 keyword 可读名称。但大部分用户只看 event_id 和 message，
keywords 列的使用频率较低。

**影响程度：** 中等偏低（keywords 列不是最常用的字段）

**修复成本：** ~35 行代码（注意 identifier 是 u64 不是 u32）

**消息流中的位置：**
```
EVTX JSON: System.Keywords = "0x8020000000000000"
    ↓
extract_fields(): keywords = Some("0x8020000000000000")
    ↓
(当前)  → column: keywords="0x8020000000000000"
(修复后) → column: keywords="0x8020000000000000", keywords_name="Audit Success, System"
```

---

### 块 3: CHAN（Channel Definition）— **无实际数据损失**

**当前行为：** Channel 已从 EVTX JSON 的 `System.Channel` 获得可读字符串。

**为什么不需要解析 CHAN：**

```
EVTX JSON 中的 Channel 字段（已提取）:
  "Security"
  "System"
  "Application"
  "Microsoft-Windows-.../Operational"

CHAN 块的 name_offset 指向的内容：
  "Security" ← 和 JSON 里的一模一样
  "System"   ← 和 JSON 里的一模一样
```

**数据来源：** EVTX 文件本身已经在 XML/JSON 渲染时提供了 Channel 的完整名称字符串。
CHAN 块只是在 CRIM 中定义了一次通道元数据，但 EVTX 每条记录的 `System.Channel`
字段已经是最终可读形式。

**结论：不解析 CHAN = 不丢失任何数据。**

---

### 块 4: VMAP/BMAP（Value Map / Bitmap）— **极少数场景有影响**

**当前行为：** VMAP 和 BMAP 完全不解析。当前也没有使用场景。

**VMAP/BMAP 是什么：**

VMAP（Value Map）和 BMAP（Bitmap）是 CRIM 中的"枚举值→字符串"映射表。
它们定义了类似这样的映射：

```
VMAP 条目:
  event_id=5136, 字段 "OperationType"
    0 → "Value Added"
    1 → "Value Deleted"
    2 → "Value Modified"

BMAP 条目:
  event_id=4688, 字段 "MandatoryLabel"
    0x00000000 → "Untrusted"
    0x00001000 → "Low"
    0x00002000 → "Medium"
    0x00003000 → "High"
    0x00004000 → "System"
```

**为什么当前没有数据损失：**

VMAP/BMAP 的用途是"把 event_data 中的数值翻译成可读文本"。但 hotech-evtx
的消息解析管道有两种方式实现同样的效果：

1. **消息模板中的 %N 展开**：如果消息模板是 `Operation: %1`，那么 `%1`
   被替换为 event_data 中的原始值（如 `"0"`）。
2. **%%N 参数引用**：如果消息模板有 `%%N`，在 `%%N Phase 1` 实现后，
   会查 ParameterMessageFile 或 DB 得到可读名称。

VMAP/BMAP 相当于 CRIM 内部自带的"%%N 映射表"——但 Windows Event Viewer
在实际渲染时用的是 FormatMessage + ParameterMessageFile，**不是 VMAP**。

**结论：不解析 VMAP/BMAP = 理论上丢失了一些枚举值→文本映射，但实际渲染路径**
**不走 VMAP。这些映射已经通过主消息模板和 %%N 机制覆盖了。**
**用户即使解析 VMAP，也没有输出列存放这些映射。**

---

### 块 5: EVNT 尾部字段（偏移 +36 ~ +44）— **无影响**

当前 `parse_single_evnt_block()` 只解析了 48 字节记录的前 36 字节，
跳过了最后 12 字节：

| 偏移 | 字段 | 含义 | 影响 |
|------|------|------|------|
| +36 | `unknown_count` | 未知 | 任何项目都不使用 |
| +40 | `unknown_offset` | 未知 | 任何项目都不使用 |
| +44 | `flags` | 标志位 | 任何项目都不使用 |

**结论：不解析 = 不丢失任何已知数据。** evtx crate 解析了它们但仅作为"不知道
有什么用但留着"的字段。dissect.eventlog 通过声明式 `header_def` 自动解析了
所有字节，但也没有业务逻辑使用它们。

---

### 块 6: TEMP.guid — **间接影响**

当前解析 TTBL/TEMP 块的 item names（参数名），但跳过了 TEMP 头的 guid 字段（16 字节）。

**evtx crate 使用 TEMP.guid 做什么：**

```rust
// evtx crate 的 EventKey JOIN 方法：
// 1. 构建 event_to_template_guids: HashMap<EventKey, Vec<Guid>>
//    EventKey = (provider_guid, event_id, version, channel, level, opcode, task, keywords)
// 2. 当 EVTX 记录中的 template GUID 缺失/损坏时，
//    用 EventKey 在 CRIM 中查找对应的 TEMP.guid
// 3. 获得 TEMP.guid → 找到 TEMP 定义 → 解析 BinXML → 得到参数名称
```

**hotech-evtx 替代方案：** 使用 `template_offset`（EVNT 记录中的 offset）
直接定位到 TEMP 定义，不依赖 GUID JOIN。

| 方案 | key | 优点 | 缺点 |
|------|-----|------|------|
| evtx crate: GUID JOIN | EventKey + TEMP.guid | provider 独立，跨版本稳定 | 复杂，需要 8 字段复合键 |
| hotech-evtx: offset 匹配 | template_offset | 简单直接 | 依赖二进制布局不变 |

**实际影响：** 当前 hotech-evtx 的 offset 匹配在所有已知场景下工作正常。
TEMP.guid 在 embedded template 缺失时作为 fallback 路径的 key——但 hotech-evtx
不依赖 embedded template。

**结论：不解析 TEMP.guid = 当前不丢失任何功能。** 如果将来需要支持
embedded template fallback，那时再考虑解析 TEMP.guid。

---

### 块 7: CRIM 版本号（+8 ~ +12）— **无影响**

当前不读取 CRIM 头的 `major_version` 和 `minor_version`（偏移 8-11）。

**影响：** 没有直接数据损失。但如果 CRIM 格式在未来 Windows 版本中变更，
hotech-evtx 不会检测到版本变化而调整解析逻辑。

**结论：不解析版本号 = 当前不丢失数据，但会丢失向前兼容性检测的能力。**

---

## 汇总

### 按数据损失量排序

| 块 | 数据损失 | 使用频率 | 用户能否从其他来源获得？ | 修复成本 |
|----|---------|---------|----------------------|---------|
| **LEVL** | 5 个 level 英文名称 | **高**（每行都有 level） | 能，但需手动查阅文档 | ~30 行 |
| **KEYW** | 每个 provider 2-10 个 keyword 名称 | **中**（不是每行都有 keywords） | 能，但需查 MSDN | ~35 行 |
| TEMP.guid | 备用 Join key | 极低（仅 template missing 时） | offset 替代 | 可忽略 |
| CRIM 版本号 | 版本检测能力 | 极低 | — | 可忽略 |
| CHAN | **无** | — | EVTX JSON 已有 | 不修复 |
| VMAP/BMAP | **实际无**（不走此路径） | — | 盖过 %%N / 消息模板 | 不修复 |
| EVNT 尾部 | **无** | — | 任何项目都不使用 | 不修复 |

### 最终结论

**hotech-evtx 因缺失 CRIM 块而丢失的真实信息 = 2 类：**

1. **Level 英文名称**（5 个字符串，每行都会用到）→ **P0，修复**
2. **Keywords 可读名称**（每个 provider 数个，不常用）→ **P0，修复**

其他缺失块导致的"数据损失"要么为零，要么被消息解析管道中的其他机制覆盖。
