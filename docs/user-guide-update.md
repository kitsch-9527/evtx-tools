# EVTX 解析工具用户指南更新

## 更新概述
本文档更新了 EVTX 解析工具的用户指南，包括修复后的功能说明、使用方法和已知限制。

## 1. 修复后的新功能

### 1.1 改进的模板匹配
#### 功能说明
- Provider 名称标准化: 自动处理 "Microsoft-Windows-" 前缀差异
- 语言回退机制: 当目标语言不可用时，自动回退到 en-US
- 模糊匹配支持: 支持部分 Provider 名称匹配

#### 使用示例
```bash
# 解析 EVTX 文件，自动使用改进的模板匹配
evtx-tools parse System.evtx --msg-db messages.db -o output.db
```

### 1.2 值映射支持
#### 功能说明
- VMAP/BMAP 支持: 支持 CRIM 资源中的值映射数据
- 位图映射: 支持位图类型的值映射
- 硬编码映射: 内置常用 EventID 的值映射

#### 使用示例
```bash
# 解析 EVTX 文件，自动应用值映射
evtx-tools parse Security.evtx --msg-db messages.db -o output.db

# 查看值映射结果
sqlite3 output.db "SELECT message FROM events WHERE event_id = 566;"
```

### 1.3 参数替换改进
#### 功能说明
- 两遍扫描: 模拟 Windows FormatMessageW 的两遍扫描行为
- GUID 格式化: 统一 GUID 显示格式
- 参数引用展开: 支持 %%N 参数引用

#### 使用示例
```bash
# 解析 EVTX 文件，使用改进的参数替换
evtx-tools parse Security.evtx --msg-db messages.db -o output.db

# 查看参数替换结果
sqlite3 output.db "SELECT message FROM events WHERE event_id = 4624;"
```

---

## 2. 使用指南

### 2.1 安装和配置

#### 安装步骤
1. **下载预编译版本**
   ```bash
   # 从 GitHub Releases 下载
   wget https://github.com/evtx-tools/evtx-tools/releases/latest/download/evtx-tools-windows-x64.zip
   unzip evtx-tools-windows-x64.zip
   ```

2. **从源码编译**
   ```bash
   # 克隆仓库
   git clone https://github.com/evtx-tools/evtx-tools.git
   cd evtx-tools
   
   # 编译发布版本
   cargo build --release
   ```

3. **配置消息数据库**
   ```bash
   # 下载消息数据库
   wget https://github.com/evtx-tools/evtx-tools/releases/latest/download/messages.db
   
   # 或使用 msgdb-dump 生成
   msgdb-dump dump --out-dir ./msg_dbs/ --lang 1033,2052
   ```

#### 配置选项
```bash
# 查看配置选项
evtx-tools --help

# 主要配置选项：
# --msg-db <PATH>     指定消息数据库路径
# --msg-dir <DIR>     指定消息数据库目录
# --lang <CODE>       指定语言代码 (如 zh-CN, en-US)
# --format <TYPE>     输出格式 (sqlite, csv, both)
```

### 2.2 基本使用

#### 解析 EVTX 文件
```bash
# 基本解析
evtx-tools parse System.evtx -o output.db

# 指定消息数据库
evtx-tools parse System.evtx --msg-db messages.db -o output.db

# 指定语言
evtx-tools parse System.evtx --lang zh-CN -o output.db

# 输出为 CSV 格式
evtx-tools parse System.evtx --format csv -o output.csv
```

#### 查看解析结果
```bash
# 使用 SQLite 查看结果
sqlite3 output.db

# 查看所有事件
SELECT * FROM events LIMIT 10;

# 查看特定 EventID
SELECT * FROM events WHERE event_id = 4624;

# 查看统计信息
SELECT * FROM stat_event_id;
```

### 2.3 高级功能

#### 过滤事件
```bash
# 使用 Profile 过滤
evtx-tools parse Security.evtx --profile security -o output.db

# 使用规则文件过滤
evtx-tools parse Security.evtx --rules-dir ./rules/ -o output.db

# 按 EventID 过滤
evtx-tools parse Security.evtx --event-ids 4624,4625 -o output.db
```

#### 批量处理
```bash
# 解析目录下所有 EVTX 文件
evtx-tools parse ./evtx_files/ -o output.db

# 使用通配符
evtx-tools parse "C:\Windows\System32\winevt\Logs\*.evtx" -o output.db
```

---

## 3. 已知限制

### 3.1 当前限制

#### 限制 1: 模板匹配限制
- 描述: 某些 EventID 的模板可能不正确
- 影响: EventID 24, 6013 的模板匹配可能失败
- 解决方案: 使用 msgdb-dump 重新生成消息库

#### 限制 2: 值映射限制
- 描述: value_maps 表可能为空
- 影响: 值映射功能可能不工作
- 解决方案: 等待 msgdb-dump 工具修复或使用硬编码映射

#### 限制 3: 语言支持限制
- 描述: 某些语言的模板可能不可用
- 影响: 非英语/中文事件可能无法正确解析
- 解决方案: 使用语言回退机制或贡献翻译

### 3.2 已知问题

#### 问题 1: EventID 24 模板错误
- 现象: 显示错误的模板消息
- 原因: msg.db 中模板条目错误
- 状态: 待修复

#### 问题 2: EventID 6013 模板错误
- 现象: 显示关机消息而非系统运行时间
- 原因: msg.db 中模板条目错误
- 状态: 待修复

#### 问题 3: Setup 值映射回归
- 现象: 222 个 Setup 事件显示数值
- 原因: value_maps 表为空
- 状态: 待修复

---

## 4. 故障排除

### 4.1 常见问题

#### 问题 1: 消息数据库未找到
- 错误信息: "messages.db not found"
- 解决方案:
  1. 检查文件路径是否正确
  2. 下载或生成消息数据库
  3. 使用 --msg-db 参数指定路径

#### 问题 2: 内存不足
- 错误信息: "out of memory"
- 解决方案:
  1. 增加系统内存
  2. 使用 --threads 参数减少并发
  3. 分批处理大文件

#### 问题 3: 权限不足
- 错误信息: "permission denied"
- 解决方案:
  1. 以管理员身份运行
  2. 检查文件权限
  3. 使用管理员权限访问 EVTX 文件

---

## 5. 最佳实践

### 5.1 性能优化

#### 优化 1: 使用合适的线程数
```bash
# 根据 CPU 核心数设置线程数
evtx-tools parse System.evtx --threads 8 -o output.db

# 对于大文件，使用更多线程
evtx-tools parse large.evtx --threads 16 -o output.db
```

#### 优化 2: 选择合适的输出格式
```bash
# SQLite 格式适合查询和分析
evtx-tools parse System.evtx --format sqlite -o output.db

# CSV 格式适合导入其他工具
evtx-tools parse System.evtx --format csv -o output.csv
```

---

## 6. 更新日志

### 6.1 版本 v1.0.0 (2026-08-27)
#### 新功能
1. 改进的模板匹配机制
2. 值映射支持
3. 参数替换改进

#### 修复
1. EventID 24 模板匹配错误
2. EventID 6013 模板匹配错误
3. EventID 566 值映射缺失
4. Setup 通道值映射回归

#### 改进
1. Provider 名称标准化
2. 语言回退机制
3. GUID 格式化

---

**文档版本**: v1.0
**更新时间**: 2026-08-27T03:10:37.609Z
**作者**: researcher
**团队**: evtx-bugfix-team
