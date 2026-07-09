# 已知渲染差异

> hotech-evtx 与 PowerShell/Event Viewer 之间无法消除的差异。
> 最后更新：2026-07-08

---

## 1. URL 中的 GUID 渲染差异

### 现象

BITS event 59/60/61 的 `%4=url` 参数中包含内嵌 GUID：
```
url = https://.../check2?crx3=true&appid=%7B430FD4D0-B729-4F61-AA34-91526481799D%7D&...
```

| 渲染器 | appid 值 |
|--------|----------|
| Event Viewer / Get-WinEvent | `1601-01-01T00:00:00.000000000ZB{430fd4d0-...}` |
| hotech-evtx (format_for_display) | `%7B{430fd4d0-...}%7D` |

### 根因

PowerShell 的 `EvtFormatMessage` 内部对消息模板替换结果再次调用 `FormatMessageW`。
`FormatMessageW` 将 `%7B` 解析为 `%7`（参数7 = fileTime）+ 字面量 `B`，
将 `%7D` 解析为 `%7` + `D`。fileTime=0 → FILETIME 纪元。

详见 [FORMATMESSAGE-TWO-PASS-RESEARCH.md](FORMATMESSAGE-TWO-PASS-RESEARCH.md)。

### 影响

| 维度 | 值 |
|------|-----|
| 受影响事件 | BITS event 59/60/61 (39/783事件 = 5%) |
| 数据损失 | 无 — 两种格式都包含完整的 GUID 信息 |
| 修复状态 | ✅ 已修复（纯 Rust `substitute_format_message()` 模拟 FormatMessageW 双传递） |

### 修复

- **方案**：`substitute_format_message()` 对 `%1`–`%99` 做两遍替换，模拟 FormatMessageW 双传递
- **效果**：`%7B` → `%7`(fileTime=`1601-01-01T00:00:00Z`) + `B`，与 PowerShell 一致
- **实现**：纯 Rust，不调 Windows API，详见 [FORMATMESSAGE-TWO-PASS-RESEARCH.md](FORMATMESSAGE-TWO-PASS-RESEARCH.md)

---

## 2. 标准类型枚举解析

### 状态

| 检查项 | 结果 |
|--------|------|
| `isRoaming` 布尔字段 | ✅ 101/101 完全匹配 PowerShell |
| 其他 `is*`/`*Enabled`/`*Active` 字段 | ✅ 无遗漏 |
| 假阳性（非布尔字段被误解析） | ✅ 无（SID 保护、数值字段保护） |

### 覆盖的字段名模式

- 前缀: `is`, `has`, `can`, `should`, `will`
- 后缀: `Enabled`, `Disabled`, `Active`, `Flag`, `State`, `Bool`
- 值映射: `0/false` → `"Disabled"`, `1/true` → `"Enabled"`

### 局限

只能处理 `win:Boolean` 约定。不覆盖：
- `LogonType`、`AccessMask`、`PrivilegeList` 等枚举（需要 CRIM VMAP）
- 自定义 value map（需要 provider manifest 解析）

---

## 3. Hex 格式差异

### HRESULT 状态码

| 渲染器 | 值 |
|--------|-----|
| Event Viewer | `0x80072EE7` |
| hotech-evtx | `0x80072EE7` |

✅ 已通过 `format_for_display()` Step 3 修复: 十进制 `2147954407` → `0x80072EE7`.

### SID 子授权值

SID `S-1-5-21-3461203602-...` 中的大数值保持十进制。已通过 SID 上下文检测保护，不误转为 hex。

### 通用 `0x` 值

`0x3e7` → `0x3E7` (hex 数字大写，x 保持小写)。

---

## 4. 行尾格式

PowerShell 使用 `\n` (LF)，Windows DLL 消息模板使用 `\r\n` (CRLF)。
`format_for_display()` Step 1 移除 `\r` 以匹配 Event Viewer 输出。

---

## 总结

| 差异 | 可修复 | 状态 |
|------|--------|------|
| CRLF vs LF | ✅ 是 | 已修复 |
| GUID 花括号+大小写 | ✅ 是 | 已修复 |
| `0x` hex 大写归一 | ✅ 是 | 已修复 |
| HRESULT 十进制→Hex | ✅ 是 | 已修复 |
| SID 误转 | ✅ 是 (保护) | 已修复 |
| 布尔枚举 `isRoaming=1` | ✅ 是 | 已修复 |
| URL 中 GUID→FILETIME | ✅ 是 (纯 Rust 模拟 FormatMessageW 双传递) | 已修复 |
