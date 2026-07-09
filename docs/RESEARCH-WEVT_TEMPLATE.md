# EventManifest / WEVT_TEMPLATE 消息解析调研

> 目标：让 System.evtx、Application.evtx 的事件描述也能被 hotech-evtx 解析。
> 当前 dump 命令只支持 RT_MESSAGETABLE（type 11），System 日志使用 EventManifest 系统。

---

## 背景

E2E 测试显示：

| EVTX | 事件数 | 消息解析率 | 说明 |
|------|-------|:--------:|------|
| Security.evtx | 25,532 | **99.98%** | adtschema.dll 使用 MESSAGETABLE ✅ |
| System.evtx | 33,941 | **0%** | system-events.dll 使用 EventManifest ❌ |

## 问题拆分

```
不能解析 System.evtx 消息的根因可能是以下之一（需要调查）:

  A. system-events.dll 有 WEVT_TEMPLATE → 但它不含消息文本
     → 消息文本来自哪里？
     
  B. system-events.dll 有 WEVT_TEMPLATE → 包含消息文本
     → 需要解析 CRIM 二进制 XML 格式提取文本
     
  C. system-events.dll 完全不含消息
     → Windows 使用系统内置的 .man manifest 文件
     → 需要 FindStringResource / FormatMessage API
```

---

## 结论：调查已完成，已修复 ✅

### 实际资源探查发现

通过在 Windows 机器上运行 `pelite` 对 system-events.dll 和 netevent.dll
进行实际 PE 资源扫描，发现：

| 文件 | Resource Types | RT_MESSAGETABLE? |
|------|---------------|:----------------:|
| `system-events.dll` | MUI, **WEVT_TEMPLATE**, RT_VERSION | ❌ |
| `en-US\xxx.dll.mui` | MUI, **RT_MESSAGETABLE**, RT_VERSION | **✅ 616 条** |
| `zh-CN\xxx.dll.mui` | MUI, **RT_MESSAGETABLE**, RT_VERSION | **✅ 616 条** |
| `netevent.dll` | MUI, **WEVT_TEMPLATE**, RT_VERSION | ❌ |
| `en-US\netevent.dll.mui` | MUI, **RT_MESSAGETABLE**, RT_VERSION | **✅ 1278 条** |
| `zh-CN\netevent.dll.mui` | MUI, **RT_MESSAGETABLE**, RT_VERSION | **✅ 1278 条** |

### 消息文本实际位置

```
┌─────────────────────────────────────────────────┐
│ system-events.dll                                │
│  ├── WEVT_TEMPLATE (CRIM, 664KB)                │
│  │   ├── 46 个 Provider 的事件定义                │
│  │   │     event_id=1.. → message_identifier     │
│  │   │     + level/task/opcode 元数据             │
│  │   └── ❌ 没有消息文本                           │
│  └── MUI resource (small, metadata only)          │
│                                                   │
│ en-US\Microsoft-Windows-System-Events.dll.mui     │
│  └── RT_MESSAGETABLE (type 11)                   │
│      └── 616 条消息文本 ✅                        │
│          keyed by resource_id (= message_identifier)
└─────────────────────────────────────────────────┘
```

### 已实现的修复（历史记录，dumper 已用 Go 重写）

在原 Rust dumper 的 `cmd_dump.rs` 中增加的 MUI+CRIM 解析路径：

1. **`extract_raw_crim_blobs()`** — 从 PE 提取原始 CRIM 字节
2. **`parse_crim_evnt_blocks_by_guid()`** — 按 GUID 提取特定 provider 的事件定义
3. **`parse_crim_provider_data()`** — 提取完整 provider 数据（events + tasks + opcodes + templates）
4. **`mui_helper`** — MUI 伴生文件路径查找 + LCID→语言代码映射
5. **Join 逻辑** — CRIM event_id ↔ MUI message_text 通过 message_identifier 关联

### CRIM 字符串格式关键发现

CRIM 内部的字符串使用**4 字节 u32 长度前缀**（包含自身），格式如下：

```
offset+0: u32 alloc_size (包含 4 字节前缀自身)
offset+4: UTF-16LE 字符数据 (alloc_size - 4 字节)
          → 解码直到第一个 null (0x0000)
```

对照实现：`evtx::wevt_templates::manifest::util::read_sized_utf16_string()`

**常见误解**：很多文档和参考实现写的是 2 字节 u16 长度前缀。
实际 CRIM 格式使用 4 字节 u32。

### 测试结果

```
dump Microsoft-Windows-Audit-CVE:
  MUI+CRIM: 1278 MUI msgs, 2 matched CRIM events, 2 resolved ✅

dump Microsoft-Windows-SoftwareRestrictionPolicies:
  MUI+CRIM: 1278 MUI msgs, 6 matched CRIM events, 6 resolved ✅

dump Microsoft-Windows-Kernel-Power:
  MUI+CRIM: 399 MUI msgs, 62 matched CRIM events, 77 resolved ✅
  enrichment: 77 tasks, 77 opcodes, 75 parameter sets

dump Microsoft-Windows-AppModel-Runtime:
  MUI+CRIM: 130 events with task/opcode enrichment ✅
  enrichment: 31 tasks, 31 opcodes, 129 parameter sets
```

### PowerShell 对比验证

| 比较项 | 本机 Get-WinEvent | hotech-evtx DB |
|--------|-----------------|----------------|
| Event 172 消息 | `Connectivity state in standby: Disconnected...` | `Connectivity state in standby: %1, Reason: %2` ✅ |
| Event 41 消息 | `The system has rebooted without cleanly shutting down...` | 完全一致 ✅ |
| Task 名称 | `(空)` | `StandbyConnectivityUpdate`, `DirtyTransition` ⭐ |
| Opcode 名称 | `Info` | `win:Info` ✅ |
| 参数映射 | 无 | `{"1":"State","2":"Reason"}` ⭐ |

注：PowerShell 显示的是替换后的文本，hotech-evtx 存的是模板字符串（含 %1 %2），
`parse` 命令的 substitute 引擎再做替换。这是设计如此（PLAN.md C3 约束）。

### 未实现的增强

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 无 GUID 回退优化 | P3 | 没有 GUID 的 provider 需要名字匹配替代 |
| Level 名称提取 | P3 | 从 CRIM LEVL 块解析 level 显示名（severity 已有映射） |

## 行动项

| # | 任务 | 前置条件 |
|---|------|----------|
| 1 | 写一个 dump 测试，用 system-events.dll 验证 WEVT_TEMPLATE 内容 | 无 |
| 2 | 在 Windows 上运行 pecheck / dumpbin 查看 DLL resource 目录 | 无 |
| 3 | 决定: CRIM 自研解析 vs libfwevt 绑定 vs evtx wevt_templates | #1, #2 |
| 4 | 实现 CRIM 解析提取消息文本 | #3 |
| 5 | 处理 netevent.dll（硬编码消息 vs FormatMessage） | #4 |
| 6 | 合并到 extractor.rs | #4, #5 |

## 参考资料

| 项目 | 许可 | 说明 |
|------|------|------|
| [libyal/libfwevt](https://github.com/libyal/libfwevt) | LGPL 2.1+ | C 库，完整 WEVT_TEMPLATE/CRIM 格式解析 |
| [omerbenamram/evtx wevt_templates](https://docs.rs/evtx/latest/evtx/wevt_templates/index.html) | MIT | Rust 模块，离线模板提取 |
| [libfwevt 格式文档](https://github.com/libyal/libfwevt/blob/main/documentation/Windows%20Event%20manifest%20binary%20format.asciidoc) | — | CRIM 二进制 XML 格式的完整文档 |
| NSA WELM events.csv | CC0 | 包含 System 日志的 Level/Task 映射（早期 Win7/Win8） |
| Schuster 2007 DFRWS | 学术 | EVTX BinXML 格式基础研究 |
