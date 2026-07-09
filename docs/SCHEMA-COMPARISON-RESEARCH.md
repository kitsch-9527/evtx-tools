# EVTX 事件查看器工具生态调研与 Schema 对比

> 调研范围：主流的 EVTX 解析、威胁狩猎、时间线生成工具
> 目的：评估 hotech-evtx 当前 events.db schema 的完备性，
> 找出字段缺失和改进方向
> 日期：2026-07-07

---

## 目录

1. [调研工具清单](#1-调研工具清单)
2. [工具能力全景对比](#2-工具能力全景对比)
3. [字段级对比](#3-字段级对比)
4. [Schema 差异分析与改进建议](#4-schema-差异分析与改进建议)
5. [新增数据表建议](#5-新增数据表建议)
6. [结论：优先级路线图](#6-结论优先级路线图)

---

## 1. 调研工具清单

### 1.1 EVTX 解析/格式化工具


| 工具                          | 类型              | 语言   | 输出格式          | 关键能力                     |
| ----------------------------- | ----------------- | ------ | ----------------- | ---------------------------- |
| **hotech-evtx**               | 解析 + 消息格式化 | Rust   | SQLite, CSV       | 跨平台消息格式化，中英双语   |
| **EvtxECmd** (Eric Zimmerman) | 解析 + 字段映射   | C#     | CSV, JSON, SQLite | Map 映射体系，EventData 平铺 |
| **dumpevtx** (Velocidex)      | 解析 + 消息格式化 | Go     | 自定义格式        | Velociraptor 底层            |
| **Plaso** (log2timeline)      | 解析 + 时间线     | Python | SQLite            | 全部位时间线生成             |

### 1.2 威胁狩猎/检测工具


| 工具             | 类型          | 语言       | Sigma 支持 | 关键能力                            |
| ---------------- | ------------- | ---------- | :--------: | ----------------------------------- |
| **Chainsaw**     | 威胁狩猎      | Rust       |  ✅ 原生  | 快速 Sigma 匹配，EventData 搜索     |
| **Hayabusa**     | 时间线 + 狩猎 | Rust       |  ✅ 原生  | 8 种输出 profile, MITRE ATT&CK 标记 |
| **DeepBlueCLI**  | 检测          | PowerShell |     ❌     | 硬编码规则，42+ 检测类型            |
| **Velociraptor** | 平台+狩猎     | Go         |  ✅ 插件  | VQL 数据流，Sigma 后端              |

### 1.3 SIEM/Schema 标准


| 体系            | 类型        |       Schema 标准       | EVTX 字段映射                                 |
| --------------- | ----------- | :----------------------: | --------------------------------------------- |
| **Elastic ECS** | 通用 Schema |  Elastic Common Schema  | `winlog.*` → `event.*`, `user.*`, `host.*`   |
| **Sigma**       | 规则语言    |        自有一致层        | `EventID`, `CommandLine`, `Image`...          |
| **Splunk CIM**  | 通用 Schema | Common Information Model | `EventCode`, `Account_Name`, `CommandLine`... |

---

## 2. 工具能力全景对比

### 2.1 输出字段覆盖

```
字段                        hotech  EvtxECmd  Hayabusa  Plaso  Chainsaw  DeepBlue
──────────────────────────────────────────────────────────────────────────────
event_record_id / RecordID    ✅       ✅       ✅        ✅       ✅       ⚠
timestamp                    ✅       ✅       ✅        ✅       ✅       ✅
provider_name                ✅       ✅       ✅        ✅       ✅       ✅
provider_guid                ✅       ❌       ❌        ❌       ❌       ❌
event_id                     ✅       ✅       ✅        ✅       ✅       ✅
qualifiers                   ✅       ❌       ❌        ✅       ❌       ❌
level                        ✅       ✅       ✅        ✅       ❌       ❌
severity (中文)              ✅       ❌       ❌        ❌       ❌       ❌
task_category                ✅       ❌       ❌        ❌       ❌       ❌
opcode                       ✅       ❌       ❌        ❌       ❌       ❌
keywords                     ✅       ✅       ❌        ❌       ❌       ❌
channel                      ✅       ✅       ✅        ❌       ✅       ✅
computer                     ✅       ✅       ✅        ✅       ✅       ✅
process_id                   ✅       ✅       ✅        ✅       ✅       ❌
thread_id                    ✅       ❌       ❌        ❌       ❌       ❌
user_name                    ✅       ✅       ⚠        ❌       ⚠       ✅
user_sid                     ✅       ✅       ❌        ✅       ❌       ❌
activity_id                  ✅       ❌       ❌        ❌       ❌       ❌
related_activity_id          ✅       ❌       ❌        ❌       ❌       ❌
message (substitute)         ✅       ❌       ❌        ⚠       ❌       ❌
event_data (JSON)            ✅       ❌       ❌        ❌       ❌       ❌
raw_xml (预留)               ⚠       ❌       ❌        ✅       ❌       ❌
file_name                    ✅       ✅       ✅        ❌       ❌       ❌
──────────────────────────────────────────────────────────────────────────────
MapDescription (EvtxECmd)    ❌       ✅       ❌        ❌       ❌       ❌
RuleTitle (Hayabusa)         ❌       ❌       ✅        ❌       ✅       ❌
MitreTactics/Tags           ❌       ❌       ✅        ❌       ⚠       ❌
message_string (Plaso)      ✅       ❌       ❌        ✅       ❌       ❌
```

**图例**: ✅ 有, ❌ 无, ⚠ 部分/可选, (中文) 中文字段

### 2.2 核心能力矩阵


| 能力                   |  hotech-evtx  |   EvtxECmd   |   Hayabusa   |     Plaso     |   Chainsaw   |
| ---------------------- | :-----------: | :-----------: | :-----------: | :------------: | :-----------: |
| **跨平台**             |    ✅ Rust    |  ❌ Windows  |    ✅ Rust    |   ✅ Python   |    ✅ Rust    |
| **消息格式化 (%1 %2)** |  ✅ 4级回退  |      ❌      |      ❌      |    ⚠ 有限    |      ❌      |
| **中英双语**           |      ✅      |      ❌      |      ❌      |       ❌       |      ❌      |
| **消息数据库**         |   ✅ SQLite   |      ❌      |      ❌      |       ❌       |      ❌      |
| **Sigma 规则匹配**     |      ❌      |      ❌      |    ✅ 原生    |       ❌       |    ✅ 原生    |
| **MITRE ATT&CK 标记**  |      ❌      |      ❌      |      ✅      |       ❌       |    ⚠ 有限    |
| **时间线生成**         |      ❌      |      ❌      | ✅ 8 profiles |    ✅ 全位    |      ❌      |
| **EventData 映射**     |  ❌ 原始JSON  |  ✅ Map系统  |  ✅ 规则抽取  | ❌ 原始strings |    ✅ 规则    |
| **CSV 输出**           |      ✅      |      ✅      |      ✅      |       ❌       |      ✅      |
| **SQLite 输出**        |      ✅      |      ✅      |      ❌      |       ✅       |      ❌      |
| **JSONL 输出**         |      ❌      |      ✅      |      ✅      |       ❌       |      ❌      |
| **EVTX 二进制解析**    | ✅ evtx crate |   ❌ 调API   | ✅ evtx crate |   ✅ pyevtx   | ✅ evtx crate |
| **BOM/中文Excel**      | ✅ UTF-8 BOM |      ❌      |      ❌      |       ❌       |      ❌      |
| **Map 文件系统**       |      ❌      | ✅ .map files |      ❌      |       ❌       |      ❌      |

### 2.3 Hayabusa 8 种输出 Profile

Hayabusa 的可配置输出是行业标杆。它定义在 `config/profiles.yaml`：

```
minimal:         Timestamp, Computer, Channel, EventID, Level, RecordID, RuleTitle, Details
standard:       + ExtraFieldInfo
verbose:        + MitreTactics, MitreTags, OtherTags, RuleFile, EvtxFile
all-field-info: Replace Detail+Extra → AllFieldInfo
timesketch:     + timesketch 兼容格式
```

**可借鉴的设计**：hotech-evtx 应该提供类似的 profile 系统，让用户选择输出字段集合。

### 2.4 EvtxECmd 的 Map 映射体系

EvtxECmd 的核心创新是 **.map 文件系统**：

```xml
<!-- Map 文件将原始 EventData 映射为可读字段 -->
<MapEntry>
  <Property>UserName</Property>       <!-- CSV 列名 -->
  <PropertyValue>%domain%\%user%</PropertyValue>  <!-- 组合模板 -->
  <Values>
    <Value name="domain" xpath="/Event/EventData/Data[@Name='SubjectDomainName']"/>
    <Value name="user" xpath="/Event/EventData/Data[@Name='SubjectUserName']"/>
  </Values>
</MapEntry>
```

输出字段：

```
UserName, RemoteHost, ExecutableInfo, PayloadData1-6, MapDescription
```

**可借鉴的设计**：hotech-evtx 可以引入类似的"映射"概念，将 events.event_data JSON 中的关键字段提取为顶级列。

---

## 3. 字段级对比

### 3.1 hotech-evtx 当前 events.db 完整字段清单

```sql
-- 当前 schema (22 列)
CREATE TABLE events (
    event_record_id     INTEGER NOT NULL,    -- ✅ 标准字段
    timestamp           TEXT,                 -- ✅ ISO8601
    provider_name       TEXT,                 -- ✅
    provider_guid       TEXT,                 -- ✅ 独有！其他工具基本没有
    event_id            INTEGER,              -- ✅
    qualifiers          INTEGER,              -- ✅ 独有！经典 provider 需要
    level               INTEGER,              -- ✅
    severity            TEXT,                 -- ✅ 独有！中文严重级别
    task_category       TEXT,                 -- ✅ 独有！DB 可读名
    opcode              TEXT,                 -- ✅ 独有！DB 可读名
    keywords            TEXT,                 -- ✅ 独有！DB 可读名
    channel             TEXT,                 -- ✅
    computer            TEXT,                 -- ✅
    process_id          INTEGER,              -- ✅
    thread_id           INTEGER,              -- ✅ 独有
    user_name           TEXT,                 -- ✅
    user_sid            TEXT,                 -- ✅
    activity_id         TEXT,                 -- ✅ 独有
    related_activity_id TEXT,                 -- ✅ 独有
    message             TEXT,                 -- ✅ 独有！substitute 后完整描述
    event_data          TEXT,                 -- ✅ 独有！完整 EventData JSON
    raw_xml             TEXT,                 -- ⚠ 预留未实现
    file_name           TEXT                  -- ✅
);
```

### 3.2 各工具字段对照表


| hotech-evtx 字段      | EvtxECmd                | Hayabusa                    | Plaso                        | Chainsaw  | Elastic ECS                      |
| --------------------- | ----------------------- | --------------------------- | ---------------------------- | --------- | -------------------------------- |
| `event_record_id`     | RecordNumber            | RecordID                    | record_number                | ⚠        | `event.start` / `event.sequence` |
| `timestamp`           | TimeCreated             | Timestamp                   | creation_time / written_time | Timestamp | `@timestamp`                     |
| `provider_name`       | Provider                | Provider                    | source_name                  | ⚠        | `event.provider`                 |
| `provider_guid`       | ❌                      | ❌                          | provider_identifier          | ❌        | ❌                               |
| `event_id`            | EventId                 | EventID                     | event_identifier             | EventID   | `event.code`                     |
| `qualifiers`          | ❌                      | ❌                          | message_identifier           | ❌        | ❌                               |
| `level`               | Level                   | Level                       | event_level                  | ❌        | `log.level`                      |
| `severity`            | ❌                      | ❌                          | ❌                           | ❌        | ❌                               |
| `task_category`       | ❌                      | ❌                          | ❌                           | ❌        | `event.action` (近似)            |
| `opcode`              | ❌                      | ❌                          | ❌                           | ❌        | ❌                               |
| `keywords`            | Keywords                | ❌                          | ❌                           | ❌        | `event.outcome` (部分)           |
| `channel`             | Channel                 | Channel                     | ❌                           | ⚠        | `log.name` / `winlog.channel`    |
| `computer`            | Computer                | Computer                    | computer_name                | ⚠        | `host.name`                      |
| `process_id`          | ProcessId               | ❌                          | ⚠                           | ❌        | `process.pid`                    |
| `thread_id`           | ❌                      | ❌                          | ❌                           | ❌        | `process.thread.id`              |
| `user_name`           | UserName                | ⚠ (Details)                | ❌                           | ⚠        | `user.name`                      |
| `user_sid`            | UserId                  | ❌                          | user_sid                     | ❌        | `user.id`                        |
| `activity_id`         | ❌                      | ❌                          | ❌                           | ❌        | `winlog.activity_id`             |
| `related_activity_id` | ❌                      | ❌                          | ❌                           | ❌        | `winlog.related_activity_id`     |
| `message`             | ❌ (MapDescription)     | Details/RuleTitle           | message_string (有限)        | ❌        | ❌                               |
| `event_data`          | PayloadData1-6 (映射后) | ExtraFieldInfo/AllFieldInfo | strings (未解析)             | ⚠        | `winlog.event_data.*` (平铺)     |
| `raw_xml`             | Payload                 | ❌                          | xml_string                   | ❌        | `event.original`                 |
| `file_name`           | SourceFile              | EvtxFile                    | ❌                           | ⚠        | `log.file.path`                  |

### 3.3 hotech-evtx **独有**字段（其他工具都没有）

这些是 hotech-evtx 差异化竞争力：


| 字段                                  |   重要性   | 说明                                 |
| ------------------------------------- | :--------: | ------------------------------------ |
| `provider_guid`                       |   ⭐⭐⭐   | 唯一标识 provider，跨机器/版本关联   |
| `qualifiers`                          |    ⭐⭐    | 经典 provider 消息 ID 计算需要       |
| `severity` (中文)                     |   ⭐⭐⭐   | 中文本地化，直观易读                 |
| `task_category` (可读)                |   ⭐⭐⭐   | 优先使用 DB 中的可读任务名           |
| `opcode` (可读)                       |    ⭐⭐    | 同上                                 |
| `keywords` (可读)                     |    ⭐⭐    | 同上                                 |
| `thread_id`                           |     ⭐     | 取证中较少用                         |
| `activity_id` / `related_activity_id` |    ⭐⭐    | 活动关联分析                         |
| `message` (substitute 后)             | ⭐⭐⭐⭐⭐ | **最核心的价值**——完整可读事件描述 |
| `event_data` (完整 JSON)              |  ⭐⭐⭐⭐  | 保留原始数据，下游可按需解析         |

### 3.4 **hotech-evtx 缺失**的常见字段


| 缺失字段                      | 存在于             | 用途                                            | 优先级 |
| ----------------------------- | ------------------ | ----------------------------------------------- | :----: |
| `MitreTactics`                | Hayabusa, Chainsaw | MITRE ATT&CK 战术映射                           | **P0** |
| `MitreTags`                   | Hayabusa           | MITRE ATT&CK 技术/组/软件 ID                    | **P0** |
| `RuleTitle`                   | Hayabusa, Chainsaw | 触发的检测规则名称                              |   P1   |
| `RuleFile`                    | Hayabusa, Chainsaw | 触发的规则文件名                                |   P1   |
| `RuleLevel`                   | Hayabusa           | 规则严重级别 (info/low/med/high/crit)           |   P1   |
| `Details` / `ExtraFieldInfo`  | Hayabusa, EvtxECmd | 从 EventData 提取的关键字段（用户名/IP/进程等） | **P0** |
| `MapDescription`              | EvtxECmd           | 映射后的事件摘要描述                            |   P2   |
| `UserName` (顶级)             | EvtxECmd           | 从 EventData 提取的用户名（Domain\User 格式）   |   P1   |
| `RemoteHost`                  | EvtxECmd           | 远程主机 IP/主机名                              |   P2   |
| `ExecutableInfo`              | EvtxECmd           | 进程/可执行文件路径                             |   P2   |
| `PayloadData1-6`              | EvtxECmd           | 通用 EventData 映射槽（可配置）                 |   P3   |
| `event.outcome`               | Elastic ECS        | 事件结果 (success/failure)                      |   P1   |
| `event.action`                | Elastic ECS        | 事件动作 (user_login, process_created)          |   P1   |
| `event.category`              | Elastic ECS        | 事件类别 (authentication, process)              |   P1   |
| `process_name`                | Chainsaw           | 进程名                                          |   P1   |
| `parent_process`              | Chainsaw           | 父进程                                          |   P2   |
| `ip_address` / `ip_port`      | 多个               | 网络连接信息                                    |   P2   |
| `logon_type`                  | 多个               | 登录类型 (2=交互, 3=网络...)                    |   P1   |
| `object_name` / `object_type` | Elastic            | 被访问对象                                      |   P3   |
| `service_name`                | Elastic            | 服务名                                          |   P2   |
| `registry_key`                | Elastic            | 注册表键                                        |   P2   |
| `tag`                         | Plaso/Timesketch   | 事件标签                                        |   P2   |

---

## 4. Schema 差异分析与改进建议

### 4.1 最重要差距：缺少威胁狩猎字段

hotech-evtx 当前定位是**解析 + 消息格式化**工具，没有规则匹配/检测能力。
但 events.db 是整个分析管道的基础——如果字段不全，下游工具（如 Timesketch、自定义分析脚本）
就需要额外工作来补充。

#### 改进方案 A：增加 MITRE ATT&CK 元数据表

新增 `mitre_attack` 表，将事件 ID → MITRE 战术/技术映射存储为结构化数据：

```sql
CREATE TABLE attack_techniques (
    id       INTEGER PRIMARY KEY,
    tactic   TEXT NOT NULL,         -- e.g. "Credential Access"
    tech_id  TEXT NOT NULL,         -- e.g. "T1110"
    tech_name TEXT NOT NULL          -- e.g. "Brute Force"
);

CREATE TABLE event_attack_map (
    provider_name TEXT NOT NULL,     -- e.g. "Microsoft-Windows-Security-Auditing"
    event_id      INTEGER NOT NULL,  -- e.g. 4625
    tech_id       TEXT NOT NULL REFERENCES attack_techniques(tech_id),
    PRIMARY KEY (provider_name, event_id, tech_id)
);
```

这样下游可以直接 join: `SELECT e.*, a.tactic, a.tech_id FROM events e LEFT JOIN event_attack_map m ON e.event_id=m.event_id ...`

#### 改进方案 B：增加 EventData 关键字段索引

当前 `event_data` 是完整的 JSON 字符串，查询 `SubjectUserName` 需要 `json_extract()`。
创建虚拟列或 materialized view 可以加速：

**方案 Bl — 虚拟列 (SQLite generated columns)**:

```sql
ALTER TABLE events ADD COLUMN eventdata_SubjectUserName TEXT
    GENERATED ALWAYS AS (json_extract(event_data, '$.SubjectUserName'));
-- 参考 EvtxECmd 的 Map 系统 + Hayabusa 的 Details 字段
```

**方案 B2 — 事件类型特定的 Top-N 字段提取**：
不提取全部 EventData 字段（太多），只提取跨事件类型的高价值字段。
参考 Elastic ECS 的映射：

```sql
-- Top-N EventData 映射字段（大多数工具都提取）
CREATE TABLE events_extended (
    event_record_id     INTEGER REFERENCES events(event_record_id),

    -- 用户/账户字段
    subject_user_name   TEXT,
    subject_user_sid    TEXT,
    target_user_name    TEXT,
    target_user_sid     TEXT,
    subject_domain_name TEXT,
    target_domain_name  TEXT,
    subject_logon_id    TEXT,
    target_logon_id     TEXT,

    -- 进程字段
    process_name        TEXT,       -- ProcessName / NewProcessName
    process_id          INTEGER,    -- ProcessId / NewProcessId
    parent_process_name TEXT,       -- ParentProcessName
    parent_process_id   INTEGER,    -- ParentProcessId

    -- 网络字段
    ip_address          TEXT,
    ip_port             INTEGER,
    workstation_name    TEXT,

    -- 登录字段
    logon_type          INTEGER,    -- 2=交互, 3=网络, 4=批处理, ...

    -- 对象字段
    object_name         TEXT,
    object_type         TEXT,
    access_mask         TEXT,

    -- Service/其他
    service_name        TEXT,
    service_type        TEXT,

    PRIMARY KEY (event_record_id)
);
```

### 4.2 第二重要差距：缺少时间线分析字段

Plaso 和 Hayabusa 的时间线能力包含：


| Plaso 字段                       | 说明                          | 建议                               |
| -------------------------------- | ----------------------------- | ---------------------------------- |
| `creation_time` + `written_time` | 两个时间戳（创建/写入）       | ⭐**建议增加 `timestamp_written`** |
| `recovered`                      | 是否从损坏中恢复              | 低优先级                           |
| `data_type`                      | Plaso 的`windows:evtx:record` | ⭐**建议增加 `event_type`**        |
| `tag`                            | 事件标签                      | ⭐**建议增加 tags 表**             |
| `MACB`                           | 改动/访问/创建/生日标志       | 低优先级（文件系统用）             |

### 4.3 第三重要差距：消息数据库的字段不足

当前 `messages.db` 中，`events` 表的 `task_category`/`opcode`/`keywords` 来自消息 DB，
但消息 DB 本身的 CRIM 块解析覆盖不全（详见 CRIM-DIMENSIONS-COMPARISON.md）：


| CRIM 块               | 当前状态 | 缺失影响                  | 优先级 |
| --------------------- | :-------: | ------------------------- | :----: |
| `LEVL` (Level 定义)   | ❌ 未解析 | level 只有数值，无英文名  |   P1   |
| `KEYW` (Keyword 定义) | ❌ 未解析 | keywords 只有 hex bitmask |   P1   |
| `CHAN` (通道定义)     | ❌ 未解析 | 通道名已存在 EVTX，影响小 |   P3   |
| `VMAP/BMAP` (值映射)  | ❌ 未解析 | 枚举值不能显示为字符串    |   P2   |

### 4.4 第四差距：输出格式单一


| 格式    | 状态 | 建议                                                    |
| ------- | :--: | ------------------------------------------------------- |
| SQLite  |  ✅  | ——                                                    |
| CSV     |  ✅  | ——                                                    |
| JSONL   |  ❌  | **高优先级**——Timesketch 导入格式、ELK 管道的标准输入 |
| Parquet |  ❌  | 低优先级——大数据分析                                  |

### 4.5 第五差距：缺少内置规则/检测

hotech-evtx 目前是"纯解析"工具，不包含：

- Sigma 规则引擎
- 内置检测逻辑1
- MITRE ATT&CK 元数据

但注意 `PLAN.md` 定义的**工具边界**：`不做威胁狩猎、不做 Sigma 规则匹配`。
所以规则引擎不应该加入 hotech-evtx，但**提供 MITRE ATT&CK 数据表**是合理的，
因为它仅仅是元数据 enrichment ——不是规则匹配引擎。

---

## 5. 新增数据表建议

### 5.1 events 表增强

```sql
-- 【建议】新增字段到 events 表（向后兼容）
ALTER TABLE events ADD COLUMN timestamp_written TEXT;     -- 事件写入时间（Plaso 的 written_time）
ALTER TABLE events ADD COLUMN event_type TEXT;             -- "windows:evtx:record" 类标记
ALTER TABLE events ADD COLUMN event_action TEXT;           -- 派生动作名 (参照 event.action)
ALTER TABLE events ADD COLUMN event_outcome TEXT;          -- success/failure (event.outcome)
ALTER TABLE events ADD COLUMN process_name TEXT;           -- EventData 高频字段
ALTER TABLE events ADD COLUMN logon_type INTEGER;          -- EventData 高频字段
ALTER TABLE events ADD COLUMN ip_address TEXT;             -- 远程 IP
ALTER TABLE events ADD COLUMN target_user_name TEXT;       -- 目标用户名
```

### 5.2 新增 `event_details` 表（EventData 平铺）

```sql
-- 【建议】EventData 高频字段索引表
-- 避免在 event_data JSON 上反复 json_extract()
CREATE TABLE event_details (
    event_record_id   INTEGER PRIMARY KEY REFERENCES events(event_record_id),
    subject_user_name TEXT,
    subject_user_sid  TEXT,
    target_user_name  TEXT,
    target_user_sid   TEXT,
    subject_domain    TEXT,
    target_domain     TEXT,
    subject_logon_id  TEXT,
    target_logon_id   TEXT,
    process_name      TEXT,
    process_id        INTEGER,
    parent_process    TEXT,
    parent_process_id INTEGER,
    ip_address        TEXT,
    ip_port           INTEGER,
    logon_type        INTEGER,
    logon_process     TEXT,
    authentication_pkg TEXT,
    workstation       TEXT,
    object_name       TEXT,
    object_type       TEXT,
    access_mask       TEXT,
    privilege_list    TEXT,
    service_name      TEXT,
    service_sid       TEXT,
    call_trace        TEXT
);
```

### 5.3 新增 MITRE ATT&CK 表

```sql
-- 【建议】MITRE ATT&CK 技术定义
CREATE TABLE attack_techniques (
    tech_id   TEXT PRIMARY KEY,            -- e.g. "T1110"
    tactic    TEXT NOT NULL,               -- e.g. "Credential Access"
    tech_name TEXT NOT NULL,               -- e.g. "Brute Force"
    url       TEXT                         -- https://attack.mitre.org/techniques/T1110/
);

-- Event ID → MITRE 技术映射
CREATE TABLE event_attack_map (
    provider_name TEXT NOT NULL,
    event_id      INTEGER NOT NULL,
    tech_id       TEXT NOT NULL REFERENCES attack_techniques(tech_id),
    PRIMARY KEY (provider_name, event_id, tech_id)
);
```

### 5.4 新增 `tags` 表

```sql
-- 【建议】事件标签（来源：规则触发、人工标注、自动分类）
CREATE TABLE event_tags (
    id              INTEGER PRIMARY KEY,
    event_record_id INTEGER NOT NULL REFERENCES events(event_record_id),
    tag             TEXT NOT NULL,          -- e.g. "suspicious_logon", "brute_force"
    source          TEXT DEFAULT 'auto',    -- 'auto', 'manual', 'rule'
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_event_tags_record ON event_tags(event_record_id);
CREATE INDEX idx_event_tags_tag ON event_tags(tag);
```

### 5.5 新增 `rule_hits` 表

```sql
-- 【建议】规则匹配结果（当 hotech-evtx 被用作规则引擎时）
-- 如果坚持"不做规则匹配"，此表不需要。
-- 但 schema 预留可以让集成者自己实现。
CREATE TABLE rule_hits (
    id              INTEGER PRIMARY KEY,
    event_record_id INTEGER NOT NULL REFERENCES events(event_record_id),
    rule_name       TEXT NOT NULL,
    rule_level      TEXT NOT NULL,          -- 'info', 'low', 'medium', 'high', 'critical'
    rule_file       TEXT,                   -- 规则源文件
    mitre_tactics   TEXT,                   -- JSON array: ["Credential Access","Lateral Movement"]
    mitre_tags      TEXT,                   -- JSON array: ["T1110","T1078"]
    matched_at      TEXT NOT NULL
);
```

### 5.6 新增 `chunk_metadata` 表

```sql
-- 【建议】EVTX chunk 元数据（低优先级，用于取证完整性分析）
CREATE TABLE chunk_metadata (
    id              INTEGER PRIMARY KEY,
    file_name       TEXT NOT NULL,
    chunk_number    INTEGER NOT NULL,
    first_record_id INTEGER,
    last_record_id  INTEGER,
    first_timestamp TEXT,
    last_timestamp  TEXT,
    record_count    INTEGER,
    is_complete     BOOLEAN DEFAULT TRUE    -- chunk 是否被覆盖/损坏
);
```

---

## 6. 结论：优先级路线图

### 6.1 核心发现

**hotech-evtx 的独特优势**（区别于其他工具）：

1. ✅ **跨平台消息格式化**——独有，其他工具不做
2. ✅ **中英双语**——独有
3. ✅ **消息数据库 + 4级回退**——独有
4. ✅ **完整 EventData JSON**——保留原始数据
5. ✅ **22列丰富 schema**——字段覆盖率领先

**hotech-evtx 的主要差距**：

1. ❌ 没有 EventData 高频字段提取（EvtxECmd 的 Map 系统）
2. ❌ 没有 MITRE ATT&CK 标记（Hayabusa 的优势）
3. ❌ 没有输出 profile 系统（Hayabusa 的 8 种 profile）
4. ❌ 没有 JSONL 输出（Timesketch/ELK 管道）
5. ❌ 没有规则匹配引擎（按设计不同，不应加入）

### 6.2 建议优先级

```
P0（立即做，大幅提升可用性）:
  ├── JSONL 输出格式（Timesketch 导入兼容）
  ├── event_details 表 + 高频 EventData 字段提取
  └── 输出 profile 系统（minimal/standard/verbose）

P1（下一阶段，显著增强分析能力）:
  ├── MITRE ATT&CK 表 + event_attack_map
  ├── timestamp_written 字段
  ├── event_action + event_outcome 派生字段
  └── message DB 补全 LEVL + KEYW 块解析

P2（持续改进）:
  ├── event_tags 表
  ├── rule_hits 表（如果决定进入检测领域）
  └── VMAP/BMAP 值映射

P3（不紧急）:
  ├── chunk_metadata 表
  ├── Parquet 输出
  ├── PayloadData 通用映射槽
  └── 可配置 Map 文件系统
```

### 6.3 向后兼容策略

```sql
-- 所有新增操作使用 ALTER TABLE ADD COLUMN，不破坏现有 DB
-- 新增表不修改现有表的 schema

-- 版本标记
PRAGMA user_version = 2;  -- 当前 events.db: user_version = 1
```

---

## 参考资料

### 工具官方文档


| 工具               | 参考来源                                                                                                                                                                                            |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hayabusa           | [GitHub](https://github.com/Yamato-Security/hayabusa), [Output Profiles](https://github.com/Yamato-Security/hayabusa#output-profiles)                                                               |
| Chainsaw           | [GitHub](https://github.com/WithSecureLabs/chainsaw)                                                                                                                                                |
| EvtxECmd           | [Introducing EvtxECmd](https://malware.news/t/introducing-evtxecmd/29228), [SANS](https://www.sans.org/tools/evtxecmd)                                                                              |
| Plaso              | [WinEvtxFormatter](https://plaso-fork.readthedocs.io/en/latest/_modules/plaso/formatters/winevtx.html), [WinEvtxParser](https://plaso.readthedocs.io/en/latest/_modules/plaso/parsers/winevtx.html) |
| Velociraptor       | [Event Logs docs](https://docs.velociraptor.app/docs/forensic/event_logs/)                                                                                                                          |
| Elastic Winlogbeat | [Winlogbeat fields](https://www.elastic.co/guide/en/beats/winlogbeat/7.4/exported-fields-winlog.html)                                                                                               |
| Elastic ECS        | [ECS fields](https://www.elastic.co/guide/en/beats/winlogbeat/8.18/exported-fields-ecs.html)                                                                                                        |
| Timesketch         | [Import from JSONL/CSV](https://timesketch.org/guides/user/import-from-json-csv/)                                                                                                                   |
| DeepBlueCLI        | [GitHub](https://github.com/sans-blue-team/DeepBlueCLI)                                                                                                                                             |

### 其它参考

- [SigmaHQ — 规则格式与字段映射](https://sigmahq.io/docs/digging-deeper/pipelines)
- [SOC Prime — 自定义字段映射](https://socprime.com/blog/custom-fields-mapping/)
- [Velociraptor Sigma — Windows 事件模型](https://sigma.velocidex.com/docs/models/windows_base_events/)
- [OLACEFS — 事件日志法医分析](https://epubl.ktu.edu/object/elaba:133122013/133122013.pdf)
- hotech-evtx [CRIM-DIMENSIONS-COMPARISON.md](CRIM-DIMENSIONS-COMPARISON.md) — CRIM 块解析覆盖度对比
- hotech-evtx [RESEARCH-WEVT_TEMPLATE.md](RESEARCH-WEVT_TEMPLATE.md) — CRIM/MUI 消息解析调研
- hotech-evtx [LEARNING-GUIDE.md](LEARNING-GUIDE.md) — 技术学习指南
