# hotech-evtx 架构文档

> 组件架构、模块职责、依赖关系、设计规则

---

## 1. 总体架构

```
┌─────────────────────────────────────────────────────────────┐
│                          CLI 层 (bin/)                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐             │
│  │cmd_parse │  │cmd_merge │  │ cmd_info     │             │
│  │ (全平台)  │  │ (全平台)  │  │ (全平台)      │             │
│  └────┬─────┘  └────┬─────┘  └──────┬───────┘             │
│       │             │                │                     │
├───────┼─────────────┼────────────────┼─────────────────────┤
│       ▼             ▼                ▼                     │
│                  Application 层 (src/)                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐             │
│  │  parser  │  │ resolver │  │  storage     │             │
│  │ (.evtx)  │  │ (消息查询)│  │ (输出)       │             │
│  └──────────┘  └──────────┘  └──────────────┘             │
│       │             │                                     │
│       ▼             ▼                                     │
│                   Domain 层 (src/event.rs)                │
│  ┌─────────────────────────────────────────────┐          │
│  │ ParsedEvent / MessageRow / Severity 等纯数据结构 │       │
│  │ 零外部依赖，纯 struct + 纯函数                   │       │
│  └─────────────────────────────────────────────┘          │
└───────────────────────────────────────────────────────────┘
```

### 依赖方向

```
CLI (bin/) → Application (src/) → Domain (src/event.rs)
  ↑              ↑                   ↑
  调命令实现      调用核心逻辑          纯数据模型
```

**核心规则：**
- Domain 层零外部依赖（只有 Rust 标准库）
- Application 层有 IO（rusqlite, evtx crate, csv crate）
- CLI 层只做参数解析 + 调用 Application 层

---

## 2. 组件职责

### 2.1 Core（src/ 下）

| 模块 | 文件 | 职责 | 外部依赖 |
|------|------|------|----------|
| event | `event.rs` | ParsedEvent 结构体，各种枚举，seveirty 映射表 | 无 |
| parser | `parser.rs` | EvtxParser 封装（records_json_value → 字段提取），build_number 检测 | evtx |
| substitute | `substitute.rs` | %1 %2 %n %t 替换引擎 | regex |
| resolver | `resolver.rs` | MessageDb 打开/预热/查询，SQLite 到 HashMap | rusqlite |
| storage | `storage/mod.rs` | 输出抽象 | — |
| storage/sqlite | `storage/sqlite.rs` | parse_to_sqlite()，建表，批量写入 | rusqlite |
| storage/csv | `storage/csv.rs` | parse_to_csv()，列顺序定义 | csv |

### 2.2 CLI（src/bin/ 下）

> 消息提取（dump）功能已用 Go 重写，Rust 端只负责 EVTX 解析。
> 消息库生成工具：[msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)（二进制 `msgdb-dump` / `msgdb-dump.exe`）

| 命令 | 文件 | 实现方式 |
|------|------|----------|
| parse | `cmd_parse.rs` | 调用 parser + resolver + storage |
| merge | `cmd_merge.rs` | 读多个 DB → 合并写入 |
| info | `cmd_info.rs` | 读 DB → 统计输出 |

---

## 3. 核心数据模型

### ParsedEvent（输出的 22 列）

```rust
pub struct ParsedEvent {
    pub event_record_id: Option<u64>,
    pub timestamp: Option<String>,           // ISO8601
    pub provider_name: Option<String>,
    pub provider_guid: Option<String>,
    pub event_id: Option<u32>,
    pub qualifiers: Option<u16>,             // 经典 EventLog Provider 的 Qualifiers 属性
    pub level: Option<u8>,
    pub severity: Option<String>,            // 从 level 派生（中文）
    pub task_category: Option<String>,
    pub opcode: Option<String>,
    pub keywords: Option<String>,
    pub channel: Option<String>,
    pub computer: Option<String>,
    pub process_id: Option<u32>,
    pub thread_id: Option<u32>,
    pub user_name: Option<String>,           // 从 EventData.SubjectUserName / TargetUserName 提取
    pub user_sid: Option<String>,            // 从 EventData.SubjectUserSid / TargetUserSid 提取
    pub activity_id: Option<String>,
    pub related_activity_id: Option<String>,
    pub message: Option<String>,             // substitute 后完整描述
    pub event_data: Option<String>,          // JSON string（evtx 0.12 平铺 Key-Value 格式）
    pub raw_xml: Option<String>,             // 可选，默认 None
    pub file_name: Option<String>,
}
```

> **注意：** `user_name` 和 `user_sid` 不再硬编码为 None。evtx 0.12 将 `System.Security` 置为 null，这些字段改从 `EventData` 的 `SubjectUserName` / `SubjectUserSid`（回退 `TargetUserName` / `TargetUserSid`）提取。

### MessageRow（缓存的消息行）

```rust
pub struct MessageRow {
    pub template: String,           // "%1 已成功登录..."
    pub task: Option<String>,
    pub opcode: Option<String>,
    pub keywords: Option<String>,
    pub parameters: Option<HashMap<String, String>>,  // {"1": "SubjectUserSid", "2": "SubjectUserName", ...}
}
```

### MessageDb（预热后的消息库）

```rust
pub struct MessageDb {
    conn: rusqlite::Connection,                    // 到 .db 的连接
    cache: HashMap<(String, u32), MessageRow>,     // Key = (provider_name, event_id)
    // lazy warmup: 解析过程首次遇到新 (provider, event_id) 时按需加载
}
```

---

## 4. 设计决策

### 4.1 为什么用 SQLite 做消息存储，不用 JSON/自定义二进制

| 方案 | 加载速度 | 可调试 | 依赖 |
|------|---------|--------|------|
| **SQLite ✅** | 快（索引+缓存） | ✅ sqlite3 直接查 | 已有 |
| JSON | 慢（全量解析） | ✅ 可读 | 已有 |
| 自定义二进制 | 最快 | ❌ 不可读 | 需要新解析器 |

SQLite 胜出：已有 rusqlite 依赖，SQLite 工具链成熟，查询灵活。

### 4.2 为什么消息预热到 HashMap，不每次都查 SQLite

每次查 SQLite → 磁盘 IO/页缓存 → ~0.1ms/次
HashMap 查 → O(1), ~0.0001ms/次

对于 100 万事件的 Security.evtx，只涉及 1-3 个 provider：
- SQLite 每次查询：100 万次 × 0.1ms = 100 秒 ❌
- HashMap 预热：1 次 SQL 读全部消息 → 3ms，后面 100 万次 0.01ms ✅

### 4.3 为什么分系统汇总和 DLL 两级文件

| 层面 | 用途 | 更新频率 | 谁维护 |
|------|------|---------|--------|
| 系统汇总 | parse 输入、用户下载 | 随 Windows 更新 | 自动 CI |
| DLL 级别 | 社区贡献、版本管理、差异比较 | 按需 | 社区 + CI |

### 4.4 为什么 parse 不直接读 dlldb/

- 系统汇总文件是"即开即用"的——下载一个文件就够
- dlldb/ 是多版本集合，parse 要匹配 build_number，逻辑在 resolver 层统一处理
- 系统汇总 = 磨好的面粉，dlldb/ = 麦子

### 4.5 为什么跳过 JSON 分两步

`template_instances()` 从 BinXML 直接取 typed values 需要正确的模板字段映射，
元数据表结构在 Windows 版本间有差异，直接解析代码较脆弱。

两步走：
- v1.0 走 `into_json_value()`（可靠，经过 evtx crate 验证）
- v1.1+ 等 `wevt_templates` 模块成熟后再走完全跳过 JSON 的路径

---

## 5. 约束检查

| # | 约束 | 检测方式 |
|---|------|----------|
| C1 | parse 全平台编译 | CI 在 Linux/macOS/Windows 上各跑一次 |
| C2 | 无 FormatMessageW 调用 | grep 代码中无 `FormatMessage` / `windows-sys` 引用 |
| C3 | 消息格式化不调 Windows API | 所有 %1 %2 替换是纯字符串操作 |

---

## 6. 性能模型

### 解析路径优化

| 版本 | 路径 | 特点 |
|------|------|------|
| v0.9 | `records_json_bytes()` → `serde_json::from_slice()` → 字段提取 | JSON 序列化+反序列化两次往返 ❌ |
| **v1.0** ✅ | **`records_json_value()` → 字段提取** | **直接得 Value，跳过序列化 ✅** |
| v1.1+ | `template_instances()` → 字段提取 | 完全跳过 JSON，v1.1+ 目标 |

### 解析流程（单遍懒加载）

旧两遍扫描（v0.9）：
```
第一遍：遍历全部 2.5M 事件 → 收集 (provider, event_id) 对
第二遍：重新遍历 → 字段提取 → 消息替换 → 写入
```

**新单遍懒加载（v1.0）✅：**
```
一次性遍历：
  字段提取（records_json_value）
  → 首次遇到新 (provider, event_id)？按需预热消息库
  → 消息替换（substitute）
  → 写入 batch
  → batch 满 → flush
```

省去一整个文件遍历，对 2.5M 事件文件约 **2× 吞吐量提升**。

### 消息库预热性能

| 文件大小 | provider 数 | 消息数 | 批量预热 | 按需预热（单 provider） |
|----------|------------|--------|----------|------------------------|
| 2 MB | 150 | 20K | ~30ms | ~0.05ms |
| 10 MB | 500 | 100K | ~150ms | ~0.05ms |
| 全量合并版 | 500+ | 200K+ | ~300ms | ~0.05ms |

> 按需预热在单遍解析中零开销：首次遇到新 pair 时触发一次 SQL 查询，后续同 pair 直接走 HashMap O(1) 查找。

### EventData 格式变更（evtx 0.12）

evtx 0.12 改变了 EventData 的 JSON 输出格式：

```
旧 (evtx 0.8):  {"EventData": {"Data": [{"#attributes":{"Name":"..."},"$":"..."}]}}
新 (evtx 0.12): {"EventData": {"SubjectUserSid":"S-1-5-...","SubjectUserName":"SYSTEM",...}}
```

解析器同时兼容两种格式。新格式下：
- `event_data` = 整个 EventData 对象的 JSON 字符串
- `user_name` = `SubjectUserName` / `TargetUserName`
- `user_sid` = `SubjectUserSid` / `TargetUserSid`
- `System.Security` 在新格式中为 null（UserID 移至 EventData）

### 消息替换策略（4 种策略依次尝试）

| # | 策略 | 条件 | 适用场景 |
|---|------|------|----------|
| 1 | 平铺对象 + 命名参数映射 | DB 有 `parameters` 列 + EventData 是 object | WAS/WELM 数据完整时 |
| 2 | 平铺对象 + 位置回退 | EventData 是 object，无需 param_map | Security-Auditing 等无参数映射的 provider |
| 3 | Data 数组 + 命名参数映射 | DB 有 `parameters` + EventData 是 array | evtx 0.8 兼容 |
| 4 | `#text` 数组位置映射 | EventData 含 `#text` 数组 | 经典 EventLog provider |
