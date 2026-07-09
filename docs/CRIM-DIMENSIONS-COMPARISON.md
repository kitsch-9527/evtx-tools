# CRIM/WEVT_TEMPLATE 解析维度对比

> 对照 evtx crate (Rust) 和 dissect.eventlog (Python)，逐块对比 hotech-evtx 的 CRIM 解析覆盖度。
> 参考 libfwevt 规范（https://github.com/libyal/libfwevt）验证字段完整性。
> 最后更新：2026-07-07

---

## 1. 总览：块类型覆盖

| 块类型 | evtx crate | dissect.eventlog | libfwevt 规范 | **hotech-evtx** | 差距 |
|--------|-----------|-----------------|--------------|----------------|------|
| **CRIM** | ✅ 完整 | ✅ 完整 | ✅ 完整 | ✅ 完整 | — |
| **WEVT** | ✅ 完整 | ✅ 完整 | ✅ 完整 | ✅ 完整 | — |
| **EVNT** | ✅ 完整 | ✅ 完整 | ✅ 完整 | ✅ 完整 (48B) | — |
| **TASK** | ✅ + 名称解析 | ✅ | ✅ 28B/条 | ✅ + 名称解析 | — |
| **OPCO** | ✅ + 名称解析 | ✅ | ✅ 12B/条 | ✅ + 名称解析 | — |
| **TTBL/TEMP** | ✅ + BinXML+Item名 | ✅ | ✅ 规范完整 | ✅ + 参数名 | — |
| **LEVL** | ✅ **+ 名称解析** | ✅ (levl) | ✅ 12B/条 | ❌ **未解析** | **高** |
| **KEYW** | ✅ **64bit标识+名称** | ✅ (keyw) | ✅ 16B/条 | ❌ **未解析** | **高** |
| **CHAN** | ✅ **名称解析** | ✅ (chan) | ✅ 16B/条 | ❌ **未解析** | **中** |
| **MAPS (VMAP)** | ✅ **值映射** | ✅ (vmap/bmap) | ✅ 规范完整 | ❌ **未解析** | **低** |
| **MAPS (BMAP)** | ✅ **位图映射** | ✅ (bmap) | ✅ 规范完整 | ❌ **未解析** | **低** |
| **PRVA** | ❌ (unknown) | ✅ (prva) | ❌ | ❌ **未解析** | **低** |

**结论：hotech-evtx 覆盖了 12 种块类型中的 4 种**（EVNT/TASK/OPCO/TTBL），
缺少 LEVL、KEYW、CHAN、VMAP、BMAP、MAPS、PRVA 共 7 种。

**但核心消息解析需要的只有 LEVL 和 KEYW** —— 其他块（CHAN/MAPS/PRVA）对消息解析无直接影响。

---

## 2. 逐块字段维度对比

### 2.1 CRIM 头（所有项目一致）

| 偏移 | 大小 | 字段 | evtx | dissect | hotech |
|------|------|------|------|---------|--------|
| +0 | 4 | signature `"CRIM"` | ✅ | ✅ | ✅ |
| +4 | 4 | size | ✅ | ✅ | ✅ |
| +8 | 2 | major_version | ✅ | ✅ | ❌ 未解析 |
| +10 | 2 | minor_version | ✅ | ✅ | ❌ 未解析 |
| +12 | 4 | provider_count | ✅ | ✅ | ✅ |

hotech 未解析版本号，但不影响功能。

### 2.2 WEVT Provider 头

| 偏移 | 大小 | 字段 | evtx | dissect | hotech |
|------|------|------|------|---------|--------|
| +0 | 4 | signature `"WEVT"` | ✅ | ✅ | ✅ |
| +4 | 4 | size | ✅ | ✅ | ✅ |
| +8 | 4 | message_table_id | ✅ | ✅ | ✅ |
| +12 | 4 | number_of_descriptors | ✅ | ✅ | ✅ |
| +16 | 4 | unknown2_count | ✅ | ✅ | ❌ |
| +20 | 8×N | element_descriptors | ✅ | ✅ | ✅ |

### 2.3 EVNT — Event Definition（48 字节/条）

所有三个项目解析完全相同的字段。这是 CRIM 中最关键也最稳定的块。

| 偏移 | 大小 | 字段 | evtx | dissect | hotech | 说明 |
|------|------|------|------|---------|--------|------|
| +0 | 2 | `event_id` | ✅ u16 | ✅ | ✅ u16 | |
| +2 | 1 | `version` | ✅ u8 | ✅ | ✅ u8 | |
| +3 | 1 | `channel` | ✅ u8 | ✅ | ✅ u8 | |
| +4 | 1 | `level` | ✅ u8 | ✅ | ✅ u8 | |
| +5 | 1 | `opcode` | ✅ u8 | ✅ | ✅ u8 | |
| +6 | 2 | `task` | ✅ u16 | ✅ | ✅ u16 | |
| +8 | 8 | `keywords` | ✅ u64 | ✅ | ✅ u64 | |
| +16 | 4 | `message_identifier` | ✅ u32 | ✅ | ✅ u32 | |
| +20 | 4 | `template_offset` | ✅ Opt<u32> | ✅ | ✅ Opt<u32> | |
| +24 | 4 | `opcode_offset` | ✅ Opt<u32> | ✅ | ✅ Opt<u32> | |
| +28 | 4 | `level_offset` | ✅ Opt<u32> | ✅ | ✅ Opt<u32> | |
| +32 | 4 | `task_offset` | ✅ Opt<u32> | ✅ | ✅ Opt<u32> | |
| +36 | 4 | `unknown_count` | ✅ u32 | ✅ | ❌ | |
| +40 | 4 | `unknown_offset` | ✅ u32 | ✅ | ❌ | |
| +44 | 4 | `flags` | ✅ u32 | ✅ | ❌ | |

hotech 跳过了 +36~+44 的三个字段，它们目前不在任何已知用例中使用。
**注意**：hotech 的 `level_offset`/`task_offset`/`opcode_offset` 已经正确解析并存储，
但解析完成后从未被用于查 LEVL/TASK/OPCO 名称表。

### 2.4 TASK — Task Definition（28 字节/条 ✅ hotech 已实现）

| 偏移 | 大小 | 字段 | evtx | dissect | hotech |
|------|------|------|------|---------|--------|
| +0 | 4 | `identifier` (u32) | ✅ | ✅ | ✅ |
| +4 | 4 | `message_identifier` | ✅ | ✅ | ✅ |
| +8 | 16 | `mui_identifier` (Guid) | ✅ | ✅ | ✅ |
| +24 | 4 | `data_offset` | ✅ | ✅ | ✅ |
| — | — | name (resolved) | ✅ | ✅ | ✅ |

✅ 完全一致，无差距。

### 2.5 OPCO — Opcode Definition（12 字节/条 ✅ hotech 已实现）

| 偏移 | 大小 | 字段 | evtx | dissect | hotech |
|------|------|------|------|---------|--------|
| +0 | 4 | `identifier` (u32) | ✅ | ✅ | ✅ |
| +4 | 4 | `message_identifier` | ✅ | ✅ | ✅ |
| +8 | 4 | `data_offset` | ✅ | ✅ | ✅ |
| — | — | name (resolved) | ✅ | ✅ | ✅ |

✅ 完全一致，无差距。

### 2.6 LEVL — Level Definition（12 字节/条 ❌ hotech 未解析）

与 OPCO 完全相同的布局：

| 偏移 | 大小 | 字段 | evtx | dissect | hotech | 说明 |
|------|------|------|------|---------|--------|------|
| +0 | 4 | `identifier` (u32) | ✅ | ✅ | ❌ | 0=Unknown, 1=Critical, 2=Error, 3=Warning, 4=Info, 5=Verbose |
| +4 | 4 | `message_identifier` | ✅ | ✅ | ❌ | |
| +8 | 4 | `data_offset` | ✅ | ✅ | ❌ | → `read_sized_utf16_string` → 可读名称 |
| — | — | name (resolved) | ✅ | ✅ | ❌ | 如 `"Critical"`, `"Error"`, `"Warning"`, `"Information"`, `"Verbose"` |

**hotech 当前做法**：`severity_from_level()` 用数值查硬编码中文映射表。
**其他项目做法**：解析 LEVL 块获取 level 的原始英文名称，存入消息 DB。
**差距影响**：消息 DB 的 `level` 列只有数值，没有可读名称。

### 2.7 KEYW — Keyword Definition（16 字节/条 ❌ hotech 未解析）

注意 KEYW 的 `identifier` 是 **u64（8 字节）**，不是 u32！

| 偏移 | 大小 | 字段 | evtx | dissect | hotech | 说明 |
|------|------|------|------|---------|--------|------|
| +0 | 8 | `identifier` (u64) | ✅ | ✅ | ❌ | Keywords bitmask (如 `0x8020000000000000`) |
| +8 | 4 | `message_identifier` | ✅ | ✅ | ❌ | |
| +12 | 4 | `data_offset` | ✅ | ✅ | ❌ | → `read_sized_utf16_string` → 名称 |
| — | — | name (resolved) | ✅ | ✅ | ❌ | 如 `"Audit Success"`, `"Audit Failure"`, `"Correlation Hint"` |

**影响**：EVTX 的 `Keywords` 字段是一个 64bit bitmask（如 `0x8020000000000000`），
关键词名称可以将 bitmask 拆解为可读名称（如 `Audit Success` + `System`）。
当前 hotech 只是原样输出 hex 值。

### 2.8 CHAN — Channel Definition（16 字节/条 ❌ hotech 未解析）

| 偏移 | 大小 | 字段 | evtx | dissect | hotech |
|------|------|------|------|---------|--------|
| +0 | 4 | `identifier` (u32) | ✅ | ✅ | ❌ |
| +4 | 4 | `name_offset` | ✅ | ✅ | ❌ |
| +8 | 4 | `unknown` | ✅ | ✅ | ❌ |
| +12 | 4 | `message_identifier` | ✅ | ✅ | ❌ |
| — | — | name (resolved) | ✅ | ✅ | ❌ |

**影响**：极低。EVTX 的 `Channel` 字段已经是可读字符串（如 `"Security"`），
不需要从 CHAN 块额外解析。

### 2.9 TTBL/TEMP — Template Table（✅ hotech 已实现）

| 字段 | evtx | dissect | hotech | 说明 |
|------|------|---------|--------|------|
| TEMP.size | ✅ | ✅ | ✅ | |
| TEMP.item_descriptor_count | ✅ | ✅ | ✅ | |
| TEMP.item_name_count | ✅ | ✅ | ❌ | 未使用 |
| TEMP.template_items_offset | ✅ | ✅ | ✅ | |
| TEMP.event_type | ✅ | ✅ | ✅ | 1=EventData, 2=UserData |
| TEMP.guid (16B) | ✅ | ✅ | ❌ | 未存储 |
| TEMP.binxml bytes | ✅ | ✅ | ❌ | 未存储 |
| Item descriptor: unknown1 | ✅ | ✅ | ❌ | |
| Item: input_type (inType) | ✅ | ✅ | ✅ | |
| Item: output_type (outType) | ✅ | ✅ | ❌ | |
| Item: count | ✅ | ✅ | ✅ | |
| Item: length | ✅ | ✅ | ❌ | |
| Item: name | ✅ | ✅ | ✅ | **参数名提取** |
| Item: name_offset | ✅ | ✅ | ✅ | |

TEMP 的 `guid` 是 evtx crate 做 `EventKey` JOIN 的核心字段，hotech 当前不存储它。
hotech 用 `template_offset`（EVNT 记录中的 offset）来匹配模板，不依赖 GUID。

### 2.10 MAPS（VMAP/BMAP）— 值映射/位图（❌ hotech 未解析）

**VMAP（Value Map）:**

| 偏移 | 大小 | 字段 | evtx | dissect | hotech |
|------|------|------|------|---------|--------|
| +0 | 4 | `"VMAP"` | ✅ | ✅ | ❌ |
| +4 | 4 | size | ✅ | ✅ | ❌ |
| +8 | 4 | map_string_offset | ✅ | ✅ | ❌ |
| +12 | 4 | entry_count | ✅ | ✅ | ❌ |
| +16 | 8×N | entries (id+msg_id) | ✅ | ✅ | ❌ |
| — | — | map_string (resolved) | ✅ | ✅ | ❌ |

**BMAP（Bitmap）:** 格式未完全公开，evtx 按 raw bytes 存储。

**影响**：极低。MAPS 用于少数 provider 的特殊值映射（如枚举类型），
对大部分事件的消息解析没有直接影响。

---

## 3. 按影响分类

### 直接影响消息解析（必须修复）

| 块 | 缺失字段 | 影响 | 优先级 |
|----|---------|------|--------|
| **LEVL** | name（Level 显示名） | 消息 DB 中 level 只有数值，无英文名称 | **P0** |
| **KEYW** | name（Keyword 显示名） | 消息 DB 中 keywords 只有 hex bitmask，无可读名称 | **P0** |

### 间接影响消息解析（可推迟）

| 块 | 缺失字段 | 影响 | 优先级 |
|----|---------|------|--------|
| **TEMP.guid** | 模板 GUID | 影响 evtx crate 风格的 EventKey JOIN，hotech 用 offset 替代 | P3 |
| **TEMP.binxml** | BinXML 片段 | 用于 template_instances() 跳过 JSON 路径，大工程 | P3 |
| **TEMP event_type** | 参数位置提示 | 不影响名称提取 | P3 |

### 不影响消息解析（可忽略）

| 块 | 原因 |
|----|------|
| **CHAN** | Channel 字段已从 EVTX JSON 获得可读名称 |
| **VMAP/BMAP** | 仅少数 provider 使用，不影响核心消息解析 |
| **PRVA** | 用途未知，无项目使用 |

---

## 4. 结论

### 需要对齐的维度 = 2 个：LEVL + KEYW

其他开源项目（evtx crate、dissect.eventlog）解析了全部 12 种 CRIM 块类型，
但其中**直接影响消息解析质量的只有 LEVL 和 KEYW** 这两个。

当前 hotech-evtx 的 `parse_crim_provider_data()` 已经有了完整的 EVNT/TASK/OPCO/TTBL
解析，并且正确地解析了每个 EVNT 事件的 `level_offset`、`opcode_offset`、`task_offset`。
**缺少的只是存进去查表的步骤。**

### 各块与消息 DB 列的关系

| 消息 DB 列 | 来源 | 当前状态 | 需要修复 |
|-----------|------|---------|---------|
| `level` (数值) | EVNT.level | ✅ 已提取 | — |
| `severity` (中文) | `severity_from_level()` | ✅ 硬编码 | — |
| **英文 Level 名** | **LEVL 块** | ❌ 缺失 | **P0** |
| `task` (可读名) | TASK 块 | ✅ 已解析 | — |
| `opcode` (可读名) | OPCO 块 | ✅ 已解析 | — |
| `keywords` (hex) | EVNT.keywords | ✅ 原样输出 | — |
| **Keywords 可读名** | **KEYW 块** | ❌ 缺失 | **P0** |
| `parameters` | TTBL/TEMP | ✅ 已解析 | — |

### 修复后效果

| 当前输出 | 修复后输出 |
|---------|-----------|
| `level: 1, severity: "关键"` | `level: 1, severity: "关键", level_name: "Critical"` |
| `keywords: "0x8020000000000000"` | `keywords: "0x8020000000000000", keyword_names: "Audit Success, System"` |

---

## 附录 A：evtx crate vs dissect.eventlog 解析深度对比

| 维度 | evtx crate (Rust) | dissect.eventlog (Python) |
|------|-------------------|---------------------------|
| 块类型支持 | 9/9 (CHAN/EVNT/KEYW/LEVL/MAPS/OPCO/TASK/TTBL/Unknown) | 12/12 (含 PRVA) |
| 字段级解析 | 完整，逐字段 named reader | 通过 `header_def`/`record_def` 声明式定义 |
| string 解析 | `read_sized_utf16_string` (4B prefix) | 类似实现 |
| TEMP BinXML | 完整解析 + 渲染函数 | 类似 |
| 名称解析 | 解析时立即 resolve | 解析时立即 resolve |
| GUID 匹配 | `EventKey` 复合键 + `build_index()` | 未公开 |
| 类型系统 | 所有结构体 typed struct | `dataclass`-like |
| wevtcache | ✅ 可序列化缓存 | ❌ |

**两个项目在 CRIM 解析深度上基本持平，都是完整覆盖所有已知块类型。**
hotech-evtx 只需要补上 LEVL 和 KEYW 两个块，就能达到同等覆盖水平。
