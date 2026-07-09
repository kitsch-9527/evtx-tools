# 开源 EVTX 解析项目调研报告

> 对照 hotech-evtx 已知限制，调研其他开源实现的可借鉴方案。
> 最后更新：2026-07-07

---

## 1. 调研项目总览

| 项目 | 语言 | 消息解析 | WEVT_TEMPLATE | MUI | 输出 | 性能 (30MB) |
|------|------|----------|---------------|-----|------|------------|
| **omerbenamram/evtx** (v0.12) | Rust | ❌ | ✅ CRIM | ❌ | JSON/XML | **~20ms** (8线程) |
| **Yamato-Security/hayabusa-evtx** | Rust (fork) | ❌ | 同上游 | ❌ | 同上游 | 同上游 |
| **libevtx** (libyal) | C | ❌ (仅文档) | ✅ 规范 | ❌ | CSV/XML | ~2.4s |
| **python-evtx** | Python | ❌ | ❌ | ❌ | XML | ~161s |
| **Chainsaw** (WithSecure) | Rust | ❌ | 依赖上游 | ❌ | JSON/CSV | ~20ms |
| **Hayabusa** (Yamato-Security) | Rust | ❌ | 依赖 fork | ❌ | CSV/HTML | ~20ms |
| **EvtxHussar** | Go | ✅ YAML maps | ❌ | ❌ | Excel/CSV | ~1s (100并发) |
| **Velociraptor / dumpevtx** | Go | ✅ SQLite DB | ❌ | ❌ | VQL/JSON | ~? |
| **golang-evtx** | Go | ❌ | ❌ | ❌ | JSON | ~1.3s |
| **hotech-evtx** (本项目) | Rust | ✅ 完整管道 | ✅ CRIM+TTBL | ✅ | CSV/SQLite | 单线程~? |

---

## 2. 关键实现维度对比

### 2.1 消息文本解析（event_id → human-readable message）

这是 EVTX 解析最核心也是最难的问题：**EVTX 文件本身不存储消息文本**，只存 Event ID，文本在 provider DLL 中。

**各项目方案：**

| 方案 | 项目 | 实现方式 |
|------|------|----------|
| **运行时 FormatMessageW** | Windows Event Viewer | 调 Win32 API，即时从 DLL 加载 |
| **SQLite 消息 DB 离线缓存** | Velociraptor/dumpevtx | 在线提取→SQLite 离线复用 |
| **YAML 映射** | EvtxHussar | 预定义常用 Event ID 的字段映射 |
| **DLL+CRIM+MUI 全管道** | **hotech-evtx** | 注册表枚举→DLL→PE→CRIM→MUI→消息文本 |
| **不做消息解析** | evtx/chainsaw/hayabusa | 只输出原始 XML/JSON 字段 |

**关键发现：**
- **Velociraptor 的方案**最接近 hotech-evtx：同样用 SQLite DB 离线复用。差异在于：
  - Velociraptor 的 DB 来自 `dumpevtx` 在**真实系统**上运行采集
  - hotech-evtx 的 DB 来自 `hotech-evtx dump` 的 CRIM+MUI 提取
  - 后者不依赖 Windows 运行时，纯离线解析
- **EvtxHussar 的 YAML 映射**对 %%N 展开有参考价值——预定义常见参数名称

### 2.2 WEVT_TEMPLATE / CRIM 解析

CRIM（Crimson Metadata）是 Windows Vista+ 事件清单格式，嵌入 PE 文件资源中。

| 项目 | CRIM 解析范围 | 备注 |
|------|-------------|------|
| **omerbenamram/evtx** | EVNT、TTBL、CHAN、KEYW、LEVL、OPCO、TASK、MAPS | 0.10+ 可选 feature，输出 .wevtcache |
| **hotech-evtx** | EVNT、TASK、OPCO、TTBL、MESSAGETABLE | 含 MUI 伴生文件查找 |
| **libevtx** | 全部（规范文档） | 有最完整的格式文档，但实际解析实现有限 |

**evtx crate 的 CRIM 解析差异：**
- 以 **TEMP GUID** 为 join key（"the cleanest join"），hotech-evtx 用 template_offset
- 实现了 MS-EVEN6 NameHash 校验（`hash = hash * 65599 + code_unit`），不匹配时直接 fail
- 输出 .wevtcache 是**纯 CRIM blob** 的 JSONL，不做任何消息文本解析
- 仅在**内嵌模板缺失/损坏**时 fallback 使用

**hotech-evtx 优势：**
- 在 CRIM EVNT + MUI 消息表之间做了 **JOIN**
- 提取了 task/opcode/keywords 名称（其他项目不做这个）
- 唯一的 MUI 支持

### 2.3 %%N 参数引用展开

`%%N` 引用另一条消息文本（如 `%%14592` → `Direction=Inbound`）。

| 项目 | 方案 |
|------|------|
| **EvtxHussar** | YAML 预定义映射（如 `%%1936` → `TokenElevationTypeDefault`） |
| **libevtx** | 文档说明可通过 `ParameterMessageFile` 解析，未实现 |
| **Velociraptor** | 不展开 |
| **hotech-evtx** | **未展开**（Know Issue: P2） |
| **Windows Event Viewer** | 调 FormatMessage + LoadLibraryEx，递归展开 |

**借鉴方案：**
1. **EvtxHussar 思路**：在 DB 中预存 `%%N → 文本` 映射表（需从 ParameterMessageFile DLL 提取）
2. **全量方案**：dump 阶段识别 ParameterMessageFile，提取其 MESSAGETABLE 合并入消息 DB
3. **混合方案**：先查 DB，查不到时 fallback 到硬编码常见映射

### 2.4 多线程解析

| 项目 | 方案 | 效果 |
|------|------|------|
| **evtx crate** | Rayon chunk 级别并行 | 8线程 20ms（30MB），7000x vs python |
| **Chainsaw** | 依赖上游 | 继承上游性能 |
| **EvtxHussar** | goroutine 100 并发 | 并发 IO 而非解析 |
| **hotech-evtx**（旧版） | **单线程**（Know Issue: P3） | — |
| **hotech-evtx**（当前） | ✅ `--threads N` 多线程（Rayon） | 1.5GB 50.3s → 33.2s |
| **libevtx** | 单线程 | 慢但稳定 |

**考察：** evtx crate 的 multithreading 实现在 `0.12` 已成熟，可通过 `--threads N` 控制。
- 6 项优化叠加后 1.5GB / 2.5M 事件从 **74.6s 降至 25.7s**（CSV）/ **33.2s**（SQLite）

### 2.5 消息 DB 方案对比

| 项目 | 格式 | 覆盖范围 | 生成方式 |
|------|------|---------|---------|
| **Velociraptor/evtx-data** | SQLite | 有限社区贡献 | dumpevtx 在线采集 |
| **hotech-evtx system DB** | SQLite | 全系统所有 ETW 提供者 | dump 命令离线提取 |
| **hotech-evtx dll DB** | SQLite | 单 DLL 粒度 | dump 命令离线提取 |
| **evtx crate wevtcache** | JSONL/自定义 | CRIM blob 原始数据 | extract-wevt-templates 命令 |

**差异总结：**
- hotech-evtx 的 DB 覆盖率最高（全注册表枚举，包括所有通道）
- Velociraptor 的 DB 更轻量但贡献者驱动的覆盖有限
- evtx crate 不试图解析消息文本，只保存 CRIM 模板

---

## 3. 针对已知限制的可借鉴方案

### P1: EVTX Build 检测失效

| 其他项目做法 | 可借鉴性 |
|-------------|---------|
| libevtx: 不依赖 build 号，用 provider GUID 直接匹配 | 高 |
| Velociraptor: 用 OS 版本匹配 DB，不检测 EVTX 本身 | 中 |
| evtx crate: 不做 build 检测，不做消息解析 | 不适用 |

**建议：**
- **方案 A**：消息 DB 中存储 OS 版本信息和所有支持的 build 范围，fallback 时遍历匹配
- **方案 B**：从 EVTX 文件的 `EventID`/`Provider` 分布推测 build（更复杂，不推荐）
- **方案 C**：使用 `wevtutil gp` 输出或注册表中读取的版本来匹配（但离线不可用）

### P2: CSV 重复 Header

**其他项目：** 所有项目都是**先写一次 header**（在文件创建时），然后只追加数据。
**hotech-evtx 问题：** 空批次 `write_events(&[], ...)` 写一次 header，首非空批次再写一次。

**修复：** 从 `create_events_table()` 中移出头写入逻辑，在首次非空批次时一次性写入。

### P2: template_instances() 跳过 JSON 路径

| 其他项目做法 | 可借鉴性 |
|-------------|---------|
| evtx crate 的 `dump-template-instances` CLI | 中（只是 CLI，非库 API） |
| libevtx: template definition → substitution 映射 | 低（C 语言移植成本高） |

**建议：** 后续版本启用 evtx crate 的 `EvtxParser::template_instances()` 路径（如存在），或者直接操纵 `BinXml` token 流。

### P2: task/opcode 不完整

**其他项目：** 无任何其他项目尝试提取 task/opcode 名称。
**hotech-evtx 已领先：** 当前已覆盖 MAIN_TASK 块 + 部分 EVNT 的 task_offset。不足在于某些 provider（如 adtschema）的 CRIM 格式特殊。

**建议：** 
- 在 `parse_single_evnt_block()` 中打印每块的原始 hex 前几个字节以调试
- 对照 libevtx 的 CRIM 文档，确保不遗漏新的 CRIM 版本（v4/v5）的字符串布局变化

### P2: %%N 消息引用未展开

**其他项目做法：** 
- **EvtxHussar**: YAML maps 预定义
- **Windows FormatMessage**: ParameterMessageFile DLL → LoadLibraryEx → FormatMessage

**可借鉴方案（分阶段）：**
1. **阶段 1（evtx-data 模式）**：dump 时从 ParameterMessageFile 提取 MESSAGETABLE，写入消息 DB 的新表 `parameter_refs`
2. **阶段 2**：解析时遇到 `%%N` 查 `parameter_refs` 表替换
3. **阶段 3**：对查不到的 `%%N`，EvtxHussar 式的硬编码常见映射表作为 fallback

### P3: Level 名称提取

| 其他项目做法 | 可借鉴性 |
|-------------|---------|
| evtx crate: 解析 LEVL 块（含名称） | 高 — 已有实现参考 |
| hotech-evtx: severity 已硬编码中文，缺 display name | — |

**建议：** CRIM 的 LEVL 块格式已知（`level_code(u32) + name(u16 prefix str)`），`parse_crim_provider_data()` 中已有类似 TASK/OPCO 的解析，直接模仿即可。

### P3: 单线程解析

| 其他项目做法 | 可借鉴性 |
|-------------|---------|
| evtx crate 的 rayon 并行已有成熟实现 | **已实现** — `--threads N` CLI 参数 |

**当前状态（2026-07-07）：** 已实现。通过 `EvtxParser::with_configuration(ParserSettings::default().num_threads(n))` 配置，
`--threads 0` 使用全部 CPU 核心。实测 1.5GB 文件单线程 74.6s → 多线程 50.3s（1.48× 加速）。

**注意事项：**
- 多线程在 hotech-evtx 的 `parse` 命令与消息解析通过 `mpsc::sync_channel` 解耦，`MessageDb` 无需 `Send + Sync`
- SQLite 写入是串行瓶颈（后台 Writer 线程负责），通过 `page_size=65536` + `journal_mode=MEMORY` + `PRAGMA synchronous=OFF` 缓解

### P3: dlldb LCID → ISO 语言代码

**其他项目：** 几乎所有项目都用 LCID 或 RFC 4646 标签。
**hotech-evtx 的 mui_helper.rs 已实现 `lcid_to_lang_code()`。** 只需在 `dll_db_filename()` 中调用即可。

---

## 4. hotech-evtx 的独有优势

| 维度 | hotech-evtx | 其他项目 | 差异价值 |
|------|------------|----------|---------|
| **消息解析覆盖率** | ~100%（全注册表枚举） | 0% 或社区有限覆盖 | **核心优势** |
| **MUI 支持** | ✅ 独家 | ❌ 全部不支持 | 多语言 Windows 必用 |
| **CRIM EVNT+TASK+OPCO+TTBL** | ✅ 全部解析 | evtx crate 同但无消息 JOIN | 名称提取+消息联合 |
| **SQLite 离线 DB 格式** | ✅ 标准 | evtx-data 用 SQLite | 易于集成和分析 |
| **CSV 输出** | ✅ 22 列标准化 | 大部分 JSON 输出 | 非技术人员可用 Excel 打开 |
| **`%%N` 展开** | ❌ 未实现 | EvtxHussar 部分 YAML | **待改善** |
| **跨平台** | ✅ 全平台 | evtx crate 同 | Linux 编译可用 |
| **多线程** | ✅ `--threads N` 已实现 | evtx crate rayon | 1.48× 加速实测 |

---

## 5. 综合建议

### 短期（当前迭代）

| 限制 | 建议方案 | 参考项目 | 预期工作量 |
|------|---------|---------|-----------|
| CSV 重复 header | 移出空批次 header 写入 | 所有项目惯例 | 小（1-2h） |
| Level 名称提取 | 模仿 TASK/OPCO 解析 LEVL | evtx crate / libevtx | 小（2-3h） |
| dlldb LCID→ISO | 复用 `lcid_to_lang_code()` | — | 微小 |

### 中期（下个迭代）

| 限制 | 建议方案 | 参考项目 | 预期工作量 |
|------|---------|---------|-----------|
| `%%N` 展开阶段1 | dump 时提取 ParameterMessageFile | EvtxHussar + MS-EVEN | 中（3-5天） |
| Build 检测 fallback | 消息 DB 存储 build 范围 | Velociraptor | 中（2-3天） |
| task/opcode 覆盖 | 调试 CRIM 解析 + hex 打印 | — | 中（3-5天） |

### 已实现（2026-07-07 迭代）

| 原本限制 | 方案 | 实测效果 |
|---------|------|---------|
| **多线程解析** | `--threads N` + `ParserSettings::num_threads()` | 1.5GB: 74.6s → 50.3s (1.48×) |
| **SQLite 写入慢** | 后台 Writer 线程 + multi-row INSERT + PRAGMAs (page_size=65536, journal=MEMORY, sync=OFF) | 50.3s → 33.2s (1.52×) |
| **Value→String→Value 往返** | `iter_events` 返回 EventData Value 避免 `extract_subst_params` 重解析 | 微优化叠加 |

### 长期（Backlog）

| 限制 | 建议方案 | 参考项目 | 预期工作量 |
|------|---------|---------|-----------|
| `template_instances()` 跳过 JSON | 直接 BinXML token 遍历 | evtx crate | 大（2-4周） |
| 进度条 | ansi_term/indicatif | — | 小（1天） |

---

## 6. 参考链接

- [omerbenamram/evtx](https://github.com/omerbenamram/evtx) — Rust EVTX 解析库
- [omerbenamram/evtx WEVT_TEMPLATE 文档](https://github.com/omerbenamram/evtx/blob/master/docs/wevt_templates.md)
- [libyal/libevtx 格式规范](https://github.com/libyal/libevtx/blob/main/documentation/Windows%20XML%20Event%20Log%20(EVTX).asciidoc)
- [williballenthin/python-evtx](https://github.com/williballenthin/python-evtx)
- [WithSecureLabs/chainsaw](https://github.com/WithSecureLabs/chainsaw)
- [Yamato-Security/hayabusa](https://github.com/Yamato-Security/hayabusa)
- [Yamato-Security/hayabusa-evtx](https://github.com/Yamato-Security/hayabusa-evtx)
- [yarox24/EvtxHussar](https://github.com/yarox24/EvtxHussar)
- [Velocidex/evtx-data](https://github.com/Velocidex/evtx-data)
- [0xrawsec/golang-evtx](https://github.com/0xrawsec/golang-evtx)
- [MS-EVEN: Expanding Unexpanded Event Description Strings](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-even/d76e867d-96dc-4c09-b747-b1dde1acc5a4)

---

## 附录 A：hotech-evtx 性能优化基准

### 测试环境

| 项目 | 值 |
|------|-----|
| CPU | Intel Core i5-10500 @ 3.10GHz (6C/12T) |
| 内存 | 未测量 |
| 存储 | C 盘 SSD |
| 测试文件 | 真实 Windows Security.evtx（1.5GB, 2,492,500 事件） |
| 构建 | `cargo build --release` |

### 优化历程

| # | 配置说明 | 格式 | 耗时 | 加速比 | 累计加速 |
|---|---------|------|------|--------|---------|
| 0 | 初始（单线程, `--threads 1`, 无 PRAGMAs） | db | 74.6s | 1.00× | — |
| 1 | `--threads 0`（12 核 Rayon） | db | 50.3s | 1.48× | 1.48× |
| 2 | + multi-row INSERT + 后台 Writer 线程 | db | 45.5s | 1.11× | 1.64× |
| 3 | + `page_size=65536` + 原生 Windows 路径 | db | 33.2s | 1.37× | 2.25× |
| 4 | + SQL 生成优化（string::replace） | db | 33.9s | 0.98× | 2.20× |
| 5 | **CSV 格式（无 B-tree 开销）** | csv | **25.7s** | 1.32× | **2.90×** |

### 小文件基准（优化后，`--threads 0`）

| 文件 | 大小 | 单线程 | 多线程 | 加速比 |
|------|------|--------|--------|--------|
| Security.evtx | 21MB | 0.994s | 0.603s | 1.65× |
| Application.evtx | 21MB | 0.432s | 0.316s | 1.37× |
| System.evtx | 18MB | 0.965s | 0.626s | 1.54× |
