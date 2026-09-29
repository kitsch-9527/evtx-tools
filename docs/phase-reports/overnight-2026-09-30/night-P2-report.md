# 夜间 P2 报告：value_maps 双语化 schema v9（msgdb-dump + evtx-tools）

日期：2026-09-30　时间盒：90 min　状态：**完成**（未提交，等编排者统一 jj commit）

## 结论

**Ntfs eid=98 簇 74→0（清零）；总 diff 157→82（System 143→68 + App 14→14）；红线全过；双端单测全绿。**

验收数字（前后对比，06-compare.py 口径）：

| 指标 | 前（brief 基线口径） | 前（report.prev.json 快照） | 后（本轮） |
|---|---|---|---|
| 总 diff | 157（System 143 + App 14） | 237（System 223 + App 14） | **82**（System 68 + App 14） |
| Ntfs eid=98（System） | 74 | 74 | **0** |
| matched/compared | — | — | 44790/44890 = 99.7772% |
| exempted | 18 | 18 | 18（同一批，无新增） |

红线逐项（acceptance-check.py）：

- [x] Security diff=0 ✓
- [x] Setup diff=0 ✓
- [x] 四通道 B/C=0 ✓（App 0→0、Setup 0→0、Security 0→0、System 0→0）
- [x] 无缺失类（对齐断裂）✓ 缺失类 diff=0
- [x] D 总数只减不增 ✓（157→82；按 prev 快照口径 237→82；各通道均不升）
- [x] R19/R20 既有 value-map 修复零回退 ✓（System 严格下降无新增簇；App 14 条持平为同一批 D-实质分歧，matched/diff/exempted 数字与 prev 完全一致）

未过的非红线门（存量，非本阶段引入）：

- [ ] G1/M2 D 闭环率 76.44%（残留 82/348，硬门 80%）——本阶段把 157→82 已把闭环率从 54.9% 推到 76.44%，差 3.56pp（≈13 条）。剩余簇：NetBT（约 38 条）、Resource-Exhaustion-Detector（9 条）、e1dexpress/VMnetAdapter（各 8 条）、Chrome 16、WER 4、SCM 4、EventLog 4、Edge 4、CodexSandbox 4、Wininit 2（report.diff 计数口径，含基线/新两侧段）。整体 verdict=FAIL（M2 未达），建议转后续阶段。
- G0/G3/G4 全过：基线语言无漂移、豁免命中 5 项/18 条与登记表一致、M1 距理想线 ≈0.00pp。

单测：**Go 135 通过 / 0 失败**（5 包，基线全绿 + 新增 5 测试）；**Rust 233 通过 / 0 失败**（29+201+3，基线 230 + 新增 3）。

## 取证（证据命令 + 输出片段）

1. 旧库 value_maps 无 lang 列、PK 三列（SQL 探针，`evtx-parity-test/msgdb/hotech-msg-*.db`）：
   ```
   CREATE TABLE value_maps (... PRIMARY KEY (provider_name, map_name, entry_value))  -- 无 lang
   metadata: schema_version='8'; value_maps 总行数 45891；display LIKE '%healthy%' 26 行（全 en）
   eid98 链接：param_value_maps → ('Microsoft-Windows-Ntfs', 98, '3', 'CorruptionActionState',
               'Microsoft-Windows-Ntfs.VolumeCorruptionActionState', 0)   ← Layer B（事件域 param→map）
   ```
2. Rust 消费点审计（`grep -rn "get_value_map\|resolve_value|value_maps" evtx-tools-{core,cli}/src`）：
   `resolver.rs` 的 `load_value_maps`(354)/`get_value_map`(388)/`get_param_value_maps`(401)/`resolve_value`(423)/`resolve_param_value_maps`(579) 全部单层 `self.value_maps`；调用点 `cmd_parse.rs:58` 唯一；`substitute.rs::apply_value_maps` 只吃 HashMap（双层 merge 放 resolver 侧后语义不变）。已有 v9 双层先例：`load_param_refs(lang)` + `param_refs_fallback`。
3. Go en 偏置合并点：
   - `provider_extractor.go:21-37`：主 DLL D2Strings 先填、MUI 只补缺（`if _, exists := d2Strings[k]; !exists`）；`:52` 的 `pd` 只用该合并表解析一次 → `pd.ValueMaps` 全 lang 共享 en 文本。
   - `extractor.go:70-131`：同一模式，`enrichFromCRIM(result, providers, stringTable)` 单 stringTable。
   - 反证：`extractor.go:616 extractMUICRIMFromBlobs` 已按语言解析（`:627-632` 用该语言 MUI D2Strings），其 ValueMaps 本是各语言的，只是被收集进扁平 map（后 lang 覆盖先 lang）。

## 完成（改动文件）

Schema v9（value_maps 加 `lang TEXT NOT NULL DEFAULT 'en-US'`，PK 加 lang；metadata schema_version=9）：

- `msgdb-dump/schema/schema.go`：value_maps DDL + PK 加 lang（带 v9 注释）。
- `msgdb-dump/schema/schema_test.go`：新增 `TestSchemaDDL_ValueMapsLangColumn`（红→绿）。
- `msgdb-dump/dump/writer_variants.go`：`insertSystemMetadata` 写 "9"；`InsertValueMap` 加 lang 参数。
- `msgdb-dump/dump/run_windows.go`：`valueMapEntry` 加 `ValueMapsByLang`；`writeValueMaps` 的 CREATE TABLE 同步 v9、按 lang 落行（legacy 层落 en-US）；CRIM+MUI fallback 收集按 lang；`importWELMSeed` Step 6 加 `'en-US'` 字面量（v7 seed 无 lang 列）。
- `msgdb-dump/dump/writer_sys_test.go`：夹具表 DDL 加 lang、InsertValueMap 调用补 lang、版本断言 8→9。

萃取端 per-lang（同键 zh 全覆盖 en）：

- `msgdb-dump/dump/types.go`：`DLLResult.ValueMapsByLang map[lang]map[mapName]map[value]display`（legacy ValueMaps 保留）。
- `msgdb-dump/dump/extractor.go`：`overlayStringTable`（base + overlay 同键全覆盖）、`buildStringTablesByLang`、`enrichValueMapsByLang`（顶层 MAPS 块 + per-provider 数据，逐 lang 解析）；CRIM+MUI fallback 收集改 per-lang、en-US 同步进 legacy 层。
- `msgdb-dump/dump/provider_extractor.go`：`ExtractProviderVariants` 返回值加 `valueMapsByLang`；逐 lang 用 MUI D2 overlay 重解析 provider 数据取 ValueMaps/BitMaps。
- `msgdb-dump/dump/run_variants.go`：`BuildProviderVariantGroups` 透传 per-lang maps。
- `msgdb-dump/dump/value_maps_lang_test.go`（新）：`TestOverlayD2Strings`（zh 同键覆盖 en / base-only 保留 / mui-only 新增，红→绿）、`TestInsertValueMap_LangRows`、`TestWriteValueMaps_ByLang`、`TestInsertSystemMetadata_SchemaVersion9`。

渲染端 lang 感知（镜像 param_refs v9 双层）：

- `evtx-tools/evtx-tools-core/src/resolver.rs`：`MessageDb` 加 `value_maps_fallback`；`open()` 版本硬校验 want "9"（错误信息提示 re-run msgdb-dump，注明 v9 加 value_maps.lang）；`load_value_maps(lang)` 目标 lang 行进首选层、en-US 行进回退层；`get_value_map`/`resolve_value` 首选→回退；`get_param_value_maps`/`resolve_param_value_maps`（Layer B，Ntfs 98 走此路径）fallback 为底、首选覆盖 merge；`has_value_maps` 双层。新增 3 测试（zh 优先 / en 渲染不受污染 / merge 次序，红→绿）；fixture 表 DDL 与 metadata 更新 v9。
- `evtx-tools/evtx-tools-cli/src/cmd_parse.rs`：`load_value_maps(&target_lang)`；测试 DB DDL/metadata 同步 v9。

闭环（evtx-parity-test/，顺序 03→04→05→06→acceptance）：

- `03-build-msgdb.ps1`：value map entries 落库 355635 行（旧 45891：zh 与 en 各 45891，全对称覆盖）；新库 schema_version=9，`Microsoft-Windows-Ntfs.VolumeCorruptionActionState` value 0 有 en-US + zh-CN（'运行状况良好。无需执行任何操作。'）双行（Ntfs/Ntfs-UBPM/Microsoft-Windows-Ntfs 三个 provider 各自双语，PK 语义正确）。
- `04-build-evtx-tools.ps1`：`ls -la tools/evtx-tools.exe` mtime Sep 29 23:36 → **Sep 30 00:34**（已核对重编译，排除"未重编译假修复"坑位）。
- `05-new-parse.ps1`：6106×4 通道全量；`06-compare.py`：compared=44890 matched=44790 diff=82 exempted=18。

## 登记-转出

- **M2 D 闭环率硬门未达（76.44% < 80%）**：非本阶段引入（起点即 54.9% 口径），本阶段推进 22.6pp/−75 条。剩余以 NetBT（≈38 条）为主体的网络簇 + Resource-Exhaustion-Detector 等，与本簇（value maps）无因果，转后续阶段；schema v9 基础设施（per-lang 双层渲染）已就绪，后续簇若涉 VMAP 值映射可直接复用。
- App 14 条 D-实质分歧（Chrome 16 计数含两侧段）为渲染格式类存量（hex/GUID 样式），本阶段不动。

## 未决与建议

1. `report.md` 已知事项照旧：新解析 CSV 每 batch 夹带 BOM+表头重复行（write_events 每批写头），建议上游修 storage/csv.rs 或 cmd_parse.rs（与本阶段无关，未动）。
2. value_maps 行数 45891→91782（×2 双语对称），DB 尺寸增加属预期；若后续要压缩，可考虑 en-US 行省略默认值的存储优化（DDL 已有 DEFAULT 'en-US'，未启用）。
3. `ExtractProviderVariants` 对每 lang 重新 `findProviderData`（同 blob 重复解析 N_lang 次），Ntfs 规模下无感；若后续全量 dump 耗时上升，可缓存 per-blob 解析结果。
4. `importWELMSeed` 仍只收 v7 seed（`'en-US'` 字面量落层）；若 WELM seed 未来带 lang 列，需同步放宽 seed 版本校验。
