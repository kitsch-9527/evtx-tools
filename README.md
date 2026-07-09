# evtx-tools

> 跨平台 EVTX 事件解析与分析工具链
>
> 将 Windows EVTX 日志文件解析为 SQLite / CSV，不依赖 Windows API。
> 中英双语消息替换，内置威胁狩猎场景分析。

## 快速开始

```bash
# 安装
cd evtx-tools
cargo build --release

# 查看帮助
./target/release/evtx-tools --help

# 基本解析（使用消息库）
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output

# 解析 + 场景分析（内置 profile）
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output \
  --profile security \
  --profile persistence

# 解析 + 自定义规则
evtx-tools parse case.evtx --rules-dir ./my-rules/ -o case_output

# 跳过统计表（追求最快解析速度）
evtx-tools parse case.evtx --no-stats -o output
```

## 功能

### 解析（parse）

| 选项 | 默认值 | 说明 |
|------|--------|------|
| `<INPUT>` | — | .evtx 文件路径 |
| `-o, --output` | — | 输出路径（SQLite / CSV） |
| `--format` | `db` | 输出格式：`db`、`csv`、`both` |
| `--msg-dir` | — | 消息库目录（自动匹配版本） |
| `--msg-db` | — | 手动指定消息库文件 |
| `--with-xml` | `false` | 输出原始 XML |
| `--threads` | CPU 核数 | 解析线程数，0=自动 |
| `--lang` | `zh-CN` | 消息语言 |
| `--profile` | — | 内置分析 profile，可多次使用 |
| `--rules-dir` | — | 自定义规则 YAML 目录 |
| `--no-stats` | `false` | 跳过统计表生成 |

- EVTX → SQLite / CSV 输出，**25 列**完整事件字段
- 自动匹配消息库版本，中英双语消息替换（%1 %2 展开）
- 多线程并行解析 + 写入分离管道
- **全局 `sequence_number`** — 单调递增解析顺序号，用于事件排序和时间线连续性检测

### 场景分析（`--profile` / `--rules-dir`）

内置 8 个分析 profile，支持自定义规则文件。规则匹配的事件自动写入场景表：

| Profile | 场景表 | 核心 EventID | 场景 |
|---------|--------|-------------|------|
| `security` | `security_events` | 4624,4625,4648,4672,4720,4728,4732,4776,1102 | 登录审计、权限提升、账户管理 |
| `persistence` | `persistence_events` | 7045,4697,4698,4699,4700,7036,5617-5620 | 服务安装、计划任务、WMI |
| `system` | `system_anomaly` | 6008,41,1074,1001,1102,104,7031,7034,1100 | 异常关机、系统崩溃、日志清空 |
| `remote` | `remote_events` | 4624,4648,4778,4779,5156,5158,1149 | RDP 登录、横向移动 |
| `process` | `process_events` | 4688,Sysmon 1/10/11/17/18 | 进程创建、Sysmon 事件 |
| `powershell` | `powershell_events` | 400,403,4103,4104,4105,4106,53504 | PowerShell 执行、脚本块 |
| `mssql` | `mssql_events` | 18456 系列 + provider 匹配 | SQL Server 审计 |
| `web` | `web_events` | provider 匹配 (ASP.NET/IIS) | ASP.NET / IIS |

#### 自定义规则 YAML 格式

```yaml
# my-rules/my_analysis.yaml
name: "My Analysis"               # 规则名称
table: "my_events"                # 输出场景表名
columns:                          # 可选，限制输出列
  - event_id
  - timestamp
  - user_name
  - message
event_ids: [4624, 4625]           # 按 EventID 过滤（最快）
channels: ["Security"]            # 按通道过滤
provider_like:                    # Provider 名 LIKE 匹配
  - "MSSQL%"
message_like:                     # 消息内容 LIKE 匹配（最慢）
  - "%登录%"
```

过滤逻辑：**同类型 OR，不同类型 AND**。所有指定的过滤条件都需满足，同一条件内任一项匹配即可。

### 统计表（自动生成，`--no-stats` 跳过）

场景无关的分组聚合，在 Rust 端用 HashMap 内存聚合后写入，零额外 SQL 全表扫描：

| 统计表 | 列 | 用途 |
|--------|-----|------|
| `stat_event_id` | event_id, provider_name, count | EventID 分布概览 |
| `stat_severity` | level, severity, count | 严重等级分布 |
| `stat_hourly` | hour_bucket, count | 每小时事件量 |
| `stat_channel` | channel, count | 日志通道分布 |
| `stat_computer` | computer, count | 机器维度分布 |
| `stat_event_id_hourly` | event_id, provider_name, hour_bucket, count, **min_sequence_number**, **max_sequence_number** | EventID × 小时矩阵 + 时间连续性检测 |

#### 时间跳变检测

```sql
-- 通过 sequence_number 范围检测时间是否被篡改
-- 如果 T10 的 max_seq > T11 的 min_seq → 时间被拨回
SELECT a.hour_bucket, a.max_sequence_number AS t10_max,
       b.hour_bucket, b.min_sequence_number AS t11_min
FROM stat_event_id_hourly a
JOIN stat_event_id_hourly b ON a.event_id = b.event_id
  AND a.hour_bucket < b.hour_bucket
  AND a.max_sequence_number > b.min_sequence_number
WHERE a.event_id = 4624;
```

### 其他命令

- `merge` — 合并多个消息库为单文件
- `info` — 查看消息库元数据

## 架构

```
evtx-tools/
├── evtx-tools-core/              # 核心库（全平台）
│   └── src/
│       ├── lib.rs                # 公开 API：ParsedEvent, Result
│       ├── parser.rs             # EVTX BinXML 解析
│       ├── substitute.rs         # %1 %2 替换引擎 + %%N 展开
│       ├── resolver.rs           # 消息库查询与预热缓存
│       ├── stats.rs              # ★ 内存统计聚合（6 表）
│       ├── routing.rs            # ★ 路由引擎（Rule, SceneRouter）
│       └── storage/
│           ├── mod.rs
│           ├── sqlite.rs         # EventsDb + 场景表写入
│           └── csv.rs            # CSV 输出
│
├── evtx-tools-cli/               # CLI 二进制
│   └── src/
│       ├── main.rs               # clap 入口 + 内置 profile 嵌入
│       ├── cmd_parse.rs          # 解析管道 + 路由集成
│       ├── cmd_merge.rs          # 消息库合并
│       └── cmd_info.rs           # 消息库信息
│
├── rules/                        # ★ 内置 profile（编译嵌入）
│   ├── security.yaml
│   ├── persistence.yaml
│   ├── system.yaml
│   ├── remote.yaml
│   ├── process.yaml
│   ├── powershell.yaml
│   ├── mssql.yaml
│   └── web.yaml
│
├── data/
│   ├── reference-dataset/        # 参考数据集（4 个 EVTX 合并）
│   └── lua-test-rules/           # Lua 1:1 对照测试规则
│
└── docs/                         # 设计文档
```

### 管道流程

```
EVTX → parser → extract_fields → sequence_number 赋值
  │
  ├── [消息替换]          substitute(%1 %2) + %%N 展开
  │
  ├── [批量写入]          events 主表（25 列）
  │
  ├── [场景路由]          profile/规则匹配 → 场景表写入
  │     event_ids → channels → provider_like → message_like
  │
  └── [统计聚合]          HashMap 内存聚合 → 6 个 stat_* 表
```

### 输出 DB 结构

```
events                        # 主表（25列含 sequence_number）
├── security_events           # --profile security（按规则过滤）
├── persistence_events        # --profile persistence
├── ...
├── stat_event_id             # 统计表
├── stat_severity
├── stat_hourly
├── stat_channel
├── stat_computer
└── stat_event_id_hourly      # 含 min/max_sequence_number
```

## 消息库

消息库由独立的 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 工具生成：

```bash
# 在 Windows 机器上提取本机所有 provider 的消息
msgdb-dump dump --out-dir ./msg_dbs/ --lang 1033,2052

# 解析时自动匹配
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output
```

## 开发与测试

### 开发环境

- Rust 1.75+
- 仅 `cargo` 构建，无外部系统依赖（SQLite bundled）

### 运行测试

```bash
# 全量测试
cargo test

# 按模块测试
cargo test routing      # 路由引擎测试
cargo test stats        # 统计模块测试
cargo test storage      # 存储层测试
cargo test parser       # 解析器测试
```

### Lua 1:1 对照验证

参考 Lua 脚本 `evtx_2_Sqlite.lua` 的 5 个分析场景，提供了对照测试规则：

```bash
# 1. 准备本地 EVTX 文件（需要管理员权限）
#    C:\Windows\System32\winevt\Logs\
#    - Security.evtx
#    - System.evtx
#    - Application.evtx
#    - Windows PowerShell.evtx

# 2. 逐个解析并验证
evtx-tools parse "C:\Windows\System32\winevt\Logs\Security.evtx" \
  --msg-db ./hotech-msg-Windows_10_Education-25H2-26220.7052.db \
  -o ./data/lua-test-output/security_lua \
  --rules-dir ./data/lua-test-rules

# 3. 对比参考数据集
sqlite3 ./data/reference-dataset/reference.db \
  "SELECT COUNT(*) FROM aevtx_7045;"

sqlite3 ./data/lua-test-output/system_lua \
  "SELECT COUNT(*) FROM lua_7045;"
```

参考数据集的生成流程：

```bash
# data/reference-dataset/create_views.sql 包含：
# - aevtx                  → 规范化视图
# - aevtx_4624_4625        → 登录事件
# - aevtx_7045             → 服务安装
# - aevtx_AnyDesk          → AnyDesk 检测
# - aevtx_ASPNET           → ASP.NET 事件
# - aevtx_MSSQL_login      → MSSQL 登录
```

### 测试数据文件

```
data/lua-test-rules/          # Lua 对照测试规则（5个 YAML）
data/reference-dataset/       # 参考数据集
  ├── reference.db            #   合并的参考 DB（74K 事件）
  └── create_views.sql        #   参考视图定义
```

## 许可

MIT
