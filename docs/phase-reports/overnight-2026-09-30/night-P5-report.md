# night-P5 报告：WER 1001×2 上游 hex 保护（2026-09-30 01:31→02:00）

## 结论

**验收通过。WER 1001 簇 2→0，diff 总数 35→33（只减不增），acceptance PASS。**

| 指标 | 前（P4 后锚点） | 后（本阶段） |
|---|---|---|
| diff 总数 | 35（System 21 + App 14） | **33（System 21 + App 12）** |
| matched | 44837/44890 | **44840/44890**（+3：WER×2 + SPP1004×1 连带） |
| M1 主线匹配率 | 99.8819% | **99.8886%** |
| M2 D 闭环率 | 89.94% | **90.52%** |
| exempted | 18 | 17（SPP 1004 豁免事件转全匹配，连带收益） |
| WER 1001（App）簇 | 2 | **0** |

红线逐项（acceptance-check.py --prev report.prev.json，总体 **PASS**）：

- [G0] baseline_lang zh-CN→zh-CN ✓
- [G1] M1=99.8886% ≥ 97.53% ✓；M2=90.52% ≥ 80% 硬门 ✓
- [G2] Security diff=0 ✓；Setup diff=0 ✓
- [G2] 四通道 B/C 不新增：App 0→0、Setup 0→0、Security 0→0、System 0→0 ✓
- [G2] 无对齐断裂（缺失类 diff=0）✓
- [G3] 豁免命中 4 项/17 条与登记表一致 ✓
- D 只减不增：35→33 ✓

单测：Rust **243 绿**（core 205 + cli 35 + 3，其中本阶段 +1 红绿 `format_for_display_keeps_guid_tail_decimal_in_wer_filename`），Go **135 绿**（count=1 重跑）。exe mtime **01:29 → 01:55 已核对**。

## 取证

1. **对齐样本**（importlib 复用 06-compare.py，compare_channel('Application','zh-CN')）：App 通道 WER 1001 共 401 条，diff 恰 2 条（event_record_id 2973 / 3430），归一化后首个真分歧点：
   - rec 2973：基线 `WER.c1438389-d2df-48d1-a9f7-ae3003916586.tmp.xml` → 我方 `...ae0xb30c212a.tmp.xml`（3003916586 = 0xB30C212A，数值实证相等）
   - rec 3430：基线 `WER.ee97f142-ee2d-48b0-8349-ab2978423997.tmp.xml` → 我方 `...ab0xb18724bd.tmp.xml`（2978423997 = 0xB18724BD）
2. **参数形态**（new-parsed/zh/Application.csv event_data）：参数名 **`AttachedFiles`**，普通字符串 `Data`（非 typed、无 InType/OutType），值为 CRLF 分隔的文件路径列表；十进制串是 WER 临时文件名中 GUID 形 token（8-4-4-4-12 全 hex）的**尾段**。
3. **hex 化点定位**（grep substitute.rs + git log 969ad58）：大十进制→hex 转换的**实现只有一处**——`format_for_display_protected` 的 Step 3（`val >= 0x80000000 && val <= 0xFFFFFFFF` 分支）。TS35 提交所谓"三处调用点"= substitute 内部 pass1 归一（substitute.rs:356）、final formatting（cmd_parse.rs:730）、Step 3 本体，**全部漏斗到同一实现**——单点收窄即三处同时生效，无须逐点改。既有 guard `is_guid_context` 要求数字串**后随**字符是 hexdigit，本案后随 `.`（段尾）故失守。
4. **P3/P4 冲突核实**：git log -- substitute.rs 最后一次触碰 = TS35 提交 969ad58；P3（dc8ef20）只动 cmd_parse.rs include_binary 接线、P4 只动 parser.rs，均不在 substitute.rs，无冲突。
5. **保护名单机制不适用**：protected 机制按"typed-owned 参数最终值"整体入保护（owned_values），`AttachedFiles` 是普通字符串参数且十进制串嵌在路径中间，不走 apply_param_types 的 typed_fields 路径——按简报"宁可用最窄条件"，采用**收窄转换触发条件**：数字串可被 `scan_guid`（Step 2 同一识别器，32 hex + 4 dash）识别为 GUID 形 token 的一部分时跳过转换。

## 完成（改动文件）

- `evtx-tools/evtx-tools-core/src/substitute.rs`（唯一代码改动）：
  - 新增红绿单测 `format_for_display_keeps_guid_tail_decimal_in_wer_filename`：两条真实样本（含 `%1` 端到端 substitute_format_message 路径）断言十进制原样、无 0x 改写；红测先复现 `ae0xb30c212a` 后转绿。
  - Step 3 decimal 分支收窄：前随字符是 hexdigit/`-` 时，向回走 ≤36 字节（一个 GUID 长度）逐位置试 `scan_guid`，若 GUID 形 span 覆盖该数字串（`end >= end_byte`）则 `in_guid_token=true` 跳过转换。孤立十进制（TS35 `3077760820.` 前随空格）不进此路径，历史 hex 族（TS35/R14/NetBT4311 等）全量单测零回退。
- 数据产物（管线再生）：new-parsed/{zh,en}/*、report.json、report.md。
- 环节：04-build（mtime 01:29→01:55 核对）→ 05-new-parse → 06-compare（diff=33 稳定复跑两次）→ acceptance-check PASS。

## 登记-转出

- 无新增转出簇。System 21 与 App 12 残留与 P4 交接清单一致（SAM8+WPD8+SPP1+CAPI2 1 豁免、e1d8/VMnet8 错库、Chrome8/Edge2/6008×2/Codex2 小簇 → P6/后续）。

## 未决与建议

1. **SPP 1004 连带转绿（exempted 18→17）非本修复直接因果**：其 152 条消息均无 ≥0x80000000 十进制串，但基线侧 24 组同时间戳重复事件使贪心配对顺序敏感，05 重排 CSV 行序后该条转入全匹配。方向为纯收益（exempted diff → matched），建议 P6 复跑时确认稳定，不动作。
2. **pipeline 无关性提示**：同类"同时间戳重复事件"簇（SPP/WER 均有）在残差盘点时建议按 (eid, provider, time, message-hash) 复核，排除配对噪声假阳/假阴。
3. Step 3 回扫开销：仅在前随字符为 hexdigit/`-` 且现有三个 guard 全部失守时触发，窗口 ≤36 字节，全量 44890 条比对耗时无感知（compare 复跑两次结果一致）。
