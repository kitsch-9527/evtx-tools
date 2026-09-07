# evtx-tools — 开发规范

## 项目概述
跨平台 EVTX 事件解析与分析 CLI 工具。
Rust workspace：`evtx-tools-core`（库）+ `evtx-tools-cli`（二进制）。

## 构建与测试

```bash
cargo build --release          # 发布构建
cargo test                     # 全量测试（169 tests）
cargo test routing             # 路由引擎测试
cargo test stats               # 统计模块测试
cargo test storage::sqlite     # 存储层测试
cargo test -p evtx-tools-cli   # CLI 测试
cargo check                    # 快速编译检查
```

## 项目结构

```
evtx-tools/
├── evtx-tools-core/          # 核心库
│   └── src/
│       ├── lib.rs            # ParsedEvent, Result, severity_from_level
│       ├── parser.rs         # EVTX 解析（evtx crate 封装）
│       ├── substitute.rs     # %1 %2 替换引擎 + %%N 展开
│       ├── resolver.rs       # MessageDb 查询与预热缓存
│       ├── stats.rs          # 6 个统计表的 HashMap 聚合
│       ├── routing.rs        # Rule / RuleFilter / SceneRouter
│       └── storage/
│           ├── sqlite.rs     # EventsDb + 场景表操作
│           └── csv.rs        # CSV 输出
│
├── evtx-tools-cli/           # CLI 二进制
│   └── src/
│       ├── main.rs           # clap 入口 + 8 内置 profile 嵌入
│       ├── cmd_parse.rs      # 解析管道（producer-writer 分离）
│       └── cmd_info.rs       # 消息库信息（要求单文件 v7）
│
├── rules/                    # 内置 profile YAML（编译嵌入）
├── data/
│   ├── reference-dataset/    # 参考数据集（74K 事件）
│   └── lua-test-rules/       # Lua 1:1 对照测试规则
└── docs/                     # 设计文档
```

## 关键技术选型

| 层 | 选型 |
|---|---|
| EVTX 解析 | `evtx` 0.12 |
| 存储 | `rusqlite` 0.32 (bundled) |
| CLI 参数 | `clap` 4 (derive) |
| 序列化 | `serde` + `serde_json` |
| YAML 规则 | `serde_yaml` 0.9 |
| 并行 | `rayon` + `mpsc::sync_channel` |
| 消息替换 | `regex` |

## 架构与约束

### 管道架构
```
EVTX → parser → extract_fields → sequence_number 赋值
  → substitute(%1 %2 + %%N)
  → events 主表 + 路由场景表 + 统计表
```

### Producer-Writer 分离
- **Producer**（主线程）：顺序迭代 EVTX，并行消息替换，按 20K batch 发送
- **Writer**（后台线程）：接收 batch，写 events 表、路由事件到场景表、更新统计
- 通过 `mpsc::sync_channel<Vec<ParsedEvent>>(4)` 通信

### 路由过滤（AND 跨字段，OR 同字段）
```
event_ids → channels → provider_like → message_like
```
- `event_ids` 最快（整数比较，extract_fields 后即可判断）
- `message_like` 最慢（需等待 substitute 完成）

### 统计表（Rust 端 HashMap 聚合）
- 6 个 `stat_*` 表，在 writer 线程中逐事件 feed
- `finalize()` 时单事务写入，零全表扫描
- `stat_event_id_hourly` 含 `min/max_sequence_number` → 时间跳变检测

### 命名约定
- Rust 函数：`snake_case`
- CLI 命令：`snake_case`（`--profile`, `--rules-dir`, `--no-stats`）
- 统计表：`stat_*`
- 场景表：规则 `table` 字段指定

### 错误处理
- 全项目使用 `Result<T, String>`（统一 `evtx_tools_core::Result`）
- `eprintln!` 输出警告，不中断解析
- CLI 入口 `main()` 捕获 error → exit 1

### 测试要求
- 所有新增功能必须带测试
- 核心逻辑优先用纯函数 + 单元测试
- SQLite 操作用 `Connection::open_in_memory()` 测试

## CLI 命令

### parse — 核心解析
```bash
evtx-tools parse <INPUT> -o <OUTPUT>
  [--format db|csv|both]
  [--msg-dir <DIR> | --msg-db <PATH>]
  [--profile <NAME> ...]
  [--rules-dir <DIR>]
  [--no-stats]
  [--lang <CODE>]
  [--threads <N>]
  [--with-xml]
```
- 单文件 v7 必需：`--msg-db` 指向 `hotech-msg-*.db`（`schema_version=7`），或 `--msg-dir` 目录下恰好一个；无库直接报错；无 merge/batch 子命令（重跑 msgdb-dump）。

### 内置 Profile（8 个）
security, persistence, system, remote, process, powershell, mssql, web

### YAML 规则格式
```yaml
name: "规则名"
table: "输出表名"
columns: [event_id, event_time, message]    # 可选
event_ids: [4624, 4625]                    # OR
channels: ["Security"]                     # AND
provider_like: ["MSSQL%"]                  # AND
message_like: ["%登录%"]                   # AND
```

## 消息库
由独立 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 工具生成（单文件 v7，`schema_version=7`）。
CLI 通过 `--msg-dir`（目录下恰好一个 `hotech-msg-*.db`，多文件报错）自动定位，或 `--msg-db` 手动指定；无库直接报错，无 merge 子命令。

## 验证

```bash
# Lua 1:1 对照验证
evtx-tools parse Security.evtx --msg-db xxx.db -o output --rules-dir data/lua-test-rules
sqlite3 output "SELECT COUNT(*) FROM lua_4624_4625;"
sqlite3 data/reference-dataset/reference.db "SELECT COUNT(*) FROM aevtx_4624_4625;"

# 时间跳变检测
sqlite3 output "
  SELECT a.hour_bucket, a.max_sequence_number, b.hour_bucket, b.min_sequence_number
  FROM stat_event_id_hourly a JOIN stat_event_id_hourly b
    ON a.event_id = b.event_id AND a.hour_bucket < b.hour_bucket
    AND a.max_sequence_number > b.min_sequence_number
  WHERE a.event_id = 4624;
"
```
