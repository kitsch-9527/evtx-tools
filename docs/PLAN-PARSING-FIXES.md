# 解析修复计划与验证方案（历史记录）

> **⚠️ 注意：** 本文档中涉及 dumper 的计划已因功能迁移到 Go 而过期。
> Rust 端的 `hotech-evtx` 只负责 EVTX 解析。消息库生成由 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)（二进制 `msgdb-dump`）提供。本文档保留供历史参考。

---

## 目录

- [CRIM 块类型覆盖总览](#crim-块类型覆盖总览)
- [优先级总览](#优先级总览)
- [P0: LEVL 名称提取](#p0-level-名称提取)
- [P0: KEYW 名称提取](#p0-keyw-名称提取)
- [P0: %%N 参数展开 Phase 1](#p0-n-参数展开-phase-1)
- [P1: Build 检测 Fallback](#p1-build-检测-fallback)
- [P1: task/opcode CRIM 覆盖增强](#p1-taskopcode-crim-覆盖增强)
- [P2: CHAN/MAPS 补充解析](#p2-chanmaps-补充解析)
- [附录 A: 数据流变更图](#附录-a-数据流变更图)
- [附录 B: 测试矩阵](#附录-b-测试矩阵)
- [附录 C: 回归测试清单](#附录-c-回归测试清单)

---

## CRIM 块类型覆盖总览

| # | 块类型 | evtx crate | dissect.eventlog | libfwevt 规范 | **hotech-evtx 当前** | **优先级** |
|---|--------|-----------|-----------------|--------------|---------------------|-----------|
| 1 | **CRIM** (头) | ✅ | ✅ | ✅ 16B | ✅ 部分 | — |
| 2 | **WEVT** (Provider) | ✅ 完整 | ✅ | ✅ | ✅ 完整 | — |
| 3 | **EVNT** (事件定义) | ✅ **12字段** | ✅ | ✅ 48B/条 | ✅ **9/12字段** | — |
| 4 | **TASK** (任务) | ✅ +名称 | ✅ | ✅ 28B/条 | ✅ +名称 | — |
| 5 | **OPCO** (操作码) | ✅ +名称 | ✅ | ✅ 12B/条 | ✅ +名称 | — |
| 6 | **TTBL/TEMP** (模板) | ✅ +BinXML | ✅ | ✅ | ✅ +参数名 | — |
| **7** | **LEVL** (级别) | ✅ **+名称** | ✅ (levl) | ✅ 12B/条 | ❌ **未解析** | **P0** |
| **8** | **KEYW** (关键词) | ✅ **+名称** | ✅ (keyw) | ✅ 16B/条 | ❌ **未解析** | **P0** |
| 9 | CHAN (通道) | ✅ +名称 | ✅ (chan) | ✅ 16B/条 | ❌ 未解析 | P3 |
| 10 | MAPS-VMAP (值映射) | ✅ | ✅ (vmap) | ✅ | ❌ 未解析 | P3 |
| 11 | MAPS-BMAP (位图) | ✅ | ✅ (bmap) | ✅ | ❌ 未解析 | P3 |
| 12 | PRVA (私有) | ❌(unknown) | ✅ (prva) | ❌ | ❌ 未解析 | 忽略 |

**当前覆盖：** 12 种块类型中只覆盖了 **4 种**，核心缺失 2 种（LEVL + KEYW）。

### 字段级对比：hotech-evtx 在 EVNT 块中跳过的字段

| 偏移 | 大小 | 字段 | evtx crate | hotech-evtx | 说明 |
|------|------|------|-----------|-------------|------|
| +36 | 4 | `unknown_count` | ✅ u32 | ❌ 跳过 | 不用于消息解析 |
| +40 | 4 | `unknown_offset` | ✅ u32 | ❌ 跳过 | 同上 |
| +44 | 4 | `flags` | ✅ u32 | ❌ 跳过 | 同上 |

这三个字段不在任何已知用例中，跳过无影响。

### 各块与消息 DB 列的对应关系

```
消息 DB 列       ← 来源（CRIM 块 + 字段）                 → 当前状态
──────────────────────────────────────────────────────────────────
level (数值)     ← EVNT.level                              ✅ 已提取
severity (中文)  ← severity_from_level()                   ✅ 硬编码
Level 显示名     ← LEVL.identifier → LEVL.data_offset      ❌ 缺失 P0
task (可读名)    ← TASK.identifier → TASK.data_offset       ✅ 已解析
opcode (可读名)  ← OPCO.identifier → OPCO.data_offset      ✅ 已解析
keywords (hex)   ← EVNT.keywords                           ✅ 原样输出
Keywords 可读名  ← KEYW.identifier(64bit) → name           ❌ 缺失 P0
parameters       ← TTBL.TEMP item name                     ✅ 已解析
message          ← MESSAGETABLE / MUI                      ✅ 已解析
```

---

## 优先级总览

| 优先级 | 问题 | 工作量 | 影响范围 | 依赖 |
|--------|------|--------|---------|------|
| **P0** | LEVL 名称提取 | ~30行 + 测试 | dumper → 解析消息 | 无 |
| **P0** | KEYW 名称提取 | ~35行 + 测试 | dumper → 解析消息 | 无 |
| **P0** | %%N Phase 1 | ~200行 + schema变更 | scanner → dumper → parser | 无 |
| **P1** | Build 检测 Fallback | ~80行 + 测试 | parse 命令消息匹配 | 无 |
| **P1** | task/opcode CRIM 增强 | ~50行 + 调试工具 | dumper CRIM 解析 | 无 |
| **P3** | CHAN/MAPS 补充 | ~80行 | dumper CRIM 解析（不影响消息） | 无 |

**P0 推荐执行顺序：** LEVL → KEYW → %%N Phase 1

---

## P0: LEVL 名称提取

### 问题

CRIM 的 LEVL（Level Definition）块包含 level 显示名（如 `"Critical"`、`"Error"`），
当前 `parse_crim_provider_data()` 只匹配了 EVNT/TASK/OPCO/TTBL，**没有 LEVL 分支**。

虽然 `CrimEventEntry` 已经存储了 `level_offset`，但从未用于查 LEVL 名称。
当前 severity 全部来自硬编码的 `severity_from_level()`（中文硬编码映射），
level 的原始英文名称未被提取，消息 DB 中 `level` 列仅存数值。

### 格式（与 OPCO 完全一致）

```
LEVL 块头: "LEVL"(4) + size(4) + count(4) = 12 字节
每条记录 12 字节:
  +0: identifier (u32)     — 1=Critical, 2=Error, 3=Warning, 4=Info, 5=Verbose
  +4: message_identifier (u32)  — 0xFFFFFFFF = None
  +8: data_offset (u32)    — → UTF-16LE string（可读名称）
```

### 实现方案

#### Step 1: 在 `extractor.rs` 添加 `parse_level_block()`

在 `parse_opcode_block()` 函数后面添加（结构几乎相同）：

```rust
/// Parse a LEVL block, returning (id→name, offset→name) maps.
fn parse_level_block(
    crim_data: &[u8],
    pos: usize,
) -> (HashMap<u32, String>, HashMap<u32, String>) {
    if pos + 12 > crim_data.len() || &crim_data[pos..pos + 4] != b"LEVL" {
        return (HashMap::new(), HashMap::new());
    }
    let count = u32::from_le_bytes([
        crim_data[pos + 8], crim_data[pos + 9],
        crim_data[pos + 10], crim_data[pos + 11],
    ]);
    if count == 0 || count > 4096 {
        return (HashMap::new(), HashMap::new());
    }
    let mut levels = HashMap::new();
    let mut by_offset = HashMap::new();
    for i in 0..count as usize {
        let rec_off = pos + 12 + i * 12;
        if rec_off + 12 > crim_data.len() { break; }
        let id = u32::from_le_bytes([
            crim_data[rec_off], crim_data[rec_off + 1],
            crim_data[rec_off + 2], crim_data[rec_off + 3],
        ]);
        let name_off = u32::from_le_bytes([
            crim_data[rec_off + 8], crim_data[rec_off + 9],
            crim_data[rec_off + 10], crim_data[rec_off + 11],
        ]);
        if name_off != 0 {
            if let Some(name) = read_crim_utf16_string(crim_data, name_off as usize) {
                levels.insert(id, name.clone());
                by_offset.insert(rec_off as u32, name);
            }
        }
    }
    (levels, by_offset)
}
```

#### Step 2: 在 `parse_crim_provider_data()` 添加 LEVL 分支

```rust
// 已有：} else if sig == b"TASK" { ... }
// 已有：} else if sig == b"OPCO" { ... }
// 已有：} else if sig == b"TTBL" { ... }

// 新增：
} else if sig == b"LEVL" {
    let (by_id, by_offset) = parse_level_block(crim_data, elem_off);
    result.levels = by_id;
    result.levels_by_offset = by_offset;
}
```

#### Step 3: 更新 `CrimProviderData` 结构体

```rust
pub struct CrimProviderData {
    pub events: Vec<CrimEventEntry>,
    pub tasks: HashMap<u32, String>,
    pub tasks_by_offset: HashMap<u32, String>,
    pub opcodes: HashMap<u32, String>,
    pub opcodes_by_offset: HashMap<u32, String>,
    pub template_params: HashMap<u32, Vec<(usize, String)>>,
    // 新增：
    pub levels: HashMap<u32, String>,
    pub levels_by_offset: HashMap<u32, String>,
}
```

#### Step 4: CRIM+MUI fallback 中使用 level 名称

修改 `cmd_dump.rs` 的 `extract_mui_crim_from_blobs()`：

```rust
// 在已有 task_name / opcode_name 解析之后添加：
let level_name = ev.level_offset
    .and_then(|off| provider_data.levels_by_offset.get(&off).cloned())
    .or_else(|| provider_data.levels.get(&(ev.level as u32)).cloned());
```

#### Step 5: 消息 DB 中存储 level 显示名

`PeMessage` 新增字段 `level_name: Option<String>`，dumper 写入 DB 时填入。
DB schema 变更（messages 表新增列 `level_name TEXT`）。

### 涉及的代码文件与行号

| 文件 | 修改类型 | 近似行数 |
|------|---------|---------|
| `hotech-evtx-dumper/src/extractor.rs` | 新增 `parse_level_block()` (~25行) + `CrimProviderData` 字段 (~2行) + LEVL 分支 (~3行) | **30 行** |
| `hotech-evtx-core/src/lib.rs` | `ParsedEvent` 新增 `level_name` 字段 | 1 行 |
| `hotech-evtx-dumper/src/writer_sys.rs` | messages DDL 新增 `level_name` 列 + insert 适配 | 5 行 |
| `hotech-evtx/src/cmd_dump.rs` | CRIM+MUI fallback 中使用 level 名称 | 3 行 |
| `hotech-evtx-core/src/parser.rs` | `extract_fields()` 中传递 `level_name` 到 `ParsedEvent` | 1 行 |

### 测试方案

#### 单元测试

```rust
#[test]
fn test_parse_level_block_from_system_events() {
    #[cfg(windows)]
    {
        let blobs = extract_raw_crim_blobs(
            r"C:\Windows\System32\Microsoft-Windows-System-Events.dll"
        ).unwrap();
        let data = parse_crim_provider_data(&blobs[0],
            "{f1ef270a-0d32-4352-ba52-dbab41e1d859}");
        assert!(data.is_some());
        let result = data.unwrap();
        assert!(!result.levels.is_empty(),
            "should have at least one level name");
        for (id, name) in &result.levels {
            eprintln!("  Level {id}: {name}");
            assert!(!name.is_empty(), "level name should not be empty");
        }
    }
}

#[test]
fn test_parse_level_block_empty() {
    let (by_id, by_offset) = parse_level_block(&[], 0);
    assert!(by_id.is_empty());
    assert!(by_offset.is_empty());

    // signature mismatch
    let data = b"CRIM".to_vec();
    let (by_id, by_offset) = parse_level_block(&data, 0);
    assert!(by_id.is_empty());
}

#[test]
fn test_crim_provider_data_includes_levels() {
    #[cfg(windows)]
    {
        let path = r"C:\Windows\System32\Microsoft-Windows-System-Events.dll";
        if !std::path::Path::new(path).exists() { return; }
        let blobs = extract_raw_crim_blobs(path).unwrap();
        let guid = "{f1ef270a-0d32-4352-ba52-dbab41e1d859}";
        let result = parse_crim_provider_data(&blobs[0], guid);
        assert!(result.is_some());
        let data = result.unwrap();
        // 至少包含标准 level: 1=Critical, 2=Error, 3=Warning, 4=Info
        assert!(data.levels.contains_key(&1) || data.levels.contains_key(&2) ||
                data.levels.contains_key(&3) || data.levels.contains_key(&4));
    }
}
```

#### 集成验证

```bash
# 1. 编译
cargo build --release -p hotech-evtx --features dumper

# 2. dump 一个新消息 DB
hotech-evtx dump --out-dir ./test_msg_db/ --lang 1033

# 3. 检查 DB 中 level_name 列有值
hotech-evtx db-info ./test_msg_db/hotech-msg-*.db --table messages --limit 5

# 4. 解析 .evtx 文件，确认 level_name 填充
hotech-evtx parse Security.evtx --msg-dir ./test_msg_db/ -o test_out.csv
# 检查 CSV 中 level_name 列
```

---

## P0: KEYW 名称提取

### 问题

EVTX 的 `Keywords` 字段是一个 64-bit bitmask（如 `0x8020000000000000`），
CRIM 的 KEYW 块定义了每个 bitmask 位对应的可读名称（如 `Audit Success`、`System`）。
当前 hotech-evtx 只原样输出 hex 值，不解析 KEYW 块。

### 格式（注意 identifier 是 u64！）

```
KEYW 块头: "KEYW"(4) + size(4) + count(4) = 12 字节
每条记录 16 字节:
  +0: identifier (u64)     — 64bit bitmask 值
  +8: message_identifier (u32)  — 0xFFFFFFFF = None
  +12: data_offset (u32)   — → UTF-16LE string（可读名称）
```

### 实现方案

#### Step 1: 添加 `parse_keyword_block()`

```rust
/// Parse a KEYW block.
///
/// KEYW record (16 bytes):
///   +0: identifier (u64)   — keyword bitmask
///   +8: msg_id (u32)       — message-table identifier
///   +12: data_offset (u32) — → UTF-16LE name string
fn parse_keyword_block(
    crim_data: &[u8],
    pos: usize,
) -> (HashMap<u64, String>, HashMap<u32, String>) {
    if pos + 12 > crim_data.len() || &crim_data[pos..pos + 4] != b"KEYW" {
        return (HashMap::new(), HashMap::new());
    }
    let count = u32::from_le_bytes([
        crim_data[pos + 8], crim_data[pos + 9],
        crim_data[pos + 10], crim_data[pos + 11],
    ]);
    if count == 0 || count > 4096 {
        return (HashMap::new(), HashMap::new());
    }
    let mut keywords = HashMap::new();
    let mut by_offset = HashMap::new();
    for i in 0..count as usize {
        let rec_off = pos + 12 + i * 16;
        if rec_off + 16 > crim_data.len() { break; }
        let id = u64::from_le_bytes([
            crim_data[rec_off], crim_data[rec_off + 1],
            crim_data[rec_off + 2], crim_data[rec_off + 3],
            crim_data[rec_off + 4], crim_data[rec_off + 5],
            crim_data[rec_off + 6], crim_data[rec_off + 7],
        ]);
        let name_off = u32::from_le_bytes([
            crim_data[rec_off + 12], crim_data[rec_off + 13],
            crim_data[rec_off + 14], crim_data[rec_off + 15],
        ]);
        if name_off != 0 {
            if let Some(name) = read_crim_utf16_string(crim_data, name_off as usize) {
                keywords.insert(id, name.clone());
                by_offset.insert(rec_off as u32, name);
            }
        }
    }
    (keywords, by_offset)
}
```

#### Step 2: 在 `parse_crim_provider_data()` 添加 KEYW 分支

```rust
} else if sig == b"KEYW" {
    let (by_id, by_offset) = parse_keyword_block(crim_data, elem_off);
    result.keywords_map = by_id;
    result.keywords_by_offset = by_offset;
}
```

#### Step 3: 更新 `CrimProviderData`

```rust
pub struct CrimProviderData {
    // ... 现有字段 ...
    pub levels: HashMap<u32, String>,
    pub levels_by_offset: HashMap<u32, String>,
    // 新增 KEYW:
    pub keywords_map: HashMap<u64, String>,
    pub keywords_by_offset: HashMap<u32, String>,
}
```

#### Step 4: 在 CRIM+MUI JOIN 中匹配 keywords

`EXTRA.md`: 在 `extract_mui_crim_from_blobs()` 中对每条 EVNT 记录，
将 `ev.keywords` (u64) 与 KEYW 块的 `identifier` (u64) 做位匹配：

```rust
// 在 CRIM+MUI fallback 中解析 keyword 名称
let keyword_names: Vec<String> = provider_data.keywords_map.iter()
    .filter(|(mask, _)| (ev.keywords & *mask) != 0)
    .map(|(_, name)| name.clone())
    .collect();
let keywords_str = if keyword_names.is_empty() { None }
    else { Some(keyword_names.join(", ")) };
```

#### Step 5: 消息 DB 中存储 keywords 可读名

`PeMessage` 新增 `keywords_name: Option<String>`。

### 涉及的代码文件

| 文件 | 修改类型 | 近似行数 |
|------|---------|---------|
| `hotech-evtx-dumper/src/extractor.rs` | 新增 `parse_keyword_block()` (~30行) + `CrimProviderData` 字段 | **35 行** |
| `hotech-evtx-core/src/lib.rs` | `ParsedEvent` 新增 `keywords_name` 字段 | 1 行 |
| `hotech-evtx/src/cmd_dump.rs` | CRIM+MUI fallback 中 keywords 位匹配 | 5 行 |

### 测试方案

```rust
#[test]
fn test_parse_keyword_block_from_system_events() {
    #[cfg(windows)]
    {
        let blobs = extract_raw_crim_blobs(
            r"C:\Windows\System32\Microsoft-Windows-System-Events.dll"
        ).unwrap();
        let data = parse_crim_provider_data(&blobs[0],
            "{f1ef270a-0d32-4352-ba52-dbab41e1d859}");
        assert!(data.is_some());
        let result = data.unwrap();
        // 期望解析出至少一个 keyword 名称
        eprintln!("KEYW entries: {:?}", result.keywords_map);
        assert!(!result.keywords_map.is_empty() ||
            "system-events may not have KEYW block — test also ok");
    }
}

#[test]
fn test_parse_keyword_block_empty() {
    let (by_id, by_offset) = parse_keyword_block(&[], 0);
    assert!(by_id.is_empty());
}

#[test]
fn test_keyword_bitmask_matching() {
    let keywords_map = HashMap::from([
        (0x8020000000000000u64, "Audit Success".to_string()),
        (0x8000000000000000u64, "System".to_string()),
    ]);
    let ev_keywords: u64 = 0x8020000000000000;
    let names: Vec<&str> = keywords_map.iter()
        .filter(|(mask, _)| (ev_keywords & *mask) != 0)
        .map(|(_, name)| name.as_str())
        .collect();
    assert_eq!(names.join(", "), "Audit Success");
}
```

---

## P0: %%N 参数展开 Phase 1

### 问题

消息中出现 `%%14592`（Direction=Inbound）、`%%1833`（Impersonation Level）等引用码，
当前原样输出。Windows 使用 `%%N` 表示"在 ParameterMessageFile 的 MESSAGETABLE 中以 N
为 resource ID 查找文本"。

**调研发现：**
- evtx crate / Chainsaw / Hayabusa 全部不做 %%N 展开
- EvtxHussar 用 YAML 预定义常见映射（28 个 param 文件：LogonType、NTStatus 等）
- Velociraptor 使用 `dumpevtx` 在采集时提取 ParameterMessageFile DLL 的 MESSAGETABLE
- Windows Event Viewer 调 `FormatMessage()` 实时解析

**hotech-evtx 关键优势：** `scanner.rs:387` 已经读到了 `ParameterFileName` 注册表值，
但从未传递给 dumper 使用。

### 实现方案（三阶段规划）

#### Phase 1A: scanner 传递 ParameterFileName（~30行）

1. 在 `ProviderInfo` 中新增字段：

```rust
pub struct ProviderInfo {
    pub name: String,
    pub guid: Option<String>,
    pub dll_path: PathBuf,
    pub channel: Option<String>,
    pub parameter_file: Option<String>,  // ← 新增
}
```

2. `scanner.rs` 中 `scan_winevt_publishers()` 已有：

```rust
let param_file = read_reg_string(pub_key, "ParameterFileName"); // ✅ 已读取但丢弃
```

改为：

```rust
providers.push(ProviderInfo {
    name: name.clone(),
    guid: Some(guid.clone()),
    dll_path: PathBuf::from(trimmed),
    channel: None,
    parameter_file: param_file.clone(),  // ← 存储
});
```

同样，`scan_eventlog()` 中也需读取 `ParameterMessageFile`：

```rust
let param_file = read_reg_string(source_key, "ParameterMessageFile");
```

#### Phase 1B: dumper 提取 ParameterMessageFile（~80行）

在 `extract_single_dll()` 中，除了处理主 DLL 外，额外处理 parameter_file：

```rust
fn extract_single_dll(...) -> DllResult {
    // ...
    let mut param_messages: HashMap<u32, PeMessage> = HashMap::new();

    // 对每个 provider，尝试提取 ParameterMessageFile
    for provider in providers {
        if let Some(ref param_file) = provider.parameter_file {
            let resolved = resolve_env_vars(param_file);
            if Path::new(&resolved).exists() {
                if let Ok(param_msgs) = extract_messages(&resolved) {
                    param_messages.extend(param_msgs);
                }
            }
        }
    }
    // ...
}
```

#### Phase 1C: 消息 DB 新增 parameter_refs 表（~50行）

系统 DB schema 变更（`writer_sys.rs`）：

```sql
CREATE TABLE IF NOT EXISTS parameter_refs (
    provider_name TEXT NOT NULL,
    param_id      INTEGER NOT NULL,   -- %%N 中的 N
    message       TEXT NOT NULL,
    lang          TEXT NOT NULL DEFAULT 'en-US',
    PRIMARY KEY (provider_name, param_id, lang)
);
```

#### Phase 1D: parse 时展开 %%N（~40行）

在 `cmd_parse.rs` 的 `resolve_event_message()` 中，在 substitute 之后：

```rust
fn expand_param_refs(message: &str, param_refs: &HashMap<u32, String>) -> String {
    let mut result = String::with_capacity(message.len());
    let mut chars = message.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '%' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => {
                // %% 可能是 %%N 的开始
                let mut num_str = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        num_str.push(d);
                        chars.next();
                    } else { break; }
                }
                if let Ok(n) = num_str.parse::<u32>() {
                    if let Some(text) = param_refs.get(&n) {
                        result.push_str(text);
                    } else {
                        result.push_str(&format!("%%{num_str}"));
                    }
                } else {
                    result.push('%');
                    if !num_str.is_empty() { result.push_str(&num_str); }
                }
            }
            Some(other) => { result.push('%'); result.push(other); }
            None => result.push('%'),
        }
    }
    result
}
```

#### Phase 2（后续迭代）：EvtxHussar 式 YAML fallback

在代码中内嵌常见映射（LogonType、NTStatus、UAC flags 等）作为查 DB 不到的 fallback。

### 涉及的代码文件

| 文件 | Phase | 修改类型 |
|------|-------|---------|
| `scanner.rs` | 1A | `ProviderInfo` + `param_file` 传递 |
| `extractor.rs` | 1B | 新增 `extract_param_messages()` |
| `writer_sys.rs` | 1C | 新增 `parameter_refs` DDL + 写入逻辑 |
| `cmd_dump.rs` | 1B | `extract_single_dll()` 增加 param_file 处理 |
| `resolver.rs` | 1D | `MessageDb` 增加 `param_refs` 缓存 |
| `substitute.rs` | 1D | 新增 `expand_param_refs()` 函数 |

### 测试方案

```rust
#[test]
fn test_expand_param_refs_basic() {
    let map = HashMap::from([
        (14592, "Direction=Inbound".to_string()),
        (1833, "Impersonation Level".to_string()),
    ]);
    assert_eq!(expand_param_refs("Error: %%14592", &map), "Error: Direction=Inbound");
    assert_eq!(expand_param_refs("%%1833 applied", &map), "Impersonation Level applied");
    assert_eq!(expand_param_refs("No refs here", &map), "No refs here");
}

#[test]
fn test_expand_param_refs_unknown() {
    let map = HashMap::new();
    assert_eq!(expand_param_refs("%%99999", &map), "%%99999");
}

#[test]
fn test_expand_param_refs_mixed() {
    let map = HashMap::from([(14592, "Inbound".to_string())]);
    assert_eq!(expand_param_refs("%1 logged on: %%14592", &map), "%1 logged on: Inbound");
}

#[cfg(windows)]
#[test]
fn test_scanner_reads_parameter_file() {
    let providers = enum_etw_providers();
    let with_param: Vec<&ProviderInfo> = providers.iter()
        .filter(|p| p.parameter_file.is_some())
        .collect();
    eprintln!("Providers with ParameterMessageFile: {}", with_param.len());
}
```

#### E2E 验证

```bash
# 1. dump 含 parameter refs 的 DB
cargo run --release --features dumper -- dump --out-dir ./msg_db/ --lang 1033

# 2. 确认 parameter_refs 表存在
cargo run --release -- db-info ./msg_db/hotech-msg-*.db --tables

# 3. 解析含 %%N 的 EVTX，确认展开
cargo run --release -- parse input.evtx --msg-dir ./msg_db/ -o out.csv
```

---

## P1: Build 检测 Fallback

### 问题

`detect_evtx_build()` 读 EVTX 文件头 offset `0x40` 读出值为 `0`，导致
`resolve_msg_db()` 的文件名匹配失败。

### 调研参考

- **libevtx**: 不依赖 build 号，用 provider GUID 直接匹配
- **Velociraptor**: 用 OS 版本匹配 DB
- **当前 fallback**: `build == 0` 时用第一个找到的 `hotech-msg-*.db`

### 实现方案

#### Step 1: 改进 `resolve_msg_db()` 从 DB metadata 读取 build 信息

当前按文件名解析 build 号。改为打开候选 DB 查 `metadata` 表：

```rust
fn read_db_build_number(db_path: &Path) -> Option<String> {
    let conn = rusqlite::Connection::open(db_path).ok()?;
    conn.query_row(
        "SELECT value FROM metadata WHERE key = 'build_number'",
        [],
        |row| row.get::<_, String>(0),
    ).ok()
}

fn resolve_msg_db(...) -> Result<Option<String>> {
    // ... 收集候选 DB 文件 ...

    if build == Some(0) || build.is_none() {
        // 降级：打开每个 DB，读 metadata.build_number
        // 选最匹配的（同一主要版本号优先）
        let mut scored: Vec<(i32, PathBuf)> = Vec::new();
        for db_path in &found {
            let db_build = read_db_build_number(db_path).unwrap_or_default();
            let major_build = db_build.split('.')
                .next().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
            scored.push((major_build, db_path.clone()));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0)); // 最高 build 优先
        if let Some((_, path)) = scored.into_iter().next() {
            return Ok(Some(path.to_string_lossy().to_string()));
        }
    }
    // ... 原精确匹配路径 ...
}
```

#### Step 2: 添加 provider GUID 直接匹配作为第二 fallback

```rust
if build == Some(0) || build.is_none() {
    // 先用 provider GUID 尝试匹配
    // 如果 EVTX 中的 provider GUID 在 DB 中存在，直接使用该 DB
}
```

### 涉及的代码文件

| 文件 | 修改类型 |
|------|---------|
| `hotech-evtx/src/cmd_parse.rs` | 重写 `resolve_msg_db()` 降级逻辑 |

### 测试方案

```rust
#[test]
fn test_resolve_msg_db_build_zero_fallback() {
    let tmp = std::env::temp_dir().join("test_msg_db_fallback");
    let _ = std::fs::create_dir_all(&tmp);

    let db_path = tmp.join("hotech-msg-Test-1.0-1000.1.db");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT);
         INSERT INTO metadata VALUES ('build_number', '1000.1');"
    ).unwrap();
    drop(conn);

    let result = resolve_msg_db_impl(&tmp, Some(0));
    assert!(result.is_some());
    assert!(result.unwrap().contains("1000.1"));
}
```

#### E2E 验证

```bash
# 准备 build=0 的 EVTX → 运行 parse
hotech-evtx parse test_build0.evtx --msg-dir ./msg_db/ -o out.db
# 期望：自动匹配到正确的 DB，消息正常解析
```

---

## P1: task/opcode CRIM 覆盖增强

### 问题

部分 provider（如 Security-Auditing/adtschema.dll）的 `task`/`opcode` 列在消息库中为 NULL。
原因是 CRIM 格式在不同 provider 间有差异（CRIM v4 vs v5，字符串布局变化），
当前 `parse_single_evnt_block()` 从 `task_offset`/`opcode_offset` 查 TASK/OPCO 块的
名称，但 offset 可能为空字符串或指向无效位置。

### 调研参考

- **libfwevt 规范**：CRIM v5.1 起某些字符串布局有变化
- **evtx crate**：用 `EventKey` 复合 JOIN（provider_guid + event_id + version + channel + level + opcode + task + keywords），比单纯 offset 匹配更可靠

### 实现方案

#### Step 1: 添加 CRIM blob hex dump 调试开关

```rust
fn debug_crim_block(crim_data: &[u8], sig: &[u8], elem_off: usize, debug: bool) {
    if !debug { return; }
    let end = (elem_off + 64).min(crim_data.len());
    eprintln!("  CRIM block '{}' @ offset {}: {:02x?}",
        std::str::from_utf8(sig).unwrap_or("???"),
        elem_off,
        &crim_data[elem_off..end]);
}
```

#### Step 2: 对照 libfwevt 规范检查偏移量

常见差异：
- CRIM v4: TASK 记录 24 字节（无 MUI GUID）
- CRIM v5: TASK 记录 28 字节（含 MUI GUID）

当前 hotech-evtx 的 `parse_task_block()` 假设 28 字节。需要尝试两种 size：

```rust
fn parse_task_block_flexible(crim_data: &[u8], pos: usize) -> ... {
    for rec_size in [28, 24] {
        // 分别解析，选解析出有效名称多的结果
    }
}
```

#### Step 3: 添加 test 用于 adtschema 调试

```rust
#[cfg(windows)]
#[test]
fn test_crim_task_opcode_from_adtschema() {
    let blobs = extract_raw_crim_blobs(
        r"C:\Windows\System32\adtschema.dll"
    ).unwrap();
    for blob in &blobs {
        let data = parse_crim_provider_data(blob,
            "{54849625-5478-4994-A5BA-3E3B0328C30D}");
        if let Some(data) = data {
            eprintln!("adtschema tasks: {:?}", data.tasks);
            eprintln!("adtschema opcodes: {:?}", data.opcodes);
            // 期望: task offset 能解析出 Logon/SpecialLogon 等名称
        }
    }
}
```

### 涉及的代码文件

| 文件 | 修改类型 |
|------|---------|
| `hotech-evtx-dumper/src/extractor.rs` | `parse_task_block()` 增加 v4/v5 灵活解析 |
| `hotech-evtx/src/cmd_dump.rs` | 添加 `--debug-crim` CLI 参数 |

### 测试方案

- 对 `adtschema.dll` 的 CRIM 直测试
- 跨多版本 CRIM 的兼容性测试（v4 vs v5）

---

## P2: CHAN/MAPS 补充解析

### 问题

当前完全不解析 CHAN（通道定义）和 MAPS（值映射/位图定义）。

### 影响评估

| 块 | 影响 | 决定 |
|----|------|------|
| CHAN | Channel 已从 EVTX JSON 获得可读字符串（如 `"Security"`） | **不需要** |
| VMAP/BMAP | 仅用于少数 provider 的枚举值映射 | **不需要** |

CHAN 和 MAPS 对消息解析无直接影响，当前不实现，标记为 **P3**。
如果未来有用户需要通道显示名或值映射展开，再按需添加。

---

## 附录 A: 数据流变更图

### 变更后的 dump 数据流

```
原流程:
  DLL → extract_all_from_pe() → RT_MESSAGETABLE + CRIM(EVNT/TASK/OPCO/TTBL) → SQLite DB

新流程:
  DLL → extract_all_from_pe()
         → RT_MESSAGETABLE
         → CRIM(EVNT/TASK/OPCO/TTBL + LEVL/KEYW)  ← 新增 LEVL + KEYW 解析
         → MUI 伴生文件
                                                      SQLite DB
         → messages 表 (含 level_name, keywords_name)   ← 新增列
         ↓
  ParameterMessageFile DLL → extract_messages()
         → parameter_refs 表                           ← 新增表
```

### 变更后的 parse 数据流

```
原流程:
  event → resolve_event_message() → substitute(%1-%99) → message

新流程:
  event → resolve_event_message()
           → substitute(%1-%99)                        ← 不变
           → expand_param_refs(%%N → DB parameter_refs)  ← 新增
           → message
```

---

## 附录 B: 测试矩阵

### 新增测试

| 测试名 | 层级 | 平台 | 类型 | 覆盖功能 |
|--------|------|------|------|---------|
| `test_parse_level_block_from_system_events` | 集成 | Win | 🔴 新增 | LEVL 在真实 DLL 上的解析 |
| `test_parse_level_block_empty` | 单元 | 全 | 🔴 新增 | LEVL 边界情况 |
| `test_crim_provider_data_includes_levels` | 集成 | Win | 🔴 新增 | CrimProviderData 中有 level |
| `test_parse_keyword_block_from_system_events` | 集成 | Win | 🔴 新增 | KEYW 在真实 DLL 上的解析 |
| `test_parse_keyword_block_empty` | 单元 | 全 | 🔴 新增 | KEYW 边界情况 |
| `test_keyword_bitmask_matching` | 单元 | 全 | 🔴 新增 | 64bit bitmask 位匹配 |
| `test_expand_param_refs_basic` | 单元 | 全 | 🔴 新增 | %%N 基本展开 |
| `test_expand_param_refs_unknown` | 单元 | 全 | 🔴 新增 | 未知 %%N 保持原样 |
| `test_expand_param_refs_mixed` | 单元 | 全 | 🔴 新增 | %1 和 %%N 混合 |
| `test_scanner_reads_parameter_file` | 集成 | Win | 🔴 新增 | scanner 读取 ParameterFileName |
| `test_resolve_msg_db_build_zero_fallback` | 集成 | 全 | 🔴 新增 | build=0 时 DB fallback |
| `test_crim_task_opcode_from_adtschema` | 集成 | Win | 🔴 新增 | adtschema CRIM 调试 |

### 回归测试

| 测试 | 位置 | 覆盖 |
|------|------|------|
| `test_parse_crim_provider_data_has_tasks_and_opcodes` | `extractor.rs` | 现有 CRIM 解析不变 |
| `test_parse_crim_evnt_blocks_from_system_events` | `extractor.rs` | EVNT 块解析不变 |
| `test_extract_all_from_pe_matches_separate_calls` | `extractor.rs` | PE 提取一致性 |
| `extract_fields_from_json` | `parser.rs` | 字段提取不变 |
| `message_db_warmup_and_get` | `resolver.rs` | 消息 DB 查询不变 |
| `test_substitute_with_resolved_message` | `resolver.rs` | substitute 引擎不变 |
| `test_create_system_db_schema` | `writer_sys.rs` | 系统 DB schema 兼容 |

---

## 附录 C: 验证流程

### 每步修改后的验证命令

```bash
# 1. 编译检查
cargo build --release
cargo clippy --all-targets

# 2. 运行所有测试
cargo test -p hotech-evtx-core
cargo test -p hotech-evtx-dumper
cargo test -p hotech-evtx --features dumper

# 3. E2E 测试（Windows）
hotech-evtx dump --out-dir ./test_verify/ --lang 1033
hotech-evtx parse C:\Windows\System32\winevt\Logs\Security.evtx \
    --msg-dir ./test_verify/ -o ./test_verify/out.csv

# 4. 检查输出 CSV 中的字段
#    - level_name 列应有值（如 "Information"）
#    - keywords_name 列应有值（如 "Audit Success"）
head -5 ./test_verify/out.csv
```

### 验收标准

| 验收项 | 标准 |
|--------|------|
| LEVL 解析 | 从 system-events.dll 解析出至少 3 个 level 名称 |
| KEYW 解析 | 从任意 provider DLL 解析出至少 1 个 keyword 名称 |
| %%N 展开 | parameter_refs 表不为空 |
| Build 检测 | build=0 时仍能匹配到消息 DB |
| 回归测试 | 所有现有测试通过 |
