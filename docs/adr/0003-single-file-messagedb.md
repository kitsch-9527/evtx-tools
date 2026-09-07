# ADR 0003: 单文件消息库

- **日期**: 2026-09-04
- **状态**: 已接受
- **关联决策**: Q1=A, F1=移植孤儿, F2=删 merge, F3=无库报错

## 背景

`msgdb-dump` 一次运行产出两个文件：v5 系统库（`hotech-msg-*.db`，单模板）与 v6 变体库（`*-variants.db`，8 元组），dual-write。首轮闭环证实 v6 的 `parameter_refs/value_maps/param_value_maps` 三表全 0 行（v4 有数），导致 `evtx-tools` 24.58% 英文消息含 `%1843` 未展开参数。读端同时维护 v4/v5/v6 三套查询分支（`warmup_lang` + `dll_messages` 兼容视图 + variants），`cmd_merge` 读的表在 v5/v6 根本不存在（已断裂），`evtx-batch` 走老单模板路径。

候选：

- A. 修 v6 映射表双写，保留 dual-write（过渡，债留存）。
- B. 单文件 v7，无兼容、无探测、无视图（无债，一次删净）。
- C. 单文件但保留兼容视图/旧读路径（半债）。

## 决策

- **B**：单文件 `hotech-msg-<Product>-<DisplayVersion>-<Build>.db`，`metadata.schema_version=7`，7 张实体表 + 0 视图（`provider_message_variants` 为唯一消息表；三映射表真写；删 `provider_messages/dll_messages/provider_aliases` 实体与视图；删 `writer_sys/fix_orphaned/--alias/--split-dlls/mergeDuplicateProviders`，孤儿合并由 `mergeDuplicateProvidersVariants` 承接；`--import-dll` 本体保留）。
- 读端：`MessageDb::open` 只认 `schema_version=7`；删 `warmup_lang/get/has_variants_table/sqlite_master` 探测；`warmup(VariantKey)` + `param_value_maps` 挂变体行；无库即 `Err`；删 `merge`/`evtx-batch`；`info` 改读 variants 计数；`--msg-dir` 只认单文件（多文件报错）。
- 版本号取 7 而非沿用 6：6 的 `-variants.db` 有三表全空坏历史，同名异物无法区分。

## 后果

- 旧库（v4/v5/v6）打开即报错，需重跑 dump；`merge`/`evtx-batch`/`--alias`/`--split-dlls` 功能退役（由重跑 dump 代替）。
- `evtx-parity-test` 以 `report.json.verdict` 为闭环口径，首轮 v7 全链路重跑验证。
