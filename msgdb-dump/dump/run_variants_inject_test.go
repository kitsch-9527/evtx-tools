//go:build windows

package dump

import "testing"

// Phase D (2026-09-14): EventLog 6013 well-known variant injection.
func TestInjectWellKnownVariantsAdds6013(t *testing.T) {
	groups := []ProviderVariantGroup{}
	out := injectWellKnownVariants(groups, []string{"en-US", "zh-CN"})
	if len(out) != 1 {
		t.Fatalf("expected 1 group after injection, got %d", len(out))
	}
	g := out[0]
	if g.Provider.Name != "EventLog" {
		t.Fatalf("provider = %q, want EventLog", g.Provider.Name)
	}
	en := g.VariantsByLang["en-US"]
	zh := g.VariantsByLang["zh-CN"]
	if len(en) != 1 || en[0].EventID != 6013 || en[0].Message != "The system uptime is %5 seconds." {
		t.Fatalf("en-US 6013 row wrong: %+v", en)
	}
	if len(zh) != 1 || zh[0].EventID != 6013 || zh[0].Message != "系统启动时间为 %5 秒。" {
		t.Fatalf("zh-CN 6013 row wrong: %+v", zh)
	}
	// Wildcard variant identity: renderer's resolve_variant ladder relies on it.
	if en[0].Version != 0 || en[0].Level != 0 || en[0].Opcode != 0 || en[0].Task != 0 || en[0].Keywords != 0 || en[0].Channel != 0 {
		t.Fatalf("6013 variant must be all-wildcard, got %+v", en[0])
	}
}

func TestInjectWellKnownVariantsSkipsWhenExtracted(t *testing.T) {
	groups := []ProviderVariantGroup{{
		Provider: ProviderInfo{Name: "EventLog"},
		VariantsByLang: map[string][]VariantRow{
			"en-US": {{ProviderName: "EventLog", Lang: "en-US", EventID: 6013, Message: "extracted"}},
		},
	}}
	out := injectWellKnownVariants(groups, []string{"en-US", "zh-CN"})
	if len(out) != 1 || len(out[0].VariantsByLang["en-US"]) != 1 {
		t.Fatalf("real extraction must win over injection, got %+v", out)
	}
	if out[0].VariantsByLang["en-US"][0].Message != "extracted" {
		t.Fatalf("extracted message overwritten: %+v", out[0].VariantsByLang["en-US"][0])
	}
}

func TestInjectWellKnownVariantsAppendsToExistingGroup(t *testing.T) {
	groups := []ProviderVariantGroup{{
		Provider:       ProviderInfo{Name: "EventLog"},
		DLLName:        "netevent.dll",
		VariantsByLang: map[string][]VariantRow{},
	}}
	out := injectWellKnownVariants(groups, []string{"en-US", "zh-CN"})
	if len(out) != 1 {
		t.Fatalf("expected existing group reused, got %d groups", len(out))
	}
	if len(out[0].VariantsByLang["en-US"]) != 1 || out[0].VariantsByLang["en-US"][0].EventID != 6013 {
		t.Fatalf("6013 not appended into existing group: %+v", out[0].VariantsByLang)
	}
}
