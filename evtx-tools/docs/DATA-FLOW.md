# 数据流

> dump 和 parse 两个命令的完整数据流，包括缓存、预热、降级路径。

---

## 1. dump 数据流

```
┌─────────────────────────────────────────────────────────────────┐
│ 开始 dump                                                        │
│ --out-dir, --lang, --split-dlls, --with-welm, --update          │
└──────────────────┬──────────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────────┐
│ 读注册表 OS 信息                                                 │
│ HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion               │
│   → product = "win11"                                           │
│   → displayver = "24h2"                                         │
│   → build = "26100"                                             │
└──────────────────┬──────────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────────┐
│ 检查 --update                                                    │
│ hotech-msg-win11-24h2-26100.db 已存在且完整？                     │
│   │                                                              │
│   ├── 是 + 没有 --split-dlls 变化 → 跳過（exit 0, "up to date"）  │
│   └── 否 → 继续                                                  │
└──────────────────┬──────────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────────┐
│ 枚举注册表 Provider                                              │
│ 两个路径:                                                        │
│   1. HKLM\SYSTEM\...\Services\EventLog\{Channel}\{Provider}     │
│      → EventMessageFile 值（DLL 路径）                           │
│      → 包含 channel 信息（Security/System/Application/...）     │
│   2. HKLM\SOFTWARE\...\WINEVT\Publishers\{GUID}                 │
│      → MessageFileName 值                                         │
│      → Provider 显示名 + GUID                                   │
│                                                                  │
│ 去重: 同一 DLL 可能被多个注册表路径引用，只处理一次               │
└──────────────────┬──────────────────────────────────────────────┘
                   │
                   ▼
         ┌─── 对每个唯一 DLL ─────────────────────────────┐
         │                                                 │
         ▼                                                 │
┌─────────────────────────┐                                │
│ pelite 解析 PE 文件      │                                │
│ 1. 打开 DLL              │                                │
│ 2. 读 PE 版本信息         │                                │
│    → FileVersion: 10.0.26220.1000                        │
│ 3. 找 RT_MESSAGETABLE 资源 │                               │
│    → 遍历 MESSAGE_RESOURCE_DATA                          │
│    → HashMap<event_id, template>                         │
│ 4. 日志级别/任务/操作码/关键字                               │
│    → 从消息表数据中提取                                     │
│ 5. WEVT_TEMPLATE（如有）                                  │
│    → 提取参数名映射                                       │
└──────────┬──────────────┘                                │
           │                                                │
           ▼                                                │
┌─────────────────────────┐                                │
│ MUI 检查                 │                                │
│ 同目录下是否存在 zh-CN/DLL_NAME.dll.mui？                  │
│ 是否存在 zh-TW/xxx.dll.mui？                              │
│   │                                                      │
│   ├── 存在 → 重复上述 pelite 解析步骤（语言版本）            │
│   │         结果标记 language_id = 2052/1028              │
│   └── 不存在 → 跳过                                      │
└──────────┬──────────────┘                                │
           │                                                │
           ▼                                                │
┌──────────────────────────┐                               │
│ 写入 DLL 级别 DB（如 --split-dlls）                        │
│ 文件名: dlldb/adtschema.en-US.10.0.26220.1000.db         │
│ 写入:   messages 表（同 DLL 内所有消息）                    │
└──────────┬──────────────┘                               │
           │                                                │
           ▼                                                │
┌──────────────────────────┐                               │
│ 累加到内存                 │                               │
│ HashMap<(provider_name,   │                               │
│   event_id, lang_id),     │                               │
│   MessageRow>            │                               │
└──────────────────────────┘                               │
         └────────── 循环结束 ─────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────────┐
│ 写入系统汇总 DB                                                  │
│ 文件名: hotech-msg-win11-24h2-26100.db                          │
│ providers 表: 写入所有 provider                                   │
│ messages 表: 批量写入内存 HashMap 中的所有数据                      │
│ _meta 表: 写入 OS 版本、提取时间、统计                             │
│ _languages: 写入提取的语言列表                                     │
└──────────────────┬──────────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────────┐
│ WELM 合并（如 --with-welm）                                      │
│ 1. 打开 data/welm/welm_combined.db                              │
│ 2. 对每个 provider+event_id 组合:                                │
│     系统中已有此消息？→ 跳过（本地优先）                           │
│     系统中没有？→ INSERT INTO messages（source='welm'）         │
│ 只有 WELM 已有的英文消息，不会覆盖本地中文消息                     │
└──────────────────┬──────────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────────┐
│ 合并 all.db（如 --merge-existing）                                │
│ 1. 扫描输出目录下所有 hotech-msg-*.db（排除 all.db）              │
│ 2. 读取每个单版本 DB 的 messages 表                                │
│ 3. 写入 all.db 的 messages 表（追加 build_number 列）            │
│ 4. 更新 _versions 表                                            │
└─────────────────────────────────────────────────────────────────┘
                   │
                   ▼
              dump 完成
```

---

## 2. parse 数据流

```
┌─────────────────────────────────────────────────────────────────┐
│ 开始 parse                                                       │
│ <INPUT>, --msg-dir / --db, -o, --format, --with-xml, --threads │
└──────────────────┬──────────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────────┐
│ 匹配消息库                                                       │
│ --db 指定了？→ 直接打开                                          │
│ --msg-dir 指定了？→                                              │
│   从 .evtx 文件头读 build_number（当前: offset 0x40 读取）        │
│   匹配规则:                                                      │
│     1. hotech-msg-{product}-{displayver}-{build}.db → 精确匹配   │
│     2. hotech-msg-all.db                               → 合并库  │
│     3. 都没有                                      → 降级       │
│ 没传消息库参数？→ 降级                                            │
└──────────────────┬──────────────────────────────────────────────┘
                   │
                   ▼
      ┌──────────────────────┐
      │ 有消息库？            │
      ├── 是 ─────────────────┤
      │                      │
      ▼                      │
┌─────────────────┐          │
│ MessageDb::open │          │
│ (只打开，不预热)  │          │
│                   │          │
│ 预热策略: 按需懒加载 │          │
│ 非旧版"收集全部对" │          │
└────────┬────────┘          │
         │                   │
         ▼                   │
┌─────────────────────────────┘
│ 单遍解析（无第一遍收集）:
│
│ 1. EvtxParser::from_path()
│ 2. 创建输出文件（events.db / .csv）
│
│ ┌── records_json_value() 循环 （单遍）────┐
│ │                                          │
│ │ 对每条 EvtxRecord:                        │
│ │                                          │
│ │ A. extract_fields(value):                │
│ │    records_json_value() → 直接 Value     │
│ │    (跳过 records_json_bytes +            │
│ │     serde_json::from_slice 往返)          │
│ │                                          │
│ │ B. 字段提取路径（evtx 0.12）:              │
│ │    provider_name = System.Provider.Name   │
│ │    event_id = System.EventID              │
│ │    level = System.Level                   │
│ │    channel = System.Channel              │
│ │    computer = System.Computer             │
│ │    process_id = Execution.ProcessID       │
│ │    thread_id = Execution.ThreadID         │
│ │    ⚠️  Security = null（evtx 0.12！）      │
│ │    → user_sid = EventData.SubjectUserSid  │
│ │    → user_name = EventData.SubjectUserName│
│ │    event_data = EventData（平铺对象）      │
│ │    (兼容旧 Data 数组格式)                  │
│ │                                          │
│ │ C. 懒加载预热（按需，仅首次遇到时）:        │
│ │    if !warmed_up.contains(key):           │
│ │      db.warmup(&[(provider, event_id)])   │
│ │      warmed_up.insert(key)               │
│ │                                          │
│ │ D. 消息格式化:                            │
│ │    row = cache.get(key)                   │
│ │    if row:                                │
│ │      替换策略（4 级回退）:                 │
│ │        1. 平铺对象 + 命名参数映射          │
│ │        2. 平铺对象 + 位置回退             │
│ │        3. Data 数组 + 命名参数映射         │
│ │        4. #text 数组位置映射               │
│ │      message = substitute(template,        │
│ │        extract_subst_params(...))          │
│ │      优先使用 DB 的 task/opcode/keywords   │
│ │    else: 留空                              │
│ │                                          │
│ │ E. 派生:                                 │
│ │    severity = LEVEL_MAP[level]            │
│ │    (0→信息 1→关键 2→错误 3→警告 5→详细)    │
│ │                                          │
│ │ F. 推入 batch → 20000 条 → flush          │
│ └──────────────────────────────────────────┘
│
│ 3. finalize: 剩余 batch flush
│ 4. ANALYZE（SQLite）
│
└───────────────────────────────────────────┘
                   │
                   ▼
              parse 完成
```

### 关键变更说明

| 项目 | 旧（v0.9） | 新（v1.0） |
|------|-----------|-----------|
| 解析 API | `records_json_bytes()` + `from_slice()` | `records_json_value()` 直接得 Value |
| 解析遍数 | 2 遍（收集 pair + 解析写入） | 1 遍（懒加载预热） |
| EventData | 找 `EventData.Data[]` 数组 | 平铺 Key-Value 对象 |
| user_name | 硬编码 `None` | 从 `SubjectUserName` 提取 |
| user_sid | 从 `System.Security.UserID` | 从 `SubjectUserSid`（fallback Security） |
| 任务/操作码 | 仅 EVTX 值为 None 时覆盖 | **始终优先使用 DB 值** |
| 消息替换策略 | 仅 Data 数组 + 命名参数 | 4 级回退（含位置映射） |

---

## 3. merge 数据流

```
输入: 两个或多个 hotech-msg-*.db 文件
      （单版本或多版本均可）

1. 读取每个源 DB:
   a. 从 _meta 表读 build_number
   b. 读 providers 表（汇总合并）
   c. 读 messages 表，每条记录追加 build_number
      如果源是单版本：build_number = _meta 中的值
      如果源是 all.db：build_number = 原有值

2. 写入目标 DB（hotech-msg-all.db）:
   a. providers 表 ← DISTINCT 所有源 provider
   b. messages 表 ← UNION ALL，每条含 build_number
   c. _versions 表 ← 每个源的 build_number + 统计
   d. _meta 表 ← 合并后的统计

冲突策略:
  同 (provider, event_id, lang, build) → 保留 priority 高的
  同 provider 不同 build → 都保留（不同行）
```

---

## 4. 降级路径总览

```
无消息库时的降级:
  输入 ──→ 字段提取 ──→ substitute(None) ──→ message=NULL
                 ↓                          task=NULL
            severity 正常                     opcode=NULL
            其他字段正常                      keywords=NULL
                                             event_data=raw JSON
                                             
无 WELM 时的降级:
  dump --out-dir ./msg_dbs/ --lang 2052  → 只有本机中文消息
  没有英文种子 → 解析时部分事件的 message=NULL

无对应 build 时的降级:
  parse --db hotech-msg-win10-22h2-19045.db
        运行在 Win11 的 .evtx（build=26100）上
  → 使用 19045 的消息（build 不完全匹配但能用）
  → 部分 Windows 11 新增事件的 message=NULL
```

---

## 5. 关键路径时序

```
┌─────────────────────────────────────────────────────────────┐
│ Security.evtx (100万事件, 1.5GB) 解析时间分布 v1.0           │
│                                                             │
│ 打开 .evtx + 预热消息库:     ~30ms                           │
│ 逐事件解析 (serialized):     ~18.7s (18.7μs × 1M)           │
│   ├── evtx 内部解析:          12μs × 1M = 12s               │
│   ├── JSON 渲染 + 解析:       5μs × 1M = 5s                 │
│   ├── 字段提取:               1μs × 1M = 1s                 │
│   ├── substitute:             0.5μs × 1M = 0.5s             │
│   └── SQLite batch flush:     0.2μs × 1M = 0.2s             │
│ 最终 SQLite 优化（ANALYZE）:   ~100ms                        │
│ ───────────────────────────────────────                     │
│ 总计:                            ~18.8s                     │
└─────────────────────────────────────────────────────────────┘
```
