# evet 项目现状说明

> 生成时间：2026-09-07。本文档由 jj 初始提交时的机器可验证状态汇总而成；
> 数字口径以 `evtx-parity-test/report.json` 为准，术语见 `CONTEXT.md`。

## 1. 项目是什么

`evet` 是 Windows 事件日志（EVTX）**离线语义还原**工程：把 EVTX 二进制事件还原成
与 Windows 运行时（`wevtapi` / `wevtutil`）一致的人类可读消息，并用双链路比对闭环
证明一致性。判定基准：本机四通道（Application / Setup / Security / System）事件
经两条路径解析后逐条严格一致（五字段：EventID / Provider / Channel / TimeCreated /
Message）。

- **路径 A（真值）**：`msys64-Tools/evtx2text.lua`（经 wevtutil/wevtapi 渲染）→ `baseline-parsed/`
- **路径 B（被测）**：`msgdb-dump`（静态萃取消息库）→ `evtx-tools`（离线渲染）→ `new-parsed/`
- **闭环**：`06-compare.py` 逐条对齐比对 → `report.json`（verdict 即闭环口径）

## 2. 顶层结构

| 目录 | 内容 | 说明 |
|---|---|---|
| `msgdb-dump/` | Go 萃取器 | PE/CRIM/MUI/注册表 → 单文件 SQLite 消息库（schema v8） |
| `evtx-tools/` | Rust 渲染器 | EVTX 解析 + 消息库查询 + 模板代入（parity 被测端） |
| `evtx-parity-test/` | 比对闭环台 | 01-export … 06-compare 管线脚本 + 报告产物 |
| `msys64-Tools/` | 基线解析器 | `evtx2text.lua`（路径 A，只跟踪顶层脚本，工具链本体不入库） |
| `docs/` | ADR + 研究文档 | ADR 0001-0005 及历史分析报告 |
| `skills/`（+`.agents/`、`.dsh/` 三镜像） | 工程技能基线 | `parity-test-skill.md` 含"勿回退"事实清单 |
| `.agent-teams/` | 历史 agent 协作档案 | 只读归档 |

## 3. 消息库（Single-File MessageDB，schema_version = 8）

`msgdb-dump` 一次运行产出唯一 SQLite 文件（`hotech-msg-<系统>-<build>.db`，约 300MB，
不入版本库——由管线 03 随时重建）。v8 = v7 的 7 张实体表 + 2 张出处表（ADR 0004/0005，
不做新旧库兼容：读端 `MessageDb::open` 硬校验版本，缺表即错）：

| 表 | 职责 |
|---|---|
| `provider_message_variants` | 唯一消息表：8 元组 Variant Identity（ADR 0001）精确匹配，约 158 万行 |
| `providers` / `dll_entries` | Provider 归属与来源 DLL |
| `parameter_refs` | `%%N` 参数引用（含 `Win32-System-Error` 回退层，9813 行） |
| `value_maps` / `param_value_maps` | 枚举/位图值映射及其与事件参数的链接 |
| `eventlog_sources`（0004） | 注册源快照（1272 行）：classic EventLog ∪ WINEVT Publishers |
| `param_types`（0005） | 参数 TDH InType/OutType 字节（71598 行）：类型化渲染依据 |

## 4. 渲染管线（evtx-tools，逐层）

1. miss-cohort 判定：库无命中时，源未注册（不在 `eventlog_sources`）且 EID≠0 → `N/A`（R12）。
2. `%%N` 展开（R15/R17：provider 私有优先，Win32 系统错误回退）。
3. TDH 类型化渲染（R10，`apply_param_types`，规则表实证于 skill 基线 #8）。
4. 枚举启发式（R11）→ CRIM 值映射（Layer 2/2b）→ LUID 大写 hex（R14）。
5. `%1-%99` 位置代入 → 展示格式化。取值层含 JSON 原生 Bool 修复（R18）。

## 5. 当前基线（2026-09-07，verdict=DIFF）

| 通道 | 比对 | 一致 | 差异构成 |
|---|---|---|---|
| Application | 3925 | 3833 | B74 + C11 + D7 |
| Setup | 222 | 222 | 全绿 |
| Security | 26031 | 26031 | **全绿** |
| System | 2623 | 1784 | B644 + C40 + D155 |

- 全量：32801 条比对，31870 一致，diff=931。
- D 类（实质分歧）：**348 → 162**（-53%，2026-09-05 起点 → 2026-09-07）。
- B/C 类为已知未覆盖场景（历史既有，见 skill 基线）。

## 6. 已落地修复（全部 TDD 回归测试保护，勿回退）

| 修复 | 日期 | 内容 |
|---|---|---|
| R1-R7 | 09-03/05 | 比对口径与管线修正（hex/花括号/SDDL 严格化、`?` 手术刀、分号对称化、尾冒号剥除） |
| R8 | 09-04 | Fallback Ladder 平分按 message_identifier 最小仲裁（确定性） |
| R9/R9b | 09-04 | ParamMapRefs 主分支补采 + warmup 挂载 |
| R10 | 09-07 | **TDH 类型化渲染**：`param_types` 采集（双路径）+ `apply_param_types`（ADR 0005） |
| R11 | 09-04 | 有真 map 字段跳过布尔启发式 |
| R12 | 09-07 | **源注册快照 + miss N/A parity**（ADR 0004） |
| R14/R15/R15b/R16/R17 | 09-04/05 | LUID 大写、Win32 系统错误 %%N、provider 维度隔离、MUI companion、参数引用按 provider 分组 |
| R18 | 09-07 | JSON 原生布尔参数取值（此前被吃成空串） |

## 7. 残留问题（D 类 162 条 / 24 簇，根因全部定位）

- **map 条目簇（约 -85，下轮优先）**：KPP 55×36、Ntfs 98×28 等 VMAP 条目缺失；
  Wininit 14 / Kernel-Boot 153 / LsaSrv 的 over-mapping（我们应用了 TDH 会忽略的 map）。
- **GUID 花括号（-11）**：KGA 11×9、DriverFrameworks×2（需 (0f,0e) 渲染规则）。
- **EventLog 6013×16**：消息源未定位（netevent MUI 已排除）。
- **零星**：WER 1001×5（基线管线吃 `?` 产物，比对级豁免候选）、SAM 16962×3（SDDL
  二进制转文本未实现）、WPD×4、SPP 1004×1（参数值本身异常）、CAPI2 4111×1。

## 8. 如何复跑

```powershell
# 一键全链路（evtx-parity-test/ 下，01 需管理员 PowerShell）
.\run-all.ps1
# 或改上游后单步：03（重建消息库）→ 04（重编渲染器）→ 05（重解析）→ 06（比对）
```

测试：`cd msgdb-dump && go test ./...`（注：`crim-probe-566/` 为历史坏探针，
`go build ./...` 会报错，属已知遗留；管线只 build 主包不受影响）；
`cd evtx-tools && cargo test --workspace`。

## 9. 版本控制

- 2026-09-07 起用 **jj（Jujutsu，git 后端）monorepo** 管理整个 `E:\Code\evet`。
- 此前 `msgdb-dump/`、`evtx-tools/` 各自有独立 git/jj 仓库：吸收前已在各自仓库内
  提交最终快照，历史备份于 **`E:\Code\evet-vcs-backup\`**（仓库外，不入库）。
- 不入库（`.gitignore`）：管线大数据（msgdb/new-parsed/baseline-*/tools，约 2.3GB）、
  cargo `target/`（4.2GB）、全部 `*.exe` 构建产物、msys64 工具链本体、`.claude/` 本地配置。

## 10. 文档索引

- 术语基线：`CONTEXT.md`、`evtx-parity-test/CONTEXT.md`
- 架构决策：`docs/adr/0001`（Variant Identity）→ `0005`（TDH 类型化渲染）
- 操作手册：`evtx-parity-test/README.md`、`skills/parity-test-skill.md`（勿回退事实 #1-#9）
- 下轮工作清单：`C:\msys64\tmp\evet-handoff-2026-09-07.md`（会话交接文档）
