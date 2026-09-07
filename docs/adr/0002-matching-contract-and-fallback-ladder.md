# ADR 0002: Matching Contract 与 Fallback Ladder

- **日期**: 2026-09-02
- **状态**: 已接受
- **关联决策**: Q4=A, Q5=A

## 背景

`parser.rs: extract_fields` 已具备 `level/opcode/task/keywords/channel/version`，但 `cmd_parse.rs: process_parallel_batch` 的 `new_pairs` 仅收集 `(provider, event_id)`，`resolver.rs: warmup_lang/get` 单键查询，导致 Q1-Q3 存全的 8 元组无法被领用。回退亦无全序，`high version 胜` 随机命中。

候选：

- Q4: A. 外部 CLI/管线不变、内部谓词写全 / B. 签名冻结双读 / C. 生成期 bake 单模板。
- Q5: A. 分层全序回退 / B. 回退单模板 / C. 回退 `FormatMessageById` 枚举。

## 决策

- **Q4=A**：`resolver.warmup_lang/get` 扩展为 `get(provider, event_id, version, level, opcode, task, keywords, channel)` 全元组；`cmd_parse.rs` 将 `ParsedEvent` 的 8 元组透传给 `warmup/new_pairs`，批量 `OR` 列表改为全元组精确匹配。
- **Q5=A**：分层回退全序：
  ```
  精确 (provider, event_id, version, level, opcode, task, keywords, channel)
    → 去 channel → 去 keywords → 去 task → 去 opcode → 去 level → 仅 (provider, event_id)
  同层内 version DESC；level=0/opcode=0/task=0 视为通配，作排序权重 (level_match, opcode_match, task_match) DESC。
  ```
  每级以 `EID 11/12/20/24` 真实 BinXML 回归表固化，`cargo test` 可断言。

## 后果

- 路径 B 的查询语义与 `EvtFormatMessage(EvtFormatMessageEvent, eventHandle)` 等价，可逐级验证。
- 外部契约（`parse <INPUT> -o <OUTPUT> --msg-db --lang`、`cargo test 169` 外部可观测行为）不变。
- 需为 `resolver` 新增回退排序与通配处理；`FormatMessageById` 的 `1..1999` 枚举兜底对多变体 Provider 退役。
