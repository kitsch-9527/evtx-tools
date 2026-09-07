# EVET Skills 索引

本目录包含 EVET 项目的各种技能文档，用于快速参考和使用指南。

## 技能列表

### 原有技能

#### 1. [evtx-tools-skill.md](evtx-tools-skill.md)
**evtx-tools 技能文档**
- 跨平台 EVTX 事件解析与分析工具链
- 核心功能、快速开始、内置 Profile
- 自定义规则 YAML 格式
- 统计表和时间跳变检测
- 架构和管道流程

#### 2. [msgdb-dump-skill.md](msgdb-dump-skill.md)
**msgdb-dump 技能文档**
- Windows ETW 提供程序消息数据库导出工具
- 三种提取路径（RT_MESSAGETABLE、CRIM+MUI、MUI-only）
- 增量导出和基线模式
- 远程 DLL 导入
- 输出数据库结构（schema v4）

#### 3. [integration-skill.md](integration-skill.md)
**集成使用指南**
- 两个工具的完整工作流程
- 消息库提取和 EVTX 解析
- 结果分析和查询示例
- 高级用法和性能优化
- 故障排除和最佳实践

#### 4. [parity-test-skill.md](parity-test-skill.md)
**闭环比对测试技能文档**
- 双链路可重复闭环验证台（`evtx-parity-test/`：基线 vs 新解析）
- 分步流水线 01~06 + `run-all.ps1` 一键重跑
- 顺序对齐 + 五字段归一化口径与 Message 四分类（A 豁免 / B / C / D）
- `report.json` 机器可读闭环（verdict + 分通道 categories）
### Matt Pocock 技能

#### 工程技能

##### 用户调用技能

- **[ask-matt](engineering/ask-matt/SKILL.md)**: 询问哪个技能或流程适合你的情况。用户调用技能的路由器。
- **[grill-with-docs](engineering/grill-with-docs/SKILL.md)**: 烤问会话，同时构建项目的领域模型，锐化术语并内联更新 `CONTEXT.md` 和 ADR。
- **[triage](engineering/triage/SKILL.md)**: 通过分诊角色状态机移动问题。
- **[improve-codebase-architecture](engineering/improve-codebase-architecture/SKILL.md)**: 扫描代码库寻找深化机会，以可视化 HTML 报告呈现，然后烤问你选择的任何一个。
- **[setup-matt-pocock-skills](engineering/setup-matt-pocock-skills/SKILL.md)**: 为工程技能配置此仓库（问题跟踪器、分诊标签、领域文档布局）。在使用其他工程技能之前，每个仓库运行一次。
- **[to-spec](engineering/to-spec/SKILL.md)**: 将当前对话转换为规范并发布到问题跟踪器。无需访谈，只需综合你已经讨论的内容。
- **[to-tickets](engineering/to-tickets/SKILL.md)**: 将任何计划、规范或对话分解为一组跟踪器子弹票据，每个票据声明其阻塞边，写为本地文件中的文本，或作为真实跟踪器上的本机阻塞链接。
- **[implement](engineering/implement/SKILL.md)**: 构建规范或票据集描述的工作，在预定义的接缝处驱动 `/tdd`，并在提交前以 `/code-review` 结束。
- **[wayfinder](engineering/wayfinder/SKILL.md)**: 将大量工作（超出单个代理会话所能容纳的范围）规划为问题跟踪器上决策票据的共享地图，并一次解决一个，直到通往目的地的道路清晰。

##### 模型调用技能

- **[prototype](engineering/prototype/SKILL.md)**: 构建一次性原型来回答设计问题，可以是用于状态/逻辑问题的单个可共享 HTML 文件，也可以是从一个路由可切换的多个完全不同的 UI 变体。
- **[diagnosing-bugs](engineering/diagnosing-bugs/SKILL.md)**: 用于难以调试的错误和性能回归的纪律诊断循环：构建一个反馈循环，从这个错误变红 → 最小化 → 假设 → 仪器 → 修复 → 回归测试。
- **[research](engineering/research/SKILL.md)**: 针对高信任主要来源调查问题，并将发现作为引用的 Markdown 文件捕获在仓库中，作为后台代理运行。
- **[tdd](engineering/tdd/SKILL.md)**: 测试驱动开发与红-绿-重构循环。一次构建一个垂直切片的功能或修复错误。
- **[domain-modeling](engineering/domain-modeling/SKILL.md)**: 主动构建和锐化项目的领域模型：挑战术语表中的术语，通过边缘案例场景进行压力测试，并内联更新 `CONTEXT.md` 和 ADR。
- **[codebase-design](engineering/codebase-design/SKILL.md)**: 设计深层模块的共享纪律和词汇：小接口后面有大量行为，放置在干净的接缝处，可通过该接口进行测试。
- **[code-review](engineering/code-review/SKILL.md)**: 对自固定点以来的差异进行双轴审查：**标准**（是否遵循仓库的编码标准，加上 Fowler 气味基线？）和 **规范**（是否忠实地实现了原始问题/规范？），作为并行子代理运行，因此两者不会相互污染。
- **[resolving-merge-conflicts](engineering/resolving-merge-conflicts/SKILL.md)**: 逐块处理进行中的 git 合并或变基冲突，通过追溯到每一方的主要来源的意图来解决，然后完成操作（永不 `--abort`）。
- **[wizard](engineering/wizard/SKILL.md)**: 生成一个交互式 bash 向导，引导人类完成只有他们才能执行的步骤：配置基础设施、设置凭据或 CI 密钥、浏览不熟悉的第三方仪表板，或运行一次性迁移或切换。

#### 生产力技能

##### 用户调用技能

- **[grill-me](productivity/grill-me/SKILL.md)**: 对计划或设计进行无情的访谈，直到设计树的每个分支都得到解决。
- **[handoff](productivity/handoff/SKILL.md)**: 将当前对话压缩为交接文档，以便另一个代理可以继续工作。
- **[teach](productivity/teach/SKILL.md)**: 在多个会话中教授用户新技能或概念，使用当前目录作为有状态的教学工作区。
- **[to-questionnaire](productivity/to-questionnaire/SKILL.md)**: 将你无法独自做出的决定转化为 Markdown 问卷，供唯一可以回答的人异步填写，或在会议中一起填写。它烤问你关于发送的内容（给谁，你需要什么回报），而不是主题。
- **[wait-what](productivity/wait-what/SKILL.md)**: 当消息没有落地时立即触发。代理用你缺失的上下文重新推销它，使用你的 `CONTEXT.md` 词汇，用简单的英语。

##### 模型调用技能

- **[grilling](productivity/grilling/SKILL.md)**: 对计划、决定或想法进行无情的访谈，直到设计树的每个分支都得到解决。可重用的访谈原语，位于 `grill-me`、`grill-with-docs`、`triage`、`wayfinder` 和 `improve-codebase-architecture` 之后。
- **[writing-for-agents](productivity/writing-for-agents/SKILL.md)**: 为代理编写文档：技能、AGENTS.md/CLAUDE.md，以及代理通过指针访问的任何文档。

## 快速参考

### evtx-tools 常用命令
```bash
# 构建
cd evtx-tools && cargo build --release

# 基本解析
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output

# 带场景分析
evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o output \
  --profile security --profile persistence

# 自定义规则
evtx-tools parse case.evtx --rules-dir ./my-rules/ -o case_output
```

### msgdb-dump 常用命令
```bash
# 构建
cd msgdb-dump && go build -o msgdb-dump-x64.exe .

# 基本提取
msgdb-dump-x64.exe --out-dir ./output --lang 1033,2052

# 增量导出
msgdb-dump-x64.exe --baseline ./baseline.json --tag third-party

# 远程导入
msgdb-dump-x64.exe --import-dll ./remote_dlls/ --alias MySQL=mysqld
```

### 闭环比对测试常用命令
详见 parity-test-skill.md 快捷节：`run-all.ps1` 一键全链路，`report.json.verdict` 判定闭合。

## 工作流程示例

### 完整分析流程
1. **提取消息库**（在 Windows 机器上）
   ```bash
   msgdb-dump-x64.exe --out-dir ./msg_dbs/ --lang 1033,2052
   ```

2. **解析 EVTX 文件**
   ```bash
   evtx-tools parse Security.evtx --msg-dir ./msg_dbs/ -o security_output \
     --profile security --profile remote
   ```

3. **分析结果**
   ```bash
   sqlite3 security_output "SELECT * FROM security_events LIMIT 10;"
   sqlite3 security_output "SELECT * FROM stat_event_id ORDER BY count DESC;"
   ```

## 相关文档

- [主 README](../README.md) - 项目概述和详细说明
- [evtx-tools README](../evtx-tools/README.md) - evtx-tools 详细文档
- [msgdb-dump README](../msgdb-dump/README.md) - msgdb-dump 详细文档
- [evtx-tools 开发规范](../evtx-tools/CLAUDE.md) - 开发规范和架构说明
- [msgdb-dump 数据库 schema](../msgdb-dump/docs/schema.md) - 数据库结构详细说明

## 更新记录

- 2026-08-25: 初始版本创建
  - evtx-tools 技能文档
  - msgdb-dump 技能文档
  - 集成使用指南
- 2026-09-03: 添加闭环比对测试技能（parity-test-skill.md）
- 2026-08-25: 添加 Matt Pocock 技能
  - 工程技能（用户调用和模型调用）
  - 生产力技能（用户调用和模型调用）