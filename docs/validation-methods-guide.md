
# EVTX 解析工具验证方法指南

## 1. value_maps 数据库验证方法

### 1.1 如何检查 value_maps 表是否有数据？

#### **SQL 查询方法**
```sql
-- 检查表是否存在
SELECT name FROM sqlite_master 
WHERE type='table' AND name='value_maps';

-- 检查表中是否有数据
SELECT COUNT(*) as total_entries FROM value_maps;

-- 查看前10条数据
SELECT * FROM value_maps LIMIT 10;

-- 按 map_name 分组统计
SELECT map_name, COUNT(*) as count 
FROM value_maps 
GROUP BY map_name 
ORDER BY count DESC;
```

#### **代码验证方法**
```rust
// 在 resolver.rs 中添加调试输出
pub fn debug_value_maps(&self) {
    println!("=== Value Maps Debug ===");
    println!("Total maps: {}", self.value_maps.len());
    for (map_name, entries) in &self.value_maps {
        println!("Map: {} -> {} entries", map_name, entries.len());
        for (value, display) in entries.iter().take(3) {
            println!("  {} -> {}", value, display);
        }
    }
}
```

### 1.2 value_maps 表的结构是什么？

#### **完整表结构**
```sql
CREATE TABLE value_maps(
    provider_name TEXT NOT NULL,      -- 提供程序名称
    map_name TEXT NOT NULL,           -- 映射名称 (如 LogonType, AccessMask)
    entry_value TEXT NOT NULL,        -- 原始值 (如 "2", "0x3")
    display_name TEXT NOT NULL,       -- 显示名称 (如 "Interactive", "Read")
    is_bitmap INTEGER NOT NULL DEFAULT 0,  -- 是否为位图映射
    PRIMARY KEY (provider_name, map_name, entry_value)
);
```

#### **字段说明**
1. **provider_name**: 提供程序名称，如 "Microsoft-Windows-Security-Auditing"
2. **map_name**: 映射名称，对应 EventData 中的字段名
3. **entry_value**: 原始数值字符串
4. **display_name**: 显示的可读字符串
5. **is_bitmap**: 位图标志 (0=普通映射, 1=位图映射)

### 1.3 如何验证 VMAP/BMAP 数据是否正确加载？

#### **步骤 1: 检查数据库内容**
```sql
-- 检查特定 provider 的映射
SELECT * FROM value_maps 
WHERE provider_name LIKE '%Kernel-Power%';

-- 检查特定 map_name 的映射
SELECT * FROM value_maps 
WHERE map_name = 'Reason';

-- 检查位图映射
SELECT * FROM value_maps 
WHERE is_bitmap = 1;
```

#### **步骤 2: 验证加载逻辑**
```rust
// 在 resolver.rs 中添加验证函数
pub fn verify_value_maps_loading() -> Result<()> {
    let mut db = MessageDb::open("test.db")?;
    
    // 尝试加载 value_maps
    db.load_value_maps()?;
    
    // 验证加载结果
    println!("Value maps loaded: {}", db.value_maps.len());
    println!("Bitmap names: {}", db.bitmap_names.len());
    
    // 测试特定映射
    if let Some(map) = db.value_maps.get("LogonType") {
        println!("LogonType mappings:");
        for (value, display) in map {
            println!("  {} -> {}", value, display);
        }
    }
    
    Ok(())
}
```

#### **步骤 3: 测试映射应用**
```rust
// 测试 apply_value_maps 函数
fn test_value_mapping() {
    let params = vec!["2".to_string(), "12".to_string()];
    let param_map = Some(HashMap::from([
        ("1".to_string(), "LogonType".to_string()),
        ("2".to_string(), "Reason".to_string()),
    ]));
    
    let value_maps = HashMap::from([
        ("LogonType".to_string(), HashMap::from([
            ("2".to_string(), "Interactive".to_string()),
        ])),
        ("Reason".to_string(), HashMap::from([
            ("12".to_string(), "SessionUnlock".to_string()),
        ])),
    ]);
    
    let result = apply_value_maps(&params, &param_map, &value_maps, &HashSet::new());
    assert_eq!(result, vec!["Interactive", "SessionUnlock"]);
}
```

---

## 2. 模板匹配验证方法

### 2.1 如何验证 EventID 24/6013 的模板匹配？

#### **步骤 1: 检查消息数据库**
```sql
-- 检查 EventID 24 的模板
SELECT provider_name, event_id, message, lang 
FROM messages 
WHERE event_id = 24 
AND provider_name LIKE '%Kernel-General%';

-- 检查 EventID 6013 的模板
SELECT provider_name, event_id, message, lang 
FROM messages 
WHERE event_id = 6013 
AND provider_name LIKE '%EventLog%';

-- 检查所有可用的语言
SELECT DISTINCT lang FROM messages;
```

#### **步骤 2: 验证模板匹配逻辑**
```rust
// 在 resolver.rs 中添加调试函数
pub fn debug_template_matching(&self, provider: &str, event_id: u32) {
    println!("=== Template Matching Debug ===");
    println!("Provider: {}", provider);
    println!("EventID: {}", event_id);
    
    // 尝试精确匹配
    let key = (provider.to_string(), event_id);
    if let Some(row) = self.cache.get(&key) {
        println!("✅ Exact match found:");
        println!("  Message: {}", row.message);
    } else {
        println!("❌ No exact match found");
    }
    
    // 尝试标准化匹配
    let normalized = provider.strip_prefix("Microsoft-Windows-").unwrap_or(provider);
    let normalized_key = (normalized.to_string(), event_id);
    if let Some(row) = self.cache.get(&normalized_key) {
        println!("✅ Normalized match found:");
        println!("  Message: {}", row.message);
    } else {
        println!("❌ No normalized match found");
    }
}
```

#### **步骤 3: 对比预期输出**
```rust
// 创建测试用例
fn test_eventid_24_template() {
    let expected = "The time zone information was refreshed with exit reason 0. Current time zone bias is -480.";
    let actual = get_message_for_event("Microsoft-Windows-Kernel-General", 24);
    
    assert_eq!(actual, expected, "EventID 24 template mismatch");
}

fn test_eventid_6013_template() {
    let expected = "The system uptime is 354558 seconds.";
    let actual = get_message_for_event("EventLog", 6013);
    
    assert_eq!(actual, expected, "EventID 6013 template mismatch");
}
```

### 2.2 消息数据库的模板数据结构是什么？

#### **messages 表结构**
```sql
CREATE TABLE messages (
    provider_name TEXT NOT NULL,      -- 提供程序名称
    lang          TEXT NOT NULL DEFAULT 'en-US',  -- 语言代码
    event_id      INTEGER NOT NULL,   -- 事件ID
    message       TEXT NOT NULL,       -- 消息模板
    level         INTEGER,             -- 级别
    task          TEXT,                -- 任务
    opcode        TEXT,                -- 操作码
    keywords      TEXT,                -- 关键字
    parameters    TEXT,                -- 参数映射 (JSON)
    PRIMARY KEY (provider_name, lang, event_id)
);
```

#### **参数映射示例**
```json
{
  "1": "TargetUserName",
  "2": "TargetDomainName", 
  "3": "TargetLogonId",
  "4": "LogonType"
}
```

---

## 3. 参数替换验证方法

### 3.1 如何验证参数替换是否正确？

#### **步骤 1: 检查参数提取**
```rust
// 在 parser.rs 中添加调试输出
pub fn debug_parameter_extraction(event_data: &JsonValue) {
    println!("=== Parameter Extraction Debug ===");
    if let Some(params) = event_data.get("EventData") {
        if let Some(data) = params.get("Data") {
            match data {
                JsonValue::Array(arr) => {
                    println!("Array format: {} parameters", arr.len());
                    for (i, param) in arr.iter().enumerate() {
                        println!("  %{}: {:?}", i + 1, param);
                    }
                }
                JsonValue::Object(obj) => {
                    println!("Object format:");
                    for (key, value) in obj {
                        println!("  {}: {:?}", key, value);
                    }
                }
                _ => println!("Unknown format"),
            }
        }
    }
}
```

#### **步骤 2: 验证替换逻辑**
```rust
// 测试 substitute 函数
fn test_parameter_substitution() {
    let template = "User %1 logged on from %2";
    let params = vec!["admin", "192.168.1.1"];
    
    let result = substitute(template, &params);
    assert_eq!(result, "User admin logged on from 192.168.1.1");
}
```

#### **步骤 3: 测试边界情况**
```rust
// 测试边界情况
fn test_edge_cases() {
    // 测试缺失参数
    let template = "User %1 %2 %3";
    let params = vec!["admin"];
    let result = substitute(template, &params);
    assert_eq!(result, "User admin %2 %3");
    
    // 测试空参数
    let result = substitute(template, &[]);
    assert_eq!(result, "User %1 %2 %3");
    
    // 测试特殊字符
    let template = "Path: C:\%1\%2";
    let params = vec!["Windows", "System32"];
    let result = substitute(template, &params);
    assert_eq!(result, "Path: C:\Windows\System32");
}
```

### 3.2 EventID 8233/8230 的参数格式是什么？

#### **EventID 8233 (Security-SPP)**
```xml
<EventData>
  <Data Name="BuildVersion">10.0.19041.1</Data>
  <Data Name="ProductVersion">10.0.19041.1</Data>
  <Data Name="EditionID">Professional</Data>
  <Data Name="ProductName">Windows 10 Pro</Data>
  <Data Name="ChannelID">Security-SPP</Data>
</EventData>
```

#### **EventID 8230 (Security-SPP)**
```xml
<EventData>
  <Data Name="BuildVersion">10.0.19041.1</Data>
  <Data Name="ProductVersion">10.0.19041.1</Data>
  <Data Name="PolicyName">Security-SPP-Action-StateData</Data>
  <Data Name="ActionName">{55c92734-d682-4d71-983e-d6ec3f16059f}</Data>
</EventData>
```

#### **验证方法**
```rust
// 验证 GUID 格式化
fn test_guid_formatting() {
    let guid = "{55c92734-d682-4d71-983e-d6ec3f16059f}";
    let formatted = format_for_display(guid);
    
    // 检查格式是否正确
    assert!(formatted.starts_with("{"));
    assert!(formatted.ends_with("}"));
    assert!(formatted.contains("-"));
}
```

---

## 4. 完整验证流程

### 4.1 验证环境准备
```bash
# 1. 编译项目
cargo build --release

# 2. 准备测试数据
cp test-logs/System.evtx test.evtx

# 3. 运行解析
./target/release/evtx-tools parse test.evtx   --msg-db messages.db   --format sqlite   --output test_output.db

# 4. 检查输出
sqlite3 test_output.db "SELECT * FROM events LIMIT 10;"
```

### 4.2 验证检查清单
- [ ] value_maps 表有数据
- [ ] 模板匹配正确
- [ ] 参数替换正确
- [ ] 值映射应用正确
- [ ] 输出格式正确
- [ ] 性能无显著下降

### 4.3 问题诊断流程
```
1. 检查数据库内容
   ↓
2. 验证加载逻辑
   ↓
3. 测试替换函数
   ↓
4. 对比预期输出
   ↓
5. 定位问题代码
   ↓
6. 修复并验证
```

---

## 5. 调试工具和技巧

### 5.1 SQLite 调试命令
```sql
-- 查看表结构
.schema value_maps

-- 查看索引
.indexes value_maps

-- 查看数据统计
SELECT COUNT(*) FROM value_maps;
SELECT DISTINCT map_name FROM value_maps;

-- 查看特定映射
SELECT * FROM value_maps 
WHERE map_name = 'LogonType' 
AND provider_name LIKE '%Security%';
```

### 5.2 Rust 调试技巧
```rust
// 使用 println! 调试
println!("Debug: {:?}", value);

// 使用 eprintln! 输出到 stderr
eprintln!("Warning: {}", message);

// 使用 RUST_LOG 环境变量
RUST_LOG=debug ./evtx-tools parse ...
```

### 5.3 性能分析
```bash
# 使用 perf 工具
perf record -g ./evtx-tools parse test.evtx
perf report

# 使用 cargo-flamegraph
cargo flamegraph -- parse test.evtx
```

---

**文档版本**: v1.0
**生成时间**: 2026-08-27T03:02:10.701Z
**作者**: researcher
**团队**: evtx-bugfix-team
