# ADR 0005: TDH 类型化渲染（R10）

- **日期**: 2026-09-07
- **状态**: 已接受
- **关联决策**: 追加 ADR 0003（schema_version 仍为 7，附表可选）；与 ADR 0004 同构的可加性附表

## 背景

wevtapi 对事件参数按 WEVT 模板的 TDH `(InType, OutType)` 字节做类型化格式化，离线路径 B 只做字符串直通，产生 6 类分歧（2026-09-07 探针直读 CRIM TEMP 描述符 +4 字段 `((OutType<<8)|InType)` 与 wevtutil 基线逐簇对照实证）：

| (InType, OutType) | TDH 语义 | wevtapi 渲染 | 我们此前的输出 |
|---|---|---|---|
| (14,12) (08,12) | HEXINT32 + HEXINT32-out | `0x8024200B` 大写 | `0x8024200b` / `1` |
| (15,13) | HEXINT64 + HEXINT64-out | `0x1DD347A3F16110C` 大写 | 小写 |
| (0A,13) | UINT64 + HEXINT64-out | `0x2` 小写 | `2` |
| (07,20) | INT32 + HRESULT-out | `The operation completed successfully.` | `0` |
| (11,02) | FILETIME + DATETIME-out | `…25.532381200Z`（9 位纳秒） | 6 位微秒 |
| (08,08) | UINT32 + UNSIGNEDINT-out | 十进制（即使存串是 hex） | hex 直通 |

TDH 只对普通整数 InType 应用 value map；hex/bool/HRESULT InType 一律忽略 map——这是 Kernel-Boot 153「Disabled vs disabled」、Wininit 14 over-mapping 类分歧的根因之一。

另发现独立缺陷 **R18**：evtx crate 对 manifest 布尔参数输出 JSON 原生 `true/false`（非字符串），取值层 `as_str()` 拿 None 导致参数被吃成空串（Kernel-Boot 20「was .」）。

## 决策

- **采集**（msgdb-dump）：`TTBLParam` 增加 `InType/OutType`（crim 包，+4 字段两字节）；`enrichFromCRIM` 主分支经纯函数 `collectParamTypes` 收集进 `DLLResult.ParamTypes`；`(01,01)` 与 `(00,00)` 不落库（直通语义，省空间）。
- **存储**：附表 `param_types(provider_name, event_id, param_index, in_type, out_type)`，PK=(provider, event_id, param_index)。**schema_version 升 8**（2026-09-07 修订：与 ADR 0004 一致，不做新旧库兼容——用户决策）：`param_types` 为 v8 必需表，读端缺表即硬错；cohort 无行为直通渲染。
- **渲染**（evtx-tools）：`MessageRow.param_types`（param_index → "in:out"）由 `warmup` 按 cohort 挂载；`apply_param_types` 在枚举/值映射层**之前**按上表改写参数值；被类型化接管的字段从 enum/value-map/LUID 层跳过（`(08,08)` 十进制化不接管，map 照常应用）。HRESULT 消息化查 `Win32-System-Error` 参数引用表（R15），`0x8007xxxx` 取低字，查不到回退 `0x%08X`。
- **R18**：取值层值强制转换补 `Value::Bool` → `"true"/"false"`。

## 后果

- 预期清零：HttpService 112×51、WU 20×22、Kernel-Boot 20/25/27×9、KGA 25×3、Wininit HRESULT/bool×9、Time-Service 35×7、Application Error 1000×8（hex 大写）等。
- 未观察到的 (InType, OutType) 组合保持直通（不猜格式）；新增规则须先探针实证再补表。
- KGA 1（时间精度 + Change Reason 枚举缺条目）、KPP 55 / Ntfs 98（VMAP 条目缺）等簇需要 R10 之外的 map 条目源修复，不在本 ADR 范围。
