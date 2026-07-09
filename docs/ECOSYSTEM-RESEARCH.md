# EVTX 解析工具生态调研与对比分析

> 调研日期：2026-07-10
> 调研范围：evtx-tools（本工具）vs Hayabusa vs EvtxECmd (Eric Zimmerman)
> 目的：明确产品定位、发现差异化优势、制定后续路线

---

## 1. 调研工具概述

### 1.1 evtx-tools（本工具）

| 属性 | 说明 |
|---|---|
| 项目 | [evtx-tools](https://github.com/kitsch-9527/evtx-tools) |
| 语言 | Rust（workspace: evtx-tools-core + evtx-tools-cli） |
| 许可 | MIT |
| 版本 | v0.1.0（早期） |
| 定位 | 跨平台 EVTX 事件解析与分析 CLI 工具，**中英双语消息格式化** |
| 开发者 | 个人项目 |

**核心能力：**
- EVTX 解析（基于 `evtx` 0.12 crate）
- 消息格式化：`%1` `%2` 替换 + `%%N` 展开 + 两遍 FormatMessageW 模拟
- 中英双语消息（**生态唯一**）
- Value Map 解析（LogonType 2 → "Interactive"）
- 场景路由（YAML 规则 → 分类表）
- 统计表自动生成（6 表，含时间跳变检测）
- SQLite 一等公民输出

### 1.2 Hayabusa（隼）

| 属性 | 说明 |
|---|---|
| 项目 | [Yamato-Security/hayabusa](https://github.com/Yamato-Security/hayabusa) |
| 语言 | Rust |
| 许可 | AGPL-3.0 |
| 版本 | v3.5.0（2025-08，成熟） |
| 定位 | Sigma 驱动的威胁狩猎与取证时间线生成工具 |
| 开发者 | Yamato Security 团队 + NTT DATA |
| 商业背书 | Black Hat USA 2025 Arsenal、SANS 推荐 |

**核心能力：**
- Sigma 规则引擎（4,000+ 规则，每日更新）
- MITRE ATT&CK 映射（所有规则标记 technique ID）
- Sigma v2 相关性规则（event_count, value_count, temporal, temporal_ordered）
- 关键词/正则搜索（`search -k` / `-r`）
- HTML 报告、Timesketch 集成
- Velociraptor 企业部署集成
- 日志通道感知规则加载（性能优化）

**生态工具：**
- **[hayabusa-rules](https://github.com/Yamato-Security/hayabusa-rules)** — 4,000+ 条规则仓库
- **[hayabusa-evtx](https://github.com/Yamato-Security/hayabusa-evtx)** — evtx crate 的 fork
- **[Takajo](https://github.com/Yamato-Security/takajo)** — 结果分析器 + 动态 Web 报告
- **[Suzaku](https://github.com/Yamato-Security/suzaku)** — 云端日志分析可视化

### 1.3 EvtxECmd (Eric Zimmerman)

| 属性 | 说明 |
|---|---|
| 项目 | [EricZimmerman/evtx](https://github.com/EricZimmerman/evtx) |
| 语言 | C# (.NET) |
| 许可 | MIT |
| 版本 | v1.5.2（2025-01，成熟） |
| 定位 | Windows 事件日志解析器 + Maps 社区生态 |
| 开发者 | Eric Zimmerman + 社区 Maps 贡献者 |
| 商业背书 | SANS 官方推荐，Kroll KAPE 集成 |

**核心能力：**
- 自研 C# EVTX 解析引擎
- Maps 系统（社区贡献的 XPath 数据提取蓝图）
- PayloadData1~6 通用槽位复用
- 锁文件读取（**生态唯一**）
- VSS（卷影副本）支持 + SHA-1 去重
- KAPE 集成（企业级自动化取证）

**生态工具：**
- **[TimelineExplorer](https://ericzimmerman.github.io)** — EZ Tools 统一 GUI 查看器
- **[KAPE](https://www.kroll.com/en/services/cyber-risk/kape)** — 企业级取证自动化框架
- **Maps 仓库** — 社区维护的 200+ EventID 蓝图
- 全套 EZ Tools（MFTECmd, PECmd, RECmd, SQLECmd 等）

---

## 2. 完整功能对比矩阵

### 2.1 基础信息

| 维度 | evtx-tools | Hayabusa | EvtxECmd |
|------|---|---|---|
| 语言 | Rust | Rust | C# (.NET) |
| 许可 | MIT | AGPL-3.0 | MIT |
| 版本 | v0.1.0 | v3.5.0 | v1.5.2 |
| 成熟度 | 早期 | 成熟 | 成熟 |
| 跨平台 | ✅ Rust，全平台 | ✅ Rust，全平台 | ⚠️ 需 .NET runtime |
| 开发者规模 | 单人 | 团队 + NTT DATA | 单人 + 社区 Maps |
| GitHub Stars | 新项目 | 3,000+ | 290+ |
| 商业背书 | 无 | Black Hat、SANS、NTT DATA | SANS、Kroll |

### 2.2 解析核心能力

| 维度 | evtx-tools | Hayabusa | EvtxECmd |
|------|---|---|---|
| EVTX 解析引擎 | `evtx` 0.12 (Rust) | `hayabusa-evtx` (fork) | 自研 C# 解析器 |
| 消息格式化（%1 %2 替换） | ✅ **唯一** | ❌ 不做 | ❌ 不做 |
| 中英双语消息 | ✅ **唯一** | ❌ 仅英文 | ❌ 仅英文 |
| Value Map 解析（LogonType→"Interactive"） | ✅ **唯一** | ❌ 原始数字 | ❌ 原始值 |
| Enum 解析（isRoaming→Enabled/Disabled） | ✅ **唯一** | ❌ | ❌ |
| %%N 参数引用展开 | ✅ **唯一** | ❌ | ❌ |
| FormatMessageW 两遍语义 | ✅ **唯一** | ❌ | ❌ |
| Fallback 消息（匹配 Event Viewer） | ✅ **唯一** | ❌ | ❌ |
| 字段提取列数 | **25 列** | ~10 列 | 20+ 列（含 PayloadData1~6） |
| 全局 sequence_number | ✅ **唯一** | ❌ | ❌ |
| 原始 EventData JSON 保留 | ✅ | ❌ 丢弃 | ⚠️ Payload 字段 |
| 原始 XML 输出 | ⏳ 计划中 | ❌ | ✅ `--fj` |
| 锁文件读取 | ❌ | ❌ | ✅ **唯一** |
| VSS 卷影副本支持 | ❌ | ❌ | ✅ **唯一** |

### 2.3 消息格式化深度对比

| 消息处理阶段 | evtx-tools | Hayabusa | EvtxECmd |
|---|---|---|---|
| 原始 EVTX EventData | `{"TargetUserName":"admin","LogonType":"2"}` | 同左 | 同左 |
| 有 message.db 时 | `已成功登录帐户。\n帐户名:\tadmin\n登录类型:\t2` | N/A | N/A |
| 有 Maps 时（新增功能） | `Target: admin (Interactive)` | N/A | `Target: admin (Interactive)` |
| 无任何资源时 | 本地化 fallback 文本（中英可选） | 原始字段值 | 原始字段值 |
| 是否是 Event Viewer 等效 | ✅ 是 | ❌ | ❌ |

### 2.4 规则/检测引擎

| 维度 | evtx-tools | Hayabusa | EvtxECmd |
|------|---|---|---|
| 规则数量 | 8 个内置 profile | 4,000+ Sigma + 170+ 内置 | 社区 Maps（200+ EventID） |
| 规则格式 | 简化 YAML | Sigma YAML | JSON Map |
| Sigma v2 支持 | ❌ | ✅ **唯一**（含 correlation） | ❌ |
| MITRE ATT&CK 映射 | ❌ | ✅ **唯一** | ❌ |
| 规则社区 | ❌ 单人 | ✅ 100+ 贡献者 | ✅ 社区 Maps |
| 自定义规则能力 | ✅ YAML | ✅ Sigma YAML | ✅ JSON Map |
| 规则更新机制 | ❌ 手动 | ✅ `update-rules` | ✅ `--sync` |
| Level 严重等级过滤 | ⚠️ 数值过滤 | ✅ 5 级过滤 | ❌ |
| 误报调优工具 | ❌ | ✅ 交互式 level-tuning | ❌ |

### 2.5 搜索与调查

| 维度 | evtx-tools | Hayabusa | EvtxECmd |
|------|---|---|---|
| 关键词搜索 | ❌ 需自己查 SQL | ✅ `search -k` | ❌ 需自己过滤 |
| 正则搜索 | ❌ 需自己查 SQL | ✅ `search -r` | ❌ |
| 按字段搜索 | ❌ 需自己查 SQL | ✅ `--field` | ❌ |
| 时间范围过滤 | ❌ 需自己写 SQL | ✅ `--timeline-start/end` | ✅ `--sd` / `--ed` |
| Base64 提取/解码 | ❌ | ✅ `extract-base64` | ❌ |
| Pivot 关键词提取 | ❌ | ✅ `pivot-keywords-list` | ❌ |
| 登录总结 | ❌ 需自己写 SQL | ✅ `logon-summary` | ❌ |
| 通道感知规则加载 | ❌ | ✅ **唯一** | ❌ |
| 统计指标 | ✅ 6 表 + 时间跳变 | ✅ computer/eid-metrics | ✅ `--met` |

### 2.6 输出与报告

| 维度 | evtx-tools | Hayabusa | EvtxECmd |
|------|---|---|---|
| SQLite 输出 | ✅ **一等公民** + 完整 schema + 索引 | ❌ | ❌ |
| CSV 输出 | ✅ 25 列 | ✅ 8 种 profile | ✅ 标准化列 |
| JSON 输出 | ❌ | ✅ JSONL + JSON | ✅ JSON + XML |
| HTML 报告 | ❌ | ✅ `--HTML-report` | ❌ |
| Timesketch 集成 | ❌ | ✅ | ❌ |
| SOF-ELK 集成 | ❌ | ✅ | ❌ |
| TimelineExplorer 兼容 | ❌ | ❌ | ✅ **原生** |
| 控制台彩色输出 | ❌ | ✅ | ⚠️ 基础 |
| 输出 profile 系统 | ❌ | ✅ 8 种 | ✅ PayloadData 槽位复用 |
| EvtxECmd CSV 格式兼容 | ⏳ **决定支持** | ❌ | ✅ 自格式 |

### 2.7 生态与部署

| 维度 | evtx-tools | Hayabusa | EvtxECmd |
|------|---|---|---|
| 消息库依赖 | ✅ 需 msgb-dump（Go 工具）生成 | ❌ 不需要 | ❌ 不需要 |
| Maps 社区依赖 | ⏳ **决定采纳** | ❌ | ✅ 核心依赖 |
| 批量目录处理 | ❌ 单文件 | ✅ `-d` | ✅ `-d` |
| 实时分析 | ✅ 解析任意 EVTX | ✅ `--live-analysis` | ✅ `--liv` |
| 空闲空间恢复 | ❌ | ✅ `--recover-records` | ❌ |
| 去重 | ❌ | ✅ `--remove-duplicate-detections` | ✅ SHA-1 |
| Velociraptor 集成 | ❌ | ✅ 官方 artifact | ❌ |
| KAPE 集成 | ❌ | ❌ | ✅ **核心优势** |
| 企业部署路径 | ❌ | ✅ Velociraptor | ✅ KAPE |
| 教程/培训材料 | ❌ | ✅ SANS + Black Hat + 博客 | ✅ SANS + Kroll + YouTube |

### 2.8 性能与架构

| 维度 | evtx-tools | Hayabusa | EvtxECmd |
|------|---|---|---|
| 多线程 | ✅ `--threads N` + rayon | ✅ `--threads N` | ✅ .NET 并行 |
| 管道架构 | ✅ Producer-Writer 分离（sync_channel） | 待确认 | 待确认 |
| 批量写入 | ✅ 20K batch + 2000 chunk + bulk pragmas | ✅ | ✅ |
| 索引策略 | ✅ 延迟创建（finalize 时） | ✅ | ✅ |
| 消息预热 | ✅ HashMap 预热（批量加载） | ❌ 不需要 | ❌ 不需要（Maps 无预热需要） |
| 规则加载优化 | ❌ 全部规则全量加载 | ✅ 通道感知加载（减 60-80%） | ✅ 文件系统读取 |

---

## 3. 功能短板分析

### 3.1 MITRE ATT&CK 映射

**当前状态：evtx-tools 不支持。这是与 Hayabusa 差距最大的功能短板之一。**

---

#### 3.1.1 MITRE ATT&CK 是什么

MITRE ATT&CK（Adversarial Tactics, Techniques, and Common Knowledge）是一个由 MITRE 公司维护的**攻击行为知识库**。它将真实世界中的网络攻击行为抽象为**层次化分类体系**：

```
ATT&CK 矩阵（14 个 tactic）
  ├── TA0001 Initial Access（初始访问）
  │   ├── T1078 Valid Accounts（有效账户）
  │   ├── T1190 Exploit Public-Facing Application（利用公开应用）
  │   └── ...
  ├── TA0002 Execution（执行）
  │   ├── T1059 Command and Scripting Interpreter（命令和脚本解释器）
  │   │   ├── T1059.001 PowerShell
  │   │   ├── T1059.003 Windows Command Shell
  │   │   └── ...
  │   ├── T1204 User Execution（用户执行）
  │   └── ...
  ├── TA0003 Persistence（持久化）
  │   ├── T1543 Create or Modify System Process（创建或修改系统进程）
  │   │   ├── T1543.003 Windows Service
  │   │   └── ...
  │   ├── T1053 Scheduled Task/Job（计划任务）
  │   └── ...
  ├── ...
  └── TA0040 Impact（影响）
```

**三层结构：**

| 层级 | 英文 | 说明 | 标识符 | 数量 |
|------|------|------|--------|:----:|
| 战术 | Tactic | 攻击者想要达成的**阶段性目标** | TA0001 ~ TA0040 | 14 |
| 技术 | Technique | 达成目标的具体**方法** | T1000 ~ T1600+ | 200+ |
| 子技术 | Sub-technique | 技术的**具体实现变种** | Txxxx.xxx | 400+ |

**实际例子——EventID 7045（服务安装）：**

```
EVTX: EventID=7045, Channel=System
  → 攻击者行为：安装恶意服务
  → MITRE Technique: T1543.003 (Create or Modify System Process: Windows Service)
  → MITRE Tactic: TA0003 (Persistence, 持久化)
                TA0002 (Execution, 执行)
```

**为什么攻击者要把一个事件映射到多个 tactic？** 因为安装服务既是持久化手段（服务随系统启动），又是执行手段（通过服务执行代码）。

---

#### 3.1.2 分析师如何使用 MITRE ATT&CK

**场景 1：事件分类（事件分析师）**

```
收到告警：EventID 4688, CommandLine = "powershell -enc SQBFAFgA..."
                   ↓
分析师看到 MITRE 标签: T1059.001 (Command and Scripting Interpreter: PowerShell)
                   ↓
立刻理解：攻击者在用 PowerShell 执行命令，可能是 T1059.001 技术
```

**场景 2：防守能力评估（检测工程师）**

```
问题："我们对 Credential Access（凭据访问）有多强的检测能力？"
                   ↓
查询 MITRE 映射表：
  检测能力覆盖 = 检测规则数量 / MITRE 该 tactic 下所有 technique 数量
                   ↓
回答："Credential Access 覆盖 60%，缺口在 T1555（凭据来自密码存储）"
```

**场景 3：攻击时间线重建（DFIR 分析师）**

```
按时间排序的事件序列：
  09:00  T1078     Initial Access     → 攻击者用窃取凭据登录
  09:05  T1059.001 Execution          → PowerShell 执行载荷
  09:10  T1543.003 Persistence        → 安装服务驻留
  09:12  T1003     Credential Access  → 抓取内存凭据
                   ↓
分析师一眼看出攻击链路：Initial Access → Execution → Persistence → Credential Access
```

**场景 4：管理层报告（CSO/CISO）**

```
从 MITRE 热图生成报告：
  ┌────────────────────────────────────────┐
  │  本月检测到的攻击覆盖                    │
  │  Initial Access  ■■■■■□□□□□ 50%        │
  │  Execution       ■■■■■■■□□□ 70%        │
  │  Persistence     ■■■■■□□□□□ 50%        │
  │  Credential Acc  ■■■□□□□□□□ 30%  ← 短板 │
  │  Lateral Movem   ■■■■■■□□□□ 60%        │
  └────────────────────────────────────────┘
```

---

#### 3.1.3 Hayabusa 的 MITRE 映射实现

**在规则级别映射：**

Hayabusa 的每条 Sigma 规则 YAML 中声明 `tags` 字段：

```yaml
# hayabusa-rules/sigma/builtin/win_service_install.yml
title: Service Installation
id: abc123
logsource:
  product: windows
  service: system
detection:
  selection:
    Channel: System
    EventID: 7045
  condition: selection
level: medium
tags:
  - T1543.003                    # Technique + Sub-technique ID
  - attack.persistence           # Tactic（持久化）
  - attack.execution             # Tactic（执行：此服务可能是恶意载荷）
falsepositives:
  - Legitimate software installation
```

**支持在输出和过滤中消费 MITRE 映射：**

```bash
# 只筛选 execution tactic 的告警
hayabusa csv-timeline -d ./evtx --tags attack.execution

# 只筛选特定 technique 的告警
hayabusa csv-timeline -d ./evtx --tags t1059

# 输出中自动包含 MITRE 列
hayabusa csv-timeline -d ./evtx -p verbose -o timeline.csv
# CSV 包含列: Timestamp, Computer, EventID, Level, RuleTitle, MITRETactic, MITRETechnique, Details, ...
```

**规则仓库中 MITRE 覆盖情况：**

| 分类 | 覆盖 | 说明 |
|------|:----:|------|
| Windows EventID 规则总量 | 4,000+ | 含 Sigma 社区规则 + Hayabusa 特有规则 |
| 有 MITRE 映射的规则 | ~95% | 绝大部分规则标记了 technique ID |
| 按 tactic 筛选 | ✅ | 14 个 tactic 全覆盖 |
| 按 technique 筛选 | ✅ | 200+ technique 覆盖 |
| 热图/可视化 | ✅ | HTML 报告含 MITRE 攻击矩阵 |

**Hayabusa 的 MITRE 配置：**

```
config/mitre_tactics.txt  ← 定义 tactic 缩写（attack.persistence → Persistence）
```

---

#### 3.1.4 EVTX EventID 到 MITRE Technique 的常见映射

以下是在 EVTX 事件分析中常见的映射关系（来自社区和 Hayabusa）：

| EventID | 描述 | MITRE Technique | MITRE Tactic |
|---------|------|-----------------|-------------|
| 4624 | 登录成功 | T1078 (Valid Accounts) | TA0001 (Initial Access) |
| 4625 | 登录失败 | T1110 (Brute Force) | TA0006 (Credential Access) |
| 4648 | 显式凭据登录 | T1078 (Valid Accounts) | TA0006 (Credential Access) |
| 4672 | 分配特殊特权 | T1078 (Valid Accounts) | TA0004 (Privilege Escalation) |
| 4688 | 进程创建 | T1059 (Command and Scripting Interpreter) | TA0002 (Execution) |
| 4698 | 计划任务创建 | T1053.005 (Scheduled Task) | TA0003 (Persistence) |
| 4700 | 计划任务启用 | T1053.005 (Scheduled Task) | TA0003 (Persistence) |
| 4720 | 用户创建 | T1136 (Create Account) | TA0003 (Persistence) |
| 4728 | 加入安全组 | T1098 (Account Manipulation) | TA0003 (Persistence) |
| 4776 | 凭据验证 | T1110 (Brute Force) | TA0006 (Credential Access) |
| 5140 | 文件共享 | T1021 (Remote Services) | TA0008 (Lateral Movement) |
| 5156 | 网络连接 | T1049 (System Network Connections Discovery) | TA0007 (Discovery) |
| 7045 | 服务安装 | T1543.003 (Windows Service) | TA0003 (Persistence) / TA0002 (Execution) |
| Sysmon 1 | 进程创建 | T1059 (Command and Scripting Interpreter) | TA0002 (Execution) |
| Sysmon 3 | 网络连接 | T1071 (Application Layer Protocol) | TA0011 (C2) |
| Sysmon 10 | 进程访问 | T1003 (OS Credential Dumping) | TA0006 (Credential Access) |
| Sysmon 11 | 文件创建 | T1204 (User Execution) | TA0002 (Execution) |
| Sysmon 13 | 注册表修改 | T1112 (Modify Registry) | TA0005 (Defense Evasion) |
| 1102 | 安全日志清空 | T1070.001 (Clear Windows Event Logs) | TA0005 (Defense Evasion) |
| 104 | 日志文件损坏 | T1070.001 (Clear Windows Event Logs) | TA0005 (Defense Evasion) |

**对于 evtx-tools，一个直接的映射方案：**

目前的 8 个内置 profile 可以立即加上 MITRE 映射：

| Profile | EventIDs | MITRE Tactic | Key Technique |
|---------|----------|-------------|---------------|
| security | 4624,4625,4648,... | Initial Access, Credential Access | T1078, T1110 |
| persistence | 7045,4698,4699,... | Persistence | T1543.003, T1053.005 |
| system | 6008,41,1074,... | Defense Evasion | T1070.001 |
| remote | 4624,4648,4778,... | Lateral Movement | T1021 |
| process | 4688,Sysmon 1 | Execution | T1059 |
| powershell | 400,403,4103,... | Execution | T1059.001 |
| mssql | 18456,... | Credential Access | T1110 |
| web | 各种 provider | Initial Access | T1190 |

---

#### 3.1.5 缺失的影响

| 影响 | 说明 | 严重程度 |
|------|------|:--------:|
| 分析师工作流断裂 | 看到可疑事件后需手动查 MITRE ID，从 evtx-tools 切到浏览器 | **高** |
| 管理层报告不完整 | 报告不能按 MITRE 框架组织，缺少行业通用语言 | **高** |
| 防守能力评估困难 | 无法快速回答"我们的检测覆盖哪些 MITRE technique" | **中** |
| 与竞品差距明显 | Hayabusa 所有规则自带 MITRE 映射，用户直接对比 | **高** |
| 下游工具集成受限 | SOF-ELK、Timesketch 等工具依赖 MITRE 标签做可视化 | **中** |
| 事件关联分析困难 | 无 MITRE 标签时，无法按攻击阶段（tactic）做多事件关联 | **中** |

---

#### 3.1.6 追赶路径评估

| 方案 | 工作量 | 复杂度 | 说明 |
|------|--------|:------:|------|
| **方案 A（推荐）：YAML 规则增加 `tags` 字段** | ~50 行 Rust + 更新 8 个 profile | **低** | 现有 YAML 规则格式扩展，在路由结果中加上 `mitre_tactic`、`mitre_technique` 列 |
| 方案 B：建 MITRE→EventID 映射数据表 | ~500 行数据文件 | **低** | 社区有 `awesome-event-ids`、`windows-event-ids` 等现成映射，只需编译嵌入 |
| 方案 C：从 Sigma 规则自动继承 MITRE 标签 | 依赖 Sigma 引擎 | **高** | 远期方案，Sigma 引擎完成后自然获得 |

**方案 A 的规格：**

```yaml
# evtx-tools YAML 规则加 tags
name: "服务安装检测"
table: "persistence_events"
columns:
  - event_id
  - event_time
  - provider_name
  - message
  - mitre_tactic        # 新增列
  - mitre_technique     # 新增列
event_ids: [7045, 4697]
channels: ["System"]
tags:                   # 新增字段（可有多个，简化用 OR 链接）
  - T1543.003
  - attack.persistence
  - attack.execution
```

在场景表中自动填充供后续查询：

```sql
-- 按 MITRE tactic 统计
SELECT mitre_tactic, COUNT(*) FROM persistence_events
WHERE event_id = 7045 GROUP BY mitre_tactic;

-- 筛选所有 execution 相关的告警
SELECT * FROM persistence_events
WHERE mitre_tactic LIKE '%execution%';
```

### 3.2 搜索能力

**当前状态：evtx-tools 不支持内置搜索，用户需自行查询 SQLite。**

#### Hayabusa 的做法

Hayabusa 提供专用 `search` 子命令，功能完善：

```bash
# 关键词搜索（全文）
hayabusa search -d ./evtx -k "mimikatz"

# 正则搜索
hayabusa search -d ./evtx -r "powershell.*-enc"

# 区分大小写/不区分
hayabusa search -d ./evtx -k "mimikatz" --case-insensitive

# 按字段搜索
hayabusa search -d ./evtx -k "attacker@evil.com" --field SubjectUserName

# 结合时间过滤
hayabusa search -d ./evtx -k "mimikatz" --timeline-start "2025-01-01"

# 输出到文件
hayabusa search -d ./evtx -k "mimikatz" -o search-results.csv
```

#### 缺失的影响

| 影响 | 说明 |
|------|------|
| 使用体验割裂 | 用户需离开工具学习 SQL，减低了工具的即用性 |
| 快速调查受阻 | 分析师要查一个 IOC（如 IP、用户名）需写 SQL 语句 |
| 对比竞品直接减分 | Hayabusa 有专用搜索命令，用户直接对比 |

#### 追赶路径评估

| 方案 | 工作量 | 说明 |
|------|--------|------|
| `search -k` 关键词搜索 | ~300 行 Rust + SQL WHERE LIKE | 最小可行方案 |
| `search -r` 正则搜索 | ~100 行追加 | 在关键词基础上加 regex 支持 |
| `--field` 指定字段 | ~50 行追加 | 限定搜索范围 |
| `--timeline-start/end` 时间过滤 | ~50 行追加 | SQL WHERE 时间范围 |
| `search -o` 输出到文件 | ~50 行 | 复用现有 CSV 写入逻辑 |

### 3.3 差异化优势分析

### 3.1 evtx-tools 的独有优势（三家对比下的唯一）

| 优势 | 说明 | 竞争壁垒 |
|------|------|----------|
| 跨平台消息格式化 | Hayabusa 和 EvtxECmd 都不做，它们是字段提取级别 | **高**——需要 PE 解析 + MUI 回退 + FormatMessageW 模拟 |
| 中英双语 | 中国 SOC/DFIR 团队的唯一选择 | **高**——需要完整 MUI 消息库 |
| Value Map 解析 | LogonType、AccessMask 等自动转可读文本 | **中**——需要 CRIM 数据 |
| 时间跳变检测 | sequence_number + stat_event_id_hourly | **中**——实现简单，生态唯一 |
| SQLite 一等公民 | 完整 schema、索引、可直接 sqlite3 查询 | **低**——别人也可以做 |
| 统计表自动生成 | 6 表包含 stat_event_id_hourly | **低**——Hayabusa 有部分 metrics |
| Fallback 消息精确模拟 Event Viewer | 精确中英文错误消息 | **中**——对分析师友好 |
| 数据完整性 | 25 列 + event_data JSON 完整保留 | **低** |

### 3.2 Hayabusa 的独有优势

| 优势 | 说明 | 追赶难度 |
|------|------|----------|
| Sigma 规则生态（4,000+） | 网络效应，社区持续贡献 | **极高**——规则数量不是代码问题，是社区问题 |
| MITRE ATT&CK 映射 | 分析师基本需求 | **高**——需要维护映射关系 |
| 搜索能力 | 内置 keyword/regex 搜索 | **中**——可以实现 |
| HTML 报告 | 管理层友好的输出 | **中**——模板工程 |
| Velociraptor 集成 | 企业部署路径 | **中**——需要合作 |

### 3.3 EvtxECmd 的独有优势

| 优势 | 说明 | 追赶难度 |
|------|------|----------|
| Maps 社区生态 | 社区贡献的 EventID 数据提取蓝图 | **低→中**——决定直接复用 |
| 锁文件读取 | 被系统占用的 EVTX 也能读 | **高**——需要 OS 级处理 |
| VSS 支持 | 历史版本日志读取 | **中**——需要 VSS API |
| KAPE 集成 | 企业级自动化取证 | **中**——需要合作 |
| TimelineExplorer | EZ Tools 统一 GUI | **低**——不是我们的市场 |
| PayloadData 槽位复用 | 6 个通用字段适配所有 EventID | **低**——我们也决定做了 |

---

## 4. 市场生态位置

### 4.1 当前格局

```
                消息可读性
                    ↑
                    |      evtx-tools（你）
                    |      「完整消息格式化 + 中英双语」
                    |
    无消息格式化 ----+----------------------------→ 规则生态
                    |                    Sigma 规则
                    |      Hayabusa（隼）
                    |      「4,000+ 规则 + MITRE」
                    |
                    |       EvtxECmd (EZ Tools)
                    |       「Maps 社区 + KAPE」
                    |
                    ↓
              字段提取级别
```

### 4.2 evtx-tools 的生态位置（Maps 支持后）

```
                消息可读性
                    ↑
                    |      evtx-tools（你）
                    |      「Maps 无门槛入门 → 消息格式化深度体验」
                    |
    无消息格式化 ----+----------------------------→ 规则生态
                    |                    
                    |      Hayabusa（隼）
                    |      「Sigma + MITRE」
                    |
                    |       EvtxECmd (EZ Tools)
                    |       「Maps 原生 + KAPE」
                    |
                    ↓
              字段提取级别
```

Maps 支持使你成为**三家交叉点**：既有消息格式化深度，又有 Maps 社区入口。

### 4.3 差异化定位语句

> **evtx-tools：唯一做到"让 EVTX 说人话"的跨平台工具。**

- 对比 Hayabusa：他们是"检测什么可疑"，你是"事件到底说了什么"
- 对比 EvtxECmd：他们是"字段规范化"，你是"完整消息还原"
- 两者不冲突，互补使用

---

## 5. 产品路线建议

### 5.1 三层产品策略

```
第1层（现有）：高质量 EVTX 解析 + 消息格式化
├── 25 列完整数据 + sequence_number + event_data JSON
├── 中英双语消息格式化（需要 messages.db）
├── 8 个场景 profile（YAML 规则）
└── 6 个统计表 + 时间跳变检测

第2层（决定添加）：社区 Maps 支持（Eric Zimmerman 生态兼容）
├── 新增列到 events 表：map_description, payload_data1~6
├── 直接读取 EvtxECmd 的 Maps JSON 文件
│   └── 模板插值：从 event_data JSON key 取值，不需要 XPath
├── --evtx-maps-dir ./Maps/ 参数
├── 导出格式兼容 Eric Zimmerman 的 CSV 格式
├── 无 messages.db 时：用 Maps 做结构化输出 → 降低使用门槛
└── 有 messages.db 时：完整格式化 + Maps 补充 PayloadData

第3层（后续方向）：
├── 批量目录处理（-d）
├── 搜索子命令（search -k / -r）
├── Sigma 引擎（独立新工具，用 rsigma-rs）
├── HTML 报告
└── 规则社区贡献机制
```

### 5.2 优先级排序

| 优先级 | 功能 | 预估工作量 | 收益 |
|--------|------|-----------|------|
| **P0** | Maps 支持 | ~200-300 行 Rust | 消除使用门槛，借用社区生态 |
| **P1** | 批量目录处理（`-d`） | ~50 行 Rust | Hayabusa/EvtxECmd 都有，基础期望 |
| **P2** | CSV 输出兼容 EvtxECmd 格式 | ~100 行 Rust | 用户可无缝切换工具 |
| **P2** | 内置搜索（`search -k` / `-r`） | ~300 行 Rust | 消除短板，提升使用体验 |
| **P2** | YAML 规则增加 MITRE tags 字段 | ~50 行 + 更新 profile | 最低成本补 MITRE 短板 |
| **P3** | HTML 报告 | ~500 行 Rust + 模板 | 提升报告能力 |
| **P3** | Sigma 引擎（独立工具） | ~1000+ 行 Rust | 缩小和 Hayabusa 最大差距 |

---

## 6. 附录

### 6.1 EvtxECmd Maps JSON 格式参考

```json
{
  "MapDescription": "An account was successfully logged on",
  "EventId": 4624,
  "Provider": "Microsoft-Windows-Security-Auditing",
  "Channel": "Security",
  "Properties": [
    {
      "Property": "PayloadData1",
      "PropertyValue": "Target: %TargetDomainName%\\%TargetUserName%",
      "Values": [
        { "Name": "TargetDomainName", "Value": "/Event/EventData/Data[@Name='TargetDomainName']" },
        { "Name": "TargetUserName",   "Value": "/Event/EventData/Data[@Name='TargetUserName']" }
      ]
    }
  ]
}
```

### 6.2 Hayabusa Sigma 规则格式参考

```yaml
title: Service Installation
logsource:
  category: service_creation
  product: windows
detection:
  selection_basic:
    Channel: System
    EventID: 7045
  condition: selection_basic
level: medium
tags:
  - T1543.003
  - attack.persistence
```

### 6.3 evtx-tools YAML 规则格式参考

```yaml
name: "登录审计"
table: "security_events"
columns:
  - event_id
  - event_time
  - user_name
  - message
  - map_description
  - payload_data1
  - payload_data2
event_ids: [4624, 4625]
channels: ["Security"]
```

### 6.4 参考链接

- [Hayabusa GitHub](https://github.com/Yamato-Security/hayabusa)
- [hayabusa-rules 仓库](https://github.com/Yamato-Security/hayabusa-rules)
- [Eric Zimmerman/evtx GitHub](https://github.com/EricZimmerman/evtx)
- [EvtxECmd Maps 目录](https://github.com/EricZimmerman/evtx/tree/master/evtx/Maps)
- [rsigma-rs（Rust Sigma 引擎）](https://github.com/rsigma-rs)
- [null-sigma（高性能 Rust Sigma 引擎）](https://crates.io/crates/null-sigma)
- [msgdb-dump（消息库生成工具）](https://github.com/kitsch-9527/msgdb-dump)
