# hotech-evtx

> 高性能跨平台 EVTX 事件解析工具链
>
> 将 Windows EVTX 日志文件解析为 SQLite / CSV，不依赖 Windows API（无需真实机器上的 DLL）
>
> **核心能力：** 在 Linux/macOS 上也能拿到和 Windows 事件查看器一样的人类可读事件描述。
> 中英双语消息，无需目标机器的系统 DLL。
>
> **消息提取（dump）已用 Go 重写**，Rust 端只负责 EVTX 解析。
> 消息库生成工具：[msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)（二进制：`msgdb-dump` / `msgdb-dump.exe`）

---

项目完整计划见 `docs/PLAN.md`，消息库架构见 `data/schemas/messages.md`。

Windows EVTX 文件只存储事件 ID（数字）和 EventData（结构化参数），**不存储**人类可读的事件描述文本。
描述文本藏在系统 DLL 的 `RT_MESSAGETABLE` 资源里，通过 `FormatMessageW` API 获取。

这意味着：
- **跨平台分析（Linux/macOS）：** 无法调 Windows API，拿不到描述
- **离线取证：** 目标机器的 DLL 不在分析机上
- **中文环境：** 绝大多数取证工具只支持英文描述

**hotech-evtx 的解法：**

```
[msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)（Windows 上跑一次）
  └── 提取所有 provider 的 DLL/MUI → messages.db（SQLite）
       └── 可带走到任意平台

parse（全平台跑任意次）
  └── 读 messages.db + .evtx → substitute(%1, %2) → events.db / .csv
       └── 不调 Windows API，纯字符串替换
```

---

## 安装

### 从源码编译

```bash
cd crates/hotech-evtx

cargo build --release -p hotech-evtx
./target/release/hotech-evtx --help
```

> 需要 Rust 1.75+。编译约需 60s（release 模式）。

---

## 消息库工具：msgdb-dump

消息提取（dump）功能由独立的 Go 工具 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 提供。

| 项目 | 说明 |
|------|------|
| 仓库 | [github.com/kitsch-9527/msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) |
| 二进制名 | `msgdb-dump`（Linux/macOS）/ `msgdb-dump.exe`（Windows） |
| 语言 | Go |
| 功能 | 注册表扫描 → PE 解析 → RT_MESSAGETABLE 提取 → SQLite 消息库 |

**安装：**

```bash
# 方式一：go install
go install github.com/kitsch-9527/msgdb-dump@latest

# 方式二：从 Releases 下载预编译二进制
# https://github.com/kitsch-9527/msgdb-dump/releases

# 方式三：从源码编译
git clone https://github.com/kitsch-9527/msgdb-dump
cd msgdb-dump && go build -o msgdb-dump .
```

**使用：**

```bash
# 提取本机所有 provider 的消息库
msgdb-dump dump --out-dir ./msg_dbs/ --lang 1033,2052

# 只提取特定 provider
msgdb-dump dump --out-dir ./contrib/ --providers "Sysmon"
```

生成的 `.db` 文件可直接被 `hotech-evtx parse` 使用。

### 从 GUI 项目引入

```toml
[dependencies]
hotech-evtx-core = { path = "crates/hotech-evtx/hotech-evtx-core" }
```

---

## CLI 使用说明

### 全局选项

```
hotech-evtx <COMMAND> [OPTIONS]

COMMANDS:
  parse    解析 EVTX 文件（全平台）
  merge    合并多个消息数据库
  db-info  查看消息数据库统计信息
  help     显示帮助信息
```

> **消息提取（dump）功能已用 Go 重写，不再属于 Rust CLI。** 请使用 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 工具生成消息库。

---

### parse 命令

解析 EVTX 文件，输出结构化事件。**全平台支持。**

自动从 .evtx 文件头读取构建版本号，匹配对应 `hotech-msg-{product}-{displayver}-{build}.db`。

```bash
hotech-evtx parse [OPTIONS] <INPUT> [--msg-dir <DIR> | --db <PATH>] -o <OUTPUT>
```

| 选项 | 默认值 | 说明 |
|------|--------|------|
| `<INPUT>` | — | **必填。** .evtx 文件路径或包含 .evtx 的目录路径 |
| `--msg-dir <DIR>` | — | 消息库目录。自动匹配 `hotech-msg-{product}-{displayver}-{build}.db`（推荐方式） |
| `--db <PATH>` | — | 手动指定消息库文件（跳过自动匹配） |
| `-o, --output <PATH>` | — | **必填。** 输出文件路径（不含扩展名） |
| `--format <FORMAT>` | `db` | 输出格式：`db`、`csv`、`both` |
| `--with-xml` | 不输出 | 开启 `raw_xml` 字段（渲染开销较大，默认 NULL） |
| `--threads <N>` | CPU 核数 | 解析线程数，0 = 自动 |

**自动匹配规则（`--msg-dir`）：**

```
1. hotech-msg-{product}-{displayver}-{evtx_build}.db  → 精确匹配（最优）
2. hotech-msg-all.db           → 合并库
3. hotech-msg-{product}-{displayver}-{向下最近}.db     → 最近低版本（如 build=19045 时匹配 17763）
4. 都没有或没传                  → 降级（message 留空）
```

**降级行为（无消息库时）：**

| 字段 | 有消息库 | 无消息库 |
|------|---------|---------|
| `message` | substitute 后完整描述（4 级回退策略） | NULL |
| `task_category` | 中文/英文任务名 | NULL |
| `opcode` | 中文/英文 Opcode | NULL |
| `keywords` | 中文/英文 Keywords | NULL |
| `event_data` | 完整 EventData JSON（evtx 0.12 平铺 Key-Value） | 同上 |
| `user_name` | EventData.SubjectUserName 提取 | 同上 |
| `user_sid` | EventData.SubjectUserSid 提取 | 同上 |
| 其余字段 | 正常填充 | 正常填充 |

> **消息替换策略（按优先级）：**
> 1. 平铺对象 + 命名参数映射（DB 有 parameters 列时）
> 2. 平铺对象 + **位置回退**（DB 无参数映射时，按 JSON key 顺序对应 %1-%N）
> 3. Data 数组 + 命名参数映射（evtx 0.8 兼容）
> 4. `#text` 数组位置映射（经典 provider）

**示例：**

```bash
# 自动匹配版本
hotech-evtx parse Security.evtx --msg-dir ./msg_dbs/ -o output

# 手动指定消息库
hotech-evtx parse Security.evtx --db hotech-msg-win11-24h2-26100.db -o output

# 解析目录下所有 .evtx，输出 CSV
hotech-evtx parse ./evtx_samples/ --msg-dir ./msg_dbs/ -o timeline --format csv

# 无消息库时降级（快速预览）
hotech-evtx parse System.evtx -o quick_look --format csv

# 输出完整 XML
hotech-evtx parse Security.evtx --msg-dir ./msg_dbs/ -o output --with-xml
```

---

### merge 命令

合并多个单版本消息库为 `hotech-msg-all.db`（合并版）。
每个单版本文件单独一个 schema，合并版追加 `build_number` 标记来源。

```bash
hotech-evtx merge <DB1> <DB2> -o <OUTPUT>
hotech-evtx merge --dir <MSG_DIR> -o <OUTPUT>
```

| 选项 | 说明 |
|------|------|
| `-o, --output <PATH>` | **必填。** 输出路径，建议命名为 `hotech-msg-all.db` |
| `--dir <DIR>` | 合并目录下所有 `hotech-msg-*.db` |

**示例：**

```bash
# 合并两个单版本文件
hotech-evtx merge hotech-msg-win11-24h2-26100.db hotech-msg-win10-22h2-19045.db -o hotech-msg-all.db

# 合并目录下所有版本
hotech-evtx merge --dir ./msg_dbs/ -o hotech-msg-all.db
```

### db-info 命令

查看消息数据库的统计信息和内容。

```bash
hotech-evtx db-info <DB> [OPTIONS]
```

| 选项 | 说明 |
|------|------|
| `--provider <NAME>` | 查看特定 provider 的详细信息 |
| `--verbose` | 详细输出 |

**示例：**

```bash
# 查看全局统计
hotech-evtx db-info msgs.db
# Providers: 152 | Messages: 20,680
# Builds: 26220, 19045, 17763
# Languages: 1033(en), 2052(zh-CN)

# 查看特定 provider 的消息覆盖
hotech-evtx db-info msgs.db --provider "Microsoft-Windows-Security-Auditing"
# 4624: source=dump build=26220 priority=50
# 4624: source=welm build=17763 priority=10
# 4625: source=dump build=26220 priority=50
```

### 使用案例

#### 案例 1：离线取证（标准工作流）

```bash
# 1. 在目标机器上使用 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 提取消息数据库
msgdb-dump dump --out-dir ./case001/msgs/ --lang 2052,1033

#   case001/msgs/
#     hotech-msg-win11-24h2-26100.db       ← 本机版本
#     hotech-msg-all.db         ← 合并后的全量库

# 2. 把 msgs/ 目录 + .evtx 文件拷贝到分析机（Linux/Mac/Windows）

# 3. 在分析机上解析（自动匹配版本）
hotech-evtx parse case001/evtx/Security.evtx --msg-dir case001/msgs/ -o case001/security
hotech-evtx parse case001/evtx/System.evtx   --msg-dir case001/msgs/ -o case001/system
hotech-evtx parse case001/evtx/ --msg-dir case001/msgs/ -o case001/all_events --format csv

# 4. 直接 SQL 查询
sqlite3 case001/security.db "
  SELECT timestamp, severity, message FROM events
  WHERE event_id = 4625;
"
```

#### 案例 2：社区贡献新 DLL 消息

```bash
# 贡献者使用 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 提取消息
msgdb-dump dump --out-dir ./contrib/ --providers "Sysmon"

# 提交到开源仓库
cp contrib/dlldb/sysmon.en-US.15.3.0.0.db data/dlldb/
git add data/dlldb/sysmon.en-US.15.3.0.0.db
git rm data/dlldb/sysmon.en-US.15.2.0.0.db  # 旧版本
git commit -m "feat(msgdb): add Sysmon 15.3.0.0 messages"
git push

# → CI 自动检测 dlldb/ 变更
# → 重建 hotech-msg-all.db
# → 创建新 Release
```

#### 案例 3：多机器合并分析

```bash
# 分析师 A 从 Win11 机器使用 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 提取
msgdb-dump dump --out-dir ./msg_a/ --lang 1033

# 分析师 B 从 Win10 机器提取
msgdb-dump dump --out-dir ./msg_b/ --lang 2052,1033

# 合并两个消息库
hotech-evtx merge msg_a/hotech-msg-win11-24h2-26100.db msg_b/hotech-msg-win10-22h2-19045.db -o hotech-msg-all.db

# 解析时使用合并库
hotech-evtx parse evidence.evtx --db hotech-msg-all.db -o result
```

#### 案例 2：临时分析（无消息库）

```bash
# 只在分析机上跑，不依赖任何外部数据
hotech-evtx parse Security.evtx -o quick_look --format csv
# message/task/opcode/keywords 为空，其余字段正常
```

#### 案例 3：GUI 项目集成

```rust
use hotech_evtx_core::{parse_to_sqlite, get_message_db, MessageDb};

// 打开消息库
let msg_db = MessageDb::open("messages.db")?;

// 批量解析 EVTX 文件
parse_to_sqlite(
    "Security.evtx",
    Some(&msg_db),
    "output.db",
)?;
```

---

## 架构

```
┌─────────────────────────────────────────────────────────────┐
│  hotech-evtx CLI (crates/hotech-evtx/hotech-evtx/src/)      │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐                │
│  │cmd_parse │ │cmd_merge │ │ cmd_info     │                │
│  └────┬─────┘ └────┬─────┘ └──────┬───────┘                │
├───────┼────────────┼───────────────┼────────────────────────┤
│       ▼            ▼               ▼                        │
│  hotech-evtx-core (crates/hotech-evtx/hotech-evtx-core/)    │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐   │
│  │ parser   │ │substitute│ │ resolver │ │ storage      │   │
│  │ (.evtx)  │ │ (%1 %2)  │ │ (消息库)  │ │ (SQLite/CSV) │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘   │
│                                                             │
│  msg.db 由独立的 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 工具生成                          │
└─────────────────────────────────────────────────────────────┘
```

### crate 结构

两个内部 crate 组成 workspace：

```
hotech-evtx/                         # workspace 根
├── Cargo.toml                       # workspace 定义
├── README.md
├──
├── hotech-evtx-core/                # 核心库（全平台）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                   # 公开 API 入口
│       ├── parser.rs                # evtx EvtxParser 封装（records_json_value）+ 字段提取
│       ├── substitute.rs            # %1 %2 替换引擎
│       ├── resolver.rs              # MessageDb 打开/预热/查询
│       └── storage/
│           ├── mod.rs
│           ├── sqlite.rs            # parse_to_sqlite()
│           └── csv.rs               # parse_to_csv()
│
├── hotech-evtx/                     # CLI 二进制
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                  # CLI 入口（clap）
│       ├── cmd_parse.rs             # parse 命令
│       ├── cmd_merge.rs             # merge 命令
│       └── cmd_info.rs              # db-info 命令
│
├── docs/                            # 设计文档
├── data/                            # 数据目录
└── tests/
```

> **注意：** 原 `hotech-evtx-dumper` crate（消息提取）已用 Go 重写，不再属于 Rust workspace。

### 三层架构

hotech-evtx 遵循 DDD 三层 + Unix KISS 原则：

| 层 | Crate | 职责 | 外部依赖 |
|------|-------|------|----------|
| **Domain** | core | `ParsedEvent`、`substitute()` — 纯数据结构 + 纯函数 | 零 |
| **Application** | core | 解析、消息格式化、存储 | evtx, rusqlite, csv |
| **CLI** | hotech-evtx | clap 参数解析 + 调用 Application 层 | clap |

---

## 数据库 Schema

### messages.db 文件架构（两层设计）

> 消息库文件由 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 工具生成。以下仅描述文件格式供 parse 使用。

```
[msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 输出目录结构:
  ./msg_dbs/
  ├── hotech-msg-win11-24h2-26100.db           ← 系统版本汇总（parse 使用）
  └── dlldb/                        ← 按 DLL 拆分（社区贡献/版本管理）
        ├── adtschema.en-US.10.0.26220.1000.db
        ├── adtschema.zh-CN.10.0.26220.1000.db
        ├── sysmon.en-US.15.2.0.0.db
        └── ...
```

**Layer 1: 系统汇总（hotech-msg-{product}-{displayver}-{build}.db）**

一个文件 = 一个 Windows 构建版本的全部消息。parse 命令通过 `--msg-dir` 自动匹配。

**Layer 2: DLL 级别（dlldb/{dll}.{lang}.{ver}.db）**

一个文件 = 一个 DLL × 一个语言 × 一个版本。文件名唯一标识：
```
adtschema.zh-CN.10.0.26220.1000.db
  ↑dll名    ↑语言   ↑版本号
```
- 用于开源项目的数据交换和版本管理
- 社区可以按 DLL 单独贡献
- 不直接用于 parse，是数据源

详见 `data/schemas/messages.md`。

### hotech-msg-{product}-{displayver}-{build}.db Schema（系统汇总）

```sql
-- 文件名: hotech-msg-{product}-{displayver}-{build}.db  例: hotech-msg-win11-24h2-26100.db

CREATE TABLE providers (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE,
    guid    TEXT
);

CREATE TABLE messages (
    provider_id   INTEGER NOT NULL REFERENCES providers(id),
    event_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    level         INTEGER,
    task          TEXT,
    opcode        TEXT,
    keywords      TEXT,
    parameters    TEXT,                   -- JSON: {"%1":"SubjectUserSid",...}
    language_id   INTEGER NOT NULL DEFAULT 1033,
    source        TEXT NOT NULL DEFAULT 'dump',  -- 'welm', 'mui', 'dump'
    extracted_at  TEXT NOT NULL,
    PRIMARY KEY (provider_id, event_id, language_id)
);

CREATE INDEX idx_messages_provider ON messages(provider_id);
CREATE INDEX idx_messages_event ON messages(event_id);

-- 元数据（整个文件只属于一个 build）
CREATE TABLE _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
-- build_number=26220, os_version="Windows 11 Enterprise 24H2", ...

CREATE TABLE _languages (id INTEGER PRIMARY KEY, code TEXT NOT NULL, name TEXT NOT NULL);
```

**设计要点：**

| 特性 | 做法 |
|------|------|
| 每版本独立文件 | `hotech-msg-win11-24h2-26100.db`、`hotech-msg-win10-22h2-19045.db` |
| parse 自动匹配 | 从 .evtx 文件头读 build_number → 匹配文件名 |
| 增量更新 | `msgdb-dump dump --update` 查文件是否存在 |
| 合并为 all.db | `merge` 命令，追加 build_number 标记 |

### events.db（parse 输出，22 列）

```sql
CREATE TABLE events (
    event_record_id     INTEGER NOT NULL,     -- 事件记录 ID
    timestamp           TEXT,                  -- ISO8601
    provider_name       TEXT,                  -- Provider 名（来自 System.Provider.@Name）
    provider_guid       TEXT,                  -- Provider GUID
    event_id            INTEGER,               -- 事件 ID
    qualifiers          INTEGER,               -- 经典 EventLog Provider 限定符
    level               INTEGER,               -- 0-5 原始等级值
    severity            TEXT,                  -- 派生可读名（中文：关键/错误/警告/信息/详细）
    task_category       TEXT,                  -- 优先 messages.task（回退 EVTX 原始数值）
    opcode              TEXT,                  -- 优先 messages.opcode（回退 EVTX 原始数值）
    keywords            TEXT,                  -- 优先 messages.keywords（回退 EVTX 原始值）
    channel             TEXT,                  -- 日志通道（System.Channel）
    computer            TEXT,                  -- 计算机名（System.Computer）
    process_id          INTEGER,               -- 进程 ID（System.Execution.@ProcessID）
    thread_id           INTEGER,               -- 线程 ID（System.Execution.@ThreadID）
    user_name           TEXT,                  -- EventData.SubjectUserName（回退 TargetUserName）
    user_sid            TEXT,                  -- EventData.SubjectUserSid（回退 TargetUserSid）
    activity_id         TEXT,                  -- Correlation.@ActivityID
    related_activity_id TEXT,                  -- Correlation.@RelatedActivityID
    message             TEXT,                  -- substitute 后完整描述（支持 4 级替换回退）
    event_data          TEXT,                  -- EventData 完整 JSON（evtx 0.12 平铺 Key-Value 格式）
    raw_xml             TEXT,                  -- 原始 EVTX XML（可选，默认 NULL，--with-xml）
    file_name           TEXT                   -- 来源 .evtx 文件名
);
```

> **字段来源说明（evtx 0.12）：** `user_name`/`user_sid` 从 EventData 的 SubjectUserName/SubjectUserSid 提取（非 `System.Security.UserID`，该字段在 evtx 0.12 中为 null）。`message` 的模板替换支持 4 级回退策略，确保即使消息库无参数映射时也能正确按位置替换。

CREATE INDEX idx_events_timestamp ON events(timestamp);
CREATE INDEX idx_events_level ON events(level);
CREATE INDEX idx_events_channel ON events(channel);
CREATE INDEX idx_events_provider ON events(provider_name);
CREATE INDEX idx_events_event_id ON events(event_id);
```

> `raw_xml` 当前尚不支持，留空待后续实现。`--with-xml` 选项已预留。

### 文件名对照表

系统汇总文件名使用微软官方版本命名，[msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 从注册表自动获取：

| 文件名 | 对应系统版本 | 构建号 |
|--------|------------|--------|
| `hotech-msg-win11-24h2-26100.db` | Windows 11 24H2 | 26100 |
| `hotech-msg-win11-23h2-22631.db` | Windows 11 23H2 | 22631 |
| `hotech-msg-win11-22h2-22621.db` | Windows 11 22H2 | 22621 |
| `hotech-msg-win10-22h2-19045.db` | Windows 10 22H2 | 19045 |
| `hotech-msg-win10-21h2-19044.db` | Windows 10 21H2 | 19044 |
| `hotech-msg-win10-1809-17763.db` | Windows 10 1809 | 17763 |
| `hotech-msg-winserver-2025-26100.db` | Windows Server 2025 | 26100 |
| `hotech-msg-winserver-2022-20348.db` | Windows Server 2022 | 20348 |
> `severity` 从 level 派生（1→关键, 2→错误, 3→警告, 4→信息, 5→详细, 0→信息）。

### CSV 输出

CSV 列与 events 表一致，无索引。每行一条事件。

---

## EVTX 解析性能优化

### 优化路径

| 阶段 | 做法 | 预期收益 |
|------|------|----------|
| v1.0 ✅ | evtx 0.12 + `records_json_value()` + **单遍懒加载** | **~2×** vs 双遍扫描 |
| v1.1+ | `template_instances()` 从 BinXML 直接提取字段，完全跳过 JSON 渲染 | **~5×** vs 当前 |

### v1.0 优化明细

**1. `records_json_value()` 替代 `records_json_bytes()` + `serde_json::from_slice()`**

旧代码将 JSON 序列化为字节再反序列化回 `Value`，新代码直接从 evtx crate 获取 `Value`，省去一次序列化/反序列化往返。

**2. 单遍懒加载替代双遍扫描**

```
旧路径（2 遍）:
  第一遍：遍历全部 2.5M 事件 → 收集 unique (provider, event_id) 对
  第二遍：重新遍历 → 字段提取 → 消息替换 → 写入

新路径（1 遍）:
  一次性遍历：
    records_json_value() → 字段提取
    → 首次遇到新 (provider, event_id)？按需预热消息库
    → message = substitute(template, extract_params(event_data))
    → 写入 batch → batch 满 → flush
```

省去整文件第一遍遍历，对 2.5M 事件 ~50% 时间节省。

**3. 消息替换 4 级回退策略**

| # | 策略 | 适用场景 |
|---|------|----------|
| 1 | 平铺对象 + 命名参数映射 | 有完整参数映射的 provider（WELM 数据） |
| 2 | **平铺对象 + 位置回退** ✅ **新增** | 无参数映射的 provider（如 Security-Auditing） |
| 3 | Data 数组 + 命名参数映射 | evtx 0.8 兼容 |
| 4 | `#text` 数组位置映射 | 经典 EventLog provider |

**4. 字段提取适配 evtx 0.12 EventData 格式变更**

evtx 0.12 将 EventData 从 `Data` 数组（带 `#attributes`/`$`）改为**平铺 Key-Value 对象**：

```
旧: {"EventData": {"Data": [{"#attributes":{"Name":"SubjectUserSid"},"$":"S-1-5-..."}]}}
新: {"EventData": {"SubjectUserSid":"S-1-5-...", "SubjectUserName":"jipeng.song", ...}}
              ↑ 直接以字段名为 key 的平铺对象
```

同时 `System.Security` 在新版本中为 `null`，`user_name`/`user_sid` 改从 EventData 提取。

### v1.1+ 目标

```
EVTX 二进制 → BinXML 解析 → template_instances() → 字段提取
                              完全跳过 JSON，从 BinXML 直接取 typed values
```

---

## 消息数据来源

### 数据来源分层

每个 `hotech-msg-{product}-{displayver}-{build}.db` 从三个来源合并（优先级递增）：

```
  1. NSA WELM（CC0 公有领域）
     ──────────────────────────
     种子数据：英文消息 + task/opcode/level/keywords/parameters
     覆盖范围：Win7→Win10 v1507，26 个版本，每版本 ~45K 事件
     来源：https://github.com/nsacyber/Windows-Event-Log-Messages
     许可：CC0 1.0 Universal（美国政府作品，公有领域）

  2. MUI + 本机 DLL 提取（[msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 工具）
     ──────────────────────────
     中文消息：从 C:\Windows\System32\zh-CN\*.dll.mui 提取
     语言优先级：zh-CN(2052) → zh-TW(1028) → en-US(1033)
     覆盖范围：当前 Windows 版本的已安装 provider

  3. 目标机器专属（[msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 工具）
     ──────────────────────────
     第三方 provider：Sysmon、防病毒、企业安全软件等
     自动扫描注册表 HKLM\SOFTWARE\...\WINEVT\Publishers\
```

### 文件发布

预构建的消息库分层发布：

```
GitHub Releases:
  hotech-msg-win11-24h2-26100.db         ← Win11 24H2（最新系统汇总）
  hotech-msg-win10-22h2-19045.db         ← Win10 22H2（系统汇总）
  hotech-msg-all.db           ← 所有版本合并
  dlldb/                      ← 按 DLL 拆分的原子文件（zip 压缩）
    adtschema.en-US.10.0.26220.1000.db
    adtschema.zh-CN.10.0.26220.1000.db
    sysmon.en-US.15.2.0.0.db
    ...
```

### 全自动更新 CI

每周自动在 Windows runner 上使用 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 提取并提交更新。

```yaml
# .github/workflows/build-msgdb.yml
name: Build Message Database
on:
  schedule:
    - cron: '0 2 * * 0'
  workflow_dispatch: {}

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-go@v4

      - name: Dump messages ([msgdb-dump](https://github.com/kitsch-9527/msgdb-dump))
        run: |
          msgdb-dump dump ^
            --out-dir ./data/ ^
            --lang 1033,2052,1028 ^
            --split-dlls

      - name: Commit new DLL data
        run: |
          git add data/dlldb/
          git diff --staged --quiet || (
            git commit -m "chore(msgdb): auto-update message DLL data" &&
            git push
          )

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: msgdb-${{ github.run_number }}
          files: |
            data/hotech-msg-*.db
            data/dlldb/*.db
```

### 覆盖示例（已验证）

| Provider | 英文消息 | 中文消息 | 来源 |
|----------|---------|---------|------|
| Security-Auditing | 544 | 544 | adtschema.dll + .mui |
| Kernel-Power | 12 | 12 | 系统 provider |
| EventLog (6005/8/9) | — | — | netevent.dll（无 RT_MESSAGETABLE，硬编码回退） |
| Sysmon | 23+ | 23+ | Sysmon.exe 消息表 |

---

## 依赖

### 运行时依赖

| crate | 版本 | 用途 | 平台 |
|-------|------|------|------|
| `evtx` | 0.12 | EVTX 二进制 BinXML 解析 | 全平台 |
| `rusqlite` | 0.32 (bundled) | SQLite 存储（自带 SQLite，零系统依赖） | 全平台 |
| `serde` + `serde_json` | 1.x | 序列化 / 反序列化 | 全平台 |
| `csv` | 1.3 | CSV 输出 | 全平台 |
| `chrono` | 0.4 | 时间戳处理 | 全平台 |
| `regex` | 1 | %1 %2 模板替换 | 全平台 |

### CLI 依赖

| crate | 版本 | 用途 |
|-------|------|------|
| `clap` | 4 (derive) | CLI 参数解析 |

---

## 已知限制

完整清单请参见 [docs/KNOWN-LIMITATIONS.md](docs/KNOWN-LIMITATIONS.md)。当前版本的主要限制：

| 限制 | 优先级 | 说明 |
|------|--------|------|
| EVTX build 检测失效 | P1 | 部分 EVTX 文件头 offset 0x40 为 0，需 `--msg-db` 手动指定 |
| CSV 重复 header | P2 | 首个空批次和首个真实批次各写一次 header |
| `template_instances` 跳过 JSON | P2 | v1.1+ 目标，当前走 `records_json_value()` |
| 消息库 task/opcode 为空 | P2 | 部分 provider 无可读名称 |
| `%%N` 消息引用未展开 | P2 | 方向/级别等枚举值显示为 `%%14592` 等引用码 |
| `--with-xml` 未实现 | P3 | `raw_xml` 字段为 NULL |
| dlldb 文件名 LCID | P3 | 应改用 ISO 语言代码 |

---

## 引用的开源项目

本项目的设计和实现对以下开源项目进行了参考、借鉴或直接使用。在此致谢。

### Rust 库（直接依赖）

| 项目 | 许可 | 在本项目中的用途 |
|------|------|----------------|
| [omerbenamram/evtx](https://github.com/omerbenamram/evtx) | MIT | 核心 EVTX 二进制 XML 解析器。本项目基于其 `EvtxParser`、`EvtxRecord`、`template_instances()` API 构建 |
| [rusqlite](https://github.com/rusqlite/rusqlite) | MIT | SQLite 绑定（bundled 模式），messages.db 和 events.db 的存储引擎 |
| [serde / serde_json](https://github.com/serde-rs/serde) | MIT/Apache-2.0 | 结构化数据序列化 |
| [chrono](https://github.com/chronotope/chrono) | MIT/Apache-2.0 | 时间戳处理与格式化 |
| [csv](https://github.com/BurntSushi/rust-csv) | MIT/Apache-2.0 | CSV 输出 |
| [regex](https://github.com/rust-lang/regex) | MIT/Apache-2.0 | 消息模板占位符匹配 |
| [clap](https://github.com/clap-rs/clap) | MIT/Apache-2.0 | CLI 参数解析 |

### 参考项目

| 项目 | 许可 | 参考内容 |
|------|------|----------|
| [Velocidex/evtx-data](https://github.com/Velocidex/evtx-data) | — | **SQLite schema 参考。** `providers(id, name)` + `messages(provider_id, event_id, message)` 的两表设计是 messages.db 的基础。其 `welm/welm_combined.db` 是 NAS WELM 的 SQLite 封装 |
| [Velocidex/evtx (Go)](https://github.com/Velocidex/evtx) | Apache-2.0 | **架构参考。** `extract` + `parse` 的命令分离模式。Rust 端聚焦 parse，Go 端实现 extract |
| [isabella232/evtx-data](https://github.com/isabella232/evtx-data) | — | **社区消息数据库参考。** 预构建的 Windows 多版本消息数据库的发布模式 |
| [WithSecureLabs/chainsaw](https://github.com/WithSecureLabs/chainsaw) | GPL-3.0 | **CSV 输出字段参考。** 威胁狩猎工具的字段设计，本项目的 CSV 格式参考了其对核心事件字段的选择 |
| [Yamato-Security/hayabusa](https://github.com/Yamato-Security/hayabusa) | AGPL-3.0 | **时间线输出字段参考。** 大规模 EVTX 时间线生成工具的字段选择（Timestamp、Computer、Channel、EventID、RecordID、Details），本文档的 `severity` 派生字段受其 `Level` 列启发 |
| [forensicskween/evtx2db](https://github.com/forensicskween/evtx2db) | — | **SQLite 输出参考。** 将 dumpevtx 输出导入 SQLite 的 Python 封装 |

### 数据来源

| 项目 | 许可 | 在本项目中的用途 |
|------|------|----------------|
| [nsacyber/Windows-Event-Log-Messages](https://github.com/nsacyber/Windows-Event-Log-Messages) | **CC0 1.0 Universal**（美国政府作品，公有领域） | 种子英文消息数据。覆盖 Win7→Win10 v1507 的 ~45K 事件/版本的 `events.csv`，包含 provider、event_id、message、level、task、opcode、keywords、parameters。通过 `--with-welm` 选项导入 |
| Microsoft Windows MUI | 参考（fair use） | 中文字体消息 DLL（`.mui` 文件）。从本机 Windows 系统的 `zh-CN/*.dll.mui` 提取中文消息翻译，不会随项目分发 |

### 学术参考

| 文献 | 参考内容 |
|------|----------|
| Schuster, A. (2007). *Introducing the Microsoft Vista event log file format*. DFRWS | EVTX BinXML 格式的奠基性研究，substitution array + template 机制的原始论文 |
| libyal/libfwevt (Joachim Metz) | WEVT_TEMPLATE 资源格式的完整文档 |

---

## 许可

本项目代码部分采用 **MIT** 许可。

引用的数据来源：
- **NSA WELM** — CC0 1.0 Universal（公有领域）
- **Windows MUI 文件** — 仅限在 Windows 系统上运行时提取使用，不随项目分发

请参见 [LICENSE](./LICENSE) 文件。
