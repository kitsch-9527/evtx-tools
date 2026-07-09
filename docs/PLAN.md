# hotech-evtx 项目计划

> 生成日期：2026-07-06
> 状态：核心实现已完成，详见 [DELIVERY.md](DELIVERY.md)
> 许可：MIT

---

## 目录

- [1. 项目目标](#1-项目目标)
- [2. 架构总览](#2-架构总览)
- [3. Crate 结构](#3-crate-结构)
- [4. 数据库 Schema](#4-数据库-schema)
  - [4.1 系统汇总 DB](#41-系统汇总-db)
  - [4.2 DLL 级别 DB](#42-dll-级别-db)
  - [4.3 合并版 DB (all.db)](#43-合并版-db-alldb)
- [5. CLI 命令设计](#5-cli-命令设计)
  - [parse 命令](#parse-命令)
  - [merge 命令](#merge-命令)
  - [db-info 命令](#db-info-命令)
- [6. 数据流](#6-数据流)
  - [parse 数据流](#parse-数据流)
- [7. 消息数据来源](#7-消息数据来源)
- [8. 性能优化](#8-性能优化)
- [9. 自动更新 CI](#9-自动更新-ci)
- [10. 社区贡献流程](#10-社区贡献流程)
- [11. 实现阶段](#11-实现阶段)
- [12. 依赖清单](#12-依赖清单)
- [13. 命名规范](#13-命名规范)

---

## 1. 项目目标

高性能跨平台 EVTX 事件解析 + 消息格式化工具链。

**核心价值：** 在 Linux/macOS 上也能拿到和 Windows 事件查看器一样的人类可读事件描述。
中英双语消息，无需目标机器的系统 DLL。

**三层边界：**

| 层 | 做什么 | 不做 |
|------|--------|------|
| hotech-evtx | EVTX 解析、消息格式化、SQLite/CSV 输出 | 不做威胁狩猎、不做 Sigma 规则匹配 |
| hotech-msg-db | 消息库的构建、合并、版本管理 | 不直接参与解析流程 |
| GUI（hotech-analyzer-gui） | 可视化、搜索、交互 | 核心解析依赖 hotech-evtx |

---

## 2. 架构总览

```
┌────────────────────────────────────────────────────────────┐
│  hotech-evtx CLI                                           │
│                                                           │
│                    parse (全平台)                           │
│  ┌─────────────────────────────────────────────────────┐  │
│  │ .evtx → serialized records + template_instances     │  │
│  │   → substitute(%1, %2) → 输出 events.db / .csv     │  │
│  │        ↑                                            │  │
│  │    msg.db (由 msgdb-dump 二进制生成)                │  │
│  └─────────────────────────────────────────────────────┘  │
│                                                           │
│  消息库: hotech-msg-{product}-{displayver}-{build}.db     │
│          dlldb/{dll}.{lang}.{version}.db                  │
└────────────────────────────────────────────────────────────┘
```

### 不可违反约束

| # | 约束 | 说明 |
|---|------|------|
| C1 | parse 是全平台纯 Rust | 除 evtx crate 外无平台绑定 |
| C2 | 消息格式化不调 Windows API | 所有 %1 %2 替换是纯字符串操作 |
| C3 | 消息提取（dump）由 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 完成 | Rust 端不涉及注册表、PE 解析 |

---

## 3. Crate 结构

```
crates/hotech-evtx/
├── Cargo.toml
├── README.md
├── LICENSE
├── docs/
│   ├── PLAN.md                     ← 本文件
│   ├── architecture.md             ← 架构图 + 数据流
│   └── schemas/
│       ├── messages.md             ← 消息库详细设计（含 CI 工作流）
│       ├── events.sql              ← events.db DDL
│       └── msg-sys.sql             ← 系统汇总 DB DDL
├── src/
│   ├── lib.rs                      # 公开 API
│   │
│   ├── event.rs                    # ParsedEvent 结构体
│   │   ├── struct ParsedEvent       # 输出数据模型（22 列）
│   │   └── fn extract()            # 从 EvtxRecord 提字段
│   │
│   ├── parser.rs                   # evtx EvtxParser 封装
│   │   ├── fn open_evtx(path, threads) # 打开 EVTX 文件（+配置线程数）
│   │   ├── fn iter_events()        # records_json_value → ParsedEvent + EventData Value
│   │   ├── fn extract_fields()     # 从 JSON Value 提 22 字段
│   │   └── fn detect_build()       # 从文件头读 build_number
│   │
│   ├── substitute.rs               # %1 %2 替换引擎
│   │   ├── fn substitute()         # 主入口
│   │   └── fn parse_template()     # 解析模板占位符
│   │
│   ├── resolver.rs                 # messages.db 查询 + 预热
│   │   ├── struct MessageDb        # 消息库句柄
│   │   ├── fn open()               # 打开 DB
│   │   ├── fn warmup()             # 批量加载到 HashMap
│   │   └── fn resolve()            # 单条查询
│   │
│   └── storage/                    # 输出存储
│       ├── mod.rs
│       ├── sqlite.rs               # parse_to_sqlite()
│       └── csv.rs                  # parse_to_csv()
│
├── bin/
│   └── hotech-evtx/
│       ├── main.rs                 # CLI 入口（clap）
│       ├── cmd_parse.rs            # parse 命令
│       ├── cmd_merge.rs            # merge 命令
│       └── cmd_info.rs             # db-info 命令
│
├── tests/
│   ├── integration.rs              # 集成测试
│   ├── test_data/                  # 测试用 EVTX 样本
│   └── fixtures/                   # 测试用消息 DB
│
└── data/                           # 数据目录（开源仓库）
    ├── dlldb/                      # DLL 级别消息文件（git 跟踪）
    ├── releases/                   # 预构建系统汇总（Release 产物）
    └── welm/                       # NSA WELM 种子数据
```

---

## 4. 数据库 Schema

### 4.1 系统汇总 DB

文件名：`hotech-msg-{product}-{displayver}-{build}.db`
例：`hotech-msg-win11-24h2-26100.db`

文件命名来源（读注册表 `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`）：

| 注册表值 | 示例 | 映射到文件名段 |
|----------|------|--------------|
| `ProductName` = "Windows 11 Pro" | → `win11` | product |
| `DisplayVersion` = "24H2" | → `24h2` | displayver |
| `CurrentBuild` = "26100" | → `26100` | build |

```sql
CREATE TABLE providers (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE,
    guid    TEXT
);

CREATE TABLE messages (
    provider_id   INTEGER NOT NULL REFERENCES providers(id),
    event_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    level         INTEGER,               -- 0/1/2/3/4/5
    task          TEXT,                   -- Task Category 名称
    opcode        TEXT,                   -- Opcode 名称
    keywords      TEXT,                   -- Keywords 显示名
    parameters    TEXT,                   -- JSON: {"%1":"SubjectUserSid",...}
    language_id   INTEGER NOT NULL DEFAULT 1033,
    source        TEXT NOT NULL DEFAULT 'dump',  -- 'welm', 'mui', 'dump'
    extracted_at  TEXT NOT NULL,
    PRIMARY KEY (provider_id, event_id, language_id)
);

CREATE INDEX idx_messages_provider ON messages(provider_id);
CREATE INDEX idx_messages_event ON messages(event_id);

CREATE TABLE _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE _languages (id INTEGER PRIMARY KEY, code TEXT NOT NULL, name TEXT NOT NULL);
```

### 4.2 DLL 级别 DB

文件名：`dlldb/{dll_name}.{lang}.{pe_version}.db`
例：`dlldb/adtschema.zh-CN.10.0.26220.1000.db`

```sql
CREATE TABLE messages (
    event_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    level         INTEGER,
    task          TEXT,
    opcode        TEXT,
    keywords      TEXT,
    parameters    TEXT,
    provider_name TEXT,                   -- 当 DLL 包含多个 provider 时使用
    PRIMARY KEY (event_id)
);
```

极简——文件名唯一确定身份：`{dll}.{lang}.{ver}` 三元组。

### 4.3 合并版 DB (all.db)

文件名：`hotech-msg-all.db`

```sql
CREATE TABLE messages (
    provider_id   INTEGER NOT NULL REFERENCES providers(id),
    event_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    level         INTEGER,
    task          TEXT,
    opcode        TEXT,
    keywords      TEXT,
    parameters    TEXT,
    language_id   INTEGER NOT NULL DEFAULT 1033,
    build_number  INTEGER NOT NULL,       -- 来源版本构建号
    source        TEXT NOT NULL DEFAULT 'dump',
    extracted_at  TEXT NOT NULL,
    priority      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (provider_id, event_id, language_id, build_number)
);

CREATE INDEX idx_messages_provider ON messages(provider_id);
CREATE INDEX idx_messages_event ON messages(event_id);
CREATE INDEX idx_messages_build ON messages(build_number);

CREATE TABLE _versions (
    build_number  INTEGER PRIMARY KEY,
    os_version    TEXT,
    extracted_at  TEXT NOT NULL,
    message_count INTEGER
);

CREATE TABLE _meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE _languages (id INTEGER PRIMARY KEY, code TEXT NOT NULL, name TEXT NOT NULL);
```

### 4.4 events.db（parse 输出）

```sql
CREATE TABLE events (
    event_record_id     INTEGER NOT NULL,
    timestamp           TEXT,
    provider_name       TEXT,
    provider_guid       TEXT,
    event_id            INTEGER,
    level               INTEGER,
    severity            TEXT,              -- 关键/错误/警告/信息/详细
    task_category       TEXT,
    opcode              TEXT,
    keywords            TEXT,
    channel             TEXT,
    computer            TEXT,
    process_id          INTEGER,
    thread_id           INTEGER,
    user_name           TEXT,
    user_sid            TEXT,
    activity_id         TEXT,
    related_activity_id TEXT,
    message             TEXT,              -- substitute 后完整描述
    event_data          TEXT,              -- JSON, key 来自 parameters
    raw_xml             TEXT,              -- 可选，默认 NULL
    file_name           TEXT
);

CREATE INDEX idx_events_timestamp ON events(timestamp);
CREATE INDEX idx_events_level ON events(level);
CREATE INDEX idx_events_channel ON events(channel);
CREATE INDEX idx_events_provider ON events(provider_name);
CREATE INDEX idx_events_event_id ON events(event_id);
```

> `raw_xml` 默认 NULL，需 `--with-xml` 开启。

---

## 5. CLI 命令设计

### parse 命令

**用途：** 解析 EVTX 文件。**全平台。**

```bash
hotech-evtx parse <INPUT> [--msg-dir <DIR> | --db <PATH>] -o <OUTPUT> [OPTIONS]
```

| 选项 | 默认值 | 说明 |
|------|--------|------|
| `<INPUT>` | — | .evtx 文件或目录 |
| `--msg-dir <DIR>` | — | 消息库目录，自动匹配版本 |
| `--db <PATH>` | — | 手动指定消息库文件 |
| `-o, --output <PATH>` | — | 输出路径（不含扩展名） |
| `--format <FORMAT>` | `db` | `db` / `csv` / `both` |
| `--with-xml` | 不输出 | 开启 raw_xml 字段 |
| `--threads <N>` | 自动 | 线程数，0=CPU 核数 |

**自动匹配规则：**

```
1. hotech-msg-{product}-{displayver}-{evtx_build}.db  → 精确匹配
2. hotech-msg-all.db                                     → 合并库
3. hotech-msg-{product}-{displayver}-{向下最近}.db     → 最近低版本
4. 都没有                                                → 降级（message 留空）
```

**降级行为：**

| 字段 | 有消息库 | 无消息库 |
|------|---------|---------|
| `message` | substitute 后完整描述 | NULL |
| `task_category` | 来自 messages.task | NULL |
| `opcode` | 来自 messages.opcode | NULL |
| `keywords` | 来自 messages.keywords | NULL |
| `event_data` | 参数名映射自 parameters | 原始 EventData JSON |
| 其余字段 | 正常填充 | 正常填充 |

**性能路径：**

```
Step 1 (v1.0, ✅ 已实现):
  EvtxRecord → records_json_value() → Value → 字段提取
  ↑ 跳过 records_json_bytes() + serde_json::from_slice() 往返
  单遍懒加载（预热 + 替换）→ 省去第一遍遍历

Step 2 (v1.1+):
  EvtxRecord → template_instances() → BinXML typed values → 字段提取
  ~5-6× vs 当前（完全跳过 JSON）
```

### merge 命令

**用途：** 合并多个单版本 DB 为 all.db。

```bash
hotech-evtx merge <DB1> <DB2> -o <OUTPUT>
hotech-evtx merge --dir <DIR> -o <OUTPUT>
```

每个消息记录追加 `build_number`、`priority`、`source` 字段。

### 5.4 db-info

**用途：** 查看消息库统计。

```bash
hotech-evtx db-info <DB> [--provider <NAME>]
```

输出：provider 数、消息数、语言、来源分布、版本清单。

---

## 6. 数据流

> **注意：** 消息提取（dump）功能已用 Go 重写。以下仅描述 Rust 解析端的数据流。

### parse 数据流

```
INPUT: .evtx + hotech-msg-{product}-{displayver}-{build}.db + [--threads N]
         │              │
         │              ▼
         │      ┌────────────────┐
         │      │ MessageDb::open │  ← 只打开，不预热
         │      │ (lazy warmup)  │     首次遇到新 pair 时按需加载
         │      └───────┬────────┘
         │              │
         ▼              │
  ┌─────────────┐       │
  │ EvtxParser   │       │
  │ .from_path() │       │
  │ +num_threads │       │  ← --threads N 控制 Rayon 并行
  └──────┬──────┘       │
         │              │
         ▼              │
  records_json_value()   │
  (多线程内部分块解析)   │
         │              │
         ▼              │
  ┌──────────────────────────┐
  │ 解析线程（Producer）：    │
  │                          │
  │ 1. extract_fields(Value) │  ← 返回 (ParsedEvent, EventData Value)
  │    22 字段提取           │     避免 Value→String→Value 往返
  │    event_data_value.clone│
  │                          │
  │ 2. lazy warmup:          │  ← 首次遇到时预热
  │    if !warmed_up(key):   │
  │      db.warmup(&[...])   │
  │                          │
  │ 3. resolve():            │  ← 查 HashMap
  │    message = substitute( │     EventData Value 直接传入
  │      template,           │     避免 JSON 重解析
  │      extract_subst_params│
  │      (4 级回退策略)      │
  │    )                     │
  │    task = resolved.task  │  ← 优先 DB 值
  │    opcode = resolved.op  │
  │    keywords = resolved.kw│
  │    severity = level_map  │
  │                          │
  │ 4. tx.send(event)        │  ← mpsc::sync_channel 发送到 Writer
  └──────────┬───────────────┘
             │
             ▼
  ┌──────────────────────────┐
  │ 写入线程（Writer）：      │
  │                          │
  │ rx.recv() → Vec<ParsedEvent> batch
  │            │
  │            ▼
  │  每 20000 事件:           │
  │  multi-row INSERT (500行/块)
  │  + PRAGMA page_size=65536  │
  │  + synchronous=OFF         │
  │  + journal_mode=MEMORY     │
  │  + cache_size=64MB         │
  │            │
  │            ▼
  │  最后: CREATE INDEX + ANALYZE
  │  (延迟索引，避免逐行维护)
  │
  │ 最终输出: events.db / events.csv
  └──────────────────────────┘
```
  │                          │
  │ 4. push → batch          │
  │    batch 满 → flush      │
  └──────────────────────────┘
         │
         ▼
  ┌──────────────┐
  │ events.db    │  ← SQLite（默认）
  │ 或 events.csv│  ← CSV（--format csv / both）
  └──────────────┘
```

---

## 7. 消息数据来源

### 三层来源（优先级递增）

```
  1. NSA WELM（CC0 公有领域）
     ──────────────────────────
     英文种子数据：Win7→Win10 v1507，~45K 事件/版本
     来源：https://github.com/nsacyber/Windows-Event-Log-Messages
     许可：CC0 1.0 Universal

  2. MUI + 本机 DLL（[msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)）
     ──────────────────────────
     中文消息：从 zh-CN/*.dll.mui 提取
     语言优先级：zh-CN(2052) → zh-TW(1028) → en-US(1033)
     自动扫描注册表全量枚举
```

### 覆盖已验证

| Provider | DLL | 英文 | 中文 |
|----------|-----|------|------|
| Security-Auditing | adtschema.dll | 544 | 544 |
| Kernel-Power | 系统内置 | 12 | 12 |
| Sysmon | sysmon.exe | 23+ | — |
| EventLog(6005/8/9) | — | 硬编码回退 | 硬编码回退 |

---

## 8. 性能优化

### 解析路径

```
v0.9 (之前): evtx 0.12 → records_json_bytes() → serde_json::from_slice() → Value → 提取
              两遍遍历（第一遍收集 pair，第二遍解析写入）

v1.0 (当前, ✅): evtx 0.12 → records_json_value() → Value → 提取
              单遍懒加载（预热 inline + 替换）
              ↑ 跳过 JSON 序列化/反序列化往返
              ↑ 省去整文件第一遍遍历

v1.1+ (未来): evtx 0.12 → template_instances() → typed values → 提取
             完全跳过 JSON 渲染，直接从 BinXML 取 typed values
```

### 优化项明细

| 优化 | 旧 | 新 | 收益 |
|------|-----|-----|------|
| JSON API | `records_json_bytes` + `from_slice` | `records_json_value` | 省去 deserialize 步骤 |
| 解析遍数 | 2 遍（收集+解析） | 1 遍（懒加载预热） | -50% 文件读取 |
| 预热策略 | 批量加载全部 pair | 首次遇到时按需加载 | 零预热开销 |
| task/opcode 覆盖 | 仅 None 时覆盖 | 始终优先 DB 值 | 正确显示可读名称 |
| 消息回退策略 | Data 数组仅一种 | 4 级回退 | 兼容更多 provider |
| **多线程解析** | 单线程 `records_json_value()` | `--threads N` Rayon 并行 | 1.48× 加速 (1.5GB) |
| **写入重叠** | 同步循环 → 写 → 循环 | `mpsc::sync_channel` 后台 Writer 线程 | 1.11× 加速 |
| **SQLite 批量写入** | 逐行 `stmt.execute()` | multi-row INSERT (500行/块) | 1.37× 加速 |
| **SQLite PRAGMAs** | 默认设置 | `page_size=65536`, `synchronous=OFF`, `journal_mode=MEMORY`, `cache_size=64MB` | 写入吞吐 +37% |
| **索引创建时机** | 表创建时立即建索引 | 数据写入后 `finalize()` 批量建索引 | 显著减少逐行 B-tree 维护 |
| **JSON Value 往返** | `to_string()` → `from_str()` 重解析 | `extract_fields` 返回 EventData Value 直接使用 | 微优化 |

### 消息解析优化

- 懒加载预热：首次遇到新 (provider, event_id) 时触发一次 SQL，后续走 HashMap O(1)
- substitute 是纯字符串操作（无 FFI、无系统调用）
- batch_size = 20000 写入 SQLite（继承已验证的优化值）
- `resolve_event_message` 接受 `Option<&Value>` 参数，EventData Value 直接传入避免重解析 JSON

---

## 9. 自动更新 CI

### GitHub Actions 工作流

```yaml
name: Build Message Database

on:
  schedule:
    - cron: '0 2 * * 0'         # 每周日凌晨 2 点 UTC
  workflow_dispatch:              # 手动触发
  push:
    branches: [main]
    paths: ['data/dlldb/**']      # 社区提交 DLL 数据时触发

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Build
        run: cargo build --release -p hotech-evtx

      - name: Dump messages ([msgdb-dump](https://github.com/kitsch-9527/msgdb-dump))
        run: |
          msgdb-dump dump ^
            --out-dir ./data/releases/ ^
            --lang 1033,2052,1028 ^
            --split-dlls

      - name: Detect changes
        id: check
        run: |
          git add data/dlldb/
          git diff --staged --quiet || echo "changed=true" >> $env:GITHUB_OUTPUT

      - name: Commit DLL data
        if: steps.check.outputs.changed == 'true'
        run: |
          git config user.name "hotech-bot"
          git config user.email "bot@hotech.dev"
          git commit -m "chore(msgdb): auto-update message DLL data"
          git push

      - name: Create Release
        if: steps.check.outputs.changed == 'true'
        uses: softprops/action-gh-release@v2
        with:
          tag_name: msgdb-${{ github.run_number }}
          files: |
            data/releases/hotech-msg-*.db
            data/dlldb/*.db
          body: |
            ## Message Database Update
            
            Windows Build: ${{ env.OS_BUILD }}
            DLLs: ${{ env.DLL_COUNT }}
```

### 触发场景

| 触发 | 自动？ | 说明 |
|------|--------|------|
| 每周定时 | ✅ | Windows runner 构建最新系统版本 |
| PR 合入（dlldb/ 变更） | ✅ | 社区贡献自动触发 |
| 手动触发 | ✅ | workflow_dispatch |
| 新工具版发布 | ✅ | release CI |

---

## 10. 社区贡献流程

```
1. 贡献者运行 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)：
     msgdb-dump dump --out-dir ./contrib/ --providers "Sysmon"
     → contrib/dlldb/sysmon.en-US.15.3.0.0.db

2. 检查文件是否已存在于 data/dlldb/：
   - 有同版本 → 跳过
   - 无此版本 → 复制到 data/dlldb/，提交 PR

3. 维护者合入 PR：
   - 检测 data/dlldb/ 变更
   - 自动运行 merge → 重建 hotech-msg-all.db
   - 创建 Release
```

---

## 11. 实现阶段

### Phase 0: 项目骨架（1-2 天）

- [ ] 创建 Cargo.toml（三个 crate 的 workspace）
- [ ] 实现模块目录结构（src/ + bin/）
- [ ] 定义核心数据结构（ParsedEvent, MessageDb, 各种枚举）
- [ ] evtx 依赖升级到 0.12.2
- [ ] 基础 CI（编译检查 + unit test）

### Phase 1: Core 核心（3-5 天）

- [ ] `event.rs` — ParsedEvent 结构体 + 字段提取
- [ ] `parser.rs` — EvtxParser 封装，serialized_records 包装
- [ ] `substitute.rs` — %1 %2 替换引擎（含 %n %t %r %%）
- [ ] `resolver.rs` — MessageDb 打开、预热、查询
- [ ] `storage/sqlite.rs` — parse_to_sqlite()
- [ ] `storage/csv.rs` — parse_to_csv()

### Phase 2: CLI 命令（2-3 天）

- [ ] `cmd_parse.rs` — parse 命令（包括 --msg-dir 自动匹配）
- [ ] `cmd_merge.rs` — merge 命令
- [ ] `cmd_info.rs` — db-info 命令
- [ ] 降级行为（无 --db 时 message 留空）
- [ ] 版本检测（从 .evtx 文件头读 build_number）

### Phase 4: 性能优化（2-3 天）

- [ ] Step 1: serialized_records + into_json_value
- [ ] Step 2: template_instances 跳过 JSON（v1.1+）
- [ ] SQLite 写入优化（bulk insert、batch size 调优）
- [ ] 消息预热策略调优（按 .evtx 实际包含的 provider 精确加载）

### Phase 5: 开源准备（2-3 天）

- [ ] GitHub Actions CI（build + test + dump）
- [ ] `data/dlldb/` 初始数据（从本机提取）
- [ ] `data/welm/` + `--with-welm` 导入逻辑
- [ ] 文档完善（README, PLAN.md, schema docs）
- [ ] MIT 许可文件
- [ ] 发布 v0.1.0

---

## 12. 依赖清单

### hotech-evtx-core（运行时，全平台）

| crate | 版本 | 用途 |
|-------|------|------|
| `evtx` | 0.12 | EVTX BinXML 解析 |
| `rusqlite` | 0.32 (bundled) | SQLite 存储 |
| `serde` | 1 (derive) | 序列化 |
| `serde_json` | 1 | JSON 处理 |
| `csv` | 1.3 | CSV 输出 |
| `chrono` | 0.4 (serde) | 时间处理 |
| `regex` | 1 | %1 %2 替换 |

### hotech-evtx CLI

| crate | 版本 | 用途 |
|-------|------|------|
| `clap` | 4 (derive) | CLI 参数 |
| `hotech-evtx-core` | workspace | 内部 crate |

**零新外部依赖。** 所有 crate 要么已在项目中，要么是 Rust 标准库生态已有依赖。

---

## 13. 命名规范

### 文件命名

| 类型 | 格式 | 例 |
|------|------|----|
| 系统汇总 DB | `hotech-msg-{product}-{displayver}-{build}.db` | `hotech-msg-win11-24h2-26100.db` |
| 合并 DB | `hotech-msg-all.db` | `hotech-msg-all.db` |
| DLL 级别 DB | `dlldb/{dll}.{lang}.{pe_version}.db` | `dlldb/adtschema.zh-CN.10.0.26220.1000.db` |
| events DB | `{basename}.db` | `security.db`, `timeline.db` |
| events CSV | `{basename}.csv` | `security.csv`, `timeline.csv` |

### 代码命名

| 语言 | 规则 | 例 |
|------|------|----|
| Rust | snake_case | `parse_to_sqlite()`, `event_record_id` |
| SQL | snake_case | `provider_name`, `event_record_id` |
| CLI flags | kebab-case | `--msg-dir`, `--with-xml` |
| Env vars | UPPER_SNAKE | `HOTECH_MSG_DIR` |

### 数据库列命名

- 系统字段：`event_record_id`, `provider_name`, `event_id`（snake_case）
- 可读字段：`severity`, `task_category`, `opcode`（清晰命名，不缩写）
- JSON 字段：`event_data`, `parameters`, `raw_xml`（标明格式）
- 元数据：`_meta`, `_languages`, `_versions`（下划线前缀表示系统表）

---

## 14. 实现进度

| 阶段 | 状态 | 说明 |
|------|------|------|
| Phase 0: 项目骨架 | ✅ 完成 | Cargo.toml workspace、模块目录结构、核心数据类型 |
| Phase 1: Core 核心 | ✅ 完成 | ParsedEvent/MessageRow 类型、parser.rs、substitute.rs、resolver.rs、storage/sqlite.rs、storage/csv.rs |
| Phase 2 (原 dump 命令) | ➡️ 已迁移 | 消息提取功能已用 Go 重写（见 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)），不再属于 Rust 端。 |
| Phase 3: CLI 命令 | ✅ 完成 | cmd_parse.rs（两遍扫描 + 消息预热 + SQLite/CSV 输出）、cmd_merge.rs（ATTACH DATABASE 合并）、cmd_info.rs（metadata 查询 + 按 provider 过滤）、降级行为（无 DB 时 message 留空）、EVTX build_number 检测 |
| Phase 4: 性能优化 | ✅ 完成 | records_json_value() 跳过了序列化往返；单遍懒加载省去了第一遍遍历；SQLite 批量写入 20K/批次 + ANALYZE。template_instances 跳过 JSON 路径待 v1.1+ |
| Phase 5: 开源准备 | ⏳ 部分完成 | 文档完善（README, PLAN, CONTEXT, DECISIONS, CONTRIBUTING）、E2E 测试。CI/CD、data/dlldb/、data/welm/、MIT 许可文件、v0.1.0 发布待完成 |
| **Phase 6: 性能冲刺** | ✅ 完成 | `--threads N` 多线程 + `ParserSettings::num_threads()`；后台 Writer 线程（mpsc::sync_channel）重叠解析与写入；multi-row INSERT (500行/块) 取代逐行 execute；SQLite PRAGMAs（page_size=65536, synchronous=OFF, journal_mode=MEMORY, cache_size=64MB）；延迟索引创建（finalize() 替代 create_table 中建索引）；消除 JSON Value→String→Value 往返。1.5GB/2.5M events 从 74.6s → 25.7s (CSV) / 33.2s (SQLite) |

### 实现详情

| 里程碑 | 日期 | 内容 |
|--------|------|------|
| Phase 1 | 2026-07-06 | core 类型 + storage + CLI 命令 |
| Phase 3 | 2026-07-06 | parser/substitute/resolver + 全部 4 个 CLI 命令 |
| Phase 4 | 2026-07-07 | 单遍懒加载 + records_json_value() + 4 级消息替换策略 + EventData 平铺对象支持 |
| Phase 5 E2E | 2026-07-06 | 真实机器 E2E 测试 + EVTX build=0 修复 |
| Phase 5 文档 | 2026-07-06 | DELIVERY.md + PLAN.md 进度更新 |

### 当前代码指标

| 指标 | 值 |
|------|-------|
| Rust 源码行数 | ~6,200 |
| 测试函数 | 49（单元测试）+ 1（doc-test） |
| Crate 数 | 3 |
| 模块数 | 14 |
| E2E 验证 | 2,492,500 事件 Security.evtx ✅ |

---

## 16. 已知限制与待调研

### 16.1 EventManifest XML 消息系统（WEVT_TEMPLATE）— 已解决 ✅

**原来现象：** System.evtx、Application.evtx 等日志的事件描述为 NULL。
**现在状态：** 已实现 MUI+CRIM 解析路径，消息文本 + task/opcode/参数名可正常提取。

**解决方法：**

Windows 有两种事件消息存储系统。对于 EventManifest 系统：

| 系统 | PE 资源 | 当前支持 | 代表 DLL |
|------|---------|:-------:|----------|
| MESSAGETABLE | type 11 | ✅ | adtschema.dll（Security） |
| EventManifest (CRIM) | "WEVT_TEMPLATE" | ✅ | system-events.dll, netevent.dll |

**解析链路：**

```
主 DLL (WEVT_TEMPLATE/CRIM, 664KB)         MUI 伴生文件 (RT_MESSAGETABLE)
  ├── EVNT 块: event_id → message_identifier  ────┐
  ├── TASK 块: task_id → display_name             │
  ├── OPCO 块: opcode_id → display_name            |  Join by
  ├── TTBL 块: template_items → param_names        |  message_identifier
  └── ❌ 没有消息文本                               ↓
                                            en-US/*.dll.mui
                                              └── (resource_id → 消息文本)
```

**实现的 Rust 函数：**

| 函数 | 位置 | 职责 |
|------|------|------|
| `extract_raw_crim_blobs()` | extractor.rs | 从 PE 提取原始 CRIM 字节 |
| `parse_crim_provider_data()` | extractor.rs | 按 GUID 解析所有 provider 数据 |
| `parse_single_evnt_block()` | extractor.rs | 解析 EVNT 事件定义块 |
| `parse_task_block()` | extractor.rs | 解析 TASK 任务名称块 |
| `parse_opcode_block()` | extractor.rs | 解析 OPCO 操作码名称块 |
| `parse_template_table()` | extractor.rs | 解析 TTBL 模板参数名 |
| `read_crim_utf16_string()` | extractor.rs | CRIM 字符串解码（4 字节 size prefix） |
| `find_mui_companion()` | mui_helper.rs | MUI 伴生文件路径查找 |
| `mui_helper::lcid_to_lang_code()` | mui_helper.rs | LCID→语言代码映射 |

**关键发现：** CRIM 字符串使用 4 字节 u32 长度前缀（包含自身），文档中常误写为 2 字节 u16。

**验证结果：**

| Provider | events | tasks | opcodes | params | 消息文本 |
|----------|--------|-------|---------|--------|---------|
| AppModel-Runtime | 130 | 31 | 31 | 129 | ✅ |

### 16.2 其他已知限制

详见 [KNOWN-LIMITATIONS.md](KNOWN-LIMITATIONS.md)。
