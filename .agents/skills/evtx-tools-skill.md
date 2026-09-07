# evtx-tools Skill

## 概述
跨平台 EVTX 事件解析与分析工具链，将 Windows EVTX 日志文件解析为 SQLite / CSV，不依赖 Windows API。

## 核心能力
- EVTX → SQLite/CSV 解析（25 列完整事件字段）
- 中英双语消息替换（%1 %2 展开 + %%N 引用）
- 8 个内置威胁狩猎 profile
- 自定义 YAML 规则引擎
- 6 个统计表 + 时间跳变检测
- 多线程并行解析（Producer-Writer 管道）

## 快速开始

### 构建
```bash
cd evtx-tools
cargo build --release
```

### 基本解析
```bash
# 使用消息库解析
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output

# 带场景分析
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output \
  --profile security --profile persistence

# 自定义规则
evtx-tools parse case.evtx --rules-dir ./my-rules/ -o case_output

# 跳过统计表（最快解析）
evtx-tools parse case.evtx --no-stats -o output
```

## 内置 Profile

| Profile | 场景表 | 核心 EventID | 场景 |
|---------|--------|-------------|------|
| security | security_events | 4624,4625,4648,4672,4720,4728,4732,4776,1102 | 登录审计、权限提升 |
| persistence | persistence_events | 7045,4697,4698,4699,4700,7036,5617-5620 | 服务安装、计划任务 |
| system | system_anomaly | 6008,41,1074,1001,1102,104,7031,7034,1100 | 异常关机、系统崩溃 |
| remote | remote_events | 4624,4648,4778,4779,5156,5158,1149 | RDP 登录、横向移动 |
| process | process_events | 4688,Sysmon 1/10/11/17/18 | 进程创建 |
| powershell | powershell_events | 400,403,4103,4104,4105,4106,53504 | PowerShell 执行 |
| mssql | mssql_events | 18456 系列 | SQL Server 审计 |
| web | web_events | provider 匹配 | ASP.NET / IIS |

## 自定义规则 YAML

```yaml
name: "My Analysis"
table: "my_events"
columns:
  - event_id
  - event_time
  - user_name
  - message
event_ids: [4624, 4625]
channels: ["Security"]
provider_like:
  - "MSSQL%"
message_like:
  - "%登录%"
```

**过滤逻辑**：同类型 OR，不同类型 AND

## 统计表

| 统计表 | 列 | 用途 |
|--------|-----|------|
| stat_event_id | event_id, provider_name, count | EventID 分布概览 |
| stat_severity | level, severity, count | 严重等级分布 |
| stat_hourly | hour_bucket, count | 每小时事件量 |
| stat_channel | channel, count | 日志通道分布 |
| stat_computer | computer, count | 机器维度分布 |
| stat_event_id_hourly | event_id, provider_name, hour_bucket, count, min_sequence_number, max_sequence_number | EventID × 小时矩阵 + 时间连续性检测 |

## 时间跳变检测

```sql
-- 通过 sequence_number 范围检测时间是否被篡改
SELECT a.hour_bucket, a.max_sequence_number AS t10_max,
       b.hour_bucket, b.min_sequence_number AS t11_min
FROM stat_event_id_hourly a
JOIN stat_event_id_hourly b ON a.event_id = b.event_id
  AND a.hour_bucket < b.hour_bucket
  AND a.max_sequence_number > b.min_sequence_number
WHERE a.event_id = 4624;
```

## 其他命令

- `merge` — 合并多个消息库为单文件
- `info` — 查看消息库元数据

## 架构

```
evtx-tools/
├── evtx-tools-core/          # 核心库
│   └── src/
│       ├── lib.rs            # ParsedEvent, Result
│       ├── parser.rs         # EVTX 解析
│       ├── substitute.rs     # 消息替换引擎
│       ├── resolver.rs       # 消息库查询
│       ├── stats.rs          # 统计聚合
│       ├── routing.rs        # 路由引擎
│       └── storage/          # 存储层
│
├── evtx-tools-cli/           # CLI 二进制
│   └── src/
│       ├── main.rs           # 入口
│       ├── cmd_parse.rs      # 解析命令
│       ├── cmd_merge.rs      # 合并命令
│       └── cmd_info.rs       # 信息命令
│
├── rules/                    # 内置 profile YAML
├── data/                     # 测试数据
└── docs/                     # 设计文档
```

## 管道流程

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

## 输出 DB 结构

```
events                        # 主表（25列含 sequence_number）
├── security_events           # --profile security
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

消息库由独立的 msgdb-dump 工具生成：

```bash
# 在 Windows 机器上提取本机所有 provider 的消息
msgdb-dump dump --out-dir ./msg_dbs/ --lang 1033,2052

# 解析时自动匹配
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output
```

## 开发与测试

### 开发环境
- Rust 1.75+
- 仅 cargo 构建，无外部系统依赖（SQLite bundled）

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

## 许可
MIT
