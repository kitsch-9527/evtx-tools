# 残留清零攻坚计划 v2（2026-09-07，grill-with-docs 评审稿）

> 决策方法：钱学森系统工程视角（边界划定 → 分解协调 → 指标体系 → 效费比），
> 经 7 轮逐题确认。数字基线：report.json 2026-09-07（diff=931，D=162/24 簇）。

## 1. 决策记录

| # | 维度 | 决策 |
|---|---|---|
| Q1 | 系统边界 | D-only；B 类（classic 源采集 718 条）另立专项，不入本轮 |
| Q2 | 验收标准 | 可修清零（~140）+ 豁免清单受控登记（06 白名单化，可审计可关闭） |
| Q3 | 6013 投入 | 有界 research（一轮会话为限；无果转豁免，不无限攻坚） |
| Q4 | 测量校准 | WER 1001 `\\?\`→`\\\` 为基线管线损伤，比对层双向归一（上游渲染不动） |
| Q5 | 指标口径 | 双口径双报：matched/compared 主线 + D 闭环率副线 |
| 调整 1 | 批次排序 | map 数据簇提前至批次 1（最大收益/最大不确定性先暴露） |
| 调整 2 | 豁免范围 | 缩窄：Ntfs 140 NTSTATUS、CAPI2 LRM 摘出硬啃；只留解析器深坑/经典二进制类 |

## 2. 批次计划

| 批次 | 内容 | 预算（清零条数） | 性质 | 退出条件 |
|---|---|---|---|---|
| 1. map 数据簇 | Ntfs 98 错条目取证（`Disabled` vs `is healthy`，疑 BMAP/VMAP 名空间错位）→ KPP 55 / KGA 1 Change Reason VMAP 补采（MapRef 有名无条目，从 WEVT/ManifestData 补） | -60±15 | msgdb-dump 数据 | 条目真不在源 → 该部分转豁免 |
| 2. 机制修复 | 桶 A：类型化渲染对部分 provider 未生效 bug（Kernel-Boot 生效而 KGA/Time-Service 不生效，~36 条被压住）→ 桶 B：bool 接管臂 `(13,*)`（bool 被值映射改写，~15）→ 桶 D：GUID `(15,14)` 花括号小写规则（-11） | -57±10 | evtx-tools 机制 | 每桶独立 03-06 闭环核对 |
| 3. 摘项硬啃 + 有界 research | Ntfs 140 NTSTATUS 消息化（需 NTSTATUS→消息源）；CAPI2 4111 LRM 字符 canon 归一；EventLog 6013 消息源定位；WPD 24577/24579 取值层取证 | -2~22 | research/取证 | research 一轮为限；无果转豁免 |
| 4. 豁免落地收尾 | 白名单登记（目标 ~15 条）+ WER×5 比对层归一实施 + 全量回归验证 + skill 基线/STATUS.md 定稿 | -10±5 | 比对器+文档 | 全量重跑红线复核 |

## 3. 目标（双口径）

- **主线**：matched/compared 97.16%（31870/32801）→ **≈97.60%**（diff 931 → ~785±15）
- **副线**：D 闭环率 **≈88%**（162 条中清零 ~143）
- **回归红线**（一票否决）：Security 26031 全绿、Setup 全绿、B/C 类不得新增

## 4. 豁免清单（登记表，落地于批次 4；根因码：PARSE=解析器深坑 / SRC=消息源未定位 / BIN=经典二进制 / RO=ROI 不足）

| 簇 | 条数 | 根因码 | 根因描述 | 关闭条件 |
|---|---|---|---|---|
| ~~SPP 1004~~ | ~~1~~ | ~~PARSE~~ | ~~参数值本身异常（`0d0xea98a6e5`），evtx crate 值读取层~~ | **已关闭（2026-09-30，夜间 P5/P7/P8）**：P5 重排后连带转绿，P7 确定性门两次复跑均稳定全匹配；EXEMPTIONS 双表同步摘除 |
| CAPI2 4111 | 1 | PARSE（**已闭合 2026-09-30**） | 基线侧 CJK 日期被基线管线打成 `?`（LRM 关联） | **R31 已实施**：`clean_baseline_artefacts` 增 CJK 日期形态手术 |
| SAM 16962 | 8 | BIN（**已闭合 2026-09-30**） | 根因修正：SDDL 值在事件内完好（`EventData Name` 包装形态），丢失于 Strategy 2 位置提取被 `#attributes` 伪键挤位 | **R33 已实施**：Strategy 2 跳过 `#` 伪键 |
| ~~disk 51~~ | ~~1~~ | ~~SRC~~ | ~~经典源二进制 EventData，模板源未定位~~ | **已关闭（2026-09-30，夜间 P8）**：R21 经典映射后该簇不再命中豁免（自然脱豁免）；EXEMPTIONS 双表同步摘除 |
| ~~WER 1001~~ | ~~2~~ | ~~专项（非豁免）~~ | ~~上游渲染器把 WER 报告文件名 GUID 尾段十进制串 hex 化（`3003916586`→`0xb30c212a`）~~ | **已关闭（2026-09-30，夜间 P5）**：Step3 GUID-span 回扫保护（R24），2→0 清零 |
| ~~Wininit 26~~ | ~~1~~ | ~~待归属~~ | **已修（2026-09-15 阶段F 核实）**：LocalStatus（in 0x14/out 0x1F）经阶段E NTSTATUS 渲染臂命中 `parameter_refs` 组 code 0 字面文本 `STATUS_SUCCESS`，与基线逐字符一致，已出 diff | 无（已闭环） |
| ~~EventLog 6013~~ | ~~16~~ | ~~SRC（条件性）~~ | ~~消息源未定位（netevent MUI 已排除）~~（2026-09-14 阶段C证伪：源=netevent.dll.mui RT_MESSAGETABLE 0x8000177D） | **已关闭（2026-09-14，阶段D）：定位到源并接入（msgdb-dump well-known 注入 6013 真实表文本 `%5` 槽位对齐），16 条全部 matched，EXEMPTIONS 白名单两处同步摘除 |
| WPD 24577/24579 | 4+4 | PARSE（**已闭合 2026-09-30**） | 根因修正：EventData 全空，wevtutil 保留占位符字面量（非 Named-Data 对齐问题） | **R32+R34 已实施**：classic 零参字面量透传 + 空串 #text 不算参数 |
| EventLog 6008（System） | 2 | BASE-ARTEFACT（转出登记，2026-09-30 夜间 P6） | 我方文本逐字节正确（真实 LRM U+200E），基线管线 iconv 把 LRM 变 `?`（斜杠形日期未被 R5 手术覆盖） | 基线侧手术扩斜杠形，或白名单 System/EventLog/6008/PARSE（须三处同步+授权） |
| CodexSandboxService 1001/1002（App） | 2 | UNREG-FALLBACK（转出登记，2026-09-30 夜间 P6） | 源未注册，基线把 eid 当 Win32 错误码回退渲染；我方 N/A | resolver 加"无模板→Win32 系统错误表"回退（牵动 N/A-parity 谓词） |
| e1dexpress 32（System） | 8 | MUI-WRONGLANG（转出登记，2026-09-30 夜间 P3/P6） | DriverStore 语言中立 e1dmsg.dll 的 MUI 目录解析错选 es-ES，msgdb zh/en 键下均存西语文案 | msgdb-dump MUI 解析重写（GetSystemMUILanguages/RC 常量） |
| VMnetAdapter 36（System） | 8 | EID-QUALIFIER（转出登记，2026-09-30 夜间 P6） | classic 消息 id=qualifiers<<16\|eid（raw Qualifiers='16392'→0x40080024），msgdb 变体键缺 qualifiers 致链首 IoLogMsg.dll 裸 id 首中（错库） | msgdb-dump qualifier 感知链式查找（与 e1dexpress 同管线可合并） |
| Wininit 23（System） | 1 | TDH-SYMBOL（转出登记，2026-09-30 夜间 P6） | HRESULT 0x10000000 经 wevtutil TDH 内建符号表渲染 "STATUS_SUCCESS"；我方 typed 渲染 0x10000000 | 内建 NTSTATUS/HRESULT 符号表 |
| （预留） | — | — | 批次 1/2 取证中发现的真不可修项 | 逐条登记 |

> 2026-09-30 夜间计划（P1–P7）终态：diff 237→21（全部为上表转出登记簇），exempted=17 条/4 项
> （CAPI2 4111×1、SAM 16962×8、WPD 24577×4/24579×4）；SPP 1004 与 disk 51 两行经验证
> 零命中后 EXEMPTIONS 双表同步摘除（`06-compare.py` + `acceptance-check.py` 同一提交，
> 复跑 diff=21 不变）。

## 5. 方法论约束（全程有效）

- 每个修复先加回归测试再改实现（TDD，仓库既有纪律）。
- 渲染规则先探针实证再写，不凭记忆（crim-probe-intype / wevtutil raw XML）。
- 比对器口径改动（豁免/canon 归一）必须：注释标注缘由与影响面、可一键关闭、
  关闭后跑全量复核（防止豁免掩盖回归）。
- 文档同步：skill 基线三镜像"勿回退事实"随每批次更新。
