# 对比报告（基线解析 vs 新解析）

- 生成时间（本地）：2026-10-02 02:27:15
- 主比对：基线（zh-CN 渲染，G0 自动探测） vs new-parsed/zh（同语言，严格五字段）
- 口径：EventID / Provider / Channel / TimeCreated（两边均为本地时间，直接比，容差2s）/ Message（空白归一）
- 对齐：同一文件出现顺序逐条对齐（基线Event[N]顺序对应新解析sequence_number顺序）
- 对照：new-parsed/en 只计覆盖率（条数/EventID顺序一致率/空消息率），不逐条比Message
- 豁免（ADR 0006）：开启——白名单命中 0 条已移出 diff 计入 exempted（--no-exemptions 关闭复核）

## 结论

- 通过：主链（基线vs en）共比对 45375 条，全部一致

## 分通道统计（主：基线 vs zh）

| 通道 | 基线数 | 新解析(zh) | 比对数 | 一致 | 差异 | zh空消息 | 状态 |
|---|---|---|---|---|---|---|---|
| Application | 11208 | 11208 | 11208 | 11208 | 0 | 0 | ok |
| Setup | 676 | 676 | 676 | 676 | 0 | 0 | ok |
| Security | 27150 | 27150 | 27150 | 27150 | 0 | 0 | ok |
| System | 6341 | 6341 | 6341 | 6341 | 0 | 0 | ok |

## 副侧覆盖率对照（基线 vs en，不比Message）

| 通道 | 新解析(en) | 空消息 | EventID顺序一致/比对 |
|---|---|---|---|
| Application | 11208 | 0 | 11208/11208 |
| Setup | 676 | 0 | 676/676 |
| Security | 27150 | 0 | 27150/27150 |
| System | 6341 | 0 | 6341/6341 |

## 已知事项（比对中发现，非结论）

- 新解析 CSV 内每 batch 夹带一行 BOM+表头重复（evtx-tools write_events 每批都写头），比对时已按 event_id 非数字过滤；建议上游修 storage/csv.rs 或 cmd_parse.rs。
- 基线 Date 看似UTC（带Z）实为本地时间：wevtutil /f:text 渲染本地时间但保留Z后缀（与raw XML SystemTime差+8h，Setup#0/EventRecordID=1实测）；evtx-tools convert_timestamp_local输出与raw UTC+8一致，无上游bug。基线小数秒被截断补零（.6720013→.6720000），容差覆盖。
