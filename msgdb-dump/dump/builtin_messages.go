package dump

// injectWellKnownMessages adds well-known classic EventLog messages to the
// DLL groups when they are not already covered by the normal extraction.
//
// Source (phase C, 2026-09-14): these classic messages live in the MUI
// satellite `C:\Windows\System32\en-US\netevent.dll.mui` /
// `zh-CN\netevent.dll.mui` RT_MESSAGETABLE (type 11); the main netevent.dll
// carries no message table, so the default PE extraction path finds nothing.
// 6013 = table ID 0x8000177D (low 16 bits 0x177D = 6013), text
// "The system uptime is %5 seconds." — verified against the live
// Get-WinEvent ground truth (8 substitution slots, slots 1-4 empty, slot 5 =
// uptime seconds).
//
// NOTE: this DllGroup-level helper feeds the legacy (non-variant) pipeline
// and is currently dormant; the live path is injectWellKnownVariants in
// run_variants.go, which injects ONLY 6013 (the one ID the parity corpus
// still diffs on).
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

// wellKnownEventLogMessages returns the classic EventLog event messages as
// recorded in the netevent.dll MUI RT_MESSAGETABLE (phase C ground truth).
//
// 6013 previously carried a fabricated text ("Windows has shut down or
// restarted...") with a comment claiming the messages "cannot be extracted
// from any message DLL" — disproved by phase C. The real table has no 6011
// entry (the old 6011 text was a mis-attributed %1 variant of the real 6013);
// it is kept out of the variant injection path and only retained here for the
// dormant legacy helper.
func wellKnownEventLogMessages() map[string][]PeMessage {
	return map[string][]PeMessage{
		"en-US": {
			{EventID: 6005, Message: "The Event log service was started."},
			{EventID: 6006, Message: "The Event log service was stopped."},
			{EventID: 6008, Message: "The previous system shutdown at %1 on %2 was unexpected."},
			{EventID: 6009, Message: "Microsoft (R) Windows (R) %1 %2 %3 %4"},
			{EventID: 6013, Message: "The system uptime is %5 seconds."},
		},
		"zh-CN": {
			{EventID: 6005, Message: "Event Log 服务已启动。"},
			{EventID: 6006, Message: "Event Log 服务已停止。"},
			{EventID: 6008, Message: "上一次系统的 %1 在 %2 的关闭是意外的。"},
			{EventID: 6009, Message: "Microsoft (R) Windows (R) %1 %2 %3 %4"},
			{EventID: 6013, Message: "系统启动时间为 %5 秒。"},
		},
	}
}
