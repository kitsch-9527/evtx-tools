package crim

import "testing"

// TestVariantIdentity — Variant Identity (CONTEXT.md) 判定：8 元组全等方为同一 Variant。
// TxR 与 AppContainer 共享 EID=11 但 Level/Opcode 不同，必须为两个独立 Message Variant。
func TestVariantIdentity_TxRvsAppContainer_EID11(t *testing.T) {
	txr := EventEntry{
		EventID:   11,
		Version:   0,
		Channel:   9, // System channel (example)
		Level:     4, // TxR path: Informational-like
		Opcode:    0,
		Task:      0,
		Keywords:  0,
		MessageID: 0x100,
	}
	app := EventEntry{
		EventID:   11,
		Version:   0,
		Channel:   9,
		Level:     0, // AppContainer path: level 0 (generic)
		Opcode:    0,
		Task:      0,
		Keywords:  0,
		MessageID: 0x101,
	}

	// Variant Identity 必须区分：同一 EID 11，因 Level + MessageID 不同，应为不同 Variant
	if txr.Key() == app.Key() {
		t.Fatalf("Variant Identity collision: TxR and AppContainer EID=11 produced same VariantKey %v; expected distinct (Level/MessageID differ)", txr.Key())
	}
}

func TestVariantIdentity_IdenticalEightTuple_Dedup(t *testing.T) {
	a := EventEntry{
		EventID:   11,
		Version:   1,
		Channel:   9,
		Level:     4,
		Opcode:    2,
		Task:      5,
		Keywords:  0x8020000000000000,
		MessageID: 0x1234,
	}
	b := EventEntry{
		EventID:   11,
		Version:   1,
		Channel:   9,
		Level:     4,
		Opcode:    2,
		Task:      5,
		Keywords:  0x8020000000000000,
		MessageID: 0x1234,
	}

	if a.Key() != b.Key() {
		t.Fatalf("Variant Identity mismatch: identical 8-tuple should produce equal VariantKey, got %v vs %v", a.Key(), b.Key())
	}
}

func TestVariantIdentity_OpcodeDistinguishesVariants(t *testing.T) {
	base := EventEntry{EventID: 20, Version: 0, Channel: 9, Level: 4, Opcode: 1, Task: 0, Keywords: 0, MessageID: 0x200}
	other := EventEntry{EventID: 20, Version: 0, Channel: 9, Level: 4, Opcode: 2, Task: 0, Keywords: 0, MessageID: 0x201}
	if base.Key() == other.Key() {
		t.Fatalf("Opcode should distinguish variants: %v == %v", base.Key(), other.Key())
	}
}

func TestDeduplicateVariants_KeepsDistinctLosesDuplicate(t *testing.T) {
	entries := []EventEntry{
		{EventID: 11, Version: 0, Channel: 9, Level: 4, Opcode: 0, Task: 0, Keywords: 0, MessageID: 0x100}, // TxR
		{EventID: 11, Version: 0, Channel: 9, Level: 0, Opcode: 0, Task: 0, Keywords: 0, MessageID: 0x101}, // App
		{EventID: 11, Version: 0, Channel: 9, Level: 4, Opcode: 0, Task: 0, Keywords: 0, MessageID: 0x100}, // duplicate TxR (from second WEVT blob)
	}
	deduped := DeduplicateVariants(entries)
	if len(deduped) != 2 {
		t.Fatalf("DeduplicateVariants: got %d variants, want 2 (TxR + App, duplicate collapsed); got %+v", len(deduped), deduped)
	}
	// Ensure both distinct variants survived (by MessageID)
	seen := map[uint32]bool{}
	for _, e := range deduped {
		seen[e.MessageID] = true
	}
	if !seen[0x100] || !seen[0x101] {
		t.Fatalf("DeduplicateVariants lost a distinct variant: seen=%v", seen)
	}
}
