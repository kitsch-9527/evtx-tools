# 路径 B 复刻精确匹配 — 可执行规格（钱学森系统工程定案）

> **定案**: Q1=A · Q2=A · Q3=B · Q4=A · Q5=A  |  全新设计，零历史包袱  |  术语以 `CONTEXT.md` 为准，结构决策见 `docs/adr/0001*` `0002*`
> 基准真值：`evtx_new.db: aevtx`（`evtx_2_Sqlite.lua → evtx2csv.exe → wevtapi!EvtFormatMessage(EvtFormatMessageEvent, eventHandle)`）
> 复现锚点：`output/system-mismatch-root-cause-2026-09-02.md`

---

## 1. 目标与非目标

- **目标**：`msgdb-dump` 生成期离线复刻 `EvtFormatMessage(EvtFormatMessageEvent, eventHandle)` 的 `(provider_guid, event_id, version, level, opcode, task, keywords, channel) → EventEntry → message_identifier → MUI RT_MESSAGETABLE` 精确择优，不改 `evtx-tools` 外部解析流程（`parser → resolver → substitute → storage` 的阶段划分与 CLI 契约不变）。
- **非目标**：兼容 `v5` 旧库；保留 `DllGroup` 批量复制；保留 `bestEvents map[uint16]` 单键去重。

## 2. 数据模型

### 2.1 新表 `provider_message_variants`

```sql
CREATE TABLE provider_message_variants (
  provider_name      TEXT NOT NULL,
  provider_guid      TEXT NOT NULL, -- 归属锚点，按 WEVT 块 GUID 写入
  lang               TEXT NOT NULL,
  event_id           INTEGER NOT NULL, -- EventEntry.EventID (u16)
  version            INTEGER NOT NULL, -- EventEntry.Version
  level              INTEGER NOT NULL, -- EventEntry.Level (0=通配)
  opcode             INTEGER NOT NULL, -- EventEntry.Opcode (0=通配)
  task               INTEGER NOT NULL, -- EventEntry.Task (0=通配)
  keywords           INTEGER NOT NULL, -- EventEntry.Keywords (u64)
  channel            INTEGER NOT NULL, -- EventEntry.Channel
  message_identifier INTEGER NOT NULL, -- EventEntry.MessageID (RT_MESSAGETABLE 资源 ID)
  dll_id             INTEGER NOT NULL REFERENCES dll_entries(dll_id),
  message            TEXT NOT NULL,    -- MUI RT_MESSAGETABLE 中 message_identifier 对应模板（含 %1 %2）
  parameters         TEXT,             -- TTBL 段该 Variant 的参数名映射 JSON {"1":"ExtraString",...}
  level_name         TEXT,
  keywords_name      TEXT,
  PRIMARY KEY (provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)
);
CREATE INDEX idx_pmv_lookup ON provider_message_variants(provider_name, lang, event_id);
CREATE INDEX idx_pmv_guid   ON provider_message_variants(provider_guid, lang, event_id);
```

- 旧表 `provider_messages` 退役；兼容视图 `dll_messages` 保留为 `SELECT * FROM provider_message_variants` 的投影或删除（全新设计，可删）。
- `parameter_refs / value_maps / param_value_maps` 保持，但 `param_value_maps` 的外键改为 `(provider_name, event_id, version, level, opcode, task)` 级联至 Variant 粒度（若实现成本高，可暂保持 event_id 级，查询期按 Variant 过滤）。

## 3. 生成期（msgdb-dump）

### 3.1 总体流水重排（`dump/run_windows.go`）

```
枚举 ETW Providers（registry: Publishers）→ 每个 Provider 含 {Name, GUID, DLLPath, ParameterFile, EventMessageFile}
  → 按 provider_guid 分组（非 dllMap[per DLL]），FsNormalizePath 仍用于 MUI 定位
  → 对每个 Provider 独立调 ExtractProviderVariants(dllPath, provider, langLCIDs, langCodes)
  → 聚合为 []ProviderVariantGroup { Provider, DLLPath, VariantsByLang }
  → CreateSystemDBVariants(dbPath, groups, osInfo, langCodes)
```

`EventMessageFile` 含 `;` 多 DLL 的 Provider：以 CRIM 所在主 DLL 的 WEVT 为归属，MUI 按 `FindMUICompanion` 对主 DLL 及其 ParameterFile 的 MUI 去重后合并 `D2Strings`/`RT_MESSAGETABLE`。

### 3.2 单 Provider 提取（`dump/extractor.go: ExtractProviderVariants` 新）

```go
func ExtractProviderVariants(dllPath string, p ProviderInfo, langs, langCodes []string) map[string][]VariantRow
```

1. `combined, _ := pe.ExtractAllFromPE(dllPath)` 取 `CrimBlobs / D2Strings / RTStrings / Version`。
2. 收集 `d2Strings` 合并主 DLL + 各 `langCode` 的 `FindMUICompanion` 的 `D2Strings`（去重）。
3. 按 `p.GUID` 调 `crim.ParseProviderDataWithStrings(blob, *p.GUID, d2Strings)` 得 `ProviderData{Events, TemplateParams, ValueMaps}`。
   - `GUID==nil` 的 Provider 跳过 WEVT 定向，退为 `RT_MESSAGETABLE` 单模板行（`version/level/opcode/task=0`）。
4. **Q3=B 去重**：`map[VariantKey]EventEntry`，`VariantKey = (event_id, version, level, opcode, task, keywords, channel, message_identifier)`。全等方去重，余则多行。
5. 对每个 `EventEntry` 取 `message_identifier` 到 MUI 的 `RT_MESSAGETABLE` 取模板：
   - 主 DLL 的 `combined.Messages[message_identifier]` 优先；否则各 `langCode` 的 `muiCombined.Messages[message_identifier]`。
   - 缺失则跳过该 Variant（对应 `EvtFormatMessage` 找不到模板的事件）。
6. `TemplateParams[message_identifier] → TTBL` 解析参数名映射，写入 `VariantRow.Parameters`。
7. 产出 `[]VariantRow`（含 `ValueMaps/BitMaps/ParamMapRefs` 的按 Variant 归属，用于后续 `value_maps` 写入）。

### 3.3 入库（`dump/writer_sys.go: CreateSystemDBVariants`）

- `insertDLLEntry` 仍按 `dll_path` 去重。
- `insertProvidersBatch` 按 Provider 写入。
- `insertVariantMessagesBatch` 按 Variant 批量 `INSERT OR IGNORE` 入 `provider_message_variants`，主键即 Dedup。
- `fix_orphaned.go` 退役（归属已在提取期正）。

## 4. 查询期（evtx-tools）

### 4.1 领料单写全（`evtx-tools-cli/src/cmd_parse.rs`）

- `ParsedEvent` 已含 `event_id / version / level / opcode / task / keywords / channel / qualifiers / provider_name / provider_guid`（`version/channel` 若缺则补 `0`，`level/opcode/task` 缺省 `0`）。
- `new_pairs/warmed_up` 改为 `Vec<VariantKey>`：`(provider_name, event_id, version, level, opcode, task, keywords, channel)`。
- `warmup_lang` 批量查询改为全元组 `OR`：
  ```sql
  WHERE m.lang = ?1 AND (
    (m.provider_name=? AND m.event_id=? AND m.version=? AND m.level=? AND m.opcode=? AND m.task=? AND m.keywords=? AND m.channel=?)
    OR ...
  )
  ```
  命中即缓存整行 `VariantRow`。

### 4.2 精确优先 + 分层回退（`evtx-tools-core/src/resolver.rs`）

```rust
pub fn get_variant(&self, key: &VariantKey, lang: &str) -> Option<&VariantRow>
// 实现：按 Fallback Ladder 逐级查询缓存/DB
// 精确 8 元组 → 去 channel → 去 keywords → 去 task → 去 opcode → 去 level → 仅 (provider, event_id)
// 同层内 version DESC；level=0/opcode=0/task=0 视为通配（SQL: (m.level=0 OR m.level=?)) 作权重）
```

`process_parallel_batch` 内：

```rust
let mid = compute_message_id(eid, qualifiers); // 保留对经典 Qualifiers 的兼容
let row = guard.get_variant(&VariantKey{provider, eid, version, level, opcode, task, keywords, channel}, lang)
         .or_else(|| guard.get_variant_by_mid(mid, lang)) // 仅对经典 provider 回退
         .or_else(|| fallback_ladder(...))?
```

`parameters` 取自该 Variant 行的 `TTBL` 映射，不再用错 Variant 的 `{"1":"PackageFullName"}` 去套 TxR 的 `ExtraString`。

### 4.3 WinAPI 兜底退役

`extractMessagesViaWinAPI` 的 `FormatMessageById` 1..1999 枚举仅保留为“无 WEVT 的纯 RT_MESSAGETABLE Provider”兜底；对 WEVT Provider 不再调用（其 `EvtFormatMessageId` 语义与精确匹配不等价）。

## 5. 回归与验证

- 以 `evtx_new.db: aevtx` 为真值，`System` 的 `EID 11/12/13/20/24` 为必过回归：BinXML 含 `Level/Opcode` 的事件必须命中对应 Variant，`%1` 空替换判失败。
- `EID 34 vmx86`（未注册 Provider）走 `generate_fallback_message`，与 Lua 的 `;;` + Binary 转储对齐为独立用例，不纳入 Variant 回归。
- `EID 35` 十/十六进制格式化在 `substitute::format_for_display` 单测覆盖。
- `cargo test -p evtx-tools-core` 与 `go test ./...` 需补 `Variant Identity / Fallback Ladder` 单测。

## 6. 实施顺序（依赖序）

1. `crim/crim.go` 暴露 `VariantKey` 与 `ParseProviderDataWithStrings` 的按 GUID 定向能力（已具备，仅需调用侧改）。
2. `dump/extractor.go` 新增 `ExtractProviderVariants` + Q3=B 复合键去重，删 `bestEvents`。
3. `dump/schema`: 新增 `provider_message_variants` DDL 与索引，`writer_sys.go` 新增 `CreateSystemDBVariants`。
4. `dump/run_windows.go` 重排为 Provider 中心流水。
5. `evtx-tools-core/resolver.rs` + `evtx-tools-cli/cmd_parse.rs` 实现全元组 `warmup/get` 与 Fallback Ladder。
6. 回归：`System` 8/30 → 0/30（除 34/35 的独立格式化分歧外，5 条核心变体 100%）。
