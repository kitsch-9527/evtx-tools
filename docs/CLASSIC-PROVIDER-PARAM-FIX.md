# Classic EventLog Provider 参数提取修复

> 调研日期：2026-07-08
> 实现：hotech-evtx `extract_subst_params()` Strategy 1.5
> 解决：MsiInstaller、MSSQLSERVER 等经典 EventLog 提供商的 %1-%N 参数为空

---

## 1. 问题描述

经典 EventLog 提供商（MsiInstaller、MSSQLSERVER 等）的消息参数始终为空：

```
修复前: "Beginning a Windows Installer transaction: . Client Process Id: ."
修复后: "Beginning a Windows Installer transaction: C:\path\file.msi. Client Process Id: 2128."
```

## 2. 根因

### 2.1 evtx crate JSON 输出格式

evtx crate (0.12.2) 根据 `Data` 元素是否有 `Name` 属性选择 JSON 格式
（`compiled.rs` 的 `is_data_container_name()`）：

| 格式 | 条件 | JSON 输出 | 示例 Provider |
|------|------|-----------|--------------|
| 命名平铺 (named) | Data 有 `Name` 属性 | `{"SubjectUserSid":"S-1-...","SubjectUserName":"SYSTEM"}` | Security-Auditing |
| 位置数组 (positional) | Data 无 `Name` 属性 | `{"Data":{"#text":["val1","val2",...]},"Binary":null}` | MsiInstaller, MSSQLSERVER |

经典提供商没有 `WEVT_TEMPLATE` 资源，所以 `Data` 元素没有 `Name` 属性，
evtx crate 渲染为 `{"Data": {"#text": [val1, val2, ...]}}`。

### 2.2 `extract_subst_params()` 的遗漏

修复前 `extract_subst_params()` 的匹配流程：

```
Strategy 1: Flat object WITH param_map
  → param_map=None → 跳过

Strategy 2: Flat object positional fallback
  → 对象有 "Data" 和 "Binary" 键，无 "#text" → 进入
  → 返回 ["{Data对象JSON字符串}", ""] ← ❌ 错误！

Strategy 3: #text array at top level
  → "#text" 在 Data 里面，不在顶层 → 跳过
```

Strategy 2 因为 `!obj.contains_key("#text")` 为真而错误匹配，
返回了 Data 对象的 JSON 字符串而不是实际的值。

### 2.3 开源项目调研

| 项目 | 经典提供商处理 | 说明 |
|------|--------------|------|
| **evtx crate** | 输出 `Data.#text` 格式 | 位置数组 |
| **python-evtx** | XML 输出，无 `Name` 的 `<Data>` | 使用者自行按位置提取 |
| **libevtx** | 用 template definition 映射 | 读取 BinXML 替换数组 |
| **chainsaw** | 依赖 evtx crate | JSON 格式 |
| **hayabusa** | 依赖 fork 版 evtx crate | 同上 |

**所有项目都依赖 evtx crate 的 JSON/XML 输出。没有项目绕过 JSON 读 BinXML 替换数组。**

## 3. 修复方案

### 3.1 Strategy 1.5

在 `extract_subst_params()` 中 **Strategy 2 之前** 插入新的匹配分支：

```rust
// Strategy 1.5: Classic EventLog #text array nested under "Data".
if let Some(obj) = parsed.as_object() {
    if let Some(data_obj) = obj.get("Data").and_then(|v| v.as_object()) {
        if let Some(text_val) = data_obj.get("#text") {
            let values: Vec<String> = match text_val {
                Value::Array(arr) => arr.iter().map(|v| match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => String::new(),
                }).collect(),
                Value::String(s) => vec![s.clone()],
                _ => Vec::new(),
            };
            if !values.is_empty() { return values; }
        }
    }
}
```

### 3.2 匹配精度保证

Strategy 1.5 的三个 `if let` 链确保只匹配 `{"Data": {"#text": [...]}}` 格式：

1. `parsed.as_object()` — event_data 必须是 JSON 对象
2. `obj.get("Data").and_then(|v| v.as_object())` — 必须有 `Data` 键且为对象
3. `data_obj.get("#text")` — Data 中必须有 `#text`

这使得 Strategy 1.5 不会错误匹配：
- Security-Auditing 的 `{"SubjectUserSid":"S-1-..."}` → 无 `Data` 键 → 不匹配
- Sysmon 的命名 Data → 无 `#text` 数组 → 不匹配
- 空事件 → `as_object()` 返回 None → 不匹配

### 3.3 变更文件

| 文件 | 变更 |
|------|------|
| `hotech-evtx/src/cmd_parse.rs` | `extract_subst_params()` 新增 Strategy 1.5 + 4 个单元测试 |

## 4. 影响范围

| 提供商 | 事件数 | 修复效果 |
|--------|--------|---------|
| MsiInstaller | 346 | `%1` = MSI 路径, `%2` = PID 正确填充 |
| MSSQLSERVER | 10 | `%1` = 错误信息 正确填充 |
| Microsoft-Windows-Security-SPP | 3 | 参数正确填充 |
| Office Software Protection | 2 | 参数正确填充 |
| **合计** | **361** | **JSON event_data 格式修复** |

## 5. 验证

```bash
# 单元测试
cargo test -p hotech-evtx -- extract_subst

# 全量测试
cargo test --release

# 数据验证
python data/test-dataset/tools/categorize_diff.py
```

### 5.1 匹配率变化

| 指标 | 修复前 | 修复后 |
|------|--------|--------|
| 精确匹配 | **65.6%** (1334/2034) | **82.6%** (1680/2034) |
| 剩余差异 | 34.4% (700) | 17.4% (354) |

### 5.2 剩余差异说明

- 39 事件：BITS appid 格式差异（PS: `BguidD` vs hot: `{guid}`，语义等价）
- 315 事件：第三方 provider 无消息（Sysmon/SQL Server 未安装），PS 与 hot 均为空
