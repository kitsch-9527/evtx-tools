# hotech-evtx 领域上下文

> Windows EVTX 日志解析 + 消息格式化的领域知识、术语定义和核心概念。
> 阅读本文档前请确保理解这些概念，它们贯穿所有设计和代码。

---

## 1. EVTX 是什么

EVTX（Windows XML Event Log）是 Windows Vista 引入的新事件日志格式，取代了旧的 `.evt` 格式。

**关键事实：** EVTX 文件**不存储**人类可读的事件描述文本。它只存储结构化的数字和参数。描述文本需要从事件提供者的 DLL 中获取。

### 文件结构

```
┌─────────────┐
│ 文件头 (Elf) │  64 字节签名 + 元数据
├─────────────┤
│ Chunk 1      │  64KB - 256KB，包含多条事件记录
│   ├─ Chunk 头 │   包含 chunk 元数据、模板定义
│   ├─ 事件记录 1│   BinXML 格式
│   ├─ 事件记录 2│
│   ├─ ...      │
│   └─ 空闲空间  │   可被后续事件覆盖
├─────────────┤
│ Chunk 2      │
├─────────────┤
│ ...          │
└─────────────┘
```

### BinXML（二进制 XML）

EVTX 不存文本 XML，而是用一种叫 BinXML 的紧凑二进制格式。

核心机制是**模板 + 替换数组**的分离：

```
模板定义（在 chunk 头或 provider DLL 中存一次）:
  <Event>
    <System>
      <Provider Name="%0"/>
      <EventID>%1</EventID>
      <Level>%2</Level>
    </System>
  </Event>

每条事件只存"替换数组"（substitution array）:
  ┌─────┬──────────────────────────┐
  │  0  │ "Microsoft-Windows-..."  │
  │  1  │ 4624                     │
  │  2  │ 0                        │
  └─────┴──────────────────────────┘

渲染时：模板 %0 → 替换数组[0], %1 → [1], ...
```

---

## 2. 事件结构

### System 部分（所有事件通用）

| 字段 | 类型 | 说明 | 总是存在？ |
|------|------|------|:---------:|
| Provider.Name | 字符串 | 事件提供者名称 | ✅ |
| Provider.Guid | GUID | 提供者唯一标识 | ❌ |
| EventID | uint16 | 事件类型编号 | ✅ |
| Qualifiers | uint16 | Vista 前遗留字段 | ❌ |
| Version | uint8 | 事件 schema 版本 | ❌ |
| Level | uint8 | 严重级别 | ✅ |
| Task | uint16 | 任务类别编号 | ❌ |
| Opcode | uint8 | 操作码编号 | ❌ |
| Keywords | uint64 | 关键字位掩码 | ❌ |
| TimeCreated.SystemTime | FILETIME | 事件时间戳 | ✅ |
| EventRecordID | uint64 | 记录唯一 ID | ✅ |
| ActivityID | GUID | 活动关联 ID | ❌ |
| RelatedActivityID | GUID | 关联活动 ID | ❌ |
| ProcessID | uint32 | 进程 ID | ❌ |
| ThreadID | uint32 | 线程 ID | ❌ |
| Channel | 字符串 | 日志通道名 | ✅ |
| Computer | 字符串 | 机器名 | ✅ |
| Security.UserID | SID | 用户 SID | ❌ |

### Level 值含义

| 值 | 名称 | severity |
|:--:|------|----------|
| 0 | Information | 信息 |
| 1 | Critical | 关键 |
| 2 | Error | 错误 |
| 3 | Warning | 警告 |
| 4 | Information | 信息 |
| 5 | Verbose | 详细 |

### EventData/UserData 部分

事件特有的结构化数据，格式由 provider 定义。例如事件 4624（登录成功）：

```
SubjectUserSid:    S-1-5-18
SubjectUserName:   SYSTEM
TargetUserSid:     S-1-5-21-...
TargetUserName:    Administrator
LogonType:         2
LogonProcessName:  Advapi
```

---

## 3. 消息格式化

### 问题

每个 EventID 对应一段描述文本，但这段文本不存在 EVTX 文件里。
它存在 Provider 的 DLL 里，作为 `RT_MESSAGETABLE` 资源。

```
EVTX 文件:  EventID=4624, EventData=[S-1-5-18, SYSTEM, ...]
              ↓
需要查 DLL: adtschema.dll → RT_MESSAGETABLE → ID=4624
              ↓
消息模板:  "已成功登录帐户。%n%n使用者:%n%t安全ID:%t%t%1..."
              ↓
substitute: %1 → EventData[0]="S-1-5-18", %2 → EventData[1]="SYSTEM"
              ↓
完成: "已成功登录帐户。\n\n使用者:\n\t安全ID:\t\tS-1-5-18..."
```

### 占位符语义

| 占位符 | 替换为 | 例 |
|--------|--------|-----|
| `%1` ~ `%99` | EventData[N-1] | `%1` = EventData[0] |
| `%%` | 字面 `%` | — |
| `%n` | `\r\n` | 换行 |
| `%r` | `\r` | 回车 |
| `%t` | `\t` | 制表符 |
| `%b` | 空格 | — |
| `%!` | 不展开标记 | — |
| `%0` | 截断标记 | 后面的内容丢弃 |

### 如何定位 Provider 的 DLL

Windows 注册表包含完整映射：

```
路径1: EventLog 注册表
  HKLM\SYSTEM\CurrentControlSet\Services\EventLog\{Channel}\{Provider}
  → 值: EventMessageFile = "%SystemRoot%\system32\adtschema.dll"

路径2: WINEVT Publishers（较新）
  HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\WINEVT\Publishers\{GUID}
  → 值: MessageFileName = "%SystemRoot%\system32\adtschema.dll"
```

---

## 4. MUI（多语言用户界面）

### 文件布局

```
C:\Windows\System32\
  adtschema.dll            ← 英文消息（默认，LCID=1033）
  zh-CN\
    adtschema.dll.mui      ← 简体中文（LCID=2052）
  zh-TW\
    adtschema.dll.mui      ← 繁体中文（LCID=1028）
  de-DE\
    adtschema.dll.mui      ← 德文（LCID=1031）
```

### MUI 文件结构

`.mui` 文件和主 DLL 是同一 PE 格式，只是提取了本地化资源。
所以可以用相同的 `pelite` API 解析：找到 `RT_MESSAGETABLE` → 按语言 ID 取消息表。

### 语言回退优先级

```
用户指定 → zh-CN(2052) → zh-TW(1028) → en-US(1033) → 第一条
```

---

## 5. PE 消息表资源

### RT_MESSAGETABLE

DLL 的 PE 资源中，`RT_MESSAGETABLE`（类型 ID=11）包含消息定义。

pelite 解析流程：

```
1. 打开 PE 文件
2. 遍历资源目录
3. 找到 RT_MESSAGETABLE 下的 MESSAGE_RESOURCE_DATA 块
4. 解析 MESSAGE_RESOURCE_BLOCK[]:
     LowID + HighID 定义事件 ID 范围
     每块包含 MESSAGE_RESOURCE_ENTRY[]
5. 每条消息：event_id → message_template_string
```

### WEVT_TEMPLATE

部分 Provider 使用 `WEVT_TEMPLATE` 资源（类型 ID=12）定义参数的名称和类型。
这是我们 `parameters` 字段的来源（`%1` = "SubjectUserSid" 的映射）。

---

## 6. NSA WELM 数据

NSA WELM（Windows Event Log Messages）是美国 NSA 发布的 CC0 公有领域数据集。

### 内容

- CSV 格式，每行一条事件的消息定义
- 覆盖 Win7 → Win10 v1507，26 个版本
- 每版本 ~45K 事件
- 字段：Provider, ID, Value(消息), Level, Task, Opcode, Keywords, **Parameters**, Message, LoggedTo

### 在本项目中的用途

- **种子数据：** `--with-welm` 将 WELM 的英文消息 + parameters + task/opcode/keywords 导入 messages.db
- **补充：** WELM > dump 的本地提取，因为 WELM 覆盖多个旧版本
- **参数名来源：** WELM 的 Parameters 列是 `%1=%SubjectUserSid%n%2=%SubjectUserName%n...` 格式

---

## 7. 同类项目参考

| 项目 | 关键差异 |
|------|----------|
| **dumpevtx (Go)** | 只能 Windows 上 FormatMessageW，跨平台不行 |
| **Chainsaw (Rust)** | 做威胁狩猎，不做消息格式化 |
| **Hayabusa (Rust)** | 做 Sigma 规则匹配，不做消息格式化 |
| **evtx2db (Python)** | 包装 dumpevtx，不可独立运行 |
| **evtx-data (SQLite)** | 数据社区但只英文，无工具链 |

**hotech-evtx 的差异点：** 跨平台消息格式化 + 中英双语 + SQLite 一等公民 + 跳过 JSON 的高性能。

---

## 8. 术语表

| 术语 | 定义 |
|------|------|
| EVTX | Windows 事件日志二进制格式，后缀 .evtx |
| BinXML | EVTX 内部使用的二进制 XML 表示 |
| Substitution Array | 每事件存储的参数值数组，按模板定义的位置填充 |
| Template | BinXML 中定义的结构骨架，包含字段名和占位符 |
| Provider | 事件来源，每个 Provider 有自己的 GUID 和 DLL |
| EventMessageFile | 注册表中指向 DLL 路径的值 |
| RT_MESSAGETABLE | PE 资源中存放消息表的资源类型（ID=11） |
| MUI | 多语言文件，`{base}.dll.mui` |
| FormatMessageW | Windows API，加载 DLL 消息表并替换 %1 %2 |
| Substitute | 纯字符串的 %1 %2 替换（本项目的做法） |
| LCID | 语言代码标识符（1033=英文, 2052=简体中文） |
| WELM | NSA 的 Windows Event Log Messages 数据集 |
| System | 事件中固定的元数据部分（Provider, EventID, Channel 等） |
| EventData | 事件中 Provider 自定义的数据部分 |
| Pelite | Rust 的 PE 文件解析库 |
| Severity | Level 值的可读派生名（关键/错误/警告/信息/详细） |
