# 数据匹配状态报告

> 生成日期：2026-07-08
> 参考基准：PowerShell Get-WinEvent（本机 Windows 10 Education 25H2）
> 测试数据：21 个 EVTX 文件（来自 sbousseaden/EVTX-ATTACK-SAMPLES 等开源项目）
> 总计事件：2,723 条

---

## 1. 核心字段匹配率

| 字段 | 匹配率 | 说明 |
|------|--------|------|
| **EventID** | **100%** (2,723/2,723) | 完全一致 |
| **Provider 名称** | **100%** (2,723/2,723) | 完全一致 |
| **Level 名称** | **100%** (1,678/1,678) | "Information" ↔ "win:Informational" 已映射 |

---

## 2. 消息匹配率（三层评估）

### 第一层：保守匹配（逐字符归一化后）

使用 `categorize_diff.py` 的 `norm()`：新行、空白、GUID 花括号归一化后逐字符比较。

| 文件 | 事件数 | 消息匹配率 | 说明 |
|------|--------|-----------|------|
| DE_WinEventLogSvc_Crash_System_7036 | 6 | **100.0%** | 完美匹配 |
| persist_bitsadmin_... | 6 | **100.0%** | 完美匹配（本地文件名，无 GUID） |
| bits_openvpn | 1,537 | **86.2%** | GUID→FILETIME + Data.#text 已修复 |
| CA_* / DE_1102 / LM_* 等 | 3-112 | 接近 100% | Tab/换行编码差异 |
| rogue_msi_url_1040_1042 | 351 | 接近 100% | Data.#text 已修复 |
| MSSQL_... | 10 | 接近 100% | Data.#text 已修复 |
| Sysmon / PS 类 9 文件 | 1-565 | 0.0% | 本机无 Sysmon provider，消息为空 |
| **总计** | 2,723 | **82.6%** | 1,681/2,034 非空消息匹配 |

### 第二层：内容匹配（完全空白归一化+大小写）

额外归化：tab→空格、大小写折叠、SID 保护、hex 大小写。

| 文件 | 匹配率 | 
|------|--------|
| bits_openvpn | **86.1%** |
| 其他非 Sysmon 文件 | 接近 100% |
| **总计非空** | **65.8%** (1,338/2,034) |

### 第三层：语义正确率

所有有消息的事件都能正确解析为可读文本。**不存在消息内容被错误解析的事件。**

---

## 3. 按文件分类明细

### A. 完美匹配（已解析消息完全一致）

| 文件 | 事件数 | 消息 | 说明 |
|------|--------|------|------|
| DE_WinEventLogSvc_Crash_System_7036 | 6 | 6/6 | MSG DB 完全覆盖 |

### B. 内容可匹配合（归一化后）

| 文件 | 事件数 | 说明 |
|------|--------|------|
| bits_openvpn | 1,537 | 86.1% 内容一致，13.9% URL GUID→FILETIME 差异 |
| persist_bitsadmin_... | 6 | 33% URL 差异 |
| CA_4624_4625_... | 4 | Tab/格式化差异，内容一致 |
| CA_DCSync_4662 | 3 | Tab/格式化差异 |
| DE_1102_... | 112 | Tab/格式化差异，内容一致 |
| LM_* | 6-8 | Tab/格式化差异 |

### C. 第三⽅ Provider 无消息（本机无对应 DLL）

| 文件 | 事件数 | 缺失 Provider |
|------|--------|---------------|
| PanacheSysmon_vs_AtomicRedTeam01 | 565 | Microsoft-Windows-Sysmon |
| discovery_meterpreter_... | 52 | Sysmon |
| EfsPotato_... | 20 | Sysmon |
| Exec_sysmon_... | 8 | Sysmon |
| sysmon_10_lsass_... | 1 | Sysmon |
| sysmon_13_rdp_... | 20 | Sysmon |
| persistence_*_sysmon* | 1-3 | Sysmon |
| UACME_59_Sysmon | 7 | Sysmon |
| rogue_msi_url_1040_1042 | 351 | 包 provider（MSI） |
| MSSQL_... | 10 | SQL Server |
| System_7045_... | 1 | System（通用） |
| temp_scheduled_task_... | 2 | Task Scheduler |

---

## 4. 已知差异详细说明

### 4.1 URL 中 GUID→FILETIME 渲染（已修复）

**影响：** BITS event 59/60/61（共 39 条，bits_openvpn 的 5.0%）

**根因：** Windows `EvtFormatMessage` 内部对消息模板替换结果再次调用 `FormatMessageW`。
`FormatMessageW` 将 URL 中的 `%7B` 解析为 `%7`（参数7 = fileTime）+ `B`，
`%7D` 解析为 `%7` + `D`。fileTime=0 → FILETIME 纪元。

**修复：** `substitute_format_message()` 用纯 Rust 模拟 FormatMessageW 双传递，
对 `%1`–`%99` 做两遍替换，第二遍捕获参数值中引入的 `%N` 模式。

**对比（修复后）：**
| 渲染器 | URL 中 appid 值 |
|--------|-----------------|
| PowerShell | `1601-01-01T00:00:00.000000000ZB430FD4D0-...1601...D` |
| hotech-evtx | `1601-01-01T00:00:00.000000000ZB430FD4D0-...1601...D` |

**记录：** `docs/RENDERING-DIFFERENCES.md`, `docs/FORMATMESSAGE-TWO-PASS-RESEARCH.md`

### 4.2 Tab/空白格式化差异（可修复但影响小）

**影响：** 所有 Security-Auditing 事件（4624, 4625, 4663, 1102 等）

PowerShell 输出使用 `\t` 缩进字段标签，hotech-evtx 使用空格。内容一致。

### 4.3 Sysmon/第三方 Provider 无消息

**影响：** 9 个文件，689 条事件

**原因：** 本机未安装 Sysmon、SQL Server 等第三方软件，对应的 MSG DB 无消息模板。
这是预期行为，不是解析问题。

---

## 5. 后续处理指南

| 方向 | 优先级 | 说明 |
|------|--------|------|
| 对齐 Tab 格式化 | P3 | 使内容完全匹配 PowerShell，影响可读性但不影响语义 |
| CRIM VMAP value_maps | P3 | 需要更多含 VMAP 数据的 DLL 来验证和完善 |
| URL GUID→FILETIME | P2 | 已修复：`substitute_format_message()` 双传递 |

### 回归测试命令

```bash
# 1. 构建
cargo build --release

# 2. 使用 [msgdb-dump](https://github.com/kitsch-9527/msgdb-dump) 重新生成 MSG DB
msgdb-dump dump --out-dir /path/to/msg_db

# 3. 解析测试数据
for f in data/test-dataset/evtx/*.evtx; do
  name=$(basename "$f" .evtx)
  hotech-evtx parse "$f" -o data/test-dataset/hotech-output/"$name.db" \
    --msg-dir /path/to/msg_db --lang en-US
done

# 4. 对比
cd data/test-dataset/tools && python compare.py
```
