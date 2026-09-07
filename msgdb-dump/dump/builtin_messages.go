package dump

// injectWellKnownMessages adds well-known classic EventLog messages to the
// DLL groups when they are not already covered by the normal extraction.
//
// These messages (6005, 6006, 6008, 6009, 6011, 6013) are hardcoded in the
// EventLog service and cannot be extracted from any message DLL.
func injectWellKnownMessages(groups []DllGroup, langCodes []string) []DllGroup {
	// Check if any existing group already covers EventLog classic messages
	alreadyCovered := false
	for _, g := range groups {
		for _, p := range g.Providers {
			if p.Name == "EventLog" {
				if msgs, ok := g.MessagesByLang["en-US"]; ok {
					for _, m := range msgs {
						if m.EventID >= 6005 && m.EventID <= 6013 {
							alreadyCovered = true
							break
						}
					}
				}
				if alreadyCovered {
					break
				}
			}
		}
		if alreadyCovered {
			break
		}
	}
	if alreadyCovered {
		return groups
	}

	// Find or create a DllGroup for netevent.dll
	var target *DllGroup
	for i := range groups {
		if groups[i].DLLPath == `C:\Windows\System32\netevent.dll` {
			target = &groups[i]
			break
		}
	}

	if target == nil {
		// Create new group
		groups = append(groups, DllGroup{
			DLLName:        "netevent.dll",
			DLLPath:        `C:\Windows\System32\netevent.dll`,
			PEVersion:      "10.0.0.0",
			Providers:      []ProviderInfo{{Name: "EventLog", Channel: strPtr("System")}},
			MessagesByLang: make(map[string][]PeMessage),
		})
		target = &groups[len(groups)-1]
	}

	// Add well-known classic EventLog messages
	for langCode, msgs := range wellKnownEventLogMessages() {
		target.MessagesByLang[langCode] = append(target.MessagesByLang[langCode], msgs...)
	}

	return groups
}

// wellKnownEventLogMessages returns the classic EventLog event messages
// that are built into the EventLog service and not extractable from DLLs.
func wellKnownEventLogMessages() map[string][]PeMessage {
	return map[string][]PeMessage{
		"en-US": {
			{EventID: 6005, Message: "The Event log service was started."},
			{EventID: 6006, Message: "The Event log service was stopped."},
			{EventID: 6008, Message: "The previous system shutdown at %1 on %2 was unexpected."},
			{EventID: 6009, Message: "Microsoft (R) Windows (R) %1 %2 %3 %4"},
			{EventID: 6011, Message: "The system uptime is %1 seconds."},
			{EventID: 6013, Message: "Windows has shut down or restarted. The system was last started on %1."},
		},
		"zh-CN": {
			{EventID: 6005, Message: "Event Log 服务已启动。"},
			{EventID: 6006, Message: "Event Log 服务已停止。"},
			{EventID: 6008, Message: "上一次系统的 %1 在 %2 的关闭是意外的。"},
			{EventID: 6009, Message: "Microsoft (R) Windows (R) %1 %2 %3 %4"},
			{EventID: 6011, Message: "系统运行时间 %1 秒。"},
			{EventID: 6013, Message: "Windows 已关闭或重新启动。系统上一次启动是在 %1。"},
		},
	}
}
