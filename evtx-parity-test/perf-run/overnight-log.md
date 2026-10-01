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

