# 已知限制

> hotech-evtx 当前版本的已知限制、未实现功能、待优化项。
> 最后更新：2026-09-02 — 变体精准匹配（Variant Precision）+ Q1/Q3 补丁已合入

---

## 按严重程度

| 优先级 | 含义 | 预期解决 |
|--------|------|----------|
| P1 | 影响核心功能，有 workaround | 当前迭代 |
| P2 | 影响体验/性能，可绕过 | 下个迭代 |
| P3 | 边缘场景或增强需求 | 长期 backlog |

---

## P1：影响核心功能

### EVTX build 检测失效

**现象：** 部分 EVTX 文件头 offset `0x40` 读出值为 `0`，导致自动匹配消息库失败。

**影响：** 使用 `--msg-dir` 时无法自动匹配正确的消息库文件，需手动指定 `--msg-db`。

**当前 workaround：** `hotech-evtx parse input.evtx --msg-db /path/to/hotech-msg-*.db -o output`

**优先级：** P1

---

## P2：影响体验/性能

### `--with-xml` 尚未实现

**现象：** `raw_xml` 字段始终为 NULL，即使指定 `--with-xml`。

**原因：** 当前使用 `records_json_value()` 路径，未实现 `from_xml()`。

**优先级：** P3（极少数场景需要 raw XML）

### CSV 输出重复 header

**现象：** 首个空批次写入 BOM+header，首个真实批次再次写入 header，导致 CSV 文件有两行 header。

**原因：** 设计问题。`write_events(&[], ...)` 生成 header，第一个非空 batch 也调用 `write_events` 写入 header。

**优先级：** P2

### `template_instances()` 跳过 JSON 路径未实现

**现象：** 当前走 `records_json_value()` → Value → 字段提取路径，比直接从 BinXML 取 typed values 多一层 JSON 访问开销。

**预期收益（v1.1+）：** `BinXML → template_instances() → 字段提取`，完全跳过 JSON 渲染。

**优先级：** P2

### 消息库 task/opcode 列为空

**现象：** 部分 provider（如 Security-Auditing/adtschema.dll）的 `task`/`opcode` 列在消息库中为 NULL。

**原因：** 消息库中部分 provider 的 `WEVT_TEMPLATE` 的 TASK/OPCO 块未覆盖所有 provider。

**当前表现：** 解析时使用 EVTX 原始数值（如 `Task=12544`），而非可读名称（如 `Logon`）。

**优先级：** P2

### `%%N` 消息引用未展开（已部分修复，残留 Template 间接引用）

**现象：** 消息中出现 `%%14592`（Direction=Inbound）、`%%1833`（Impersonation Level）等引用码。

**当前状态（2026-09-02）：** `parameter_refs` 已落地，Template 层的 `%%N` 与参数值中的 `%%N` 均已展开（双层：TemplatePass + ParamsPass）；残留仅对无 `parameter_refs` 行且 Template 间接引用未覆盖到的 Provider（约 5%）。

**优先级：** P3

---

## P3：边缘场景与增强

### dlldb 文件名使用 LCID 数字而非 ISO 语言代码

**现象：** `dlldb/adtschema.1033.10.0.26220.1000.db`（使用 LCID），应改为 `dlldb/adtschema.en-US.10.0.26220.1000.db`。

**优先级：** P3

### 无 GUID 回退优化

**现象：** 没有 GUID 的 provider 需要名字匹配替代，当前匹配逻辑较脆弱。

**优先级：** P3

### Level 名称提取

**现象：** CRIM LEVL 块包含 level 显示名，当前未提取（severity 已有从 level 值到中文的硬编码映射）。

**优先级：** P3

### CSV / SQLite 输出无进度条

**现象：** 仅输出 `[progress] 20000 events processed` 文本日志，无 ANSI 进度条或百分比。

**优先级：** P3（功能不受影响）

---

## 已修复的已知限制

以下限制已在之前版本中解决，保留在此供历史参考：

| 限制 | 修复版本 | 修复方式 |
|------|----------|----------|
| MESSAGETABLE-only 提取 | phase5 | 新增 WEVT_TEMPLATE（CRIM）+ MUI 伴生文件提取路径 |
| System.evtx 0% 消息解析 | phase5 | MUI 伴生文件 + CRIM EVNT 块解析 |
| EventData 全空（evtx 0.8→0.12 格式变更） | phase6 | 适配 evtx 0.12 平铺 Key-Value EventData 格式 |
| user_name/user_sid 硬编码 None | phase6 | 改从 EventData 的 SubjectUserName/SubjectUserSid 提取 |
| 消息替换参数提取失败 | phase6 | 新增 4 级回退策略（命名映射→位置回退→Data 数组→#text 数组） |
| 两遍扫描性能浪费 | phase6 | 改为单遍懒加载预热 |
| task/opcode 不覆盖 | phase6 | 始终优先使用 DB 的可读名称值 |
| **单线程解析** | phase6+ | `--threads N` + `ParserSettings::num_threads()` + Rayon 并行 + 后台 Writer 线程 |
| **SQLite 写入慢** | phase6+ | multi-row INSERT + `page_size=65536` + `synchronous=OFF` + `journal_mode=MEMORY` |
| **Value→String→Value 往返** | phase6+ | `extract_fields` 返回 EventData Value，`extract_subst_params` 直接引用 |
| **单 EID 多变体错命中（11/12/13/20/24 22/30 失配）** | 2026-09-02 v6 | Provider 归属隔离（`provider_guid→WEVT`）+ `VariantKey` 8 元组 + `Fallback Ladder` + 微秒仅裁 `000` 三倍数；Setup `Servicing 5064→Staged` Textized Companion 直取；`evtx-rates Setup 100% / Security 100% / System Kernel-General 90%` |
| **System 12 微秒 `500000Z→5Z` 过度裁剪** | 2026-09-02 | `format_for_display` 仅裁 `000` 三倍数，`500000Z` 保留 |
| **Setup Servicing 空参（`%1 %2` 为空）** | 2026-09-02 | `extract_subst_params` 对 `current/targetPackageState` 优先取 `Initial/IntendedPackageStateTextized` |
| **WEVT 多变体 providers → 单模板压扁** | 2026-09-02 | `provider_message_variants` 单行每变体，`warmup_variants` + `resolve_variant` |
| **`%%N` 消息引用未展开（部分）** | 2026-09-02 | `parameter_refs` + Template 间接引用的双层展开（TemplatePass ↔ ParamsPass） |
