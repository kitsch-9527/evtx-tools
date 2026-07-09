# hotech-evtx 交付文档

> 高性能跨平台 EVTX 事件解析 + 消息格式化工具链。
> 在 Linux/macOS 上也能拿到和 Windows 事件查看器一样的人类可读事件描述。

---

## 1. Crate 结构

两个内部 crate 组成 workspace，对外发布一个 CLI 二进制 `hotech-evtx`：

> **注意：** 消息提取（dump）功能已用 Go 重写（见 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)），Rust 端的 `hotech-evtx` 只负责 EVTX 解析。

```
crates/hotech-evtx/
├── Cargo.toml              # workspace 根
├── hotech-evtx-core/        # 全平台运行时的核心库
└── hotech-evtx/             # CLI 二进制（clap 参数解析）
```

| Crate | 职责 | 平台 | 外部依赖 |
|-------|------|------|----------|
| hotech-evtx-core | EVTX 解析、消息替换、SQLite/CSV 存储 | 全平台 | evtx, rusqlite, serde, csv, regex, chrono |
| hotech-evtx (CLI) | clap CLI、parse/merge/info 命令 | 全平台 | clap, rusqlite |

## 2. 关键特性

### parse 命令（全平台）

解析 EVTX 文件为结构化事件，自动匹配消息库进行 %1-%99 替换。

```
hotech-evtx parse Security.evtx --msg-dir ./msg_dbs/ -o output.db
hotech-evtx parse System.evtx --msg-dir ./msg_dbs/ -o output.csv --format csv
```

- 自动从 EVTX 文件头 offset 0x40 读取 build_number
- 根据 build_number 精确匹配消息库
- 两遍扫描：第一遍收集 unique (provider, event_id) pairs，第二遍预热 HashMap + 逐事件替换
- 消息预热到内存 HashMap，避免逐事件查 SQLite（O(1) vs O(log n)）
- `--format` 支持 `db` / `csv` / `both`
- 无消息库时降级（message 留空，其余字段正常）
- 批量提交（20,000 条/批次）+ WAL 模式
- CSV 输出含 UTF-8 BOM（中文 Excel 兼容）

### merge 命令

合并多个消息库为单一 all.db。

```
hotech-evtx merge db1.db db2.db -o hotech-msg-all.db
```

### db-info 命令

查询消息库统计和 provider 详情。

```
hotech-evtx db-info msgs.db [--provider Security]
```

> **注意：** 消息库由 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)（二进制 `msgdb-dump`）工具生成。Rust 端的 parse/merge/info 命令只消费已有的消息库文件。

## 3. 数据库 Schema

### 系统汇总 DB（hotech-msg-{product}-{displayver}-{build}.db）

```sql
CREATE TABLE providers (
    name     TEXT NOT NULL PRIMARY KEY,
    guid     TEXT,
    dll_path TEXT,
    channel  TEXT
);

CREATE TABLE messages (
    provider_name TEXT NOT NULL,
    event_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    level         INTEGER,
    task          TEXT,
    opcode        TEXT,
    keywords      TEXT,
    parameters    TEXT,        -- JSON: {"1":"SubjectUserSid",...}
    PRIMARY KEY (provider_name, event_id)
);

CREATE TABLE metadata (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);
```

### DLL 级别 DB（dlldb/{dll}.{lang}.{ver}.db）

```sql
CREATE TABLE messages (
    event_id   INTEGER NOT NULL PRIMARY KEY,
    message    TEXT NOT NULL,
    level      INTEGER,
    task       TEXT,
    opcode     TEXT,
    keywords   TEXT,
    parameters TEXT
);
```

### events.db（parse 输出，22 列）

```sql
CREATE TABLE events (
    event_record_id     INTEGER NOT NULL,
    timestamp           TEXT,
    provider_name       TEXT,
    provider_guid       TEXT,
    event_id            INTEGER,
    level               INTEGER,
    severity            TEXT,        -- 关键/错误/警告/信息/详细
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
    message             TEXT,        -- substitute 后完整描述
    event_data          TEXT,        -- JSON
    raw_xml             TEXT,        -- 可选，默认 NULL
    file_name           TEXT
);
```

## 4. E2E 测试结果

在 Windows 10 Education 25H2 (build 26220.7052) 上实测：

| Step | Status | Detail |
|------|--------|--------|
| Build | PASS | `cargo build --release` 59s |
| Parse Security.evtx (v0.9) | PASS | 2,492,500 事件，无消息库时 message=NULL |
| Parse Security.evtx (v1.0) | PASS | 2,492,500 事件，**消息替换正常**，user_name/user_sid 从 EventData 提取 ✅ |
| CSV output | PASS | 含 BOM，中文正常 |
| Per-DLL databases | PASS | 按 DLL 拆分，含 CRIM provider |
| Fallback (no DB) | PASS | 所有消息 NULL 正确降级 |
| MUI+CRIM Resolution | PASS | system-events.dll + netevent.dll 的 WEVT_TEMPLATE provider ✅ |
| Enrichment | PASS | Task/Opcode 名称 + 参数名从 CRIM 提取 ✅ |
| **单遍解析** | **PASS** | **省去第一遍遍历，~2× 吞吐量提升** ✅ |
| **消息替换** | **PASS** | **EventData 平铺对象正确提取 + 位置回退替换** ✅ |

### 已修复 Bug

| Bug | 修复 |
|-----|------|
| EVTX build 检测返回 0（Windows build 26220） | 当 build==0 时标记为不可靠，使用 --msg-dir 中第一个可用 DB |
| System.evtx 0% 消息解析 | MUI 伴生文件 + CRIM EVNT 块解析 ✅ |
| **EventData 全空（核心 Bug）** | **evtx 0.12 输出平铺 Key-Value 对象，旧代码找 `EventData.Data[]` 数组导致全空 → 改为直接取 EventData 对象** ✅ |
| **user_name/user_sid 硬编码 None** | **改从 EventData 的 SubjectUserName / SubjectUserSid 提取** ✅ |
| **消息替换参数提取失败** | **新增 4 级回退策略：平铺对象命名映射 → 平铺对象位置回退 → Data 数组 → #text 数组** ✅ |
| **任务/操作码不覆盖** | **之前仅在 EVTX 值为 None 时用 DB 值 → 改为始终优先使用 DB 值** ✅ |
| **事件数 2.5M 第一遍遍历浪费** | **改为单遍懒加载，首次遇到新 provider/event_id 对时按需预热** ✅ |

## 5. 已知限制

详见 [KNOWN-LIMITATIONS.md](KNOWN-LIMITATIONS.md)。当前版本主要限制：

| 限制 | 优先级 | 影响 |
|------|--------|------|
| EVTX build 检测失效 | P1 | 需用 `--msg-db` 显式指定消息库 |
| CSV 重复 header | P2 | 每个 CSV 文件有两行 header |
| `template_instances` 优化 | P2 | v1.1+ 跳过 JSON 渲染 |
| 消息库 task/opcode 列为空 | P2 | 部分 provider 无可读名称 |
| `%%N` 消息引用未展开 | P2 | 方向/级别等枚举值显示为引用码 |
| `--with-xml` 未实现 | P3 | `raw_xml` 字段为 NULL |
| dlldb 文件名 LCID | P3 | 应改用 ISO 语言代码 |

## 6. 代码质量指标

| 指标 | 值 |
|------|-------|
| Rust 源码行数 | ~3,000 |
| 测试函数 | 49+（单元测试）+ 1（doc-test） |
| Crate 数 | 2 |
| 模块数 | 8 |
| 文档文件数 | 10（CONTEXT.md, DECISIONS.md, DATA-FLOW.md, CONTRIBUTING.md, PLAN.md, README.md, E2E-REPORT.md, DELIVERY.md, messages.md, dlldb.md） |

> **注意：** 原先的 `hotech-evtx-dumper` crate 约 2,600 行代码已用 Go 重写（见 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)）。以上指标仅为 Rust 解析侧代码。

### 各 crate 代码分布

| Crate | 源码行数 | 测试数 | 模块 |
|-------|---------|--------|------|
| hotech-evtx-core | ~1,900 | 49+ | lib, parser, substitute, resolver, storage::sqlite, storage::csv |
| hotech-evtx (CLI) | ~1,100 | 6+ | main, cmd_parse, cmd_merge, cmd_info |
| **Total** | **~3,000** | **55+** | **8 模块** |

## 7. 字段来源对照表

| ParsedEvent 字段 | evtx 0.12 JSON 路径 | 说明 |
|------------------|---------------------|------|
| `event_record_id` | EvtxRecord 直接字段 | 来自 EVTX chunk header |
| `timestamp` | `Event.System.TimeCreated.@SystemTime` | ISO8601 |
| `provider_name` | `Event.System.Provider.@Name` | |
| `provider_guid` | `Event.System.Provider.@Guid` | |
| `event_id` | `Event.System.EventID` | |
| `qualifiers` | `Event.System.EventID.@Qualifiers` | 经典 EventLog 提供器 |
| `level` | `Event.System.Level` | 0=信息, 1=关键, ... |
| `severity` | 从 level 派生 | 中文映射 |
| `task_category` | `Event.System.Task`（优先 DB 值） | |
| `opcode` | `Event.System.Opcode`（优先 DB 值） | |
| `keywords` | `Event.System.Keywords`（优先 DB 值） | |
| `channel` | `Event.System.Channel` | |
| `computer` | `Event.System.Computer` | |
| `process_id` | `Event.System.Execution.@ProcessID` | |
| `thread_id` | `Event.System.Execution.@ThreadID` | |
| `user_name` | `Event.EventData.SubjectUserName`（回退 `TargetUserName`） | evtx 0.12 路径 |
| `user_sid` | `Event.EventData.SubjectUserSid`（回退 `TargetUserSid`） | evtx 0.12 路径 |
| `activity_id` | `Event.System.Correlation.@ActivityID` | |
| `related_activity_id` | `Event.System.Correlation.@RelatedActivityID` | |
| `message` | 消息表 + substitute | |
| `event_data` | `Event.EventData`（整个对象序列化） | 平铺 Key-Value |
| `raw_xml` | — | 未实现（--with-xml 预留） |
| `file_name` | 来源 .evtx 文件名 | |

## 8. jj 变更历史

```
@  xqpvpkpr  phase6: delivery documentation
○  zsrkmmys  phase5: e2e test and bug fixes
○  ynzsorwq  phase4: CLI commands
○  zpsqumlr  phase3: parser, substitute and resolver
○  ouzrlznz  phase1: core types and storage module
○  pnrorqro  chore: stop tracking build artifacts in apps/gui/src-tauri/target
```
