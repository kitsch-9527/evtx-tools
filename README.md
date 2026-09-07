# EVET 项目说明

## 项目概述

本仓库包含两个相互关联的 Windows 事件日志分析工具：

1. **evtx-tools** - 跨平台 EVTX 事件解析与分析工具链（Rust）
2. **msgdb-dump** - Windows ETW 提供程序消息数据库导出工具（Go）

这两个项目共同构成了一个完整的 Windows 事件日志分析生态系统，用于安全分析、威胁狩猎和系统监控。

---

## 项目 1: evtx-tools

### 简介
跨平台 EVTX 事件解析与分析 CLI 工具，将 Windows EVTX 日志文件解析为 SQLite / CSV，不依赖 Windows API。支持中英双语消息替换，内置威胁狩猎场景分析。

### 核心功能
- **EVTX 解析**：将 Windows EVTX 文件解析为 SQLite 或 CSV 格式
- **消息替换**：自动匹配消息库版本，支持 %1 %2 参数展开和 %%N 引用
- **场景分析**：内置 8 个分析 profile，支持自定义规则
- **统计分析**：自动生成 6 个统计表，支持时间跳变检测
- **多线程并行**：Producer-Writer 分离管道架构

### 技术栈
- **语言**：Rust 1.75+
- **EVTX 解析**：evtx 0.12
- **存储**：rusqlite 0.32 (bundled)
- **CLI**：clap 4 (derive)
- **序列化**：serde + serde_json
- **并行**：rayon + mpsc::sync_channel

### 架构
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

### 内置分析 Profile
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

### 构建与使用
```bash
# 构建
cd evtx-tools
cargo build --release

# 基本解析
./target/release/evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output

# 带场景分析
./target/release/evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output \
  --profile security --profile persistence
```

---

## 项目 2: msgdb-dump

### 简介
Windows ETW 提供程序消息数据库导出工具，从 Windows 消息 DLL 中提取事件消息模板、CRIM 元数据和参数引用，写入便携式 SQLite 数据库（hotech-msg 格式，schema v4）。

### 核心功能
- **消息提取**：从 Windows DLL 中提取事件消息模板
- **CRIM 解析**：解析 CRIM 二进制格式（EVNT/TASK/OPCO/LEVL/KEYW/TTBL/VMAP/BMAP）
- **多语言支持**：支持英语、中文等多种语言
- **增量导出**：基线模式，只导出第三方软件新增消息
- **远程导入**：从远程机器收集的 DLL 中提取消息

### 技术栈
- **语言**：Go 1.20+
- **SQLite**：modernc.org/sqlite（纯 Go，无 CGo，XP 兼容）
- **Windows API**：golang.org/x/sys
- **PE 解析**：github.com/kitsch-9527/wcorefx

### 架构
```
msgdb-dump/
├── main.go                   # 入口
├── dump/                     # 主流程编排
├── pe/                       # PE 文件解析
├── crim/                     # CRIM 格式解析
├── schema/                   # SQLite DDL
├── docs/                     # 文档
└── .jj/                      # Jujutsu 版本控制
```

### 三种提取路径
1. **RT_MESSAGETABLE** - DLL 直接包含消息表
2. **CRIM + MUI** - DLL 有 CRIM 元数据，消息在 MUI 语言文件中
3. **MUI-only** - DLL 无消息表也无 CRIM，直接读取 MUI 伴生 DLL

### 输出数据库结构（schema v4）
| 表 | 说明 |
|---|------|
| dll_entries | DLL 文件清单，含 PE 版本号 |
| providers | ETW 提供程序注册信息 |
| dll_messages | 消息模板（按 DLL、语言、事件 ID 索引） |
| provider_aliases | 提供程序名称到 DLL 的别名映射 |
| metadata | 数据库元数据（OS、schema 版本、创建时间） |
| parameter_refs | %%N 参数引用 |
| value_maps | VMAP/BMAP 枚举值映射 |

### 构建与使用
```bash
# 构建
cd msgdb-dump
go build -o msgdb-dump-x64.exe .

# 基本使用
./msgdb-dump-x64.exe --out-dir ./output --lang 1033,2052

# 增量导出
./msgdb-dump-x64.exe --baseline ./baseline.json --tag mysql-vmware
```

---

## 项目关系

```
┌─────────────────┐    消息库    ┌─────────────────┐
│   msgdb-dump    │ ──────────▶ │   evtx-tools    │
│   (Go)          │             │   (Rust)        │
│                 │             │                 │
│ 提取消息模板    │             │ 解析 EVTX 文件  │
│ 生成 .db 文件   │             │ 使用消息库替换  │
└─────────────────┘             └─────────────────┘
```

**工作流程**：
1. 使用 `msgdb-dump` 从 Windows 系统提取消息模板，生成 `.db` 文件
2. 使用 `evtx-tools` 解析 EVTX 文件，自动匹配消息库进行消息替换
3. 生成包含完整事件信息的 SQLite 数据库，支持场景分析和统计

---

## 开发环境

### evtx-tools
- Rust 1.75+
- cargo 构建，无外部系统依赖（SQLite bundled）

### msgdb-dump
- Go 1.20+
- 支持 Windows XP 32-bit（GOARCH=386 + 纯 Go SQLite）

---

## 测试

### evtx-tools
```bash
cd evtx-tools
cargo test                     # 全量测试（169 tests）
cargo test routing             # 路由引擎测试
cargo test stats               # 统计模块测试
cargo test storage::sqlite     # 存储层测试
```

### msgdb-dump
```bash
cd msgdb-dump
go test ./...                  # 全量测试
```

---

## 许可证

两个项目均采用 MIT 许可证。

---

## 相关资源

- [evtx-tools README](evtx-tools/README.md)
- [msgdb-dump README](msgdb-dump/README.md)
- [evtx-tools 开发规范](evtx-tools/CLAUDE.md)
- [msgdb-dump 数据库 schema](msgdb-dump/docs/schema.md)
