package crim

import (
	"testing"
)

func TestNormalizeGUID(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"{f1ef270a-0d32-4352-ba52-dbab41e1d859}", "F1EF270A-0D32-4352-BA52-DBAB41E1D859"},
		{"f1ef270a-0d32-4352-ba52-dbab41e1d859", "F1EF270A-0D32-4352-BA52-DBAB41E1D859"},
		{"{85A62A0D-7E17-485F-9D4F-749A287193A6}", "85A62A0D-7E17-485F-9D4F-749A287193A6"},
		{"", ""},
	}
	for _, tt := range tests {
		got := NormalizeGUID(tt.input)
		if got != tt.expected {
			t.Errorf("NormalizeGUID(%q) = %q, want %q", tt.input, got, tt.expected)
		}
	}
}

func TestGUIDBytesToString(t *testing.T) {
	// CRIM binary GUID for {F1EF270A-0D32-4352-BA52-DBAB41E1D859}
	// GUID struct: Data1(u32 LE) + Data2(u16 LE) + Data3(u16 LE) + Data4[8](raw)
	// Data1 = 0xF1EF270A → LE bytes: [0x0A, 0x27, 0xEF, 0xF1]
	// Data2 = 0x0D32     → LE bytes: [0x32, 0x0D]
	// Data3 = 0x4352     → LE bytes: [0x52, 0x43]
	// Data4 = BA52 DBAB41E1D859 → raw bytes
	b := [16]byte{
		0x0A, 0x27, 0xEF, 0xF1, // Data1 = 0xF1EF270A (LE)
		0x32, 0x0D, // Data2 = 0x0D32 (LE)
		0x52, 0x43, // Data3 = 0x4352 (LE)
		0xBA, 0x52, 0xDB, 0xAB, 0x41, 0xE1, 0xD8, 0x59, // Data4 raw
	}
	expected := "F1EF270A-0D32-4352-BA52-DBAB41E1D859"
	got := GUIDBytesToString(b)
	if got != expected {
		t.Errorf("GUIDBytesToString(%#v) = %q, want %q", b, got, expected)
	}
}

