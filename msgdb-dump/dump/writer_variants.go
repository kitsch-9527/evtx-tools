package dump

import (
	"database/sql"
	"encoding/json"
	"fmt"

	_ "modernc.org/sqlite"

	"github.com/kitsch-9527/msgdb-dump/schema"
)

// CreateSystemDB creates the single-file message database (schema_version 8,
// ADR 0003 + 0004 + 0005: v7 entity tables plus the eventlog_sources and
// param_types provenance tables). One file holds all 9 tables; no compat views. The caller
// writes parameter_refs / value_maps / param_value_maps afterwards via the
// shared write* helpers (same transaction scope as the legacy v5 path).
func CreateSystemDB(dbPath string, groups []ProviderVariantGroup, osInfo OsInfo, langCodes []string) error {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return fmt.Errorf("failed to open system db '%s': %w", dbPath, err)
	}
	defer db.Close()

	if _, err := db.Exec(schema.BulkInsertPragmas); err != nil {
		return fmt.Errorf("failed to set PRAGMAs: %w", err)
	}
	if _, err := db.Exec("BEGIN"); err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	if _, err := db.Exec(schema.SchemaDDL); err != nil {
		return fmt.Errorf("failed to create schema: %w", err)
	}

	for _, g := range groups {
		dllID, err := insertDLLEntry(db, g.DLLName, g.DLLPath, g.PEVersion)
		if err != nil {
			return err
		}
		if err := insertProvidersBatch(db, dllID, []ProviderInfo{g.Provider}); err != nil {
			return err
		}
		// Writer no longer expands per-provider copies (Q2=A); one Provider → its Variants only.
		for lang, variants := range g.VariantsByLang {
			if err := insertVariantMessagesBatch(db, g.Provider.Name, providerGUIDOrEmpty(g.Provider.GUID), dllID, lang, variants); err != nil {
				return err
			}
		}
	}

	if err := insertSystemMetadata(db, osInfo, langCodes); err != nil {
		return err
	}
	if _, err := db.Exec("COMMIT"); err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}
	if _, err := db.Exec(schema.SchemaIndexes); err != nil {
		return fmt.Errorf("failed to create indexes: %w", err)
	}
	if _, err := db.Exec("ANALYZE"); err != nil {
		return fmt.Errorf("failed to ANALYZE: %w", err)
	}
	return nil
}

// InsertParameterRef inserts a single %%N parameter reference.
func InsertParameterRef(db *sql.DB, providerName string, paramID int, message, lang string) error {
	_, err := db.Exec(
		"INSERT OR IGNORE INTO parameter_refs (provider_name, param_id, message, lang) VALUES (?, ?, ?, ?)",
		providerName, paramID, message, lang,
	)
	if err != nil {
		return fmt.Errorf("failed to insert param_ref %s/%%%d: %w", providerName, paramID, err)
	}
	return nil
}

// InsertValueMap inserts a single value map entry. v9 (2026-09-30): rows are
// lang-scoped — the same (provider, map, value) key can hold one display per
// language (zh-CN MUI overlay vs en-US base).
func InsertValueMap(db *sql.DB, providerName, mapName, entryValue, displayName string, isBitmap bool, lang string) error {
	bitmapVal := 0
	if isBitmap {
		bitmapVal = 1
	}
	_, err := db.Exec(
		"INSERT OR IGNORE INTO value_maps (provider_name, map_name, entry_value, display_name, is_bitmap, lang) VALUES (?, ?, ?, ?, ?, ?)",
		providerName, mapName, entryValue, displayName, bitmapVal, lang,
	)
	if err != nil {
		return fmt.Errorf("failed to insert value_map '%s' for %s: %w", mapName, providerName, err)
	}
	return nil
}

// EventlogSource is one registered event source name (R12, ADR 0004).
// Type records provenance: "classic" (EventLog\<channel>\<name> key) or
// "winevt" (WINEVT Publishers default-value name).
type EventlogSource struct {
	Name string
	Type string
}

// WriteEventlogSources writes the registry source snapshot into the optional
// eventlog_sources table (R12, ADR 0004). Readers treat an absent or empty
// table as "registry state unknown" and keep legacy fallback rendering.
func WriteEventlogSources(dbPath string, sources []EventlogSource) error {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return fmt.Errorf("open db for eventlog_sources: %w", err)
	}
	defer db.Close()

	if _, err := db.Exec(schema.BulkInsertPragmas); err != nil {
		return fmt.Errorf("pragmas eventlog_sources: %w", err)
	}
	if _, err := db.Exec("BEGIN"); err != nil {
		return fmt.Errorf("begin eventlog_sources txn: %w", err)
	}
	written := 0
	for _, s := range sources {
		if s.Name == "" {
			continue
		}
		if _, err := db.Exec(
			"INSERT OR IGNORE INTO eventlog_sources (source_name, source_type) VALUES (?, ?)",
			s.Name, s.Type,
		); err != nil {
			fmt.Printf("[warn] eventlog_source '%s': %v\n", s.Name, err)
			continue
		}
		written++
	}
	if _, err := db.Exec("COMMIT"); err != nil {
		return fmt.Errorf("commit eventlog_sources: %w", err)
	}
	return nil
}

func providerGUIDOrEmpty(guid *string) string {
	if guid == nil {
		return ""
	}
	return *guid
}

func insertVariantMessagesBatch(db *sql.DB, providerName, providerGUID string, dllID int64, lang string, variants []VariantRow) error {
	if len(variants) == 0 {
		return nil
	}
	const chunkSize = 400
	for start := 0; start < len(variants); start += chunkSize {
		end := start + chunkSize
		if end > len(variants) {
			end = len(variants)
		}
		chunk := variants[start:end]
		stmt := "INSERT OR IGNORE INTO provider_message_variants " +
			"(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message, parameters, level_name, keywords_name) VALUES "
		for i, v := range chunk {
			if i > 0 {
				stmt += ", "
			}
			var paramsJSON string
			var hasParams bool
			if v.Parameters != nil {
				b, err := json.Marshal(*v.Parameters)
				if err != nil {
					return fmt.Errorf("failed to marshal parameters for variant %d: %w", v.EventID, err)
				}
				paramsJSON = string(b)
				hasParams = true
			}
			stmt += fmt.Sprintf("('%s','%s','%s',%d,%d,%d,%d,%d,'%s',%d,%d,%d,'%s',%s,%s,%s)",
				escapeSQL(providerName),
				escapeSQL(providerGUID),
				escapeSQL(lang),
				v.EventID,
				v.Version,
				v.Level,
				v.Opcode,
				v.Task,
				escapeSQL(fmt.Sprintf("%d", v.Keywords)),
				v.Channel,
				v.MessageID,
				dllID,
				escapeSQL(v.Message),
				optStrPtr(&paramsJSON, !hasParams),
				optStrSQL(v.LevelName),
				optStrSQL(v.KeywordsName),
			)
		}
		if _, err := db.Exec(stmt); err != nil {
			return fmt.Errorf("failed to batch insert variants for %s/%s: %w", providerName, lang, err)
		}
	}
	return nil
}

// --- DLL / provider row helpers (moved from writer_sys.go, ADR 0003) ---

// insertDLLEntry inserts or ignores a DLL entry, returns dll_id.
// Idempotent: same dll_path -> same dll_id.
func insertDLLEntry(db *sql.DB, dllName, dllPath, peVersion string) (int64, error) {
	_, err := db.Exec(
		"INSERT OR IGNORE INTO dll_entries (dll_name, dll_path, pe_version) VALUES (?, ?, ?)",
		dllName, dllPath, peVersion,
	)
	if err != nil {
		return 0, fmt.Errorf("failed to insert dll_entry '%s': %w", dllPath, err)
	}

	var id int64
	err = db.QueryRow("SELECT dll_id FROM dll_entries WHERE dll_path = ?", dllPath).Scan(&id)
	if err != nil {
		return 0, fmt.Errorf("failed to read dll_id for '%s': %w", dllPath, err)
	}
	return id, nil
}

// insertProvidersBatch inserts providers for a DLL using multi-row VALUES syntax.
func insertProvidersBatch(db *sql.DB, dllID int64, providers []ProviderInfo) error {
	if len(providers) == 0 {
		return nil
	}

	stmt := "INSERT OR IGNORE INTO providers (provider_name, guid, channel, dll_id) VALUES "
	for i, p := range providers {
		if i > 0 {
			stmt += ", "
		}
		stmt += fmt.Sprintf("('%s',%s,%s,%d)",
			escapeSQL(p.Name),
			optStrSQL(p.GUID),
			optStrSQL(p.Channel),
			dllID,
		)
	}

	if _, err := db.Exec(stmt); err != nil {
		return fmt.Errorf("failed to batch insert providers for dll_id=%d: %w", dllID, err)
	}
	return nil
}

// --- SQL escaping helpers (moved from writer_sys.go) ---

// escapeSQL doubles single quotes for safe SQL string embedding.
func escapeSQL(s string) string {
	escaped := make([]byte, 0, len(s)*2)
	for i := 0; i < len(s); i++ {
		if s[i] == '\'' {
			escaped = append(escaped, '\'', '\'')
		} else {
			escaped = append(escaped, s[i])
		}
	}
	return string(escaped)
}

// optStrSQL formats an optional string for SQL: wraps in quotes if present, NULL otherwise.
func optStrSQL(s *string) string {
	if s == nil || *s == "" {
		return "NULL"
	}
	return "'" + escapeSQL(*s) + "'"
}

// optStrPtr formats an optional string pointer for SQL.
// If isNil is true, returns NULL; otherwise wraps in quotes.
func optStrPtr(s *string, isNil bool) string {
	if isNil || s == nil {
		return "NULL"
	}
	return "'" + escapeSQL(*s) + "'"
}

// joinLangCodes joins language codes into a comma-separated string.
func joinLangCodes(codes []string) string {
	result := ""
	for i, c := range codes {
		if i > 0 {
			result += ","
		}
		result += c
	}
	return result
}

func insertSystemMetadata(db *sql.DB, osInfo OsInfo, langCodes []string) error {
	rows := map[string]string{
		"product_name":    osInfo.ProductName,
		"display_version": osInfo.DisplayVersion,
		"build_number":    osInfo.BuildNumber,
		"schema_version":  "9",
		"langs":           joinLangCodes(langCodes),
		"created_at":      "2026-09-02T00:00:00Z",
	}
	for k, v := range rows {
		if _, err := db.Exec("INSERT OR REPLACE INTO metadata (key, value) VALUES (?, ?)", k, v); err != nil {
			return fmt.Errorf("failed to insert metadata '%s': %w", k, err)
		}
	}
	if osInfo.Architecture != nil {
		if _, err := db.Exec("INSERT OR REPLACE INTO metadata (key, value) VALUES ('architecture', ?)", *osInfo.Architecture); err != nil {
			return fmt.Errorf("failed to insert architecture: %w", err)
		}
	}
	return nil
}
