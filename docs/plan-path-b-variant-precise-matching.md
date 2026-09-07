# 路径 B 复刻精确匹配 — 完整修改与验证计划

> **定案**: Q1=A · Q2=A · Q3=B · Q4=A · Q5=A | 全新设计，零兼容 | 术语 `CONTEXT.md`，结构 `ADR 0001/0002`，可执行规格 `docs/spec-path-b-precise-matching.md`，问题链 `output/system-mismatch-root-cause-2026-09-02.md`
> 基准真值：`evtx_new.db: aevtx`（`evtx_2_Sqlite.lua → evtx2csv.exe → wevtapi!EvtFormatMessage(EvtFormatMessageEvent, eventHandle)`）
> 五问烤问已收敛，本计划为**唯一可执行路径**，按依赖序分 7 阶段，含文件级改动、验收标准与回归门禁。

---

## 0. 总览与成功标尺

| 指标 | 现状 | 目标 | 判定 |
|---|---|---|---|
| System `EID 11/12/13/20/24` 5 条核心变体（TxR/闰秒/关机等） | 错模板，`%1` 空 | 命中正确 Variant，`%N` 正确替换 | 与 `aevtx` 文本归一化比对通过 |
| System 8/30 | 22/30 (73.3%) | 27/30+（5 变体 100%，34/35 独立格式化分歧单列） | `output/evtx-v5-winapi-report.md` 复测 |
| Application/Security/Setup | 30/30 100% | 保持 100%（无回归） | 同表复测 |
| `cargo test -p evtx-tools-core` / `go test ./...` | 绿 | 绿 + 新增 Variant/Fallback 单测 | CI 门禁 |

非目标：兼容 `v5` 旧 `provider_messages` 单模板表；`evtx2csv` 的 `;;` + Binary 转储与 `0x` 进制分歧在 34/35 单列，不纳入 Variant 回归。

---

## 1. 阶段 P0 — 基线固化与门禁脚本（先立靶，再开枪）

**产出**：`output/system-mismatch-root-cause-2026-09-02.md` 已有；本阶段补**可重复的门禁脚本与回归表**。

| 任务 | 文件 | 动作 | 验收 |
|---|---|---|---|
| P0-1 冻结真值快照 | `evtx_new.db` `evtx/*.evtx` | `sha256sum evtx/*.evtx` 入 `docs/regression/evtx.sha256`；`SELECT EventID, Provider, Message FROM aevtx WHERE Channel='System' AND EventID IN (11,12,13,20,24,34,35) ORDER BY rowid LIMIT 50` 导出 `docs/regression/aevtx-system-variants.csv` | 真值可重放 |
| P0-2 门禁脚本 | `output/compare_with_reference.py` 扩展或新增 `scripts/regression-system-variants.py` | 输入：`aevtx` vs `output/evtx-parsed-winapi/System.db` 的归一化 `Message` 比对；输出：`output/regression-report.md` 含 8 条逐条 diff 与通过率 | `python scripts/regression-system-variants.py` 一键复测 |
| P0-3 冻结当前失败基线 | `output/evtx-parsed-winapi/System.db` | 记录 `rec 861/3/13/41` 等 5 条的 `event_data` 与错消息快照 | 改动前后可 diff |

**门禁（必须绿才进 P1）**：P0-2 脚本在现状下复现 8/30 失败，且 5 条变体失败原因均为“错参数映射→空替换”。

---

## 2. 阶段 P1 — 数据模型（Schema）

| 任务 | 文件 | 改动 | 验收 |
|---|---|---|---|
| P1-1 新表 DDL | `msgdb-dump/schema/v6.go` 新建（或 `v5.go` 原地扩展，选新建以显式全新设计） | 建 `provider_message_variants`，`PRIMARY KEY (provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)`，`provider_guid TEXT NOT NULL`，`message/parameters/level_name/keywords_name` 列；索引 `idx_pmv_lookup(provider_name, lang, event_id)` `idx_pmv_guid(provider_guid, lang, event_id)` | `go test ./schema -run TestV6` 建表成功 |
| P1-2 兼容视图退役 | `msgdb-dump/schema/v5.go: SchemaV5CompatView` | 删除或改为 `CREATE VIEW dll_messages AS SELECT ... FROM provider_message_variants` 的投影（全新设计，可直接删除） | 无旧视图依赖 |
| P1-3 旧表退役声明 | `docs/adr/0001*` 已定 | `provider_messages` 单模板表不再写入，仅 `provider_message_variants` 为真源 | 文档一致 |

**门禁**：`go test ./...` 绿；`sqlite3 :memory: < v6.sql` 建表无错。

---

## 3. 阶段 P2 — CRIM 变体身份（Q3=B）

| 任务 | 文件 | 改动 | 验收 |
|---|---|---|---|
| P2-1 暴露 VariantKey | `msgdb-dump/crim/crim.go` | 新增 `type VariantKey struct { EventID uint16; Version uint8; Channel uint8; Level uint8; Opcode uint8; Task uint16; Keywords uint64; MessageID uint32 }` 与 `func (e EventEntry) Key() VariantKey`；为 `level=0/opcode=0/task=0` 注释“通配”语义 | 单测覆盖 |
| P2-2 复合键去重单测 | `msgdb-dump/crim/crim_test.go` 新增 | 用真实 `System-Events.dll` 的 CRIM blob 断言：`EID=11` 的 TxR 与 App 变体 `Key()` 不等，各自保留；全等行仅一条 | `go test ./crim -run TestVariantKey` 绿 |

**门禁**：TxR vs App 的 8 元组不等性单测绿。

---

## 4. 阶段 P3 — 生成期重排（Q2=A，P1/P2 依赖）

| 任务 | 文件 | 改动 | 验收 |
|---|---|---|---|
| P3-1 新增 Provider 中心提取 | `msgdb-dump/dump/extractor.go` 新增 `func ExtractProviderVariants(dllPath string, p ProviderInfo, langLCIDs, langCodes []string, d2Strings map[uint32]string) ([]VariantRow, ValueMaps, ParamMapRefs)` | 按 `p.GUID` 调 `crim.ParseProviderDataWithStrings(blob, *p.GUID, d2Strings)` 定向取 `WEVT`；`map[VariantKey]EventEntry` 去重；每 `EventEntry.MessageID → MUI RT_MESSAGETABLE` 取模板；`TTBL` 取 `parameters` | `EID=11` 产出 ≥2 行 Variant |
| P3-2 删除单键去重 | `msgdb-dump/dump/extractor.go: bestEvents map[uint16]` `buildMessageIDMap` 的高 version 胜 | 整段删除或改为 VariantKey 去重；`applyCRIMMessageMapping` 退役 | 无 `map[uint16]` 残留 |
| P3-3 重排总体流水 | `msgdb-dump/dump/run_windows.go: RunDump` | 废 `dllMap[per DLL] → DllGroup` 批量复制；改为 `providers → per Provider ExtractProviderVariants → []ProviderVariantGroup{Provider, DLLPath, VariantsByLang}`；`EventMessageFile` 含 `;` 的 Provider 以 CRIM 主 DLL 为归属，MUI 合并 de-dup 的 `D2Strings` | 46 Provider 各自独立行，`SELECT COUNT(DISTINCT provider_name) WHERE dll_id=163` 在新表仍 46 但互不染 |
| P3-4 新写入路径 | `msgdb-dump/dump/writer_sys.go` 新增 `func CreateSystemDBVariants(dbPath string, groups []ProviderVariantGroup, osInfo OsInfo, langCodes []string) error` | `insertVariantMessagesBatch` 批量 `INSERT OR IGNORE` 入 `provider_message_variants`；`fix_orphaned.go` 退役 | `output/msgdb-v6-full-final/*.db` 可生成 |
| P3-5 清理 WinAPI 单模板兜底的滥用 | `msgdb-dump/dump/run_windows.go: extractMessagesViaWinAPI` | 仅对无 `WEVT` 的纯 `RT_MESSAGETABLE` Provider 保留 `FormatMessageById` 1..1999 枚举；WEVT Provider 不再枚举 | 无变体 Provider 误走枚举 |

**门禁（P3 自检）**：

```powershell
python output/_inspect_msgdb.py  # 改为查 provider_message_variants
# 断言：SELECT provider_name, event_id, version, level, opcode, substr(message,1,80) FROM provider_message_variants WHERE provider_name='Microsoft-Windows-Kernel-General' AND event_id=11 ORDER BY version, level;
# 期望：≥2 行，TxR 与 App 变体并存，且 parameters 分别为 {"ExtraString","TmId"...} 与 {"PackageFullName","ErrorCode"...}
```

---

## 5. 阶段 P4 — 查询期写全领料单（Q4=A，P1/P3 依赖）

| 任务 | 文件 | 改动 | 验收 |
|---|---|---|---|
| P4-1 扩展 ParsedEvent 透传 | `evtx-tools/evtx-tools-core/src/parser.rs` `evtx-tools-cli/src/cmd_parse.rs` | `ParsedEvent` 已含 `level/opcode/task/keywords/channel/version`，`cmd_parse.rs: new_pairs/warmed_up: HashSet<VariantKey>` 改为全元组；`VariantKey` 含 `provider_name/provider_guid/event_id/version/level/opcode/task/keywords/channel` | `new_pairs` 含 8 元组 |
| P4-2 全元组 warmup/get | `evtx-tools-core/src/resolver.rs` 新增 `struct VariantKey` `struct VariantRow` `fn warmup_variant(&mut self, keys: &[VariantKey], lang: &str)` `fn get_variant(&self, key: &VariantKey) -> Option<&VariantRow>` | 批量 `OR` 改为 `(provider_name=? AND event_id=? AND version=? AND level=? AND opcode=? AND task=? AND keywords=? AND channel=?)`；经典 `Qualifiers` 兼容保留 `compute_message_id` 分支 | 单测：TxR 事件的 `level/opcode` 命中 TxR 行 |
| P4-3 订阅替换参数源 | `evtx-tools-core/src/substitute.rs` 调用侧 | `parameters` 取自命中 Variant 行的 `TTBL` 映射，不再错套 `PackageFullName` 到 `ExtraString` | `EID=11` 的 `%1/%2/%3` 非空 |

**门禁**：`cargo test -p evtx-tools-core -- resolver` 新增 `get_variant` 单测绿。

---

## 6. 阶段 P5 — Fallback Ladder（Q5=A，P4 依赖）

| 任务 | 文件 | 改动 | 验收 |
|---|---|---|---|
| P5-1 实现分层回退 | `evtx-tools-core/src/resolver.rs: resolve_variant` | 精确 8 元组 → 去 `channel` → 去 `keywords` → 去 `task` → 去 `opcode` → 去 `level` → 仅 `(provider, event_id)`；SQL 用 `(m.level=0 OR m.level=?)` 作通配权重，`ORDER BY version DESC, level_match DESC, opcode_match DESC` | `EID=13` 的“关闭时间”(level=4) 与“存储文件夹”(level=2) 不串 |
| P5-2 回归表固化 | `evtx-tools-core/src/resolver.rs` `#[cfg(test)]` | 用 `evtx/System.evtx` 的真实 `rec 861/3/13/41` 的 BinXML 字段为输入，断言每级回退命中行 | `cargo test` 回归绿 |

**门禁**：5 条变体在精确命中与逐级回退下均可复现。

---

## 7. 阶段 P6 — 端到端验证（全依赖收敛）

| 步骤 | 命令 | 判定 |
|---|---|---|
| P6-1 生成新库 | `$env:GO111MODULE="on"; go build -o msgdb-dump-final.exe .; .\msgdb-dump-final.exe --out-dir output/msgdb-v6-full-final --lang 1033,2052` | `output/msgdb-v6-full-final/hotech-msg-*.db` 含 `provider_message_variants` 且 `Kernel-General EID=11` ≥2 行 |
| P6-2 解析 | `cargo build --release; .\target\release\evtx-tools.exe parse evtx/System.evtx --msg-db output/msgdb-v6-full-final/hotech-msg-*.db --lang zh-CN -o output/evtx-parsed-v6/System.db`（余 3 通道同） | `output/evtx-parsed-v6/{Application,Setup,Security,System}.db` 生成 |
| P6-3 回归 | `python scripts/regression-system-variants.py --aevtx evtx_new.db --parsed output/evtx-parsed-v6` | System 5 变体 100%，Application/Security/Setup 保持 100% |
| P6-4 全量门禁 | `go test ./...` `cargo test` | 全绿 |
| P6-5 报告 | 覆盖 `output/evtx-v5-winapi-report.md` 为 `output/evtx-v6-winapi-report.md` | 5 变体逐条 diff 清零，34/35 单列为独立格式化分歧 |

---

## 8. 风险与回滚

| 风险 | 缓解 |
|---|---|
| Variant 行数膨胀（单 Provider 多 Variant） | 8 元组主键天然去重；索引 `idx_pmv_lookup` 覆盖查询；`warmup` 批量 `OR` 仍为单次往返 |
| `level=0` 通配误判为强匹配 | SQL 通配权重 + 排序，非等值匹配；单测固化 |
| `EventMessageFile` 多 DLL 合并 | 以 CRIM 主 DLL 为准，MUI 去重合并，已在 P3-3 定义 |

回滚：`output/msgdb-v6-full-final` 与 `output/evtx-parsed-v6` 为新目录，旧 `msgdb-v5-full-final`/`evtx-parsed-winapi` 保留，门禁失败即保留旧库为回退基线。

---

## 9. 产出清单（DoD）

- [ ] `CONTEXT.md` / `ADR 0001/0002` / `spec` 已落盘（本计划前置，已完成）
- [ ] `schema/v6.go` + `provider_message_variants` DDL/索引
- [ ] `crim: VariantKey` + 8 元组去重单测
- [ ] `extractor.go: ExtractProviderVariants` + `run_windows.go` Provider 中心流水 + `writer_sys.go: CreateSystemDBVariants`
- [ ] `resolver.rs: get_variant/warmup_variant` + `cmd_parse.rs` 全元组透传 + `Fallback Ladder`
- [ ] `scripts/regression-system-variants.py` + `docs/regression/aevtx-system-variants.csv`
- [ ] P6 端到端绿，报告覆盖
