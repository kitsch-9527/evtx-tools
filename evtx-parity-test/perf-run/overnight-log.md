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

## Round 1（进行中 23:57——并发轮请跳过本轮窗口，等"已提交"标记）
B 项探针执行中……
