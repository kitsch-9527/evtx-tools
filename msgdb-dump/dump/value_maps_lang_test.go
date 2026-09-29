package dump

import (
	"path/filepath"
	"testing"

	"github.com/kitsch-9527/msgdb-dump/schema"
)

// v9 (2026-09-30) bilingual value_maps: the lang column lands per-language
// display rows so the Rust renderer can prefer the render language with an
// en-US fallback (mirrors load_param_refs' dual-layer pattern).

// TestOverlayD2Strings — per-language string table assembly: the main DLL's
// RTStrings+D2Strings form the base; a language's MUI D2Strings fully overlay
// it (same key overrides — mirrors MUI whole-family localization semantics,
// the Ntfs 98 %3 en-bias root cause).
func TestOverlayD2Strings(t *testing.T) {
	base := map[uint32]string{0xD2000001: "is healthy.", 0xD2000002: "base only"}
	mui := map[uint32]string{0xD2000001: "运行状况良好。", 0xD2000003: "mui only"}
	got := overlayStringTable(base, mui)
	if got[0xD2000001] != "运行状况良好。" {
		t.Errorf("MUI must fully override base at same key, got %q", got[0xD2000001])
	}
	if got[0xD2000002] != "base only" {
		t.Errorf("base-only key must survive, got %q", got[0xD2000002])
	}
	if got[0xD2000003] != "mui only" {
		t.Errorf("MUI-only key must be added, got %q", got[0xD2000003])
	}
}

func TestOverlayD2Strings_EmptyMUIKeepsBase(t *testing.T) {
	base := map[uint32]string{0xD2000001: "is healthy."}
	got := overlayStringTable(base, nil)
	if got[0xD2000001] != "is healthy." {
		t.Errorf("empty overlay must keep base, got %q", got[0xD2000001])
	}
}

// TestInsertValueMap_LangRows — InsertValueMap writes lang-scoped rows: the
// same (provider, map, value) key can hold both a zh-CN and an en-US display.
func TestInsertValueMap_LangRows(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "vm.db")
	db := openTestDB(t, dbPath)
	defer db.Close()
	if _, err := db.Exec(`CREATE TABLE value_maps (
		provider_name TEXT NOT NULL,
		map_name      TEXT NOT NULL,
		entry_value   TEXT NOT NULL,
		display_name  TEXT NOT NULL,
		is_bitmap     INTEGER NOT NULL DEFAULT 0,
		lang          TEXT NOT NULL DEFAULT 'en-US',
		PRIMARY KEY (provider_name, map_name, entry_value, lang)
	)`); err != nil {
		t.Fatal(err)
	}
	if err := InsertValueMap(db, "Microsoft-Windows-Ntfs", "VolumeCorruptionActionState", "0", "is healthy.", false, "en-US"); err != nil {
		t.Fatal(err)
	}
	if err := InsertValueMap(db, "Microsoft-Windows-Ntfs", "VolumeCorruptionActionState", "0", "运行状况良好。", false, "zh-CN"); err != nil {
		t.Fatal(err)
	}
	var zhDisplay string
	if err := db.QueryRow(
		"SELECT display_name FROM value_maps WHERE provider_name=? AND map_name=? AND entry_value=? AND lang=?",
		"Microsoft-Windows-Ntfs", "VolumeCorruptionActionState", "0", "zh-CN",
	).Scan(&zhDisplay); err != nil {
		t.Fatalf("zh-CN row must exist: %v", err)
	}
	if zhDisplay != "运行状况良好。" {
		t.Errorf("zh display = %q, want 运行状况良好。", zhDisplay)
	}
	var n int
	db.QueryRow("SELECT COUNT(*) FROM value_maps").Scan(&n)
	if n != 2 {
		t.Errorf("want 2 lang rows, got %d", n)
	}
}

// TestWriteValueMaps_ByLang — valueMapEntry.ValueMapsByLang rows land with
// their own lang; the legacy ValueMaps layer lands as en-US.
func TestWriteValueMaps_ByLang(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "vmw.db")
	maps := []valueMapEntry{{
		ProviderName: "Microsoft-Windows-Ntfs",
		ValueMaps: map[string]map[string]string{
			"VolumeCorruptionActionState": {"0": "is healthy.  No action is needed."},
		},
		ValueMapsByLang: map[string]map[string]map[string]string{
			"zh-CN": {
				"VolumeCorruptionActionState": {"0": "运行状况良好。无需执行任何操作。"},
			},
		},
	}}
	if err := writeValueMaps(dbPath, maps); err != nil {
		t.Fatal(err)
	}
	db := openTestDB(t, dbPath)
	defer db.Close()
	var zhDisplay string
	if err := db.QueryRow(
		"SELECT display_name FROM value_maps WHERE lang='zh-CN' AND entry_value='0'",
	).Scan(&zhDisplay); err != nil {
		t.Fatalf("zh-CN row must exist: %v", err)
	}
	if zhDisplay != "运行状况良好。无需执行任何操作。" {
		t.Errorf("zh display = %q", zhDisplay)
	}
	var enDisplay string
	if err := db.QueryRow(
		"SELECT display_name FROM value_maps WHERE lang='en-US' AND entry_value='0'",
	).Scan(&enDisplay); err != nil {
		t.Fatalf("en-US row must exist: %v", err)
	}
	if enDisplay != "is healthy.  No action is needed." {
		t.Errorf("en display = %q", enDisplay)
	}
}

// TestInsertSystemMetadata_SchemaVersion9 — the writer stamps v9.
func TestInsertSystemMetadata_SchemaVersion9(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "meta.db")
	db := openTestDB(t, dbPath)
	defer db.Close()
	if _, err := db.Exec(schema.SchemaDDL); err != nil {
		t.Fatal(err)
	}
	arch := "amd64"
	if err := insertSystemMetadata(db, OsInfo{ProductName: "Windows 10 Education", DisplayVersion: "25H2", BuildNumber: "26220.9343", Architecture: &arch}, []string{"en-US", "zh-CN"}); err != nil {
		t.Fatal(err)
	}
	var v string
	if err := db.QueryRow("SELECT value FROM metadata WHERE key='schema_version'").Scan(&v); err != nil {
		t.Fatal(err)
	}
	if v != "9" {
		t.Errorf("schema_version = %q, want 9", v)
	}
}
