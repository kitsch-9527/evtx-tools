# evtx-tools 解析性能优化说明（R36–R40，2026-10-01~02）

适用版本：提交 `1b6dba07`（R36-R38）→ `82c6bcac`（值守收尾），部署 exe SHA256
`3ff3e68d…ba1dc0e4`（含 R39 + R40 + 诊断仪表化）。全部改动 TDD 红绿 + 全量测试绿 +
06 门禁 PASS（compared=45375 matched=45375 diff=0 exempted=0，M1=100% 全程零回归）。
深度数据：`evtx-parity-test/perf-run/perf-report.md` §1–§10（含原始计时与归因）。

---

## 1. 背景与目标

前提：渲染一致性已达成绝对 100%（M1=100.0000%，EXEMPTIONS 双表清空）。
本战役**只优化性能、不动输出语义**——所有路径的输出必须与基线逐字节可对账
（内容级；.db 因 stat 行序天生非确定，比对用 CSV/内容多重集）。

起点画像（2026-10-01 上午，12 核 Windows，标准配置 csv+msgdb+zh-CN+默认线程）：
Security.evtx 1.07GB/203 万事件热跑 **91.5s**（19.4k ev/s），Amdahl 串行占比 f≈0.70，
线程曲线 2–4 线程即饱和（加速比仅 1.28×）。

## 2. 最终成绩单

| 指标 | 战役前 | 战役后 | Δ |
|---|---|---|---|
| Security.evtx（新默认配置） | 91.5 s | **58.9 s** | **−36%** |
| Security t0/12 核 | 91.5 s | 66.1 s | −28% |
| system.evtx t0 | 3.84 s | 1.73 s | −55% |
| Application.evtx t0 | 1.34 s | 0.83 s | −38% |
| Setup.evtx | 1.43 s | 0.81 s | −43%（固定开销主导） |
| 全量测试 | 251 | 264 | +13（全部守护性回归测试） |

最终瓶颈（阶段实测，`EVTX_TRACE_PHASES=1`）：消息替换/类型化渲染（rayon 段）占
80–95%，为 R31–R34 语义密集路径的 intrinsic 成本（~29ns/输出字节），**明确不动**。

## 3. 管线架构（现状）

```
EVTX 文件
  │ evtx crate chunk_pipeline（t>1 时 crate 内建 chunk 级并行：rayon 派发+
  │   max_in_flight=n×2+BTreeMap 按序重排+arena 池；t=1 内联串行）
  ▼
raw 流 (Value, record_id[, raw_xml])          —— 生产者线程，只迭代
  │ 每 20k 条成批
  ▼ extract_and_prepare_batch
  ├─ extract_batch_parallel：22 字段 JSON 遍历 → rayon 保序并行（R38）
  └─ 批内保序 sequence_number 赋号 + variant warmup 收集（语义与串行版逐字一致）
  ▼ process_parallel_batch（R36 两段式）
  ├─ 阶段1 全批一次锁：variant 梯子 + R26 Qualifier 探针 + value maps + %% 预展开
  └─ 阶段2 rayon 无锁渲染（替换/类型化/表驱动）
  ▼ 写线程（CSV/SQLite 快照语义）→ 场景路由表 → 统计表
```

## 4. 逐层修改说明

### R36 锁竞争消除 + --with-xml 接线（1b6dba07）

- **问题**：每事件全局 `db.lock()` + MessageRow 深克隆（~25 次堆分配/事件），
  rayon"并行"渲染被锁实质串行化（12 核仅 1.28×）。
- **经典归类**：锁竞争 / 每事件元数据深拷贝 → 共享不可变数据（Arc）+ 批处理合并锁。
- **修改**：①`variant_cache` 值改 `Arc<MessageRow>`（命中路径原子递增替代克隆）；
  ②cohort 二级索引 `(provider,eid)→Vec<VariantKey>`（梯子回退从全缓存线性扫描降为
  本 cohort）；③`process_parallel_batch` 两段式——阶段1 全批一次锁完成解析，阶段2
  无锁渲染，锁次数从 ~2N+ 降到 1/批，miss 回退链逐字保留；④`--with-xml` 修复生效
  （此前三处硬编码 `raw_xml: None`）：新增 `iter_events_with_xml`（第二解析器 XML 流
  按 event_record_id zip，缺侧留空）。
- **实测**：Security 91.5→77.1s（−16~18%），小文件 −36~40%。

### R37 CSV 逐批重复表头修复（1b6dba07）

- **问题**：`flush_batch` 每 2 万事件批复用 `write_events`，而它每次调用都写 BOM+表头
  ——Application 34,255 事件产出 3 条表头、Security 约 102 条，文件中部混入垃圾行
  （pandas/Excel 批次边界报错）。此前未暴露：06 比对端容忍垃圾行 + SHA256 新旧对照
  双方同缺陷字节一致。
- **经典归类**：资源初始化与增量写入职责未分离 → header/rows 写入器拆分。
- **修改**：`storage/csv.rs` 拆 `write_csv_header`（一次性 BOM+表头）+
  `write_event_rows`（批次只追加数据行），`write_events` 保留为两者组合；
  回归测试固化"表头全文件只出现一次"。

### R38 extract_fields 并行化（1b6dba07）

- **问题**：串行生产者 = crate 树构建 + 22 字段 JSON 遍历（extract_fields）。
  探针 `probe_serial_split_tree_build_vs_extract`（`#[ignore]` + `EVTX_PROBE_FILE`）
  实测后者占串行段 23%（system.evtx：树构建 1.835s / 抽取 0.562s）。
- **经典归类**：fork-join 重排——把无跨事件状态的纯函数抽取前移进并行段
  （保序并行 map）。
- **修改**：core 新增 `extract_batch_parallel`（rayon `into_par_iter` + 索引序 collect
  保序）；CLI 生产者改迭代 raw 流，批级 `extract_and_prepare_batch`（并行抽取→批内
  保序赋号/warmup，语义逐字保留）。core 增 rayon 依赖。
- **实测**：Security t4 73.2→58.9s（−19.5%），f 0.70→≈0.59（t4 加速比 1.29×→1.45×）。

### R39 默认 --threads 0→4（1be9812b）

- **问题**：R38 后 12 核 t0（全核）66.1s 反比 t4 58.9s 慢 12%——`--threads` 控制的
  crate 解压池与 rayon 全局池（抽取+渲染）叠池过订阅。
- **经典归类**：协同线程池过订阅 → 默认池大小右移到实测甜点。
- **修改**：`default_value_t = 4`；显式 `--threads 0` 保留全核语义；顺带实证输出内容
  与线程拓扑无关（sequence_number 源自流序非线程序）。

### R40 chunk 级分片（C①②③，d2785fb6 / 074c1ff7 / 9acda652）——研究开关，**不推荐使用**

- **实现**（`core/src/shard.rs`）：`ShardReader`（逐块 seek+读 64KB，脏块容忍与 crate
  逐字同语义：短读/全零/坏魔数跳过）+ `shard_records_parallel`（dispatcher 顺序走块 →
  N worker 静态轮转独立 cap-1 队列 → merger BTreeMap 连续游标保序，乱序缓冲 ≤ worker
  数）+ CLI `--shard N`（默认 0 关闭，`--with-xml` 互斥显式报错）。
- **等价性**：分片 record_id 流与全流逐一相等（单测）+ System.evtx `--shard 4` 与
  默认路径 CSV SHA256 **逐字节一致**（端到端）。
- **基准否决**：Security `--shard 4` = 85.7s vs 58.9s（+45% 回归）。归因见 §6。
  开关保留为研究遗留，文档已标注不推荐。

### 诊断仪表化（4922f334）

`EVTX_TRACE_PHASES=1` 时输出 `[phase] warmup/resolve(lock)/render(rayon)` 三段累计
毫秒；env 未设零行为变化（全套 264 测试即回归证明）。

## 5. 否决项档案（探针先行，数据说话）

| 候选 | 经典归类 | 探针结果 | 结论 |
|---|---|---|---|
| 多输入摊销固定开销（B） | 批处理 amortization | 4 文件热跑 20.7s 固定开销仅占 13.5%，收益 ~10-12% | <30% 门槛，不做（仅适用海量 KB 级小文件场景） |
| 分片设默认（C⑤） | redundant parallelism | --shard 4 = 85.7s（+45%） | 不设默认，留研究开关 |
| 阶段1 锁并发化（R41） | 锁竞争（已缓解） | resolve(锁内) 占 4-10% << 20% 门槛 | no-go |
| SAX 化（BinXML token 直抽） | 语义面全换 | R31-R34 全部边缘语义需重新证明 | 红线否决（威胁 100% parity 资产） |

## 6. 认知修正（重要）

优化前结论"剩余串行 ~59% 在 evtx crate JSON 树构建"**已被修正**：读 crate 源码实锤，
`records_json_value` 底层 `chunk_pipeline` 在 `num_threads>1` 时**已内建与自研分片完全
同构的 chunk 并行管线**（`rayon::spawn` 派发解析+序列化、`max_in_flight=n×2`、BTreeMap
按序重排、Bump arena 池复用）；`t1` 走内联串行分支。因此：

- R38 的 t1/t4 差值正是 crate 内建 chunk 并行，树构建在 t4 下并不串行；
- 真实串行余量 = 批解析锁段（4-10%）+ 批组装/编排 + 写线程；
- 自研分片的 +45% 回归 = 在已并行资源上叠加冗余管线（+6 OS 线程、+2 跳通道、
  每块 Vec 中转、无 arena 复用）。教训：动手前除了 API 可行性探针，还要查
  **平台是否已有同构实现**（reinvented wheel）。

## 7. 使用者可见变化

1. **默认 `--threads` 从 0（全核）改为 4**；显式 `--threads 0` 仍=全核。大文件跑批
   一般无需再调参。
2. **新参数 `--shard N`**（默认 0=关闭）：chunk 级分片并行研究开关。输出与默认路径
   逐字节一致，但**实测比默认慢 ~45%，不推荐开启**；与 `--with-xml` 互斥。
3. **`--with-xml` 实际生效**（R36 起）：CSV 第 23 列 `raw_xml` / SQLite `events.raw_xml`，
   代价约 +10~30% 解析耗时（第二趟 XML 序列化）。
4. CSV 输出不再含批次边界的重复表头行（R37 前生成的 CSV 建议重跑或过滤垃圾行）。
5. `EVTX_TRACE_PHASES=1`：stderr 输出 `[phase]` 三段耗时（诊断用）。

## 8. 遗留与后续方向

- `--shard` 研究开关与 `EVTX_TRACE_PHASES` env 长期保留（零默认影响）。
- 多输入摊销（B）：仅当出现海量 KB 级小文件采集场景再立项（探针已备）。
- render 路径（~29ns/字节）为语义面 intrinsic，属正确性资产，不做提速手术。
- 值守过程与全部实测：`evtx-parity-test/perf-run/`（perf-report.md §1-10、
  overnight-log.md、log-*.txt、parse-parallel.sh）。

## 附：提交索引

| 提交 | 内容 |
|---|---|
| 1b6dba07 | R36+R37+R38 战役三层合一 |
| 1be9812b | R39 默认线程右移 |
| d2785fb6 | R40-C① shard.rs 遍历器 |
| 074c1ff7 | R40-C② 并行驱动+保序归并 |
| 9acda652 | R40-C③ --shard 开关 |
| 4922f334 | 归因收档 + EVTX_TRACE_PHASES |
| 82c6bcac / fb26684f | 值守总结 + 结束哨兵 |
