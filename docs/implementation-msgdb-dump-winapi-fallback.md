# 实施文档：msgdb-dump 使用 Windows API 补充消息提取

## 版本信息

- **版本**: 1.0.0
- **日期**: 2026-08-31T02:58:52.859Z
- **状态**: 待实施

---

## 1. 问题描述

### 1.1 现象

同一台机器上，`evtx2csv.exe` 可以显示完整的消息（如 ESENT 102），
但 `msgdb-dump` 无法提取消息，导致 `evtx-tools` 显示 fallback 消息。

### 1.2 根本原因

| 工具 | 消息来源 | 结果 |
|------|----------|------|
| evtx2csv.exe | Windows Event Log API (EvtFormatMessage) | ✅ 获取完整消息 |
| msgdb-dump | PE 解析 (RT_MESSAGETABLE/WEVT_TEMPLATE) | ❌ esent.dll 没有这些资源 |

### 1.3 技术分析

- `esent.dll` 没有 RT_MESSAGETABLE、WEVT_TEMPLATE、CRIM 资源
- 消息存储在 Windows 系统内部缓存中
- 只有通过 Windows API 才能访问

---

## 2. 解决方案

### 2.1 设计决策

| # | 决策 | 选择 | 理由 |
|---|------|------|------|
| Q1 | 实现范围 | 补充 fallback | 保留 PE 解析，离线场景也能工作 |
| Q2 | 调用时机 | 提取阶段 | 结果存入 DB，evtx-tools 离线可用 |
| Q3 | API 调用方式 | Provider 名称 | EvtOpenPublisherMetadata，简单直接 |
| Q4 | 消息模板格式 | EvtFormatMessageId | 获取带 %1 %2 的原始模板 |
| Q5 | 语言处理 | 循环所有语言 | 默认 en-US + zh-CN |
| Q6 | 判断时机 | 整体为空时补充 | 安全 fallback，不冲突 |
| Q7 | CLI 参数 | 自动启用 | 无需新参数，用户无感知 |

### 2.2 架构

```
msgdb-dump 提取流程:
  1. 扫描注册表 → 获取 Provider 列表
  2. PE 解析 → 提取消息模板
  3. 如果 MessagesByLang 为空:
     → 调用 Windows API (EvtFormatMessage)
     → 获取消息模板
     → 存入 DB
  4. 写入 SQLite 数据库
```

---

## 3. 实施步骤

### 步骤 1：wcorefx — 新增 FormatMessageById 函数

**文件**: `E:\Code\go\wcorefx\evtx\wevtapi.go`

**位置**: 在 `OpenPublisherMetadata` 函数之后

**修改内容**: 新增公开函数

```go
// FormatMessageById 获取指定 EventID 的消息模板。
//
// 不需要事件句柄，直接通过 Provider 名称和 EventID 获取消息模板。
// 使用 EvtFormatMessageId 标志获取带 %1 %2 占位符的原始模板。
//
// 参数：
//   - publisherName: Provider 名称（如 "ESENT"、"Microsoft-Windows-Security-Auditing"）
//   - eventID: 事件 ID（如 102、4624）
//   - lang: 语言 LCID（如 1033=en-US、2052=zh-CN）
//
// 返回：
//   - 消息模板字符串（带 %1 %2 占位符）
//   - 错误（如果 API 调用失败）
func FormatMessageById(publisherName string, eventID uint32, lang uint32) (string, error) {
	ph, err := OpenPublisherMetadata(0, publisherName, lang)
	if err != nil {
		return "", fmt.Errorf("OpenPublisherMetadata(%s, %d): %w", publisherName, lang, err)
	}
	defer ph.Close()

	// First call: get required buffer size
	var bufferUsed uint32
	err = evtFormatMessage(ph, 0, eventID, 0, nil, EvtFormatMessageId, 0, nil, &bufferUsed)
	if err != nil && err != ERROR_INSUFFICIENT_BUFFER {
		return "", fmt.Errorf("EvtFormatMessage(%s, %d) size: %w", publisherName, eventID, err)
	}

	// Second call: get the actual message
	bufferUsed *= 2 // UTF-16 bytes
	buffer := make([]byte, bufferUsed)
	bufferUsed = 0
	err = evtFormatMessage(ph, 0, eventID, 0, nil, EvtFormatMessageId,
		uint32(len(buffer)/2), &buffer[0], &bufferUsed)
	if err != nil {
		return "", fmt.Errorf("EvtFormatMessage(%s, %d): %w", publisherName, eventID, err)
	}

	// Convert UTF-16 bytes to Go string
	if bufferUsed == 0 {
		return "", nil
	}
	u16 := unsafe.Slice((*uint16)(unsafe.Pointer(&buffer[0])), bufferUsed)
	return windows.UTF16ToString(u16), nil
}
```

**依赖检查**:
- `golang.org/x/sys/windows` — 已存在于 wcorefx
- `unsafe` — 标准库
- `fmt` — 标准库

---

### 步骤 2：msgdb-dump — 新增 extractMessagesViaWinAPI 函数

**文件**: `E:\Code\evet\msgdb-dump\dump\extractor.go`

**位置**: 文件末尾，在 `extractMUICRIMFromBlobsWithMaps` 函数之后

**修改内容**: 新增两个函数

```go
// extractMessagesViaWinAPI 使用 Windows Event Log API 提取消息模板。
//
// 当 PE 解析失败时（DLL 没有 RT_MESSAGETABLE/WEVT_TEMPLATE 资源），
// 调用 EvtFormatMessage API 获取消息模板。
//
// 只在 Windows 上可用，其他平台返回 nil。
//
// 参数：
//   - providerName: Provider 名称（如 "ESENT"）
//   - langLCID: 语言代码（如 "zh-CN"）
//
// 返回：
//   - map[uint32]string: EventID → 消息模板
//   - nil: 如果提取失败或没有消息
func extractMessagesViaWinAPI(providerName string, langLCID string) map[uint32]string {
	// Convert lang code to LCID
	lcid := langCodeToLCID(langLCID)
	lcidInt, err := strconv.ParseUint(lcid, 10, 32)
	if err != nil {
		return nil
	}

	msgs := make(map[uint32]string)

	// Try common event ID ranges
	// Event IDs are typically in ranges: 1-9999
	for eid := 1; eid <= 9999; eid++ {
		msg, err := evtx.FormatMessageById(providerName, uint32(eid), uint32(lcidInt))
		if err != nil {
			continue // Skip errors (message not found for this ID)
		}
		if msg != "" {
			msgs[uint32(eid)] = msg
		}
	}

	if len(msgs) == 0 {
		return nil
	}
	return msgs
}
```

**新增导入**:

```go
import (
	// ... existing imports ...
	"strconv"
	wcorefxevtx "github.com/kitsch-9527/wcorefx/evtx"
)
```

**注意**: 需要将 `evtx` 包导入为 `wcorefxevtx` 以避免与本地包名冲突。

---

### 步骤 3：msgdb-dump — 修改 run_windows.go 提取逻辑

**文件**: `E:\Code\evet\msgdb-dump\dump\run_windows.go`

**位置**: 第 160-170 行，在 `ExtractSingleDLL` 调用之后

**修改前**:

```go
result := ExtractSingleDLL(dllPath, providers, langs, langCodes)
if result != nil {
    dllResults = append(dllResults, result)
}
```

**修改后**:

```go
result := ExtractSingleDLL(dllPath, providers, langs, langCodes)
if result != nil {
    // Step 7.5: Windows API fallback
    // If PE extraction found no messages, try Windows Event Log API
    if len(result.MessagesByLang) == 0 {
        fmt.Printf("[info] PE extraction empty for '%s', trying Windows API fallback...\n", result.DLLName)
        for _, provider := range providers {
            for _, langCode := range langCodes {
                msgs := extractMessagesViaWinAPI(provider.Name, langCode)
                if len(msgs) > 0 {
                    // Convert to PeMessage format
                    var peMsgs []PeMessage
                    for eid, msg := range msgs {
                        peMsgs = append(peMsgs, PeMessage{
                            EventID: eid,
                            Message: msg,
                        })
                    }
                    result.MessagesByLang[langCode] = append(
                        result.MessagesByLang[langCode], peMsgs...)
                    fmt.Printf("[info]   WinAPI: %d messages for '%s' (lang=%s)\n",
                        len(peMsgs), provider.Name, langCode)
                }
            }
        }
    }
    dllResults = append(dllResults, result)
}
```

---

## 4. 测试计划

### 4.1 单元测试

#### 测试 1：wcorefx FormatMessageById

**文件**: `E:\Code\go\wcorefx\evtx\wevtapi_test.go`

```go
func TestFormatMessageById(t *testing.T) {
	// Test ESENT provider (no RT_MESSAGETABLE)
	msg, err := FormatMessageById("ESENT", 102, 2052) // zh-CN
	if err != nil {
		t.Fatalf("FormatMessageById failed: %v", err)
	}
	if msg == "" {
		t.Fatal("FormatMessageById returned empty message")
	}
	t.Logf("ESENT 102 (zh-CN): %s", msg)

	// Test with en-US
	msgEn, err := FormatMessageById("ESENT", 102, 1033)
	if err != nil {
		t.Fatalf("FormatMessageById failed: %v", err)
	}
	t.Logf("ESENT 102 (en-US): %s", msgEn)
}

func TestFormatMessageById_NotFound(t *testing.T) {
	// Test with non-existent event ID
	_, err := FormatMessageById("ESENT", 99999, 2052)
	if err == nil {
		t.Log("FormatMessageById succeeded for non-existent event ID (may be expected)")
	}
}
```

### 4.2 集成测试

#### 测试 2：msgdb-dump 完整提取

```bash
# 测试 ESENT（之前失败的 Provider）
msgdb-dump --out-dir ./test-esent --lang 1033,2052 --providers "ESENT"

# 验证结果
sqlite3 ./test-esent/hotech-msg-*.db "SELECT provider_name, event_id, lang, substr(message, 1, 80) FROM provider_messages WHERE provider_name = 'ESENT' LIMIT 10;"
```

**预期结果**:
- 提取到 ESENT 的消息模板（en-US 和 zh-CN）
- 消息包含 %1 %2 占位符

#### 测试 3：MsiInstaller 提取

```bash
# 测试 MsiInstaller（之前失败的 Provider）
msgdb-dump --out-dir ./test-msi --lang 1033,2052 --providers "MsiInstaller"

# 验证结果
sqlite3 ./test-msi/hotech-msg-*.db "SELECT provider_name, event_id, lang, substr(message, 1, 80) FROM provider_messages WHERE provider_name = 'MsiInstaller' LIMIT 10;"
```

**预期结果**:
- 提取到 MsiInstaller 的消息模板

#### 测试 4：已有 Provider 不受影响

```bash
# 测试已有 Provider（不应触发 API fallback）
msgdb-dump --out-dir ./test-existing --lang 1033,2052 --providers "Microsoft-Windows-Security-Auditing"

# 验证结果
sqlite3 ./test-existing/hotech-msg-*.db "SELECT provider_name, event_id, lang, substr(message, 1, 80) FROM provider_messages WHERE provider_name = 'Microsoft-Windows-Security-Auditing' AND event_id = 4624;"
```

**预期结果**:
- PE 解析成功，不触发 API fallback
- 消息内容与之前一致

### 4.3 端到端测试

#### 测试 5：完整 DB 重新生成

```bash
# 重新生成完整 DB
msgdb-dump --out-dir ./output --lang 1033,2052

# 验证 ESENT 有消息
sqlite3 ./output/hotech-msg-*.db "SELECT COUNT(*) FROM provider_messages WHERE provider_name = 'ESENT';"

# 验证 MsiInstaller 有消息
sqlite3 ./output/hotech-msg-*.db "SELECT COUNT(*) FROM provider_messages WHERE provider_name = 'MsiInstaller';"
```

**预期结果**:
- ESENT 消息数 > 0
- MsiInstaller 消息数 > 0

#### 测试 6：evtx-tools 解析验证

```bash
# 使用新 DB 解析 EVTX
evtx-tools parse Application.evtx --msg-db ./output/hotech-msg-*.db -o ./output/Application.db --lang zh-CN

# 验证 ESENT 消息
sqlite3 ./output/Application.db "SELECT event_id, provider_name, substr(message, 1, 100) FROM events WHERE provider_name = 'ESENT' AND event_id = 102 LIMIT 1;"
```

**预期结果**:
- 消息不再是 fallback（"无法找到来自源 ESENT 的事件 ID 102 的描述"）
- 消息包含中文模板（如 "svchost (41104,P,98,0) DS_Token_DB: 数据库引擎..."）

#### 测试 7：与 Lua 基线对比

```bash
# 对比 ESENT 102 消息
# Lua 基线: svchost (41104,P,98,0) DS_Token_DB: 数据库引擎(10.00.26220.0000)正在启动新实例(0)。
# evtx-tools: 应该匹配或接近

sqlite3 ./output/Application.db "SELECT event_id, substr(message, 1, 120) FROM events WHERE provider_name = 'ESENT' AND event_id = 102 LIMIT 1;"
sqlite3 ./evtx_new.db "SELECT EventID, substr(Message, 1, 120) FROM aevtx WHERE Provider = 'ESENT' AND EventID = 102 LIMIT 1;"
```

**预期结果**:
- 消息内容匹配或接近 Lua 基线

---

## 5. 验收标准

### 5.1 功能验收

| 标准 | 验证方法 | 通过条件 |
|------|----------|----------|
| ESENT 有消息 | sqlite3 查询 | COUNT > 0 |
| MsiInstaller 有消息 | sqlite3 查询 | COUNT > 0 |
| 已有 Provider 不受影响 | 对比前后 DB | 消息内容一致 |
| 消息格式正确 | 检查占位符 | 包含 %1 %2 |
| 中英文都有 | 检查 lang 字段 | en-US 和 zh-CN 都有 |

### 5.2 性能验收

| 标准 | 验证方法 | 通过条件 |
|------|----------|----------|
| 提取时间 | 计时 | 不超过原来的 2 倍 |
| DB 大小 | 检查文件 | 不超过原来的 1.5 倍 |

### 5.3 兼容性验收

| 标准 | 验证方法 | 通过条件 |
|------|----------|----------|
| Windows 10/11 | 在两台机器测试 | 都能正常工作 |
| evtx-tools 解析 | 解析 EVTX 文件 | 消息正确显示 |

---

## 6. 回滚计划

如果实施后出现问题，回滚步骤：

1. **wcorefx**: 删除 `FormatMessageById` 函数
2. **msgdb-dump**: 删除 `extractMessagesViaWinAPI` 函数
3. **msgdb-dump**: 恢复 `run_windows.go` 原始代码
4. **重新编译**: `go build`

---

## 7. 后续改进

1. **优化 EventID 范围**: 从 CRIM 资源提取实际 EventID 列表
2. **缓存 Provider 元数据**: 避免重复调用 OpenPublisherMetadata
3. **并行提取**: 多个 Provider 并行调用 API
4. **增量更新**: 只提取新增的 Provider
5. **错误处理**: 更详细的错误日志和重试机制

---

## 8. 相关文档

- `docs/plan-msgdb-dump-winapi-fallback.md` — 设计计划
- `output/deep-investigation-report.md` — 问题调研报告
- `output/evtx-v5-comparison-report-final.md` — 对比测试报告
- `evtx-tools/docs/CONTEXT.md` — 领域上下文
