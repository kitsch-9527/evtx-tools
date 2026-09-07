# hotech-evtx 架构决策记录

> Architecture Decision Records — 关键决策和取舍记录。

---

## ADR-001: 使用 SQLite 作为消息存储格式

**状态：** 已采纳 · 2026-07-06

**背景：** dump 命令需要存储从 Windows DLL 提取的消息模板，供后续 parse 命令离线使用。备选方案：JSON、自定义二进制、SQLite。

**决策：** SQLite。

**理由：**
- rusqlite 已是项目依赖（bundled 模式，零系统依赖）
- 消息数据需要按 provider + event_id 索引查询，SQLite 索引天然支持
- 人类可用 `sqlite3` 直接查看调试
- 不需要定义和解析自定义二进制格式

**后果：**
- 每个文件 10KB–2MB，对 SQLite 来说很小，性能不是问题
- 跨平台（rusqlite bunded 携带完整 SQLite，全平台一致）

---

## ADR-002: 消息预热到 HashMap，不每次查 SQLite

**状态：** 已采纳 · 2026-07-06

**背景：** 解析 Security.evtx 100 万事件时，所有事件只来自 1-3 个 provider。每次事件都查 SQLite 浪费 IO。

**决策：** 打开 .evtx 时批量加载该文件涉及的所有 provider 的消息 → HashMap。

**理由：**
- 100 万事件只查 1-3 次 SQLite（预热）而非 100 万次
- HashMap O(1) 查询 vs SQLite O(log n) 索引查询 + 磁盘 IO
- 预热 SQL：`SELECT * FROM messages WHERE provider_name IN (?)`

**代价：** 内存占用增加 ~2-10MB（取决于 .evtx 涉及的消息数）。

---

## ADR-003: 两级文件（系统汇总 + DLL 级别）而非单一文件

**状态：** 已采纳 · 2026-07-06

**背景：** dump 命令输出什么文件给 parse 用，什么文件给社区用。

**决策：**
- `hotech-msg-{product}-{displayver}-{build}.db` — 系统汇总，parse 的输入
- `dlldb/{dll}.{lang}.{ver}.db` — DLL 级别，社区贡献和版本管理

**理由：**
- parse 用户只需要一个文件（系统汇总），不暴露复杂度
- 社区可以按 DLL 单独贡献，不用提交整个系统库
- 文件小（10-200KB vs 2MB+），容易 git 跟踪和 diff
- 文件名本身就是版本标识，不需要查内部字段

**代价：**
- 维护两套写入逻辑（writer_sys + writer_dll）
- 合并时需要跨文件操作

---

## ADR-004: 使用微软官方产品命名而非纯构建号

**状态：** 已采纳 · 2026-07-06

**背景：** 文件名需要用人类可读的方式标识 Windows 版本。

**决策：** `hotech-msg-win11-24h2-26100.db` 格式（从注册表读取 ProductName + DisplayVersion + CurrentBuild）。

**理由：**
- `hotech-msg-26100.db` 不直观 — 用户要查表才知道是 Win11 24H2
- `hotech-msg-win11-24h2-26100.db` 一眼看懂
- 从注册表 `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` 获取，不需要外部映射表
- 自动降级：注册表读不到时 fallback 到纯构建号

**代价：** 文件名变长了，但可读性收益大于长度代价。

---

## ADR-005: 消息格式化一次做完，不延迟到查询时

**状态：** 已采纳 · 2026-07-06

**背景：** 事件描述的 %1 %2 替换可以在加载时或查询时做。

**决策：** parse 时（加载时）一次做完 substitute，存入 events.message。

**理由：**
- 下游用户直接拿到完整描述，不需要再查任何外部资源
- CSV 输出的 message 列直接可用
- SQLite 查询 events.db 时拿到的就是最终结果
- substitute 是纯字符串操作，开销极小（~0.5μs/evt）

**代价：** 如果不输出 raw_xml，无法恢复原始模板和 EventData 的关系。但 event_data JSON 字段已经保留了原始数据。

---

## ADR-006: 无消息库时降级而非拒绝

**状态：** 已采纳 · 2026-07-06

**背景：** 用户可能没有 messages.db 但仍然想解析 EVTX。

**决策：** `--db` 改为可选。无消息库时 message/task/opcode/keywords 留空，其他字段正常输出。

**理由：**
- "没有消息库就不能用" 的用户体验差
- 很多分析场景只需要原始字段（event_id, provider, timestamp）
- 降级行为清晰：message 列 NULL，不是错误

**代价：** 多了一条代码分支，需要明确文档说明。

---

## ADR-007: skip JSON 分两步走

**状态：** 已采纳 · 2026-07-06

**背景：** `evtx::EvtxRecord::template_instances()` 可以从 BinXML 直接取 typed values，跳过 JSON 渲染。

**决策：** v1.0 走 `into_json_value()`（可靠路径），v1.1+ 走 `template_instances()`（优化路径）。

**理由：**
- System 模板的字段索引在 Windows 版本间有差异
- `into_json_value()` 是 evtx crate 官方推荐的路径，经过验证
- `template_instances()` + `wevt_templates` 需要额外的测试覆盖
- 让 evtx crate 的 `wevt_templates` 模块先成熟

**代价：** v1.0 比最优路径慢 ~25%。但 evtx 0.12.2 本身的性能提升（4×）已经很大了。

---

## ADR-008: NSA WELM 作为种子数据（CC0）

**状态：** 已采纳 · 2026-07-06

**背景：** dump 命令需要英文消息种子数据。WELM 覆盖 Win7→Win10，但只到 2017 年。

**决策：** 使用 NSA WELM（CC0 1.0 Universal，公有领域）作为种子数据，本地 dump 补充中文和新版本消息。

**理由：**
- CC0 许可，无任何法律风险
- WELM 提供英文消息 + task/opcode/level/keywords/parameters 完整字段
- 本地 dump 自动覆盖新版本和第三方 provider
- 中英文自动合并

**代价：** WELM 数据到 2017 年为止，Win10 之后的消息依赖本地 dump。

---

## ADR-009: 单一 CLI 二进制，两个内部 crate

**状态：** 已采纳 · 2026-07-06 · **2026-07-08 更新：dumper 已用 Go 重写**

**背景：** 项目拆多少 crate，发布时给用户几个二进制。

**决策：** 一个 CLI 二进制 `hotech-evtx`，内部两个 crate（core + cli），对外发布一个包。

**理由：**
- 用户不用思考装哪个工具——就一个 `hotech-evtx` 命令
- 内部拆 split 保证编译期隔离
- CLI 依赖 clap，core 不依赖 clap，职责清晰

**代价：** 消息提取（dump）功能由独立的 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)（二进制 `msgdb-dump`）工具提供

---

## ADR-010: 先在 monorepo 内开发，再抽离独立仓库

**状态：** 已采纳 · 2026-07-06

**背景：** hotech-evtx 目前是 hotech-analyzer-gui monorepo 的一部分，但最终要开源独立。

**决策：** 先在 `crates/hotech-evtx/` 内开发成型，发 v1.0 后用 `git subtree` 抽离为独立仓库。

**理由：**
- 开发迭代快（不需要跨仓库协调）
- CI 共享现有配置
- 等接口稳定了再抽离，减少双向同步成本
- GUI 项目通过 `path =` 依赖直接引用

**代价：** 抽离时 preserve git history 需要额外操作（git subtree split）。
