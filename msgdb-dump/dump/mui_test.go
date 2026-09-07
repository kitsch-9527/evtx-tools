package dump

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestLCIDToLangCode(t *testing.T) {
	tests := []struct {
		lcid string
		want string
	}{
		{"1033", "en-US"},
		{"2052", "zh-CN"},
		{"1028", "zh-TW"},
		{"1041", "ja-JP"},
		{"1042", "ko-KR"},
		{"0000", "en-US"},   // unknown -> fallback
		{"9999", "en-US"},   // unknown -> fallback
		{"", "en-US"},       // empty -> fallback
	}
	for _, tt := range tests {
		got := LCIDToLangCode(tt.lcid)
		if got != tt.want {
			t.Errorf("LCIDToLangCode(%q) = %q, want %q", tt.lcid, got, tt.want)
		}
	}
}

func TestLangCodeToLCID(t *testing.T) {
	tests := []struct {
		code string
		want string
	}{
		{"en-US", "1033"},
		{"zh-CN", "2052"},
		{"zh-TW", "1028"},
		{"ja-JP", "1041"},
		{"ko-KR", "1042"},
		{"fr-FR", "1033"}, // unknown -> fallback
		{"", "1033"},       // empty -> fallback
	}
	for _, tt := range tests {
		got := LangCodeToLCID(tt.code)
		if got != tt.want {
			t.Errorf("LangCodeToLCID(%q) = %q, want %q", tt.code, got, tt.want)
		}
	}
}

func TestFindMUICompanion_nonexistentDLL(t *testing.T) {
	if p := FindMUICompanion(`C:\nonexistent.dll`, "en-US"); p != nil {
		t.Errorf("expected nil for nonexistent DLL, got %q", *p)
	}
	if p := FindMUICompanion("", "en-US"); p != nil {
		t.Errorf("expected nil for empty path, got %q", *p)
	}
}

func TestFindMUICompanion_systemDLL(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	// Microsoft-Windows-System-Events.dll is present on all modern Windows
	sysDLL := `C:\Windows\System32\Microsoft-Windows-System-Events.dll`
	if _, err := os.Stat(sysDLL); os.IsNotExist(err) {
		t.Skipf("%s does not exist on this system", sysDLL)
	}

	mui := FindMUICompanion(sysDLL, "en-US")
	if mui == nil {
		t.Fatalf("FindMUICompanion(%q, en-US) returned nil", sysDLL)
	}
	if !filepath.IsAbs(*mui) {
		t.Fatalf("expected absolute path, got %q", *mui)
	}
	if !strings.HasSuffix(*mui, ".mui") {
		t.Fatalf("expected .mui extension, got %q", *mui)
	}
	if !strings.Contains(*mui, "en-US") {
		t.Fatalf("expected en-US in path, got %q", *mui)
	}
	if _, err := os.Stat(*mui); os.IsNotExist(err) {
		t.Fatalf("MUI path does not exist: %s", *mui)
	}
}

func TestFindAllMUICompanions_systemDLL(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	sysDLL := `C:\Windows\System32\Microsoft-Windows-System-Events.dll`
	if _, err := os.Stat(sysDLL); os.IsNotExist(err) {
		t.Skipf("%s does not exist", sysDLL)
	}

	companions := FindAllMUICompanions(sysDLL)
	if len(companions) == 0 {
		t.Fatal("expected at least one MUI companion")
	}

	found := false
	for _, c := range companions {
		if c.Lang == "en-US" {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("expected en-US in companions, got %v", companions)
	}
}

func TestFindMUICompanion_temporaryDLL(t *testing.T) {
	// Create a fake DLL in a temp directory with a matching .mui file
	dir := t.TempDir()
	dllPath := filepath.Join(dir, "test.dll")
	muiDir := filepath.Join(dir, "en-US")
	muiPath := filepath.Join(muiDir, "test.dll.mui")

	if err := os.MkdirAll(muiDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(dllPath, []byte("fake dll"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(muiPath, []byte("fake mui"), 0644); err != nil {
		t.Fatal(err)
	}

	p := FindMUICompanion(dllPath, "en-US")
	if p == nil {
		t.Fatal("expected MUI companion for temp DLL")
	}
	if *p != muiPath {
		t.Fatalf("got %q, want %q", *p, muiPath)
	}

	// Non-existent language returns nil
	if p := FindMUICompanion(dllPath, "fr-FR"); p != nil {
		t.Fatalf("expected nil for fr-FR, got %q", *p)
	}
}
