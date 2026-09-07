# EVET 集成使用指南

## 概述
本指南介绍如何将 evtx-tools 和 msgdb-dump 两个工具结合使用，构建完整的 Windows 事件日志分析工作流。

## 工作流程

```
┌─────────────────┐    消息库    ┌─────────────────┐
│   msgdb-dump    │ ──────────▶ │   evtx-tools    │
│   (Go)          │             │   (Rust)        │
│                 │             │                 │
│ 提取消息模板    │             │ 解析 EVTX 文件  │
│ 生成 .db 文件   │             │ 使用消息库替换  │
└─────────────────┘             └─────────────────┘
```

## 步骤 1: 提取消息库

### 在 Windows 机器上提取消息模板

```bash
# 提取英语和中文消息
msgdb-dump-x64.exe --out-dir ./msg_dbs/ --lang 1033,2052

# 提取特定提供程序
msgdb-dump-x64.exe --out-dir ./msg_dbs/ --providers "*Security*"

# 增量导出（只提取第三方软件消息）
msgdb-dump-x64.exe --baseline ./baseline.json --tag third-party
```

### 输出文件
- `hotech-msg-Windows_10_Education-25H2-26220.7052.db` - 系统级消息数据库
- `hotech-msg-Windows_10_Education-25H2-26220.7052.json` - 基线文件（可选）

## 步骤 2: 解析 EVTX 文件

### 基本解析

```bash
# 使用消息库解析 Security.evtx
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o security_output

# 解析多个文件
evtx-tools parse System.evtx --msg-dir ./msg_dbs/ -o system_output
evtx-tools parse Application.evtx --msg-dir ./msg_dbs/ -o application_output
```

### 带场景分析

```bash
# 安全分析（登录审计、权限提升）
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o security_output \
  --profile security

# 持久化检测（服务安装、计划任务）
evtx-tools parse System.evtx --msg-dir ./msg_dbs/ -o system_output \
  --profile persistence

# 多 profile 组合
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o full_output \
  --profile security --profile remote --profile process
```

### 自定义规则

```bash
# 使用自定义规则目录
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o custom_output \
  --rules-dir ./my-rules/

# 规则文件示例（my-rules/suspicious_logins.yaml）
# name: "Suspicious Logins"
# table: "suspicious_logins"
# event_ids: [4624, 4625]
# channels: ["Security"]
# message_like: ["%失败%", "%错误%"]
```

## 步骤 3: 分析结果

### SQLite 查询示例

```bash
# 查看所有表
sqlite3 security_output ".tables"

# 查看事件统计
sqlite3 security_output "SELECT COUNT(*) FROM events;"

# 查看安全事件
sqlite3 security_output "SELECT * FROM security_events LIMIT 10;"

# 查看统计表
sqlite3 security_output "SELECT * FROM stat_event_id ORDER BY count DESC LIMIT 10;"
```

### 时间跳变检测

```sql
-- 检测时间是否被篡改
SELECT a.hour_bucket, a.max_sequence_number AS t10_max,
       b.hour_bucket, b.min_sequence_number AS t11_min
FROM stat_event_id_hourly a
JOIN stat_event_id_hourly b ON a.event_id = b.event_id
  AND a.hour_bucket < b.hour_bucket
  AND a.max_sequence_number > b.min_sequence_number
WHERE a.event_id = 4624;
```

### 威胁狩猎查询

```sql
-- 查找可疑登录
SELECT event_time, user_name, ip_address, message
FROM security_events
WHERE event_id IN (4624, 4625)
  AND message LIKE '%失败%';

-- 查找服务安装
SELECT event_time, user_name, message
FROM persistence_events
WHERE event_id = 7045;

-- 查找 PowerShell 执行
SELECT event_time, user_name, message
FROM powershell_events
WHERE event_id IN (4103, 4104, 4105, 4106);
```

## 高级用法

### 批量处理

```bash
# 批量解析目录下所有 EVTX 文件
for file in *.evtx; do
  evtx-tools parse "$file" --msg-dir ./msg_dbs/ -o "${file%.evtx}" \
    --profile security --profile persistence
done
```

### 合并消息库

```bash
# 合并多个消息库
evtx-tools merge ./msg_dbs/ -o merged_msg.db
```

### 查看消息库信息

```bash
evtx-tools info ./msg_dbs/hotech-msg-Windows_10_Education-25H2-26220.7052.db
```

## 性能优化

### 跳过统计表

```bash
# 最快解析速度
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output --no-stats
```

### 多线程

```bash
# 指定线程数
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output --threads 8
```

### 输出格式

```bash
# 输出 CSV
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output --format csv

# 同时输出 DB 和 CSV
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output --format both
```

## 故障排除

### 消息库不匹配

```bash
# 手动指定消息库
evtx-tools parse Security.evtx --msg-db ./specific_msg.db -o output

# 查看可用消息库
ls ./msg_dbs/*.db
```

### 解析错误

```bash
# 查看详细错误
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output 2>&1 | tee error.log
```

### 内存不足

```bash
# 减少线程数
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output --threads 2

# 跳过统计表
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output --no-stats
```

## 最佳实践

1. **消息库管理**
   - 为每个 Windows 版本维护独立的消息库
   - 定期更新消息库以包含新版本的 DLL
   - 使用基线模式只导出第三方软件消息

2. **分析流程**
   - 先使用内置 profile 进行初步分析
   - 根据需要创建自定义规则
   - 使用统计表快速了解事件分布

3. **性能优化**
   - 大文件解析时跳过统计表
   - 使用多线程加速解析
   - 考虑输出 CSV 格式便于后续处理

4. **结果验证**
   - 使用 Lua 1:1 对照验证结果
   - 检查时间跳变检测结果
   - 验证消息替换是否正确

## 相关资源

- [evtx-tools 详细文档](evtx-tools/README.md)
- [msgdb-dump 详细文档](msgdb-dump/README.md)
- [evtx-tools 开发规范](evtx-tools/CLAUDE.md)
- [msgdb-dump 数据库 schema](msgdb-dump/docs/schema.md)
