# events.db Schema

> parse 命令的输出数据库，每条事件一行，20 列。

---

## 文件

events.db（扩展名可自定义，内容格式固定）

## DDL

```sql
CREATE TABLE events (
    event_record_id     INTEGER NOT NULL,
    timestamp           TEXT,                  -- ISO8601
    provider_name       TEXT,
    provider_guid       TEXT,
    event_id            INTEGER,
    level               INTEGER,
    severity            TEXT,                  -- level 派生可读名
    task_category       TEXT,
    opcode              TEXT,
    keywords            TEXT,
    channel             TEXT,
    computer            TEXT,
    process_id          INTEGER,
    thread_id           INTEGER,
    user_name           TEXT,
    user_sid            TEXT,
    activity_id         TEXT,
    related_activity_id TEXT,
    message             TEXT,                  -- substitute 后完整描述
    event_data          TEXT,                  -- JSON, key 来自 parameters
    raw_xml             TEXT,                  -- 原始 XML，默认 NULL
    file_name           TEXT                   -- 来源 .evtx 文件名
);

CREATE INDEX idx_events_timestamp ON events(timestamp);
CREATE INDEX idx_events_level ON events(level);
CREATE INDEX idx_events_channel ON events(channel);
CREATE INDEX idx_events_provider ON events(provider_name);
CREATE INDEX idx_events_event_id ON events(event_id);
```

## 字段说明

### 来源分类

| 来源 | 字段 |
|------|------|
| 直接从 .evtx 二进制读取 | event_record_id、timestamp、channel、computer |
| 从 System.BinXML 提取 | provider_name、provider_guid、event_id、level、process_id、thread_id、user_name、user_sid、activity_id、related_activity_id |
| 从 messages.db 查询 | task_category、opcode、keywords |
| 派生计算 | severity（level → 中文名） |
| substitute 生成 | message（模板 + EventData 替换） |
| 序列化生成 | event_data（JSON）、raw_xml（可选） |
| 文件来源 | file_name |

### 特殊字段

#### severity

从 level 派生的可读名称，供 CSV 直接看：

| level | severity |
|:-----:|----------|
| 0 | 信息 |
| 1 | 关键 |
| 2 | 错误 |
| 3 | 警告 |
| 4 | 信息 |
| 5 | 详细 |

#### event_data

JSON 字符串，key 来自 messages.parameters 的映射。
有 parameters 时：`{"SubjectUserSid":"S-1-5-18","SubjectUserName":"SYSTEM"}`
无 parameters 时：`{"param1":"value1","param2":"value2"}`（原始字段名）

#### raw_xml

默认 NULL，需 `--with-xml` 开启。
使用 `evtx::EvtxRecord::into_xml()` 渲染，与 Windows 事件查看器 XML 基本一致。

## CSV 输出

列与 events 表一致，无索引。UTF-8 BOM，逗号分隔，双引号转义。

```csv
event_record_id,timestamp,provider_name,event_id,level,severity,message,...
1,2024-01-01T00:00:00Z,Microsoft-Windows-Security-Auditing,4624,0,信息,已成功登录帐户...
```

## 查询示例

```sql
-- 最近登录失败事件
SELECT timestamp, computer, severity, message
FROM events
WHERE event_id = 4625
ORDER BY timestamp DESC
LIMIT 10;

-- 按 severity 统计
SELECT severity, COUNT(*) as cnt
FROM events
GROUP BY severity
ORDER BY cnt DESC;

-- 搜索包含"管理员"的事件（message 已格式化，可直接搜索）
SELECT timestamp, event_id, message
FROM events
WHERE message LIKE '%管理员%';

-- 按时间段 + channel 过滤
SELECT timestamp, provider_name, event_id, message
FROM events
WHERE channel = 'Security'
  AND timestamp >= '2024-01-01'
  AND timestamp < '2024-02-01'
ORDER BY timestamp ASC;

-- event_data JSON 查询（SQLite JSON 函数）
SELECT event_record_id,
       json_extract(event_data, '$.TargetUserName') as target_user,
       message
FROM events
WHERE event_id = 4624
  AND json_extract(event_data, '$.LogonType') = '10';
```
