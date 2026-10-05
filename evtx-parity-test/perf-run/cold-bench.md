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
