# `--import-dll` 设计文档

## 问题

msgdb-dump 在构建机运行，无法预知目标机的第三方软件。当解析远程 EVTX 时，
provider（如 MySQL）的消息不在 msg-db 中，导致 message 为空。

## 方案：DLL 离线导入 + Provider 别名

### 核心思路（与 Windows 一致）

Windows 的消息解析是 **DLL 中心** 的：
```
provider "MySQL" → reg → EventMessageFile="mysqld.exe"
                  → LoadLibrary(mysqld.exe)
                  → FormatMessage(hModule, (qualifiers<<16)|event_id)
```

所有指向同一 DLL 的 provider 共享消息表。

### Schema：新增 `provider_aliases` 表

```sql
CREATE TABLE provider_aliases (
    provider_name TEXT NOT NULL,
    dll_id        INTEGER NOT NULL REFERENCES dll_entries(dll_id),
    PRIMARY KEY (provider_name, dll_id)
);
```

用途：把 EVTX 中的 provider 名映射到 msg-db 中的 DLL 消息。

### 数据流

```
用户收集远程 DLL（如 mysqld.exe）
  → msgdb-dump --import-dll <DIR> [--alias MySQL=mysqld]
    → ExtractAllFromPE(DLL) → 消息表
    → 创建 provider，名 = DLL 文件名 (如 "mysqld")
    → 写入 provider_aliases (如 "MySQL" → dll_id of mysqld)
    → 写入 dll_messages

用户执行 evtx-tools parse --msg-db msg.db
  → warmup 时检查 aliases
    → ("MySQL", 100) → alias 发现 → ("mysqld", 100) 也加入 warmup
  → resolve 时
    → cache.get("MySQL", 100) → miss
    → cache.get("mysqld", 100) → hit → 返回消息
```

### CLI 语法

```bash
# 基本用法：导入 DLL 目录
msgdb-dump --out-dir DIR --lang 1033,2052 --import-dll ./remote_dlls

# 添加别名
msgdb-dump --out-dir DIR --lang 1033,2052 \
  --import-dll ./remote_dlls \
  --alias MySQL=mysqld \
  --alias MSSQLSERVER=sqlservr

# 合并到已有 DB
msgdb-dump --update --out-dir DIR --import-dll ./remote_dlls
```

### 跨项目变更

| 项目 | 变更 |
|------|------|
| **msgdb-dump** | `--import-dll`, `--alias` CLI flag |
| | `provider_aliases` 表 DDL（schema v4 新增） |
| | `importDLLDir()` 函数 |
| | `writeAliases()` 函数 |
| | DLL 文件名 → provider 名逻辑 |
| **evtx-tools** | warmup 时加载 aliases |
| | 别名感知的 warmup（把 alias 展开为 canonical name） |
| | 新增 `provider_aliases` 表的 SQLite 查询 |

## 参考

- `provider_aliases` 表完整 DDL 见 [schema.md](schema.md)
- `--import-dll` CLI 使用示例见 [README.md](../README.md#远程-dll-导入)
