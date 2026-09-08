# parity-test Skill

## 概述
双链路可重复闭环验证台：本机四通道事件查看器经两条解析路径
（`evtx2text.lua` 基线 vs `evtx-tools` 新解析）归一化比对后生成对比报告。
每次改动上游后重跑 `run-all.ps1` 即得同一口径的 `report.md`（人类结论）+
`report.json`（机器可读，下轮迭代的对比基线）+ `report.diff`（差异明细）。
闭环语义：`report.json.verdict` 为 `PASS` 即闭合；为 `DIFF` 则按
`by_channel.*.categories` 定位回归分类，修上游后再跑下一轮。

项目位置：`evtx-parity-test/`（术语别名见其 `CONTEXT.md`）。

## 核心能力
- 四通道本机导出（应用=Application / 补丁安装=Setup / 安全=Security / 系统=System）
- 原脚本中转解析（`evtx2text.lua` 不动，复制到 `msys64-Tools/evtx/` 跑完搬回）
- 双源码重编（`msgdb-dump` 用 `go build`，`evtx-tools` 用 `cargo build --release`）
- 同一双语单文件 en-US / zh-CN 各跑一遍（`--msg-db` 显式指向 `hotech-msg-*.db`，v8 = v7 + eventlog_sources + param_types，ADR 0003/0004/0005）
- 顺序对齐 + 五字段归一化比对（标准库 Python，uv-ready）
- 机器可读汇总 `report.json`（verdict + 分通道 categories，供下轮回归对比）

## 快速开始

前置条件：管理员 PowerShell（步骤 01 需导出 Security 通道）、Go 1.20+、cargo、Python 3.12+。

一键全链路（`evtx-parity-test/` 下执行）：
```powershell
.\run-all.ps1
```
单步（可单独重跑）：`01-export` / `02-baseline-parse` / `03-build-msgdb` /
`04-build-evtx-tools` / `05-new-parse` / `06-compare.py`，详见测试项目 README。

## 比对口径（06-compare.py）

- 对齐：同一文件按出现顺序逐条对齐，基线 `Event[N]` 序号顺序对应新解析 `sequence_number` 顺序。
  基线 `Event[N]` 是 wevtutil 查询序号（从 0 起），不是 `event_record_id`，不可按记录号对齐。
- 主链：wevtutil 文本基线 vs `new-parsed/en`（同语言严格五字段：EventID / Provider /
  Channel / TimeCreated 两边本地时间容差 2s / Message 空白归一）。
- 对照：`new-parsed/zh` 只计覆盖率（条数 / EventID 顺序一致率 / 空消息率），不逐条比 Message。
- Message 分类：A-渲染格式差（已豁免）/ B-裸参数未渲染 / C-无模板fallback / D-实质分歧。
<!--P2-->
## 已核实的关键事实（勿回退）

1. `evtx2text.lua` 无参数、输入写死 `msys64-Tools` 下 `evtx` 子目录、GBK 编码、
   依赖 `lfs` + `wevtutil/tr/sed/perl/iconv`，结尾有 `PAUSE`（02 脚本以 stdin=NUL 跳过）。
2. 02 脚本必须用 `System32` 下 `cmd.exe` 全路径调用，裸 `cmd` 会被 PATH 上某 msys64 的
   `cmd` bash 垫片遮蔽，导致 lua 根本没跑（2026-09-03 实测 0 产出根因）。
3. 基线 Date 看似 UTC（带 Z）实为本地时间：wevtutil 文本渲染本地时间但保留 Z 后缀
   （与 raw XML SystemTime 差 +8h，Setup 首条 EventRecordID=1 实测）；`evtx-tools` 的时间
   转换输出与 raw UTC+8 一致，无上游 bug。基线小数秒被截断补零，容差覆盖。
4. `evtx-tools --format both` 与 `-o` 单路径会让 db 与 csv 抢写同一文件，05 脚本按 db/csv 分两次跑。
5. 新解析 CSV 内每 batch 夹带一行 BOM+表头重复，06 按 `event_id` 非数字过滤；建议上游修存储层。
6. 基线块尾 TRAIL 剥除必须含分号冒号与空白：块尾冒号来自原文末尾 NUL 经 sed 转换，非消息语义。
   曾尝试保留冒号导致全量失配（matched=0），已回退。
7. **R12（2026-09-07，ADR 0004）**：miss cohort 的基线渲染由**源注册状态**决定——未注册源
   EID≠0 → `N/A`（101/101 全量实证）；classic/winevt 注册源 miss → 真消息（EventSystem 即
   winevt miss 也有消息，不得按 winevt 判 N/A）。实现：`eventlog_sources` 附表
   （1272 行快照，v8 必需表），缺表=坏库即错；空表=采集失败=状态未知=特性关。采集器开子键必须用全路径（关闭的父句柄
   开子键会静默失败，2026-09-07 bug）。
8. **R10/R18（2026-09-07，ADR 0005）**：wevtapi 按 CRIM TEMP 描述符 +4 的 TDH
   `(InType,OutType)` 字节类型化渲染，规则表（探针实证，勿凭记忆扩充）：
   `(14,12)/(08,12)→0x%X` 大写、`(15,13)→0x%X` 大写、`(0A,13)→0x%x` 小写、
   `(07,20)/(14,20)→HRESULT FormatMessage`（0x8007xxxx 取低字，查 R15 表回退 0x%08X）、
   `(11,02)→9 位纳秒小数`、`(08,08)→十进制且不接管（map 照常）`。
   hex/bool/HRESULT InType 一律**跳过 value map**（TDH 语义，over-mapping 根因）。
   `param_types` 附表（v8 必需，缺表即错），双采集路径（enrichFromCRIM + ExtractProviderVariants）都挂——
   只挂主分支会漏掉 CRIM+MUI/provider 中心路径的 WU/Kernel-Boot/Wininit 等大簇（实测教训）。
9. **R18**：evtx crate 对 manifest 布尔参数输出 JSON 原生 `true/false`，取值层必须
   `Value::Bool → "true"/"false"`（Kernel-Boot 20「was .」根因）。
10. **R19（2026-09-08，批次 1）**：CRIM VMAP 条目字符串引用是 **0xDn 字符串表家族**
    （0xD2/0xD1/0xD0/0xDC…，按完整 message ID 键），resolves 须按 `(ref>>24)&0xF0==0xD0`
    泛化——只认 0xD200 会漏掉 Ntfs(0xD1)/KPP(0xD0)/System-Events(0xDC) 的全部条目
    （value_maps 8540→29718）。表可能只在 MUI 中（KPP 主 DLL 0 条、MUI 36 条）。
11. **R20（2026-09-08，批次 2）**：①param_types/param_value_maps 是 (provider,eid,index)
    键，但同事件不同 variant 模板的同名 index 可能是**不同参数**（KGA 1 v3 index3=Reason
    vs v4 index3=TimeDeltaInMs）——链接/类型带 param_name 落库，读端按 variant 自身参数名
    校验后挂载（空名=未知=信任）；②bool (13,*) 需接管臂（否则被 enum/value-map 层改写成
    Disabled/Enabled）；③(8,8) 无 map 链接时接管（否则 Layer-1 hex 启发式再 hex 化）；
    ④format_for_display Step 2.5（9 位小数剥 000）已移除——它与 (17,2) 纳秒补位冲突，
    小数位差异由 06 norm_msg_time_frac 在比对层归一；⑤03 重建前必须删旧 DB 文件
    （CREATE TABLE IF NOT EXISTS 会保留旧表结构，param_name 列曾静默缺失）。

## 已知上游缺口（首轮闭环发现；ADR 0003 单文件迁移已修 #1）

1. [已修，已验证] msgdb-dump v6 双写漏映射表：单文件 v7 写三表（5732/8540/13352，2026-09-04 实测），
   v7 首轮 Security D 类 8078→59，`%1843` 缺口已闭合；剩余 59 为长串 GUID/时间格式残留（0.2%）。
2. Variant 降格分歧实例（Application#309，SPP 1016）：事件实例 opcode=0 时阶梯选了 Grace-timer，
   而 wevtutil 现场拿到 Proof-of-Purchase，降格策略与运行时语义的真实分歧点。
3. CSV 表头重复、`--format both` 同路径抢写（见上）。

## 基线（严格口径 v8+R12+R10，2026-09-07，verdict=DIFF，31870/32801 一致）

| 通道 | 基线 | 新解析 | 一致 | 差异构成 |
|---|---|---|---|---|
| Application | 3925 | 3925 | 3833 | B74 + C11 + D7 |
| Setup | 222 | 222 | 222 | 全绿 |
| Security | 26031 | 26031 | 26031 | 全绿 |
| System | 2623 | 2623 | 1784 | B644 + C40 + D155 |

历史：2026-09-05 起点 diff=1127（App D102/Sys D246）；R12 后 1026；R18+R10 后 931。
D 类 348 → 162（-53%）。会话落地：ADR 0004（eventlog_sources 源注册快照 + N/A parity）、
ADR 0005（param_types TDH 类型化渲染）、R18（JSON 布尔参数取值）。
残余 D 主簇：KPP 55×36 / Ntfs 98×28（VMAP 条目缺）、KGA 1×26（枚举缺条目+纳秒）、
EventLog 6013×16（消息源未定位）、KGA 11×9 + DriverFrameworks×2（GUID 花括号小写）、
Wininit 14 / Kernel-Boot 153 / LsaSrv（over-mapping：TDB 忽略 map 而我们应用）、
WER 1001×5（基线管线吃 `?` 产物，`\\?\` vs `\\\`，比对级豁免候选）。

严格口径（2026-09-04 起）：hex 大小写 / GUID 花括号 / SDDL 分号 / 基线 `?` 不再豁免，
上游渲染必须完全一致。Security 全绿实证链：`%1843` 映射修复 + 零 GUID 加花括号 +
`*LogonId/Status/SubStatus/EventSourceId` 大写 + SDDL `S:ARAI(` 扩展。R1（hex 统一小写）证伪回退：
基线同消息内大小写混杂（`0x3E7` vs `0xC20`），大小写来自参数值本身。R2（GUID 去花括号）证伪回退：
花括号 GUID 仅零 GUID 特殊，其余 passthrough（路径 `{A73E…}` 大写 vs SDDL `{cb4…}` 小写为两源）。
B（裸参数1029）/ C（无模板53）为已知未覆盖场景。历史：v6 双写轮 22692/32801；v7 豁免轮 30387/32801。

## 循环闭合用法

1. 改上游（建议先修缺口 1：v6 映射表双写）。
2. 重跑全链路 `run-all.ps1`。
3. 对比新 `report.json` 与本轮 `by_channel.categories` 分布：D 类归零即闭合；新增分类即回归。
4. `report.json` 即下轮的对比基线。

## 相关文档

- 测试项目 README（`evtx-parity-test/README.md`）与术语（`evtx-parity-test/CONTEXT.md`）
- 比对脚本（`evtx-parity-test/scripts/06-compare.py`）
- 集成指南（`integration-skill.md`）、`evtx-tools` 技能、`msgdb-dump` 技能
