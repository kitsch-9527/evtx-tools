# night-P1 报告 — R12b N/A 精度（registered-but-no-resources）

- 日期：2026-09-29 夜间（计划档 overnight-2026-09-30 / P1）
- 执行代理：P1（evtx-tools R12b）
- 时间盒：60 分钟（实际未超）
- 变更仓库：`evtx-tools`（core+cli）、`evtx-parity-test`（新增 1 个取证探针脚本）

## 结论

**任务完成，验收全过，无红线触发。**

CfgChangeMonitor 簇 80→0，diff 总数 237→157（只减不增）。

### 验收数字（前 → 后）

| 指标 | 前（report.prev.json，09-29 18:33） | 后（report.json，本轮） |
|---|---|---|
| 总 diff | 237（matched 44635/44890） | **157**（matched 44715/44890） |
| System diff | 223（全 D） | **143**（全 D，恰减 80 = CfgChangeMonitor 簇） |
| Application diff | 14（全 D） | 14（不变，非本簇） |
| Security diff | 0 | 0 |
| Setup diff | 0 | 0 |
| baseline_lang | zh-CN | zh-CN |

### 红线逐项（硬性验收）

| 红线 | 结果 |
|---|---|
| CfgChangeMonitor 簇 80→0 | ✓（System 223→143，report.json by_channel） |
| D 总数只减不增 | ✓ 237→157（-80） |
| Security diff=0 | ✓ |
| Setup diff=0 | ✓ |
| 四通道 B/C 不新增 | ✓ B 0→0、C 0→0（Application/Setup/Security/System 全通道，acceptance-check G2） |
| 无缺失类（对齐断裂） | ✓ 缺失类 diff=0 |
| baseline_lang=zh-CN | ✓（G0 一致） |
| 单测全绿 | ✓ `cargo test --workspace`：29+198+3 = 230 通过（基线 228 + 新增 2），0 失败 |

acceptance-check 总体 FAIL，唯一失败项是 **G1/M2 D 闭环率硬门**：54.89%（残留 157/348，硬门 80%）。归属判定见下「未决与建议」——该门是整夜计划累计目标（分母为 2026-09-05 的 D 总数 348），prev=237 时仅 31.9%，同样不达标；P1 单任务关闭 80 条后剩余 143 条 System D（6013×16 等未清簇，G4 披露）+14 条 App D 属 P2+ 阶段工作，非本任务回归。

## 取证

### 1. SQL 实证（msgdb/hotech-msg-Windows_10_Education-25H2-26220.9343.db）

```
SELECT COUNT(*) FROM provider_message_variants
 WHERE provider_name='Microsoft-Windows-Configuration-Change-Monitor'
→ 0
SELECT source_name FROM eventlog_sources WHERE source_name LIKE '%Configuration-Change%'
→ [('Microsoft-Windows-Configuration-Change-Monitor',)]
```

已注册 ∧ 零消息资源，模型前提成立。

### 2. dry-run 模型（scripts/probe-r12b-registered-no-resources.py，复用 06-compare 的 parse_baseline/load_new_csv）

四通道全量枚举「出现在比对中 ∧ 已注册 ∧ variants=0」：

- 命中 provider **仅 1 个**：`Microsoft-Windows-Configuration-Change-Monitor`，基线 80 行，N/A=80、非 N/A=0、eid0=0 → **规则可硬上，无需收窄**。
- 对照组（未注册 ∧ variants=0）13 个 provider，其中 CodexSandboxService.OpenAI.Codex 有 2 条 eid≠0 且基线非 N/A——属既有 R12「未注册→N/A」语义覆盖面（该 provider 实际未注册但基线渲染了系统错误文本），本任务不触碰其语义，改动后行为不变（见 cmd_parse.rs miss 臂：`known && !registered` 分支原样保留）。
- 全库规模：eventlog_sources=1273 行、providers_with_variants=1170 行（两集合均非空 → known 状态成立，谓词精确生效）。

## 完成（改动文件）

1. **`evtx-tools/evtx-tools-core/src/resolver.rs`**
   - `MessageDb` 新增字段 `providers_with_variants: HashSet<String>`（open() 初始化）。
   - 新增 `load_variant_providers()`：`SELECT DISTINCT provider_name FROM provider_message_variants`，表缺失=硬错误；表空=未知=谓词保守 true（镜像 ADR 0004 空 registry 快照语义）。
   - 新增 `pub fn has_any_variant(&self, name: &str) -> bool`：空集保守 true，否则精确 contains。
   - 测试区新增 2 个单测（参照 create_v8_schema/insert_metadata_8 夹具）：
     - `load_variant_providers_registered_without_resources`（先红后绿；首版夹具缺 dll_entries 触发外键约束失败，已补）
     - `has_any_variant_empty_or_unloaded_is_conservative_true`
2. **`evtx-tools/evtx-tools-cli/src/cmd_parse.rs`**
   - `run_parse`：`load_registry_sources()` 后追加 `db.load_variant_providers()?;`
   - miss 臂：保留既有 `known && !registered → N/A` 原样；追加 `known && registered && !has_variants → N/A`（锁中毒时保守默认 `(false, true, true)`，行为不变）。
3. **`evtx-parity-test/scripts/probe-r12b-registered-no-resources.py`**（新增，取证探针，只读）。

TDD 流程：先加单测确认编译期红（方法不存在）→ 实现 → 夹具修外键 → 全绿。

## 闭环记录

- `04-build-evtx-tools.ps1`：release 重编成功，**tools/evtx-tools.exe mtime 09-29 18:31 → 23:36、size 5309440 → 5311488**（已核对，排除「未重编译假象」坑位）。
- `05-new-parse.ps1`：四通道 en/zh 全量重解析完成。
- `06-compare.py`：compared=44890 matched=44715 **diff=157** exempted=18。
- `acceptance-check.py --prev report.prev.json`：G0/G2/G3/G4 全绿；G1 M1 ✓（99.61%）；**G1 M2 ✗**（54.89% < 80%，见上）。

## 登记-转出

- 本任务无转出工单（簇内 80 条全部关闭，零残留）。
- 承接既有登记（非本任务新增）：System 剩余 143 条 D 与 Application 14 条 D 归 P2+ 簇处理；M2 累计硬门待全阶段落地后复核。

## 未决与建议

1. **M2 硬门归属**：acceptance-check 的 `D_baseline_total=348` 分母含已豁免 18 条与历史簇；建议编排者在全部夜间阶段（P1..Pn）收口后统一复核 M2，P1 阶段不应因该累计门回退（本任务前后对比只减不增，红线全绿）。
2. **对照组 CodexSandboxService**（未注册 ∧ variants=0，基线却渲染 Win32 错误文本 2 条）：与 R12b 相反方向的疑似模型偏差（本机注册表快照 vs 基线机状态差异或经典源继承），量级 2 条，建议后续单独立案取证，勿并入本规则。
3. `has_any_variant` 的谓词快照在 parse 启动时一次性加载（DISTINCT 全表扫描，1170 行，开销可忽略）；若未来 msgdb 增量重建，沿用既有「重跑 03→05」管线即可，无需额外失效逻辑。
