//go:build windows

package dump

import (
	"path/filepath"
	"testing"
)

// R12 / ADR 0004: the eventlog_sources snapshot must round-trip and dedupe,
// and CreateSystemDB must leave the (empty) table in place for it.
func TestWriteEventlogSources(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test_sources.db")

	group := makeTestVariantGroup(
		"C:\\test.dll",
		makeTestProvider("Test-Provider", "{GUID}", "Application"),
		[]VariantRow{makeTestVariantRow(100, "Test event")},
	)
	if err := CreateSystemDB(dbPath, []ProviderVariantGroup{group}, testOsInfo(), []string{"en-US"}); err != nil {
		t.Fatalf("CreateSystemDB failed: %v", err)
	}

	sources := []EventlogSource{
		{Name: "Chrome", Type: "classic"},
		{Name: "Microsoft-Windows-AppXDeploymentServer", Type: "winevt"},
		{Name: "", Type: "classic"}, // empty names must be skipped
	}
	if err := WriteEventlogSources(dbPath, sources); err != nil {
		t.Fatalf("WriteEventlogSources failed: %v", err)
	}

	db := openTestDB(t, dbPath)
	defer db.Close()

	if n := countRows(db, "eventlog_sources"); n != 2 {
		t.Errorf("eventlog_sources rows = %d, want 2", n)
	}
	var typ string
	if err := db.QueryRow(
		"SELECT source_type FROM eventlog_sources WHERE source_name = 'Microsoft-Windows-AppXDeploymentServer'",
	).Scan(&typ); err != nil {
		t.Fatalf("winevt source missing: %v", err)
	}
	if typ != "winevt" {
		t.Errorf("source_type = %q, want winevt", typ)
	}

	// Idempotent rewrite must not duplicate rows.
	if err := WriteEventlogSources(dbPath, sources[:2]); err != nil {
		t.Fatalf("rewrite failed: %v", err)
	}
	if n := countRows(db, "eventlog_sources"); n != 2 {
		t.Errorf("after rewrite rows = %d, want 2 (dedupe)", n)
	}
}

// Live-registry smoke test: a real Windows box always has both classic
// sources and WINEVT publishers; the EventLog service's own source must be
// present. Failure here means collection silently broke (empty snapshot
// would disable N/A parity downstream, ADR 0004).
func TestCollectEventlogSources_Live(t *testing.T) {
	sources, err := CollectEventlogSources()
	if err != nil {
		t.Fatalf("CollectEventlogSources failed: %v", err)
	}
	if len(sources) < 100 {
		t.Fatalf("got %d sources, want >=100 on a real Windows box", len(sources))
	}
	seen := map[string]string{}
	for _, s := range sources {
		if s.Name == "" {
			t.Fatal("empty source name in snapshot")
		}
		if s.Type != "classic" && s.Type != "winevt" {
			t.Errorf("source %q has unknown type %q", s.Name, s.Type)
		}
		seen[s.Name] = s.Type
	}
	if _, ok := seen["EventLog"]; !ok {
		t.Error("classic source 'EventLog' missing from snapshot")
	}
}
