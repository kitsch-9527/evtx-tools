# msgdb-dump Skill

## 概述
Windows ETW 提供程序消息数据库导出工具，从 Windows 消息 DLL 中提取事件消息模板、CRIM 元数据和参数引用，写入便携式 SQLite 数据库（hotech-msg 格式，schema v4）。

## 核心能力
- 从 Windows DLL 提取事件消息模板
- 解析 CRIM 二进制格式（EVNT/TASK/OPCO/LEVL/KEYW/TTBL/VMAP/BMAP）
- 支持多语言（英语、中文等）
- 增量导出（基线模式）
- 远程 DLL 导入
- Windows XP 32-bit 兼容（纯 Go SQLite）

## 快速开始

### 构建
```bash
# 标准 64-bit 构建
go build -o msgdb-dump-x64.exe .

# Windows XP 32-bit（无 CGo，纯 Go SQLite）
GOARCH=386 GOOS=windows CGO_ENABLED=0 go build -ldflags="-s -w" -o msgdb-dump-xp.exe .
```

### 基本使用
```bash
# 默认：提取英语+中文到可执行文件同级目录
msgdb-dump-x64.exe

# 指定输出目录
msgdb-dump-x64.exe --out-dir ./output

# 单语言（英语 1033）
msgdb-dump-x64.exe --lang 1033

# 多语言
msgdb-dump-x64.exe --lang 1033,2052,1041
```

## 三种提取路径

1. **RT_MESSAGETABLE** — DLL 直接包含消息表（adtschema.dll 等）
2. **CRIM + MUI** — DLL 有 CRIM 元数据，消息在 MUI 语言文件中（netevent.dll 等）
3. **MUI-only** — DLL 无消息表也无 CRIM，直接读取 MUI 伴生 DLL（IoLogMsg.dll、esent.dll 等）

## 过滤

```bash
# 按提供程序名称 glob 过滤（支持 * 和 ?）
msgdb-dump-x64.exe --providers "*Security*"
msgdb-dump-x64.exe --providers "Microsoft-Windows-Kernel-*"
msgdb-dump-x64.exe --providers "EventLog,DNS"
```

## 增量更新

```bash
# 重新执行提取并更新已存在的数据库
msgdb-dump-x64.exe --update --out-dir ./output
```

## 远程 DLL 导入

从远程机器收集的 DLL/EXE/SYS 文件中提取消息表，合并到系统消息库。

```bash
# 导入远程 DLL 目录
msgdb-dump-x64.exe --import-dll ./remote_dlls/

# 带别名：注册 MySQL 提供程序名称 → 对应 DLL 文件名
msgdb-dump-x64.exe --import-dll ./remote_dlls/ --alias MySQL=mysqld

# 多个别名
msgdb-dump-x64.exe --import-dll ./remote_dlls/ \
  --alias MySQL=mysqld \
  --alias MSSQLSERVER=sqlservr
```

## 基准线 / 增量导出

从一台干净的基准机器生成基线说明（baseline.json），然后在目标机器上只导出不在基线中的 DLL 消息。

```bash
# 在基准机上完整导出（自动附带 baseline.json）
msgdb-dump-x64.exe --out-dir ./baseline

# 或从已有 .db 手动生成基线
msgdb-dump-x64.exe --generate-baseline ./existing.db

# 在目标机上增量导出（只导第三方 DLL）
msgdb-dump-x64.exe --baseline ./baseline/hotech-msg-*.json --tag mysql-vmware

# 无新增时输出提示
msgdb-dump-x64.exe --baseline ./baseline.json
```

### 增量数据库 metadata 键值

| key | 示例 | 说明 |
|-----|------|------|
| is_delta | 1 | 标记为增量数据库 |
| baseline_db | hotech-msg-xxx.db | 来源基线数据库文件名 |
| baseline_os | Windows 10 Education | 基线机器的操作系统 |
| baseline_dll_count | 856 | 基线中的 DLL 总数 |

## 导入 WELM 种子数据

```bash
msgdb-dump-x64.exe --with-welm ./welm.db
```

> 常用经典事件（6005/6006/6009/6013 等）已内置于 `builtin_messages.go`，默认自动注入。

## 分库输出

```bash
msgdb-dump-x64.exe --split-dlls
```

输出结构：
```
./output/
  hotech-msg-Windows_10_Education-25H2-26220.7052.db   # 系统级消息数据库
  dlldb/                                                # 按 DLL 拆分的数据库
    adtschema.1033.10.0.22621.1.db
    netevent.1033.10.0.22621.1.db
    ...
```

## 命令行选项参考

| 选项 | 默认值 | 说明 |
|------|--------|------|
| --out-dir | 可执行文件目录 | 输出目录 |
| --lang | 1033,2052 | 逗号分隔的 LCID 列表 |
| --providers | (全部) | 逗号分隔的提供程序名称 glob 模式 |
| --split-dlls | false | 在 dlldb/ 子目录中按 DLL 拆分输出 |
| --with-welm | (无) | WELM 种子数据库路径 |
| --update | false | 更新已有数据库（重新执行提取） |
| --import-dll | (无) | 远程机器 DLL 目录路径 |
| --alias | (无) | 提供程序别名映射 Name=Dll，逗号分隔 |
| --generate-baseline | (无) | 从已有 .db 文件生成 baseline.json |
| --baseline | (无) | 基线 JSON 路径，加载后只导出不在基线中的 DLL |
| --tag | (无) | 增量 DB 文件名标签（不传则用时间戳） |

## 消息提取机制

### 扫描来源

默认执行会扫描注册表中所有已注册的 ETW Provider：

```
HKLM\SYSTEM\...\EventLog\*              # 经典 EventLog Provider
HKLM\SOFTWARE\...\WINEVT\Publishers\*   # 现代 Manifest Provider
```

### 特殊处理

| 场景 | 处理方式 |
|------|---------|
| CRIM 资源 ID ≠ 事件 ID | 从 CRIM EventEntry 提取 message_id→event_id 映射后转换 |
| EventLog 经典事件 | 代码内置已知消息（6005/6006/6008/6009/6011/6013） |
| EventMessageFile 指向多个 DLL | 扫描所有 DLL，合并孤立消息 → fixOrphanedMessages() |
| 注册表 FriendlyName 为空 | 读取注册表键默认值补全 |
| WELM 种子 dll_id 不匹配 | 自动解析为已有 dll_id |
| 同一 Provider 跨多个 DLL | 合并消息 → mergeDuplicateProviders() |
| WoW64 文件系统重定向 | 使用 wcorefx/fs.NormalizePath 自动处理 |
| 基线过滤 | --baseline 加载 JSON，与 DLL 路径做不区分大小写的匹配 |

## 输出数据库

### hotech-msg 格式（schema v4）

包含 7 张表：

| 表 | 说明 |
|---|------|
| dll_entries | DLL 文件清单，含 PE 版本号 |
| providers | ETW 提供程序注册信息 |
| dll_messages | 消息模板（按 DLL、语言、事件 ID 索引） |
| provider_aliases | 提供程序名称到 DLL 的别名映射 |
| metadata | 数据库元数据（OS、schema 版本、创建时间） |
| parameter_refs | %%N 参数引用，用于参数化消息展开 |
| value_maps | VMAP/BMAP 枚举值映射，用于字段值→显示名 |

### 文件名格式

```
# 系统级数据库
hotech-msg-{ProductName}-{DisplayVersion}-{BuildNumber}.db

# 增量数据库（--baseline）
hotech-msg-{tag}_delta.db                    # 有 --tag
hotech-msg_{YYYYMMDD}_delta.db               # 无 --tag（使用当前日期）

# 每 DLL 数据库（--split-dlls）
{dll_stem}.{LCID}.{PE_Version}.db
```

## 架构

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

### 核心包

| 包 | 用途 |
|---|------|
| dump | 主流程编排：注册表扫描、DLL 分组、提取调度、数据写入 |
| pe | PE 文件离线解析：资源目录遍历、RT_MESSAGETABLE 解析、WEVT_TEMPLATE 提取 |
| crim | CRIM 二进制格式解析：EVNT/TASK/OPCO/LEVL/KEYW/TTBL/VMAP/BMAP 块 |
| schema | SQLite DDL 定义：7 张表的 schema v4 |

## 依赖

- modernc.org/sqlite — 纯 Go SQLite（无 CGo，XP 兼容）
- golang.org/x/sys — Windows 注册表访问
- github.com/kitsch-9527/wcorefx — PE 解析辅助、注册表封装、WoW64 文件系统重定向

## 许可
MIT
