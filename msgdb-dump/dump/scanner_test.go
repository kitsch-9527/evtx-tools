//go:build windows

package dump

import (
	"testing"
)

func TestEnumETWProviders_returnsNonEmpty(t *testing.T) {
	providers, err := EnumETWProviders()
	if err != nil {
		t.Fatalf("EnumETWProviders() error: %v", err)
	}
	if len(providers) == 0 {
		t.Fatal("EnumETWProviders() returned empty list")
	}
}

func TestEnumETWProviders_noDuplicateNameDLL(t *testing.T) {
	providers, err := EnumETWProviders()
	if err != nil {
		t.Fatalf("EnumETWProviders() error: %v", err)
	}
	seen := make(map[string]bool)
	for _, p := range providers {
		key := p.Name + "|" + p.DLLPath
		if seen[key] {
			t.Fatalf("duplicate (name, dll_path) pair: '%s' @ '%s'", p.Name, p.DLLPath)
		}
		seen[key] = true
	}
}

func TestEnumETWProviders_hasNameAndDLL(t *testing.T) {
	providers, err := EnumETWProviders()
	if err != nil {
		t.Fatalf("EnumETWProviders() error: %v", err)
	}
	for _, p := range providers {
		if p.Name == "" {
			t.Error("provider has empty Name")
		}
		if p.DLLPath == "" {
			t.Errorf("provider '%s' has empty DLLPath", p.Name)
		}
	}
}

func TestEnumETWProviders_standardChannels(t *testing.T) {
	providers, err := EnumETWProviders()
	if err != nil {
		t.Fatalf("EnumETWProviders() error: %v", err)
	}

	channels := make(map[string]bool)
	for _, p := range providers {
		if p.Channel != nil {
			channels[*p.Channel] = true
		}
	}

	for _, want := range []string{"Application", "Security", "System"} {
		if !channels[want] {
			t.Logf("Note: standard channel %q not found among %d providers", want, len(providers))
		}
	}
}

func TestScanEventLog_and_WINEVT(t *testing.T) {
	// Test that both scan functions work independently
	elProviders, err := scanEventLog()
	if err != nil {
		t.Fatalf("scanEventLog() error: %v", err)
	}
	t.Logf("EventLog providers: %d", len(elProviders))

	weProviders, err := scanWINEVTPublishers()
	if err != nil {
		t.Fatalf("scanWINEVTPublishers() error: %v", err)
	}
	t.Logf("WINEVT publishers: %d", len(weProviders))

	if len(elProviders) == 0 && len(weProviders) == 0 {
		t.Fatal("both EventLog and WINEVT returned empty")
	}
}
