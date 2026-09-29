package schema

import (
	"strings"
	"testing"
)

// Single-file schema (v7, ADR 0003): 7 entity tables, no views.
var tableNames = []string{
	"dll_entries",
	"metadata",
	"param_value_maps",
	"parameter_refs",
	"provider_message_variants",
	"providers",
	"value_maps",
}

var indexNames = []string{
	"idx_pmv_lookup",
	"idx_pmv_guid",
	"idx_providers_dll",
	"idx_param_value_maps_eid",
}

func TestSchemaDDL_ContainsAllTables(t *testing.T) {
	for _, name := range tableNames {
		if !strings.Contains(SchemaDDL, "CREATE TABLE IF NOT EXISTS "+name) {
			t.Errorf("SchemaDDL missing table: %s", name)
		}
	}
}

func TestSchemaDDL_NoLegacyTablesOrViews(t *testing.T) {
	for _, s := range []string{"provider_messages", "dll_messages", "provider_aliases", "CREATE VIEW"} {
		if strings.Contains(SchemaDDL, s) {
			t.Errorf("SchemaDDL must not contain legacy %q", s)
		}
	}
}

func TestSchemaDDL_VariantPK(t *testing.T) {
	want := "PRIMARY KEY (provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)"
	if !strings.Contains(SchemaDDL, want) {
		t.Errorf("variants table should have 11-column PK")
	}
}

func TestSchemaIndexes_ContainsAllIndexes(t *testing.T) {
	for _, name := range indexNames {
		if !strings.Contains(SchemaIndexes, "CREATE INDEX IF NOT EXISTS "+name) {
			t.Errorf("SchemaIndexes missing index: %s", name)
		}
	}
}

func TestBulkInsertPragmas_ContainsPragmas(t *testing.T) {
	expectedPragmas := []string{"synchronous=OFF", "journal_mode=MEMORY", "cache_size=-64000", "page_size=65536"}
	for _, p := range expectedPragmas {
		if !strings.Contains(BulkInsertPragmas, p) {
			t.Errorf("BulkInsertPragmas missing: %s", p)
		}
	}
}

// v9 (2026-09-30): value_maps carries a lang column so per-language display
// strings can coexist (zh-CN MUI D2 string overlay over the en-US base).
func TestSchemaDDL_ValueMapsLangColumn(t *testing.T) {
	if !strings.Contains(SchemaDDL, "lang          TEXT NOT NULL DEFAULT 'en-US'") {
		t.Errorf("value_maps table should have lang column")
	}
	want := "PRIMARY KEY (provider_name, map_name, entry_value, lang)"
	if !strings.Contains(SchemaDDL, want) {
		t.Errorf("value_maps PK should include lang")
	}
}
