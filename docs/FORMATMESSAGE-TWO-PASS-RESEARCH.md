# FormatMessageW 双传递替换调研与实现

> 调研日期：2026-07-08
> 实现：hotech-evtx `substitute_format_message()`
> 解决：BITS 事件 59/60/61 的 URL GUID→FILETIME 渲染差异

---

## 1. 背景

### 1.1 问题描述

BITS event 59/60/61 的 URL 参数中包含 URL 编码的大括号：
```
url = https://...&appid=%7B430FD4D0-B729-4F61-AA34-91526481799D%7D&...
```

| 渲染器 | appid 值 |
|--------|----------|
| PowerShell Get-WinEvent | `1601-01-01T00:00:00.000000000ZB430FD4D0-...1601...D` |
| hotech-evtx (旧) | `%7B{430fd4d0-...}%7D` |

### 1.2 根因

Windows `EvtFormatMessage` (wevtapi.dll) 内部对消息模板替换结果**再次调用** `FormatMessageW` (kernel32.dll)。`FormatMessageW` 将 URL 中的 `%7B` 解析为：

```
%7B → %7 (参数7 = fileTime) + B (字面量)
%7D → %7 (参数7 = fileTime) + D (字面量)
```

fileTime=0 (FILETIME 纪元) → `1601-01-01T00:00:00.000000000Z`

---

## 2. Windows API 行为验证

### 2.1 C# P/Invoke 测试

通过直接调用 `kernel32!FormatMessageW` 验证（[FmtMsgTest.cs](../data/test-dataset/tools/FmtMsgTest.cs)）：

| 测试 | 输入 | 输出 | 结论 |
|------|------|------|------|
| %7B 在参数值中 | template=`%4`, param[3]=`URL %7B here%7D` | `URL %7B here%7D` | ❌ 单次不递归 |
| %7B 在格式字符串 | template=`%7B` | `FILETIME-7B` | ✅ %7B = %7 + B |
| 两次调用 | pass1→`%7B`, pass2→`%7` | `FILETIME-7B` | ✅ 两次调用生效 |

**结论：FormatMessageW 不做递归扫描。Windows EvtFormatMessage 做的是两次 FormatMessageW 调用。**

### 2.2 PowerShell Get-WinEvent 验证

```powershell
# 对比原始 Properties 值和 Message
$evt = Get-WinEvent -Path bits_openvpn.evtx | Where-Object { $_.Id -eq 61 } | Select-Object -First 1

# Properties[3] = 原始 URL 参数值（未经 FormatMessage 处理）
$evt.Properties[3].Value  # → "...appid=%7B430FD4D0...%7D..."

# Message = EvtFormatMessage 输出（经过两次 FormatMessageW）
$evt.Message              # → "...appid=1601-01-01T00:00:00.000000000Z..."
```

原始脚本：[deep_compare.ps1](../data/test-dataset/tools/deep_compare.ps1)

### 2.3 FormatMessageW 格式字符串解析规则

来自 Wine 源码（`dlls/kernel32/format_msg.c`）：

```c
insertnr = *f - '0';                          // 第一位数
switch (f[1]) {
    case '0'...'9':
        insertnr = (insertnr * 10) + (f[1] - '0');  // 可选的第二位数
        f++;
        break;
}
```

所以 `%7B`：
- `%` → 触发转义/插入序列
- `7` → `insertnr = 7`
- `B` → 不是数字 → 不匹配第二位
- 结果：**参数 7 + 字面量 B**

参考：
- [FormatMessageW 文档](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-formatmessagew)
- Wine 实现：`dlls/kernel32/format_msg.c`
- [Full-Disclosure 2006: Fun with event logs](https://lists.openwall.net/full-disclosure/2006/12/21/17)

---

## 3. 开源项目调研

### 3.1 evtx crate (0.12.2)

- 只渲染 XML/JSON，不做任何 FormatMessage 处理
- 有 `BinXmlValueType::FileTimeType`、`BinXmlValueType::GuidType` 等类型信息，但仅用于 XML 渲染
- 不做格式化、不做参数替换

### 3.2 libevtx (libyal)

- 只做 BinXML 解析，提取替换值
- 无 FormatMessageW 等效实现
- `%%N` 参数展开在文档中提及但未实现
- 未填充占位符显示为 `%#`

### 3.3 python-evtx (williballenthin)

- 纯 Python BinXML 解析器
- 只输出 XML，不加载 provider DLL
- 不做任何格式化

### 3.4 chainsaw / hayabusa

- 基于 Rust，使用 evtx crate
- 使用自己的检测规则引擎，绕开 provider 消息格式化
- 无 FormatMessageW 等效实现

**结论：没有开源项目实现 FormatMessageW 双传递行为。**

---

## 4. 实现方案

### 4.1 核心改动

在 `substitute.rs` 新增函数（纯 Rust，不调 Windows API）：

```rust
pub fn substitute_format_message(template: &str, params: &[&str]) -> String {
    let pass1 = substitute(template, params);
    substitute(&pass1, params)
}
```

### 4.2 调用链变化

```diff
 // cmd_parse.rs: process_parallel_batch()
- event.message = Some(substitute(&row.template, &param_refs));
+ event.message = Some(substitute_format_message(&row.template, &param_refs));
```

### 4.3 执行流程

```
Pass 1: substitute(template, [%1=transferId, %4=url含%7B...%7D, ...])
         ↓
         "BITS ... %4 URL ..."
         → "...appid=%7B430FD4D0-B729-4F61-AA34-91526481799D%7D..."

Pass 2: substitute(pass1_result, [%1=..., %7=fileTime, ...])
         ↓
         %7B → %7→fileTime=1601-01-01T00:00:00.000000000Z + B
         %7D → %7→fileTime=1601-01-01T00:00:00.000000000Z + D
         → "...appid=1601-01-01T00:00:00.000000000ZB430FD4D0-...1601...D..."

format_for_display(GUID归一化+Hex转换)
         ↓
         final result
```

### 4.4 影响分析

| 维度 | 评估 |
|------|------|
| 性能 | 第二次替换是 O(n) 字符扫描，参数值无 `%N` 时几乎无开销 |
| 精度 | 与 Windows FormatMessageW 双传递行为完全一致 |
| 副作用 | 任何参数值中的 `%N` 都会被二次替换（这是 Windows 行为） |
| 兼容性 | 当前测试集中只有 BITS URL 含 `%7B`/`%7D`，无其他 `%N` 模式 |

### 4.5 变更文件

| 文件 | 变更 |
|------|------|
| `hotech-evtx-core/src/substitute.rs` | 新增 `substitute_format_message()` + 7 个单元测试 |
| `hotech-evtx/src/cmd_parse.rs` | 替换 `substitute()` 调用为 `substitute_format_message()` |

---

## 5. 差异状态更新

参见 [MATCHING-STATUS.md](MATCHING-STATUS.md) 第 4.1 节和 [RENDERING-DIFFERENCES.md](RENDERING-DIFFERENCES.md) 第 1 节。

| 差异 | 可修复 | 状态 |
|------|--------|------|
| URL 中 GUID→FILETIME | ✅ 是（纯 Rust 模拟 FormatMessageW 双传递） | 已修复 |

---

## 6. 测试

```bash
# 单元测试
cargo test -p hotech-evtx-core --lib substitute

# 全量测试
cargo test --release

# 回归验证（需要 PowerShell 参考数据）
python data/test-dataset/tools/categorize_diff.py
```
