# evet 夜间值守日志（2026-10-01 晚 → 10-02 08:30）

协议：每 30 分钟一轮；改动必须 TDD 红→绿 + cargo test --workspace 全绿 + 06 门 PASS
（45375/45375 diff=0 exempted=0）才可 `jj describe -m "<长信息>" && jj new` 固化；
门不过 30 分钟内修不完 → `jj restore` 回滚并记录。性能产物（CSV/DB）即跑即删。
路线：A 默认线程 → B 多输入摊销 → C chunk 分片（partition-merge）。语义红线：SAX 化否决。

---

## Round 0（2026-10-01 23:24–23:34，主会话完成，已提交）

**完整项目检查（全绿）**：
- jj 工作副本干净（R36-R38 已入库 1b6dba07）；E 盘 26G 可用（98% 满，产物纪律照旧）
- cargo test --workspace 257 绿；部署 exe 与 target/release SHA256 一致（5814…）
- Go 侧 scoped 测试（msgdb-dump/crim、crim-probe-intype、dump、pe、schema）全 ok

**A 项完成并提交（R39，1be9812b）**：
- 经典归类：co-pool oversubscription（--threads 控制的 crate 解压池 × rayon 全局池叠池）
  → 经典解法：默认池右移到实测甜点（12 核 t0 66.1s vs t4 58.9s，慢 12%）
- TDD：test_default_threads_is_four 红（left:0 right:4）→ `default_value_t = 4` 绿；
  +test_explicit_threads_zero_still_allowed 守护显式 `--threads 0` 全核语义
- 259 测试绿；exe 部署（b3b75e5e）；05（隐式 t4）+06 门 PASS
- 附带实证：输出内容与线程数无关（sequence_number 源自流序非线程序）

**下轮建议**：直接进入 B 项（多输入摊销，探针先行）。值守自动化
automation-bbe62e97 每 30 分钟触发，08:30 后自行 CronDelete 收尾。

## Round 1（23:56–00:08，已提交 d2785fb6）

**B 项探针完成——结论：不实现（低于收益门槛）**：
- 实测（热跑 2 轮，baseline-evtx 4 文件）：round2 SUM=20.7s（Application 0.90 / Security
  17.5 / Setup 0.8-1.5 / System 0.86）；固定开销 ~0.7s/进程 × 4 = 2.8s 仅占 13.5%
- 多输入理论收益 ≈ 10-12%（8 文件外推同量级）——**< 30% 门槛，按协议不做**
- 经典归类：批处理摊销（amortization）——结论：本库真实负载的文件不够小，固定开销
  不是主要矛盾；该优化留给"海量 KB 级小文件"场景再做
- 顺带发现：Setup round2 1.47s vs round1 0.80s——单跑波动 ~0.7s，与磁盘/调度有关，
  小文件计时须多轮取中位

**C① 完成并提交（R40-C①，d2785fb6）**：
- 经典归类：partition-merge（EVTX 原生 64KB 自包含 chunk，records 不跨块）
- `core/src/shard.rs`：ShardReader（逐块 seek+读，脏块容忍与 crate 自身逐字同语义：
  短读跳过+warn / 全零静默 / 坏魔数跳过+warn，fatal 仅 seek 失败）+
  ShardChunk::records_json_value（chunk.parse→iter→into_json_value，与全流同源）+
  count_chunks（(size-4096)/65536，与 crate 算法一致）
- TDD 红（编译失败）→绿 3 测试；关键等价断言：**分片 record_id 流 == 全流逐一相等**
- 262 测试绿（40+219+3）；06 快门 PASS（45375/45375 diff=0；exe 未变故未跑 05）

**下轮建议**：C② 有界通道并行驱动 + 保序归并 TDD（worker 原子领 chunk_id → 读+解析
→ 带 chunk_id 发有界 mpsc（cap 4）；消费端 BTreeMap 缓存乱序结果、按 chunk_id 连续
序排干——乱序缓冲 ≤ worker 数天然有界，经典 ordered merge of out-of-order partitions）；
等价性红测试=分片全链 record_id 流与 extract_batch_parallel 后 ParsedEvent JSON 序列化
逐字节对照 iter_events。之后 C③ --shard N 开关（默认关）→ C④ Security 基准 vs 58.9s
→ C⑤ 全门禁。值守 automation-bbe62e97 每 30 分钟触发，08:30 后自行 CronDelete 收尾。


## Round 2（00:27–00:50，已提交 9acda652 + 本日志提交）

**C② 并行驱动+保序归并（已提交 074c1ff7）**：
- 经典归类：out-of-order partitions 的 ordered merge + 有界流水线背压
- shard_records_parallel 三线程形：dispatcher（顺序走块，Skipped(id) 上报使游标跨脏文件
  推进）→ N worker 静态轮转各持 cap-1 独立队列（共享 Mutex<Receiver> 的 recv 持锁泊车
  会阻塞全部 worker——锁持有泊车经典陷阱，故轮转）→ merger BTreeMap 连续游标排干
  （乱序缓冲 ≤ worker 数；cap-4 out 通道背压，内存有界）
- TDD 红→绿：workers=1/4 双档，分片全链每记录 JSON Value 与全流逐字段相等 +
  chunk_id 严格递增；263 测试绿；06 快门 PASS

**C③ --shard N CLI 开关（已提交 9acda652）**：
- TDD 红（E0609）→绿（默认 0 断言+显式 N 断言）；shard>0 不建 crate parser，
  rx→flat_map 适配为既有 raw 流，下游 extract/render 零改动；--with-xml 互斥显式报错
- **端到端等价实证：baseline System.evtx --shard 4 与默认路径 CSV SHA256 逐字节一致**
- 264 测试绿；部署后 05+06 全门 PASS（45375/45375 diff=0 exempted=0）

**C④ 首测：分片 4 worker 反而更慢（⚠️ 回归，未达 C⑤ 门槛，不设默认）**：
- Security.evtx --shard 4 热跑 = **85.7s**（事件 2,034,603 精确）vs R38 t4 基线 58.9s
  → +45%，分片路径净亏损
- 初步嫌疑（下轮探针归因）：
  ① worker 端 tree-build 吞吐未随 worker 数扩展（静态轮转无 work stealing，chunk 等
  尺寸假设可能被模板复杂度方差破坏）
  ② ParserSettings 差异：分片路径用 default（crate 全流路径可能有共享模板/program
  缓存或不同 codec 设置）
  ③ 池拓扑：4 shard worker + rayon 全局池（extract+render 12 线程）叠加行为未知
- 归因实验设计：①分片 tree-build-only 吞吐 vs iter_raw_json_values（隔离 worker 段）
  ②workers 2/4/8/12 曲线 ③ParserSettings 对齐 crate 全流路径设置

**下轮建议**：C④ 归因三连探针；若 worker 段本身吞吐不足则 C 项定性为「crate 内部
已有更优 chunk 管线，公开 API 分片无增益」并留 --shard 开关+报告记录；若对齐
ParserSettings 后逆转则重测。值守至 08:30 自行收尾。

## Round 3（00:56–01:1x，已提交——归因收档+R41 探针）

**C④ 归因实锤（代码级）**：evtx 0.12 `records_json_value` 底层 `chunk_pipeline`
在 num_threads>1 时已内建与本分片完全同构的并行管线（rayon::spawn 派发 chunk 解析+
序列化、max_in_flight=num_threads×2、BTreeMap 按 seq 重排、Bump arena 池复用、
懒拉取）——t1 走内联串行分支、t4 即 4 路 chunk 并行。**§8「剩余串行 ~59% 在 crate
树构建」结论修正**：树构建 t4 下已并行；分片 +45% 回归=在已并行资源上叠加冗余管线
（+6 OS 线程/+2 跳通道/每块 Vec 中转/无 arena 复用）。经典归类=reinvented wheel /
redundant parallelism；教训=API 可行性探针做了，但缺「crate 内部已有同构实现」的
文献综述。处置：--shard 留开关（默认关、docs 标注不推荐）、报告 §9 收档。

**R41 立项探针（EVTX_TRACE_PHASES 阶段计时，env 门控零行为变化）→ no-go**：
- system.evtx：resolve(锁内)=165ms ≈10%；Security.evtx：3.16-3.3s ≈4-5%
  ——远低于 20% 门槛（R36 整批单锁后锁已不是矛盾）
- render(rayon) 占 80-95% 且已并行，其内部为 R31-R34 语义密集路径，不动
- 264 测试绿；05+06 全门 PASS（45375/45375 diff=0）

**结论：性能战役到达自然底部**——落地 R36-R39 四项（Security 91.5→58.9s，−36%），
三个否决各有数据（B=10-12%、C=+45%、R41=4-10%）。剩余轮次转验证轮：每轮
cargo test --workspace + 06 门 + 磁盘/jj 状态核对，无新问题不动代码；08:30 总结收尾。

## Round 4（01:26–01:31，验证轮，已提交）

- cargo test --workspace 264 绿（41+220+3）；06 门 PASS（45375/45375 diff=0 exempted=0）
- exe 双侧哈希一致（3ff3e68d，Round 3 仪表化构建）；E 盘 25G 可用；jj 树干净
- 无新问题，代码零改动。后续轮次继续验证轮，08:30 总结收尾。

## Round 5（01:57–02:02，验证轮，已提交）

- 3 个测试套件全 ok（264 测试）；06 门 PASS（45375/45375 diff=0 exempted=0）
- E 盘 25G 稳定；jj 树干净。无新问题，零改动。继续验证轮。

## Round 6（02:26–02:31，验证轮，已提交）

- 264 测试全绿（3 套件）；06 门 PASS（45375/45375 diff=0）；树干净。无新问题，零改动。

## Round 7（02:56–03:01，验证轮，已提交）

- 264 测试全绿；06 门 PASS（45375/45375 diff=0）；树干净。无新问题，零改动。

## Round 8（03:26–03:31，验证轮，已提交）

- 264 测试全绿；06 门 PASS（45375/45375 diff=0）；树干净。无新问题，零改动。

## Round 9（03:56–04:01，验证轮，已提交）

- 264 测试全绿；06 门 PASS（45375/45375 diff=0）；树干净。无新问题，零改动。

## Round 10（04:26–04:31，验证轮，已提交）

- 264 测试全绿；06 门 PASS（45375/45375 diff=0）；树干净。无新问题，零改动。
