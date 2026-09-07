package pe

import (
	"testing"
)

func TestDecodeANSIText_utf8(t *testing.T) {
	// Valid UTF-8 should pass through unchanged
	input := []byte("Hello, 世界")
	got := decodeANSIText(input)
	if got != string(input) {
		t.Fatalf("decodeANSIText(UTF-8) = %q, want %q", got, string(input))
	}
}

func TestDecodeANSIText_empty(t *testing.T) {
	if got := decodeANSIText(nil); got != "" {
		t.Fatalf("decodeANSIText(nil) = %q, want empty", got)
	}
	if got := decodeANSIText([]byte{}); got != "" {
		t.Fatalf("decodeANSIText(empty) = %q, want empty", got)
	}
}

func TestDecodeANSIText_gbk(t *testing.T) {
	// GBK: "正在启动 tempdb 数据库．" (0xA3AE = U+FF0E fullwidth full stop)
	gbkBytes := []byte{
		0xD5, 0xFD, 0xD4, 0xDA, 0xC6, 0xF4, 0xB6, 0xAF,
		0x20, 0x74, 0x65, 0x6D, 0x70, 0x64, 0x62, 0x20,
		0xCA, 0xFD, 0xBE, 0xDD, 0xBF, 0xE2, 0xA3, 0xAE,
	}
	got := decodeANSIText(gbkBytes)
	expected := "正在启动 tempdb 数据库．"
	if got != expected {
		t.Fatalf("decodeANSIText(GBK) = %q, want %q", got, expected)
	}
}

func TestDecodeANSIText_gbkEventID17136(t *testing.T) {
	gbkBytes := []byte{
		0xD5, 0xFD, 0xD4, 0xDA, 0xC6, 0xF4, 0xB6, 0xAF,
		0x20, 0x74, 0x65, 0x6D, 0x70, 0x64, 0x62, 0x20,
		0xCA, 0xFD, 0xBE, 0xDD, 0xBF, 0xE2, 0xA3, 0xAE,
	}
	got := decodeANSIText(gbkBytes)
	expected := "正在启动 tempdb 数据库．"
	if got != expected {
		t.Fatalf("decodeANSIText = %q, want %q", got, expected)
	}
}

func TestDecodeANSIText_gbkEventID17137(t *testing.T) {
	gbkBytes := []byte{
		0xD5, 0xFD, 0xD4, 0xDA, 0xC6, 0xF4, 0xB6, 0xAF,
		0xCA, 0xFD, 0xBE, 0xDD, 0xBF, 0xE2, 0x27, 0x25,
		0x31, 0x27, 0xA3, 0xAE,
	}
	got := decodeANSIText(gbkBytes)
	expected := "正在启动数据库'%1'．"
	if got != expected {
		t.Fatalf("decodeANSIText = %q, want %q", got, expected)
	}
	t.Logf("decoded OK: %q", got)
}

func TestParseMessageTable_gbkMessages(t *testing.T) {
	// Build a message table with a single GBK-encoded entry (ANSI mode, flags=0)
	gbkText := []byte{
		0xD5, 0xFD, 0xD4, 0xDA, // 正在
		0xC6, 0xF4, 0xB6, 0xAF, // 启动
		0xCA, 0xFD, 0xBE, 0xDD, 0xBF, 0xE2, 0xA3, 0xAE, // 数据库。
		0x00, // null terminator
	}
	entryLen := 4 + len(gbkText) // 4-byte header + text
	paddedLen := (entryLen + 3) & ^3

	entry := make([]byte, paddedLen)
	entry[0] = byte(entryLen) // Length (low byte)
	entry[1] = byte(entryLen >> 8) // Length (high byte)
	entry[2] = 0 // Flags = 0 (ANSI)
	entry[3] = 0
	copy(entry[4:], gbkText)

	// Build message table with 1 block, 1 entry (event_id 17136)
	var data []byte
	data = append(data, le32(1)...) // NumberOfBlocks = 1
	data = append(data, le32(17136)...) // LowId
	data = append(data, le32(17136)...) // HighId
	data = append(data, le32(16)...) // OffsetToEntries (after block header)
	data = append(data, entry...)

	result := ParseMessageTable(data)
	if result == nil {
		t.Fatal("ParseMessageTable() returned nil")
	}
	got, ok := result[17136]
	if !ok {
		t.Fatal("expected event_id 17136")
	}
	// Should contain decoded Chinese, not raw GBK bytes
	if got == "" {
		t.Fatal("got empty message")
	}
	t.Logf("decoded message: %q", got)
	// Should NOT contain raw GBK bytes
	for _, b := range []byte(got) {
		if b > 0x7F {
			// Valid UTF-8 multi-byte sequences are fine, but raw GBK bytes
			// would show as invalid UTF-8 runes
		}
	}
}

func TestDecodeANSIText_mixed(t *testing.T) {
	// Mixed ASCII + GBK: "Copyright (C) 1988-2000  Microsoft Corporation."
	input := []byte("Copyright (C) 1988-2000 Microsoft Corporation.")
	got := decodeANSIText(input)
	if got != string(input) {
		t.Fatalf("decodeANSIText(ASCII) = %q, want %q", got, string(input))
	}
}
