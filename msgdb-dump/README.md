# msgdb-dump

Windows ETW 提供程序消息数据库导出工具。从 Windows 消息 DLL 中提取事件消息模板、CRIM 变体（Variant Identity）与参数引用，写入单文件 SQLite 数据库（hotech-msg 格式，单文件 v7，schema_version=7）。

Rust [`hotech-evtx-dumper`](https://github.com/kitsch-9527/hotech-evtx-dumper) crate 的 Go 移植版本，通过 `GOARCH=386` + `modernc.org/sqlite`（纯 Go，无 CGo）实现 Windows XP 32-bit 兼容。

## 架构

```
┌──────────────────────────────────────────────────────────────────────┐
│                      msgdb-dump 数据流                               │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐    ┌───────────────┐    ┌──────────────────┐   │
│  │ Registry Scan     │───▶│ DLL Grouping  │───▶│ PE Extraction    │   │
│  │ • EventLog        │    │ (dedup by     │    │ • RT_MESSAGETABLE│   │
│  │ • WINEVT/Publishers│   │  DLL path)    │    │ • CRIM + MUI     │   │
│  └──────────────────┘    └───────────────┘    │ • MUI-only        │   │
│                                                └────────┬─────────┘   │
│                                                         ▼            │
│  ┌──────────────────┐    ┌───────────────┐    ┌──────────────────┐   │
│  │ SQLite Output    │◀───│ Enrich & Merge │◀───│ Post-Processing  │   │
│  │ • System DB      │    │ • CRIM metadata│    │ • %%N param refs │   │
│  │ • Single file v7 │    │ • WELM seed    │    │ • VMAP/BMAP      │   │
│  └──────────────────┘    └───────────────┘    └──────────────────┘   │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### 核心包

| 包 | 用途 |
|---|------|
| `dump` | 主流程编排：注册表扫描、DLL 分组、提取调度、数据写入 |
| `pe` | PE 文件离线解析：资源目录遍历、RT_MESSAGETABLE 解析、WEVT_TEMPLATE 提取 |
| `crim` | CRIM 二进制格式解析：EVNT/TASK/OPCO/LEVL/KEYW/TTBL/VMAP/BMAP 块 |
| `schema` | SQLite DDL 定义：单文件 v7（7 张表，`provider_message_variants` 为唯一消息表，`schema_version=7`） |

### 提取与归属（v7 变体精准匹配）

- **CRIM 变体身份（Variant Identity，8 元组）**：`(provider_guid, event_id, version, level, opcode, task, keywords, channel, message_identifier)` 全等方为同一 Variant，余则多行并存（`crim.VariantKey{Dedup}`；Q3=B）。同 `dll_id=163` 的 46 个 Provider 各自独立，TxR 与 AppContainer（EID 11）、闰秒与关机（EID 20）互不染。
- **Provider 归属（Q2=A）**：`ExtractProviderVariants(provider_guid→WEVT)` 按 Provider 定向取 `WEVT`，再到 MUI 的 `RT_MESSAGETABLE` 取正确变体；`MUI` 与 `RTStrings` 按 `en-US` 合并去重，`keywords` `TEXT` 防 `0x8000…` 溢出。

### 三种提取路径

1. **RT_MESSAGETABLE** — DLL 直接包含消息表（adtschema.dll 等），无 `WEVT` 的 Provider 走单模板回退（`version/level/opcode/task=0`）
2. **CRIM + MUI** — DLL 有 CRIM 元数据，消息在 MUI 语言文件中（netevent.dll 等），按 Provider 单独走 `EVNT → message_identifier → MUI` 映射
3. **MUI-only** — DLL 无消息表也无 CRIM，直接读取 MUI 伴生 DLL（IoLogMsg.dll、esent.dll 等）

## 构建

**要求：** Go 1.20+

```bash
# 标准 64-bit 构建
go build -o msgdb-dump-x64.exe .

# Windows XP 32-bit（无 CGo，纯 Go SQLite）
GOARCH=386 GOOS=windows CGO_ENABLED=0 go build -ldflags="-s -w" -o msgdb-dump-xp.exe .
```

## 使用

### 基础

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

### 过滤

```bash
# 按提供程序名称 glob 过滤（支持 * 和 ?）
msgdb-dump-x64.exe --providers "*Security*"
msgdb-dump-x64.exe --providers "Microsoft-Windows-Kernel-*"
msgdb-dump-x64.exe --providers "EventLog,DNS"
```

### 增量更新

```bash
# 重新执行提取并更新已存在的数据库
msgdb-dump-x64.exe --update --out-dir ./output
```

### 远程 DLL 导入

从远程机器收集的 DLL/EXE/SYS 文件中提取消息表，合并到系统消息库。
适用于目标机器安装了第三方软件（MySQL、VMware 等）但构建机上没有的情况。

```bash
# 导入远程 DLL 目录
msgdb-dump-x64.exe --import-dll ./remote_dlls/
```

### 导入 WELM 种子数据

WELM 种子数据库用于补充已知的经典 EventLog 消息。

```bash
msgdb-dump-x64.exe --with-welm ./welm.db
```

> 常用经典事件（6005/6006/6009/6013 等）已内置于 `builtin_messages.go`，默认自动注入。

### 命令行选项参考

| 选项 | 默认值 | 说明 |
|------|--------|------|
| `--out-dir` | 可执行文件目录 | 输出目录 |
| `--lang` | `1033,2052` | 逗号分隔的 LCID 列表 |
| `--providers` | (全部) | 逗号分隔的提供程序名称 glob 模式 |
| `--with-welm` | (无) | WELM 种子数据库路径 |
| `--update` | false | 更新已有数据库（重新执行提取） |
| `--import-dll` | (无) | 远程机器 DLL 目录路径 |

## 消息提取机制

### 扫描来源

默认执行会扫描注册表中所有已注册的 ETW Provider：

```
HKLM\SYSTEM\...\EventLog\*              # 经典 EventLog Provider
HKLM\SOFTWARE\...\WINEVT\Publishers\*   # 现代 Manifest Provider
```

第三方软件安装时会自动注册到上述路径。只要 DLL 存在于构建机上，就会被扫描并提取消息。

### 特殊处理

| 场景 | 处理方式 |
|------|---------|
| CRIM 资源 ID ≠ 事件 ID | 从 CRIM EventEntry 提取 message_id→event_id 映射后转换 |
| EventLog 经典事件 | 代码内置已知消息（6005/6006/6008/6009/6011/6013） |
| EventMessageFile 指向多个 DLL | 扫描所有 DLL，合并孤立消息 → `fixOrphanedMessages()` |
| 注册表 FriendlyName 为空 | 读取注册表键默认值补全 |
| WELM 种子 dll_id 不匹配 | 自动解析为已有 dll_id |
| 同一 Provider 跨多个 DLL | 合并消息 → `mergeDuplicateProviders()` |
| WoW64 文件系统重定向 | 使用 `wcorefx/fs.NormalizePath` 自动处理 |

### 获取第三方软件消息

```bash
# 方式 1：在安装了三方软件的机器上直接运行
msgdb-dump-x64.exe

# 方式 2：从远程机收集 DLL 后导入
msgdb-dump-x64.exe --import-dll ./remote_dlls/

# 方式 3：手动编写 WELM 种子库（适用于已知消息）
msgdb-dump-x64.exe --with-welm ./welm_seed.db
```

## 输出数据库

### hotech-msg 格式（单文件 v7，schema_version=7）

- **单文件 v7**（`hotech-msg-*.db`，`evtx-tools --msg-db` 应指向此库）：7 张表，以 `provider_message_variants` 为唯一消息表（Variant Identity 变体真源）；
- 详见 `docs/schema.md` 与 `CONTEXT.md`。

#### 系统库 v7（7 张表，现行）

| 表 | 说明 |
|---|------|
| `dll_entries` | DLL 文件清单，含 PE 版本号 |
| `providers` | ETW 提供程序注册信息（`provider_guid` 归属锚点，ADR 0001 Q2=A） |
| `provider_message_variants` | 变体真源（Variant Identity，`keywords TEXT`；TxR/闰秒等各归其主） |
| `metadata` | 数据库元数据（OS、schema 版本、创建时间，`schema_version=7`） |
| `parameter_refs` | `%%N` 参数引用，用于参数化消息展开 |
| `value_maps` / `param_value_maps` | `VMAP`/`BMAP` 与 Variant 粒度参数映射 |

详见 [docs/schema.md](docs/schema.md)。

### 文件名格式

```
# 系统级数据库（单文件 v7，evtx-tools --msg-db 指向）
hotech-msg-{ProductName}-{DisplayVersion}-{BuildNumber}.db
```

## 依赖

- [`modernc.org/sqlite`](https://modernc.org/sqlite) — 纯 Go SQLite（无 CGo，XP 兼容）
- [`golang.org/x/sys`](https://pkg.go.dev/golang.org/x/sys) — Windows 注册表访问
- [`github.com/kitsch-9527/wcorefx`](https://github.com/kitsch-9527/wcorefx) — PE 解析辅助、注册表封装、WoW64 文件系统重定向
