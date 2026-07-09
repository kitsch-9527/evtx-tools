# dlldb Schema — DLL 级别消息数据库

> 一个文件 = 一个 DLL × 一种语言 × 一个版本。文件名唯一标识身份。

---

## 文件命名

```
{dll_name}.{lang}.{pe_version}.db

dll_name    = DLL 基名降全小写
lang        = ISO 语言代码（en-US, zh-CN, zh-TW, de-DE, ...）
pe_version  = PE 文件的 4 段版本号
```

例：`adtschema.zh-CN.10.0.26220.1000.db`

## DDL

```sql
-- 极简：文件名已经标识了 DLL 名和版本
CREATE TABLE messages (
    event_id      INTEGER NOT NULL,       -- 事件 ID
    message       TEXT NOT NULL,           -- 消息模板（含 %1 %2）
    level         INTEGER,                -- 0/1/2/3/4/5
    task          TEXT,                   -- Task Category 名称
    opcode        TEXT,                   -- Opcode 名称
    keywords      TEXT,                   -- Keywords 显示名
    parameters    TEXT,                   -- JSON: {"%1":"SubjectUserSid",...}
    provider_name TEXT,                   -- 仅当此 DLL 包含多个 provider 时使用
    PRIMARY KEY (event_id)
);
```

## 为什么这么简单

| 传统设计 | dlldb 设计 |
|----------|-----------|
| 需要 providers 表 | 不需要—文件名标识 DLL |
| 需要 language_id | 不需要—文件名标识语言 |
| 需要 build_number | 不需要—文件名标识版本 |
| 需要 meta 表 | 不需要—身份在文件名 |

## 与系统汇总 DB 的关系

```
dlldb/adtschema.en-US.10.0.26220.1000.db
     ↓ 合并到
hotech-msg-win11-24h2-26100.db
  messages 表中: provider_name='Microsoft-Windows-Security-Auditing',
                 language_id=1033, source='dump'

解析时 resolver 从系统汇总 DB 读取，不直接读 dlldb。
dlldb 是数据源层，用于版本管理和社区贡献。
```

## 文件大小参考

| DLL | 消息数 | 文件大小 |
|-----|--------|----------|
| adtschema.dll | 544 | ~180 KB |
| authz.dll | 89 | ~30 KB |
| kernelbase.dll | 210 | ~70 KB |
| sysmon.exe v15 | 23+ | ~15 KB |
