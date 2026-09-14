# Phase E 报告（2026-09-15 00:22）

## 结论

- **验收数字**：compared 32801，**matched 32018→32020（+2）**，**diff（report.json 口径）773→771（−2）**，
  exempted 10（不动）。**Ntfs 140×1 已清零 ✓**（渲染 `Failure status: 0xc000000e` →
  `Failure status: A device which does not exist was specified.`，与基线逐字符一致）。
  超收 +1：System 通道另一条 out=0x1F 参数事件（文本 "The device is not ready"）
  同表命中归位，**D 残留 4→2（M2 = 99.43%）**。
- **门：G1 ✓**（M1 = **97.6190%** ≥97.53%，距理想线 97.59% ≈0.00pp）
  **G2 ✓**（Security diff=0、Setup diff=0、四通道 B/C 零新增 A74→74/C11→11/B644→644/C40→40、
  无缺失类；G3 豁免 6 项/10 条与登记表一致；M5 有效分歧 763→761 单调下降）。
  `acceptance-check.py --prev report.prev.json` 全门 PASS，**exit=0**。

## 完成

- **探针**：`param_types` 实测 Ntfs 140 参数 5 `Error`，**InType=0x14（HEXINT32）/ OutType=0x1F**，
  与计划预期 out=0x1F 一致。全库 out_type=31 共 1168 行（MPTF/WinNat/Ntfs 等多 provider）——
  影响面必须封顶（见渲染臂设计）。
- **消息源定位（RT_MESSAGETABLE 路径，验证阶段 C 教训）**：
  `C:\Windows\System32\ntdll.dll` RT_MESSAGETABLE（type 11）name=1 **2975 条**，
  **0xC000000E = 'A device which does not exist was specified.\r\n'** ✓；
  en-US `ntdll.dll.mui` 同表同文本；`ntoskrnl.exe` 表 538 条**无**此文本（非源）。
  基线（zh 环境 wevtutil）对该参数亦输出英文文本——ntdll 内嵌表即源，口径一致。
- **萃取实现**：NTSTATUS 稀疏散布（2975 条散在 37 个 64K 簇、约 280 万扫描空洞），
  R15 的 FormatMessage 暴力扫描不可行 → 新增 `dump/ntstatus.go`：移植
  `probe-6013-msgtable.py` 的 PE/RT_MESSAGETABLE 解析（Go 版，秒级、精确），
  直读 `%SystemRoot%\System32\ntdll.dll`。入库沿用 **R15 模式**：全局 provider 组
  `NTSTATUS-System-Error` 写入 `parameter_refs`（run_windows.go 步骤 11c，03 日志确认
  2975 entries 入库）。
- **渲染臂**：`apply_param_types` 新增 `(_, 0x1F)`（任意 InType）→ 查
  `MessageDb::lookup_ntstatus`（resolver.rs 新增，读 NTSTATUS-System-Error 组）。
  **hit-only 封顶**：查表未命中、或文本含 `%N` 占位符时保持原样 passthrough、不接管
  （不 owned）——1168 行 0x1F 影响面中未命中值零改动。
- **测试**：Go `TestCollectNtstatusMessages`（本机 ntdll 必含 0xC000000E 且文本精确）；
  Rust `unit_ntstatus_outtype_1f_lookup_hit_only`（hit→文本 owned / miss→passthrough
  not owned）；Rust 全量 225 tests 绿、`go vet` 干净。
- **管线**：动了消息库（03）与渲染器（04）→ 03→04→05→06 全链路重跑（en+zh），
  全部 exit 0。渲染抽查如上。

## 登记/转出

- **Ntfs 140 → 已修复**（定位到源并接入，非豁免）。EXEMPTIONS 零改动；
  `docs/residual-zeroing-plan-2026-09-07.md` 无需新登记（该条原不在豁免表）。
- 残留豁免仍为 6 项/10 条（SPP 1004、CAPI2 4111、SAM 16962×3、disk 51、WPD 24577/24579），
  G3 核对一致。

## 未决与建议

- **NTSTATUS 表内容**：ntdll 内嵌表 2975 条，英文消息文本（zh 基线同英文，
  `parameter_refs` 语言无关与 R15 同口径；zh 管线仅计覆盖率不受影响）。
- **给 F（Wininit 26）**：若需**符号名**表（STATUS_NO_SUCH_DEVICE 等），ntdll 二进制
  内不含符号名，需离线符号清单；消息**文本**表（本阶段）已按全局组就位，F 可直接
  复用 `lookup_ntstatus` 同款接线。
- B 644/C 40 不变：NTSTATUS 臂只作用于带 `param_types` 行且值查表命中的参数，
  对 fallback 渲染簇无外溢。
- D 交接的坑位均未复发（03 前旧库已由脚本自清；未触发单文件契约 assert）。

## 变更文件

- `msgdb-dump/dump/ntstatus.go`（新增：PE/RT_MESSAGETABLE 解析 + collectNtstatusMessages）
- `msgdb-dump/dump/ntstatus_test.go`（新增：0xC000000E 冒烟）
- `msgdb-dump/dump/run_windows.go`（步骤 11c：NTSTATUS-System-Error 组接线）
- `evtx-tools/evtx-tools-core/src/resolver.rs`（lookup_ntstatus）
- `evtx-tools/evtx-tools-cli/src/cmd_parse.rs`（`(_, 0x1F)` hit-only 渲染臂 + 单测）
- `docs/phase-reports/phase-E-report.md`（本报告）
- 管线产物（不入 jj）：`msgdb/hotech-msg-*.db`、`new-parsed/{en,zh}/*`、`report.{json,md,diff}`

（jj change id：vwsnkvnr）
