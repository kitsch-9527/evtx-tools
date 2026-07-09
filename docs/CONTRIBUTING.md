# 贡献指南

> 欢迎贡献 hotech-evtx！本文档说明如何参与开发、提交消息数据、报告问题。

---

## 目录

- [代码贡献](#代码贡献)
- [消息数据贡献](#消息数据贡献)
- [问题报告](#问题报告)
- [开发环境](#开发环境)
- [代码规范](#代码规范)
- [测试要求](#测试要求)
- [PR 流程](#pr-流程)

---

## 代码贡献

### 开发环境

- Rust 1.75+（2024 edition）
- Linux/macOS/Windows（全平台编译）
- 依赖管理使用 Cargo workspace，bun 不参与 Rust 编译

### 快速开始

```bash
# 克隆仓库（目前是 monorepo 的一部分）
git clone <repo-url>
cd hotech-analyzer-gui/crates/hotech-evtx

# 编译全部
cargo build --release

# 运行测试
cargo test

# 仅编译 core（全平台）
cargo build -p hotech-evtx-core

# 编译 CLI
cargo build -p hotech-evtx
```

### 代码规范

- Rust 使用 `snake_case` 命名函数和变量，`PascalCase` 命名类型和 trait
- 模块级注释在 `//!`，函数级注释在 `///`，内部逻辑用 `//`
- 遵循 Rust 2024 edition 风格，用 `cargo fmt` 格式化
- 无 unsafe 代码（除非与 FFI 交互）
- 错误处理使用 `Result<T, String>`（不引入自定义错误类型 crate）

### Domain 层规则

`event.rs` 中的纯数据结构：
- 不引入任何外部依赖
- 不包含 IO 操作
- 纯函数，输入 → 输出
- 必须有单元测试覆盖

### Application 层规则

`parser.rs`、`resolver.rs`、`storage/` 中的代码：
- 允许 IO（evtx、rusqlite、csv）
- 需要集成测试（用临时文件）
- 不包含业务逻辑（业务逻辑在 Domain 层）

### 测试要求

| 层 | 测试类型 | 要求 |
|------|---------|------|
| Domain（event.rs） | 单元测试 | ✅ 必须 |
| Application（parser/resolver） | 集成测试 | ✅ 必须 |
| Application（storage） | 集成测试（临时文件） | ✅ 必须 |
| CLI（命令） | 集成测试 | ✅ 建议 |
```bash
# 运行全部测试
cargo test

# 只运行 core 测试（全平台）
cargo test -p hotech-evtx-core
```

---

## 消息数据贡献

如果您的 Windows 系统上有本项目尚未覆盖的 provider 消息，可以通过贡献 DLL 级别的 .db 文件来帮助项目。

### 贡献步骤

```bash
# 1. 使用 msgdb-dump 工具提取消息库
msgdb-dump dump --out-dir ./my_msgs/

# 2. 复制到项目 data/dlldb/
cp ./my_msgs/dlldb/yourprovider.en-US.1.0.0.0.db data/dlldb/

# 3. 提交 PR
git add data/dlldb/yourprovider.en-US.1.0.0.0.db
git commit -m "feat(msgdb): add YourProvider 1.0.0.0 messages"
```

### 贡献检查清单

- [ ] 文件命名符合 `{dll}.{lang}.{ver}.db` 格式
- [ ] 文件在 `data/dlldb/` 下，不在其他地方
- [ ] 如果更新已有 DLL 的版本，请同时删除旧版本文件
- [ ] 非 Windows 用户也可以提交代码 PR，消息数据 PR 需要 Windows
- [ ] 不要提交包含隐私信息的 .evtx 文件

> **注意：** 消息提取功能（dump）已用 Go 重写，Rust 端的 `hotech-evtx` 只负责 EVTX 解析。消息库由 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump)（二进制 `msgdb-dump`）工具生成。

### 文件大小限制

- 单个 DLL .db 文件通常在 10KB–200KB
- 超过 1MB 的文件可能需要解释原因
- git LFS 不需要（文件小）

---

## 问题报告

### Bug 报告

请包含：
- hotech-evtx 版本（`hotech-evtx --version`）
- 操作系统版本
- 完整命令和执行输出
- （如有）相关 .evtx 文件的信息（大小、来源系统版本）
- （如有）错误日志和截图

### 功能请求

请说明：
- 你要解决的问题
- 你期望的输出/行为
- 替代方案（如果有）

### 数据覆盖问题

如果发现某个 provider 的消息在 events.db 中显示为 NULL（预期有值）：
- 运行 `hotech-evtx db-info <msgdb> --provider <name>` 查看是否已有该 provider
- 如没有，参考"消息数据贡献"部分提交
- 如有但消息为空，提 bug 报告

---

## PR 流程

```
1. fork 仓库
2. 创建特性分支: feat/xxx 或 fix/xxx
3. 开发 + 测试
4. cargo test 全绿
5. cargo clippy 无 warning
6. 提交 PR
7. 维护者 review + approve
8. merge
```

### PR 命名

| 类型 | 例 |
|------|----|
| 新功能 | `feat(msgdb): add Sysmon 15.3.0.0 messages` |
| Bug 修复 | `fix(parser): handle missing Provider.Guid` |
| 性能优化 | `perf(core): lazy-load MessageDb for large evtx` |
| 文档 | `docs: update CONTRIBUTING.md` |
| CI | `ci: update build workflow` |

### CI 检查

PR 自动运行：
- `cargo build --release`（全平台）
- `cargo test`（全平台）
- `cargo clippy`（lint）
- `cargo fmt --check`（格式化）

Windows-only 测试由 CI 的 Windows runner 执行。

---

## 行为准则

本项目遵循 [Contributor Covenant](https://www.contributor-covenant.org/) 行为准则。
