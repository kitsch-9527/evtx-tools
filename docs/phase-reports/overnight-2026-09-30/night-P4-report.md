# night-P4 报告：Resource-Exhaustion 2004×9 参数空置 —— diff 44→35

日期：2026-09-30 凌晨。时间盒 45 分钟，实际约 35 分钟（取证约 18 分 + TDD/闭环约 17 分）。

## 结论

| 指标 | 前 | 后 |
|---|---|---|
| diff 总数 | 44（System 30 + App 14） | **35（System 21 + App 14）**，只减不增（-9） |
| RE-Detector 2004 簇 | 9 | **0（清零）** |
| matched | 44828/44890 | 44837/44890（M1=99.8819%） |
| M2 D 闭环率 | 87.36%（上轮实测） | **89.94%**（D 残留 35/348，硬门 80%） |

红线逐项（acceptance-check --prev 全 PASS）：
- [G0] baseline_lang zh-CN→zh-CN ✓
- [G1] M1 99.8819% ≥ 97.53% 下限 ✓；M2 89.94% ≥ 80% 硬门 ✓
- [G2] Security diff=0 ✓；Setup diff=0 ✓；四通道 B/C=0（0→0）✓；无缺失类（缺失类 diff=0）✓
- [G3] 豁免命中 5 项/18 条与登记表一致 ✓（DSAM 16962×8、WPD 24577/24579×8、SPP 1004×1、CAPI2 4111×1）
- 单测：Rust 242 绿（workspace：core 204 + cli 35 + 集成 3；新增 4 测先红后绿）；Go 135 绿（`-count=1 -v` 实测 135 PASS）

硬性验收「该簇 9→0」达成：RE-Detector 2004 ×9 全部与基线逐字一致
（`…以下程序使用了大部分虚拟内存: agent-win.exe (18268) 使用了 7402741760 字节；Quicker.exe (7860) 使用了 1702064128 字节；ZCode.exe (36156) 使用了 801992704 字节。`）。

## 取证（Data 结构证据）

1. **现象复现**（importlib 复用 06-compare.py 的 parse_baseline/load_new_csv/cmp_pair，System 通道 zip 对齐）：
   该簇 9 条（idx 3613/3614/3618/3619/3620/3621/3622/3623/3630）基线实参完整，我方模板命中但参数全空：
   `…以下程序使用了大部分虚拟内存: () 使用了 字节； () 使用了 字节； () 使用了 字节。`
2. **raw XML**（`wevtutil qe … /q:*[System[Provider[@Name="Microsoft-Windows-Resource-Exhaustion-Detector"] and (EventID=2004)]] /f:xml`）：
   事件**无 `<EventData>`**，数据在 `<UserData><MemoryExhaustionInfo xmlns='…/Resource/Exhaustion/Detector/Events'>` 嵌套结构里：
   - `SystemInfo`（8 个叶子：SystemCommitLimit/SystemCommitCharge/ProcessCommitCharge/PagedPoolUsage/PhysicalMemorySize/PhysicalMemoryUsage/NonPagedPoolUsage/Processes）
   - `PagedPoolInfo`/`NonPagedPoolInfo`（各 Tag_1..3 → Name/PoolUsed）
   - `ProcessInfo`（Process_1..6 → Name/ID/CreationTime/CommitCharge/HandleCount/Version/TypeInfo；Process_4..6 为空壳 Name=""）
   - `ExhaustionEventInfo`（→ Time）
3. **我方解析形态**：new-parsed/zh/System.csv 该 9 行 `event_data` **全空**（全 4 通道 44890 条中空 event_data 的簇**有且仅有这 9 条**）。
   raw_xml 列全事件为空（正常，with_xml 未开）。结论：**不是取值策略没选中，是解析层丢失**——简报嫌疑中的"Data nested/多值形态未被取值层处理"定位修正为 parse 层 `extract_userdata_as_eventdata` 丢弃嵌套结构。
4. **evtx-0.12 真实 JSON 形态**（临时 Rust probe 测试读 baseline System.evtx 真实记录实证，probe 用后即删）：
   嵌套容器在 JSON 里是**无 `#text`/`$` 叶子的 Object**（数字为 JSON Number、空 Name 为 null）。旧逻辑 `Value::Object(o) => o.get("#text").or(o.get("$"))` 落空 → 整支丢弃 → `flat` 为空 → `None`。
5. **msgdb 期望命名**（SQL 单点探针 provider_message_variants）：
   zh-CN 模板 `%21 (%22) 使用了 %24 字节；%28 (%29) 使用了 %31 字节；%35 (%36) 使用了 %38 字节。`，
   parameters 表 63 项：`"21"→Process_1_Name、"22"→Process_1_ID、"24"→Process_1_CommitCharge`（Process_2/3 同构）、`"1"→SystemCommitLimit`。
   即 msgdb 命名规则=**深度2 叶子用裸字段名，深度3 用"结构体名_字段名"**（与 ProcessInfo.Process_1.Name → `Process_1_Name` 精确同名）。

## 完成（改动文件）

| 文件 | 改动 |
|---|---|
| `evtx-tools/evtx-tools-core/src/parser.rs` | `extract_userdata_as_eventdata` 增加**形态门控**的嵌套展平（`flatten_container` 辅助）：仅当根子节点是无 `#text`/`$` 叶子的 Object 时触发（旧逻辑该形态产出零键，行为只增不改）；深度2 丢容器名（`SystemInfo.SystemCommitLimit`→`SystemCommitLimit`），深度3 结构体名作前缀（`ProcessInfo.Process_1.Name`→`Process_1_Name`），null/更深嵌套不产出。新增 3 单测（嵌套展平红→绿、Setup 单层形态不变、#text 包裹叶子不回归）。 |
| `evtx-tools/evtx-tools-cli/src/cmd_parse.rs` | 无实现改动（P3 include_binary 门控原样未动）。仅新增 1 单测：用 msgdb 真实 63 参数表切片验证展平形态经既有 Strategy 1（精确同名查表）命中 9 槽——证明取值层无需改动，缺的只是上游扁平对象。 |

闭环链：`04-build-evtx-tools.ps1`（release，tools/evtx-tools.exe mtime=01:29 已核对）→ `05-new-parse.ps1`（4 通道 en+zh 全量重解析，CSV 该 9 行 event_data 已带 `{"SystemCommitLimit":65288781824,…,"Process_1_Name":"agent-win.exe",…}`）→ `06-compare.py`（diff 35）→ `acceptance-check.py --prev`（**PASS**）。

## 登记-转出

无新增登记项。本簇清零，无遗留。

## 未决与建议

1. **剩余 35 diff 与本阶段无关**：System 21 + App 14（上轮 P3 已鉴别的 e1dexpress 32 错语言 ×8、VMnetAdapter 36 错模板 ×8 等萃取侧问题 + App hex/GUID 族），未触碰。
2. **展平规则的残余观察面**（低风险备忘）：嵌套展平对"根子节点为无叶子嵌套 Object"的 UserData 全局生效。实证影响面=全 4 通道恰这 9 条（空 event_data 簇排查）；且模板未引用的槽位（如 PagedPoolTag_N/TotalProcesses/EventGenerationTime 等 msgdb 命名与展平键不一致者）值不参与渲染（substitute 对未引用 %N 无输出差）。若未来出现新的嵌套 UserData provider 且模板引用了"深度2 裸名/深度3 前缀名"之外的命名（如 `PagedPoolTag_1`），需在展平层补显式别名——当前 corpus 无此形态。
3. 建议 P5+ 若动 msgdb 萃取（e1dexpress/VMnetAdapter 错库问题），顺带核对 nested-UserData 模板命名与 TDH 属性名的对应关系，作为别名表的潜在数据源。
