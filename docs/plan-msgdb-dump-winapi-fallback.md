# 修改计划：msgdb-dump 使用 Windows API 补充消息提取

## 背景

### 问题
`msgdb-dump` 通过 PE 解析提取消息模板（RT_MESSAGETABLE / WEVT_TEMPLATE / CRIM），
但某些 DLL（如 `esent.dll`）没有这些资源，导致消息提取失败。

`evtx2csv.exe` 使用 Windows Event Log API（`EvtFormatMessage`），可以获取所有消息，
包括存储在系统内部缓存中的消息。

### 目标
在 PE 解析失败时，使用 Windows API 补充提取消息模板，存入 DB。

### 决策记录

| # | 决策 | 选择 |
|---|------|------|
| Q1 | 实现范围 | 补充 fallback — PE 解析失败时用 Windows API 补充 |
| Q2 | 调用时机 | 提取阶段 — `msgdb-dump` 运行时调用，结果存入 DB |
| Q3 | API 调用方式 | 通过 Provider 名称 — `EvtOpenPublisherMetadata` |
| Q4 | 消息模板格式 | `EvtFormatMessageId` — 获取带 %1 %2 的原始模板 |
| Q5 | 语言处理 | 循环所有语言 — 默认 en-US + zh-CN |
| Q6 | 判断时机 | 整体为空时补充 — `MessagesByLang` 为空才调用 API |
| Q7 | CLI 参数 | 自动启用 — 无需新参数 |

---

## 修改范围

### 涉及的项目

| 项目 | 文件 | 修改类型 |
|------|------|----------|
| **wcorefx** | `evtx/wevtapi.go` | 新增函数 |
| **msgdb-dump** | `dump/extractor.go` | 新增函数 |
| **msgdb-dump** | `dump/run_windows.go` | 修改调用逻辑 |

---

## 步骤 1：wcorefx — 新增 FormatMessageById 函数

### 文件：`E:\Code\go\wcorefx\evtx\wevtapi.go`

在 `OpenPublisherMetadata` 函数之后新增：

```go
// FormatMessageById 获取指定 EventID 的消息模板。
//
// 不需要事件句柄，直接通过 Provider 名称和 EventID 获取消息模板。
// 使用 EvtFormatMessageId 标志获取带 %1 %2 占位符的原始模板。
func FormatMessageById(publisherName string, eventID uint32, lang uint32) (string, error) {
	ph, err := OpenPublisherMetadata(0, publisherName, lang)
	if err != nil {
		return "", fmt.Errorf("OpenPublisherMetadata(%s, %d): %w", publisherName, lang, err)
	}
	defer ph.Close()

	var bufferUsed uint32
	err = evtFormatMessage(ph, 0, eventID, 0, nil, EvtFormatMessageId, 0, nil, &bufferUsed)
	if err != nil && err != ERROR_INSUFFICIENT_BUFFER {
		return "", fmt.Errorf("EvtFormatMessage(%s, %d) size: %w", publisherName, eventID, err)
	}

	bufferUsed *= 2
	buffer := make([]byte, bufferUsed)
	bufferUsed = 0
	err = evtFormatMessage(ph, 0, eventID, 0, nil, EvtFormatMessageId,
		uint32(len(buffer)/2), &buffer[0], &bufferUsed)
	if err != nil {
		return "", fmt.Errorf("EvtFormatMessage(%s, %d): %w", publisherName, eventID, err)
	}

	if bufferUsed == 0 {
		return "", nil
	}
	u16 := unsafe.Slice((*uint16)(unsafe.Pointer(&buffer[0])), bufferUsed)
	return windows.UTF16ToString(u16), nil
}
```

---

## 步骤 2：msgdb-dump — 新增 Windows API 提取函数

### 文件：`E:\Code\evet\msgdb-dump\dump\extractor.go`

在文件末尾新增：

```go
// extractMessagesViaWinAPI 使用 Windows Event Log API 提取消息模板。
// 当 PE 解析失败时调用。
func extractMessagesViaWinAPI(providerName string, langLCID string) map[uint32]string {
	lcid := langCodeToLCID(langLCID)
	lcidInt, err := strconv.ParseUint(lcid, 10, 32)
	if err != nil {
		return nil
	}

	msgs := make(map[uint32]string)
	// Try common event ID ranges
	for eid := 1; eid <= 9999; eid++ {
		msg, err := evtx.FormatMessageById(providerName, uint32(eid), uint32(lcidInt))
		if err != nil {
			continue
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

---

## 步骤 3：msgdb-dump — 修改提取逻辑

### 文件：`E:\Code\evet\msgdb-dump\dump\run_windows.go`

在 `ExtractSingleDLL` 调用之后，检查 `MessagesByLang` 是否为空：

```go
result := ExtractSingleDLL(dllPath, providers, langs, langCodes)
if result != nil {
    // Windows API fallback
    if len(result.MessagesByLang) == 0 {
        fmt.Printf("[info] PE extraction empty for '%s', trying Windows API fallback...\n", result.DLLName)
        for _, provider := range providers {
            for langIdx, langCode := range langCodes {
                msgs := extractMessagesViaWinAPI(provider.Name, langCode)
                if len(msgs) > 0 {
                    var peMsgs []PeMessage
                    for eid, msg := range msgs {
                        peMsgs = append(peMsgs, PeMessage{EventID: eid, Message: msg})
                    }
                    result.MessagesByLang[langCode] = append(result.MessagesByLang[langCode], peMsgs...)
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

## 步骤 4：测试

| 测试 | Provider | EventID | 预期 |
|------|----------|---------|------|
| 1 | ESENT | 102 | 获取中文消息模板 |
| 2 | MsiInstaller | 1033 | 获取中文消息模板 |
| 3 | SecurityCenter | 15 | 获取中文消息模板 |
| 4 | Microsoft-Windows-Security-Auditing | 4624 | PE 解析成功，不调用 API |
| 5 | EdgeWebView | 256 | API 可能失败，使用 Data.#text |

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| API 调用慢 | 只在 PE 解析失败时调用 |
| 枚举 EventID 不现实 | 使用常见范围 1-9999 |
| Windows 版本差异 | wevtapi.dll Vista+ 都支持 |

---

## 后续改进

1. 从 CRIM 资源提取实际 EventID 列表，只查询存在的 ID
2. 缓存 Provider 元数据句柄
3. 并行提取多个 Provider
