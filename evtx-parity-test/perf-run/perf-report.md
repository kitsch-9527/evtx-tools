# big-evtx 解析性能剖析（2026-10-01）

实测环境：12 核 Windows，E: 盘（98% 满）。工具 `tools/evtx-tools.exe`（2026-09-30 构建），
消息库 `msgdb/hotech-msg-Windows_10_Education-25H2-26220.9343.db`（schema v9）。
标准配置 = `--format csv --msg-db ... --lang zh-CN --threads 0`（默认 12 线程）。
全部串行实测，互不并发；"冷"= 当天该文件/消息库首读，"热"= 页缓存已命中。

## 1. 解析时间表（标准配置）

| 文件 | 大小 | 事件数 | 墙钟 | 吞吐 | 输出 |
|---|---|---|---|---|---|
| Application.evtx | 21.0 MB | 34,255 | 1.34 s | 25,545 ev/s | 19 MB |
| Setup.evtx | 21.0 MB | 14,875 | 2.35 s 冷 / 1.43 s 热 | 6,324 / 10,400 ev/s | 17 MB |
| system.evtx | 53.5 MB | 147,082 | 3.84 s | 38,352 ev/s | 62 MB |
| Security.evtx | 1073.8 MB | 2,034,603 | 104.7 s 冷 / 91.5–94.2 s 热 | 19,433 / 22,200 ev/s | 3182 MB |

- 固定开销 ≈ 0.7 s 热（启动 ~0.15 s + 消息库加载 ~0.55 s），冷再 +0.9 s。小文件被它主导（Setup 占 32–49%）。
- 吞吐与单事件输出字节反相：Security 1,563 B/事件 → 45 µs/事件；system 435 B/事件 → 26 µs/事件。
  渲染+序列化的成本随消息长度线性走，≈17 ns/输出字节。
- 冷缓存惩罚：Security +10.5 s（1 GB EVTX + 消息库首读）。200 万事件级跑批前预热（先 cat 一遍）有约 10% 收益。

## 2. 瓶颈分解（Security.evtx，热缓存）

### 线程曲线（`--threads` 控制 evtx crate 解压池）

| threads | 1 | 2 | 4 | 6 | 8 | 12(默认0) |
|---|---|---|---|---|---|---|
| 墙钟 s | 111.3 | 88.7 | **86.9** | 90.3 | 88.6 | 94.2 / 91.5 |

- **2~4 线程即饱和**；总加速比仅 **1.28×**（Amdahl：~71% 墙钟完全串行 ≈ 38.7 µs/事件，可并行段仅 ~12 µs/事件）。
- 默认 12 线程**反而比 4 线程慢 5~8%**（线程抖动）。文档建议值应为 2–4。

### 写侧与格式

| 变体 | 墙钟 | 结论 |
|---|---|---|
| csv → 磁盘 | 91.5 s | 基准 |
| csv → NUL（零写盘） | 91.1 s | **3.2 GB 写盘成本 ≈ 0**，磁盘 I/O 不是瓶颈 |
| db（SQLite，含统计表） | 101.4 s | +11%（插入+建表） |
| db `--no-stats` | 110.3 s | 统计表成本在方差内，不可见 |
| csv `--with-xml` | 91.9 s | ⚠️ 无效对照：raw_xml 列恒为空（见 §4） |

无消息库对照做不了：v7（ADR 0003）起 CLI 强制要求 msgdb，无库直接报错退出。

## 3. 瓶颈归因（代码级）

管线：`evtx crate 解压+JSON 树（--threads 池）→ 单线程生产者 extract_fields → rayon 批内渲染（每事件抢全局 Mutex）→ 单写线程 CSV/SQLite`。

1. **每事件全局 Mutex + MessageRow 深克隆**（`cmd_parse.rs` process_parallel_batch）：
   200 万次 `db.lock()`，锁内 `resolve_variant(...).cloned()` 深克隆整个 MessageRow
   （template String + parameters/param_map_refs/param_bitmap_refs 等多个 HashMap/HashSet，`lib.rs:115`），
   另有 `provider_str.to_string()` 每事件堆分配、task/opcode 字符串重复 parse。
   锁把 rayon 的"并行渲染"实质串行化——12 核只有 1.28× 的主因。
2. **单线程生产者**（`parser.rs:133` iter_events → extract_fields）：
   evtx crate 每记录建完整 serde_json Value 树，extract_fields 逐字段 String clone，全程串行。
3. **get_param_value_maps 每事件重建 HashMap 合并**（`resolver.rs:439`）。
4. 写线程与磁盘：免费（NUL 对照），被上述更重段掩盖。

### 若做优化，优先级建议
1. MessageRow 缓存命中返回 `Arc<MessageRow>`（原子递增替代深克隆）——单点最大收益；
2. 整批一次锁内完成 resolve_variant 批查，渲染段完全无锁；
3. VariantKey 借用键/intern provider，task/opcode 解析一次复用；
4. extract_fields 并行化（前移进 rayon 段）；
5. `--threads` 文档标注甜点 2–4。

## 4. 顺带发现的两个文档/缺陷项

- **`--with-xml` 无效**：`cmd_parse.rs` 三处硬编码 `raw_xml: None`（1781/1843/2149），开了也不产出 XML 列。
  → **R36 已修复**（见 §5），实测 system.evtx 147,082 事件 raw_xml 3000/3000 采样非空。
- `docs/evtx-tools-usage.md` §4.3 "无消息库快速看事件"已过时：v7 起无库直接报错。
  → R36 已同步改写该节并标注 --threads 甜点。

## 5. R36 优化实测（2026-10-01 下午，同机同盘）

改动（TDD：特征化测试先在旧代码上绿，重构后保持绿；工作区 213+38+3 全绿）：

1. **缺陷修复**：`--with-xml` —— core 新增 `iter_events_with_xml`（第二解析器走 `records()`
   XML 序列化，按 event_record_id 与 JSON 流 zip 对齐，缺侧留空），CLI 接线 `--with-xml`。
2. **Arc 化变体缓存**（resolver/variant_resolver）：`variant_cache: HashMap<VariantKey, Arc<MessageRow>>`，
   resolve 返回 Arc 克隆（原子递增）替代每事件深克隆（~25 次堆分配/事件）；warmup 附加
   （param_value_maps/param_types）改 `Arc::make_mut`；`bitmap_names` Arc 共享。
3. **cohort 二级索引**：`(provider, eid) → Vec<VariantKey>`——梯子回退从全缓存线性扫描
   （安全通道 1000+ 行）降为只扫本 cohort。
4. **整批单锁解析**（cmd_parse `process_parallel_batch` 两段式）：阶段1 一次锁完成
   全批 variant 梯子 + R26 Qualifier 探针 + Layer A/B value maps + %% 模板预展开；
   阶段2 rayon 无锁渲染。每批锁次数从 ~2N+ 降到 1（命中路径）；miss 回退链逐字保留。

结果（Security.evtx 1073.8MB / 2,034,603 事件，标准配置 csv+msgdb+zh-CN）：

| 配置 | 优化前 | 优化后 | Δ |
|---|---|---|---|
| t0/12核 热 | 91.5–94.2 s | **77.1 s** | **−16~18%** |
| t0/12核 冷 | 104.7 s | 89.9 s | −14% |
| t1 热 | 111.3 s | 97.6 s | −12% |
| t4 热 | 86.9 s | 78.1 s | −10% |
| system.evtx 热 | 3.84 s | 2.29 s | **−40%** |
| Application.evtx 热 | 1.34 s | 0.86 s | **−36%** |

（小文件 −36~40% 收益更大：其每事件锁+克隆占比更高。注：15:36 基准组里
r36-app=4.1s 为紧跟 3.2GB 写盘后的磁盘压力噪声，安静复跑 0.855s。）

**验收**：
- `06-compare.py`：compared=45375 matched=45375 **diff=0 exempted=0 PASS**（M1=100% 维持）。
- 新旧 exe 全部 8 个 CSV **SHA256 逐字节一致**；.db 内容全表多重集一致
  （events 6341 行逐行相等；stat_* 行序差异为既有 HashMap 迭代序非确定性——
  同一 exe 跑两次字节也不同，与本次改动无关）。

## 6. 复测：R36 构建四文件时间表（2026-10-01 16:13–16:19，同机同盘）

同一 R36 exe（15:30:36 构建，SHA256 C922AAC3…），标准配置 csv+msgdb+zh-CN+t0，
先 cat 全部 EVTX+消息库预热页缓存，全部热跑、串行执行：

| 文件 | 大小 | 事件数 | 墙钟(热) | 吞吐 | µs/事件 | 输出 B/事件 |
|---|---|---|---|---|---|---|
| Application.evtx | 21.0 MB | 34,255 | 1.02 s | 33,716 ev/s | 29.7 | 552 |
| Setup.evtx | 21.0 MB | 14,875 | 0.83 s | 17,814 ev/s | 56.1* | 1,148 |
| system.evtx | 53.5 MB | 147,082 | 2.55 s | 57,724 ev/s | 17.3 | 440 |
| Security.evtx | 1073.8 MB | 2,034,603 | **72.97 s** | 27,884 ev/s | 35.9 | 1,639 |

\* Setup 含 ~0.7 s 固定开销（启动 ~0.15 s + 消息库加载 ~0.55 s），边际成本仅 ~9 µs/事件——
小文件墙钟被固定开销主导（Setup 占 84%）。

对 §1（R36 优化前同配置热跑）：Security 91.5→73.0 s（−21%）、system 3.84→2.55 s（−34%）、
Application 1.34→1.02 s（−24%）、Setup 1.43→0.83 s（−42%）。比 §5 首轮 R36 复测（77.1 s）
再快 ~5%，为完全预热 + 机器安静下的当前最优数字。

线程对照（Security.evtx 热）：t1=94.4 s / t4=73.2 s / t0(12 核)=73.0 s——
加速比仅 1.29×，Amdahl 串行占比 **f≈0.70**，与 §2 优化前（0.71）几乎不变：
R36 削掉的是锁竞争与深克隆（可并行段的成本），串行生产者段（evtx crate JSON 树 + extract_fields）
未动，仍是主瓶颈。--threads 甜点 2–4，默认 12 核无额外收益。

边际成本与输出字节：system 440 B/ev→17.3 µs、Security 1,639 B/ev→35.9 µs
（≈15.5 ns/边际字节，与 §1 的 17 ns/B 一致）；Setup 反例说明事件同质性也有影响
（同一 provider/eid 高频重复 → 变体缓存命中率近 100%，长模板命中后仅 memcpy 级成本）。

### 当前瓶颈优先级（R36 后仍然成立）

1. **串行生产者 ≈70% 墙钟**：evtx crate 每记录建 serde_json Value 树 + `iter_events`
   单线程 extract_fields（`parser.rs`）。若要再上台阶需并行化 extract_fields（前移进
   rayon 段）或绕过 JSON 树流式提取；
2. 小文件的固定开销 ~0.7 s（消息库加载）——批量跑小文件时可复用进程/常驻服务化；
3. .db 输出 +11%（上午对照），比对/看数用 CSV 更快；
4. 写盘与 SQLite 统计表可忽略。

复跑产物：`log-{Application,Setup,system,Security}-t{0,1,4}.txt`；CSV 产物已即跑即删
（Security 3.34 GB，磁盘 98% 满勿留存）。

## 7. 附带发现与修复：R37 CSV 逐批重复表头（2026-10-01 17:23 部署）

验证 `--with-xml` 时发现：CSV 输出在批次边界混入重复的 BOM+表头行——
`flush_batch`（cmd_parse.rs）每 2 万事件批次复用 `write_events`，而后者每次调用都写
BOM+表头（storage/csv.rs）。实测 Application 34,255 事件产出 **3 条表头**（offset 0/279/35M），
Security 203 万事件约 **102 条**。与 `--with-xml` 无关，所有 CSV 输出皆然；
此前未暴露是因为 06 门比对端容忍垃圾行、SHA256 新旧对照双方同缺陷逐字节一致。

**结论：`--with-xml` 本身 R36 接线已生效**——修复后 `raw_xml` 34,255/34,255 全非空
（csv 与 sqlite writer 均有 raw_xml 列，db 格式同样支持）。

修复（TDD，255 测试绿 + 06 门 PASS 45375 diff=0 exempted=0）：
- `storage/csv.rs`：拆出 `write_csv_header`（一次性 BOM+表头）与 `write_event_rows`（批次只追加行）；
  `write_events` 保留为两者组合；
- `cmd_parse.rs`：run_parse 初始预写改 `write_csv_header`，flush_batch 改 `write_event_rows`；
- 新回归测试 `test_csv_batched_appends_do_not_repeat_header` 固化"表头只出现一次"契约。
- exe 已重建部署（SHA256 2D81ADDB…，build-info 标注 R37）。

## 8. R38：extract_fields 并行化（2026-10-01 晚，同机同盘）

按"成熟解法"执行两件事：①Gustafson 文件级并行跑批（脚本层）；②主菜——把 22 字段
JSON 遍历（extract_fields）从串行生产者前移进 rayon fork-join 段。

### 串行分裂探针（`probe_serial_split_tree_build_vs_extract`，system.evtx 单线程）

| 段 | 耗时 | 占比 |
|---|---|---|
| evtx crate JSON 树构建（`records_json_value`） | 1.835 s | **77%** |
| extract_fields 22 字段遍历 | 0.562 s | **23%** |

→ 可移动的只有 extract 那份；树构建在 evtx crate 内部，须 SAX 化/chunk 分片（远期）才能再动。

### 改动（TDD：257 测试绿 + 06 门 PASS diff=0）

- core：`extract_batch_parallel`（rayon 保序抽取，raw 元组含 raw_xml）+
  `iter_raw_json_values_with_xml`（--with-xml 的 raw 版 zip，与既有 zip 逐字同逻辑）；
  `iter_events_with_xml` 重构为其上的薄封装（既有 2 测试守护无回归）；core 增 rayon 依赖。
- CLI：生产者只迭代 raw 流，每 2 万事件批调用 `extract_and_prepare_batch`
  （并行抽取 → 批内保序赋 sequence_number + warmup 收集，语义逐字保留）→
  原有两段式 resolve+render 不动。

### 实测（标准配置 csv+msgdb+zh-CN，热）

| 配置 | R37 | R38 | Δ |
|---|---|---|---|
| Security t0/12核 | 73.0 s | 66.1 s | −9.4% |
| Security **t4** | 73.2 s | **58.9 s** | **−19.5%** |
| Security t1 | 94.4 s | 85.3 s | −9.6% |
| system.evtx t0 | 2.55 s | 1.73 s | −32% |
| Application.evtx t0 | 1.02 s | 0.83 s | −19% |
| Setup.evtx t0 | 0.84 s | 0.81 s | 持平（固定开销主导） |

- **Amdahl 串行占比 f: 0.70 → ≈0.59**（t4 加速比 1.29×→1.45×），与探针模型一致
  （0.70 − 0.23×0.70 ≈ 0.54，Security 事件 Data 偏小故略高）。
- **t0/12 核现在比 t4 慢 12%**：crate 解压池与 rayon 抽取/渲染池过订阅（此前仅 5~8%）。
  文档已改：大文件建议显式 `--threads 4`。
- 回归验证：06 门 PASS（45375 diff=0 exempted=0）；`--with-xml` 34255/34255 raw_xml
  非空、表头 1 条（R37 修复保持）；剩余串行 ~59% 在 evtx crate 内部。
- 跑批脚本 `perf-run/parse-parallel.sh`（一文件一进程扇出）：big-evtx 实测 4 进程×2 线程
  墙钟 71.7s ≈ 串行总和——**单文件主导的负载吃不到文件级并行**（Gustafson 的适用边界），
  其价值场景是大量同量级文件（单文件上限 1.7×，扇出随核数线性）。

### R38 之后的瓶颈排序

1. evtx crate 串行 JSON 树构建 ≈54% 墙钟——SAX 化（BinXML token 直抽）或 chunk 级
   分片（EVTX chunk 自包含、记录不跨 chunk）才能再上台阶，均为 crate 级工程；
2. t0 过订阅——默认值仍 0，文档建议显式 t4（也可考虑把默认改 4，未动）；
3. 小文件固定开销 ~0.7 s——多文件单进程/常驻化（未做）。

## 附：产物

- `bottleneck.txt` 计时汇总（含各 run 的 ms 与事件数核对）；`log-*.txt` 各 run 完整输出；
  `bench.sh` 可复跑。大体积 CSV/DB 产物已清理（约 12.7 GB）。
- 复跑注意：Git Bash 无 `bc`，计时用 `date +%s%N` 整数运算；内建 `time` 抓不到子进程 CPU 时间。
