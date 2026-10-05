# 冷跑基准（含消息库加载）——2026-10-05 晚，E:=Seagate 1TB 7200RPM HDD，32GB RAM

场景：GUI 首跑形态（--format db --threads 4 --msg-db，write_stats 默认开）。
冷缓存模拟：PowerShell 分配+触碰 28GB 强制回收 standby list（evict-standby.ps1）。

| 基准 | 冷 | 热 | Δ（冷惩罚） |
|---|---|---|---|
| A：Setup.evtx 1.1MB（隔离 启动+消息库加载） | **1.06 s** | 1.62 s* | ≈0（噪声级） |
| B：Security.evtx 1.07GB 全管线 | **159.7 s** | 138.5 s | **+21.2 s** |

\* A-hot 略高于 A-cold 属噪声（B 大跑后缓存挤压）；消息库"冷加载"实测 ≈1s——
`load_variant_providers` 的 DISTINCT 走覆盖索引，并非 336MB 全表读。**库加载不是瓶颈。**

## B 分解（热 138.5s = 63.1 计算 + ~75 存储）

| 段 | 实测 | 说明 |
|---|---|---|
| 计算（warmup 5ms + resolve 3.4s + render 59.7s） | 63.1 s | EVTX_TRACE_PHASES，语义面 intrinsic |
| finalize 11 索引构建 | **31.8 s** | 逐个 DROP/重建实测（2M 行，HDD）；R44+logon_type 新增 5 个 |
| 3.4GB db 落盘 + 写回节流 + stats | ~43 s | HDD ~120MB/s 写带宽墙，异步脏页回压 |
| 冷读惩罚（EVTX 1GB + msgdb 冷读，读写混寻道） | +21.2 s | B-cold − B-hot |

## 结论

1. **库加载（用户原假设）洗清**：冷跑 1s，SQLite 按索引取页非全量加载。
2. **真瓶颈 = db 输出的存储侧（~75s）+ 冷读（+21s），根因是 E: 为 HDD**：
   3.4GB 输出落盘 + 11 索引构建都在机械盘上。热跑 138.5s 中计算仅 63s。
3. GUI「解析过慢」= 160s 冷跑 vs 页缓存热跑 59s（CSV 形态）的落差，
   主体是输出盘 I/O，不是解析算力也不是消息库。

## 缓解方向（按杠杆排序，均不动语义）

1. **产物落 SSD**（250GB 860 EVO）：写侧 + 索引构建预期 75s→10s 级（GUI 默认输出目录改 SSD 或安装期选择）；SSD 对照实验按用户指示取消，此为推断。
2. **finalize 索引延迟化**：R44 的 3 列索引（user_name/task_category/keywords_name）
   可延后到 GUI 首次用到对应筛选时由 ensure 路径补建（机制已存在），解析期只建
   record_id/event_time 等基础 7 个 → 省 ~13s/次解析。
3. 长线：输出与输入分盘（读写分离消除 HDD 混寻道），或 msgdb 与 EVTX 同盘预读。

## R47 延迟化实测补遗（2026-10-05 深夜）

| 场景（Security db t4） | 全量 finalize | --defer-indexes | Δ |
|---|---|---|---|
| 热 | 138.5 s | **82.4 s** | **−40.5%**（扩展索引 32s+回压联动消除） |
| 冷 | 159.7 s | 158.4 s | ≈0（盘被 1GB 读+3.2GB 写打满，索引藏在读写等待里） |

- 新日志形态：`messages db loaded in 0.36s`（冷 0.28s——再次证实库加载非瓶颈）、
  progress 行含 elapsed/overall ev/s/batch 速率（尾批可见写盘回压：末批降至 ~20k ev/s）、
  `[done] … in 82.39s (24694 ev/s)`。
- 冷跑结论不变：绑定约束是单 HDD 读写混寻道；代码侧延迟化只在热/温缓存场景兑现。
  GUI 冷跑要质变只有输出换盘（SSD/第二块盘）。

## R47c mimalloc A/B（2026-10-05 深夜，空闲机器双轮，Security db t4 defer）

| 变体 | wall（ps1） | wall（[done]） | CPU（TotalProcessorTime） |
|---|---|---|---|
| A 默认堆 | 96.8 / 101.2 s | 95.89 / 95.15 s | 353.8 s |
| **B mimalloc** | **73.2 / 80.5 s** | **72.47 / 73.77 s** | **276.1 / 285.7 s** |
| Δ | **−20%** | −23% | **−20.6%** |

- 经典归类：多核分配器争用（锁竞争×伪共享变体）——render/extract 每事件几十次
  String 构造、12 线程并行，默认 Windows 堆的中心化分配是渲染段最大非语义开销。
- 方法注意：首轮 A 基线（200.9/188.4s）被并发 GUI 分析负载污染废弃；
  A/B 必须同条件交错/相邻测。CPU 时间对磁盘等待与调度不敏感，适合隔离分配器效应。
- 落地：evtx-tools-cli 与 GUI src-tauri 双侧 `#[global_allocator] MiMalloc`。
- 叠加坐标（Security db t4 defer 热）：R47 前 138.5s → R47 延迟化 82.4s → R47c 73.2s
  （对战役起点 91.5s 的 CSV 形态：db 形态如今反超 CSV 历史值）。
