package dump

import (
	"database/sql"
	"path/filepath"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

// Test helpers ----------------------------------------------------------------

func makeTestProvider(name, guid, channel string) ProviderInfo {
	var g, c *string
	if guid != "" {
		g = &guid
	}
	if channel != "" {
		c = &channel
	}
	return ProviderInfo{
		Name:    name,
		GUID:    g,
		DLLPath: "C:\\test.dll",
		Channel: c,
	}
}

func makeTestVariantRow(eventID uint32, msg string) VariantRow {
	return VariantRow{
		ProviderName: "Test-Provider",
		EventID:      eventID,
		Message:      msg,
	}
}

func makeTestVariantGroup(dllPath string, provider ProviderInfo, rows []VariantRow) ProviderVariantGroup {
	byLang := map[string][]VariantRow{}
	if len(rows) > 0 {
		byLang["en-US"] = rows
	}
	return ProviderVariantGroup{
		Provider:       provider,
		DLLName:        filepath.Base(dllPath),
		DLLPath:        dllPath,
		PEVersion:      "10.0.22621.1",
		VariantsByLang: byLang,
	}
}

func countRows(db *sql.DB, table string) int {
	var n int
	db.QueryRow("SELECT COUNT(*) FROM " + table).Scan(&n)
	return n
}

func openTestDB(t *testing.T, dbPath string) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		t.Fatalf("failed to open db: %v", err)
	}
	return db
}

func testOsInfo() OsInfo {
	return OsInfo{
		ProductName:    "TestOS",
		DisplayVersion: "1.0",
		BuildNumber:    "1000",
	}
}

// Tests ----------------------------------------------------------------------

func TestCreateSystemDB_Schema(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test_schema.db")

	group := makeTestVariantGroup(
		"C:\\test.dll",
		makeTestProvider("Test-Provider", "{GUID}", "Application"),
		[]VariantRow{makeTestVariantRow(100, "Test event")},
	)

	if err := CreateSystemDB(dbPath, []ProviderVariantGroup{group}, testOsInfo(), []string{"en-US"}); err != nil {
		t.Fatalf("CreateSystemDB failed: %v", err)
	}

	db := openTestDB(t, dbPath)
	defer db.Close()

	expectedTables := []string{
		"dll_entries",
		"eventlog_sources",
		"metadata",
		"param_types",
		"param_value_maps",
		"parameter_refs",
		"provider_message_variants",
		"providers",
		"value_maps",
	}

	rows, err := db.Query(
		"SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
	)
	if err != nil {
		t.Fatalf("failed to query tables: %v", err)
	}
	defer rows.Close()

	var tables []string
	for rows.Next() {
		var name string
		rows.Scan(&name)
		tables = append(tables, name)
	}

	if len(tables) != len(expectedTables) {
		t.Fatalf("got %d tables, want %d: %v", len(tables), len(expectedTables), tables)
	}
	for i, name := range tables {
		if name != expectedTables[i] {
			t.Errorf("table[%d] = %s, want %s", i, name, expectedTables[i])
		}
	}

	// No views: the legacy dll_messages compat view must be gone.
	var viewCount int
	if err := db.QueryRow("SELECT COUNT(*) FROM sqlite_master WHERE type='view'").Scan(&viewCount); err != nil {
		t.Fatalf("failed to query views: %v", err)
	}
	if viewCount != 0 {
		t.Errorf("got %d views, want 0 (no dll_messages compat view)", viewCount)
	}

	var dllMsgs int
	if err := db.QueryRow("SELECT COUNT(*) FROM sqlite_master WHERE name='dll_messages'").Scan(&dllMsgs); err != nil {
		t.Fatalf("failed to query dll_messages: %v", err)
	}
	if dllMsgs != 0 {
		t.Error("dll_messages table/view must not exist in v7 single-file DB")
	}

	// schema_version metadata must be 9 (v9, 2026-09-30: value_maps lang).
	var sv string
	if err := db.QueryRow("SELECT value FROM metadata WHERE key = 'schema_version'").Scan(&sv); err != nil {
		t.Fatalf("failed to query schema_version: %v", err)
	}
	if sv != "9" {
		t.Errorf("schema_version = %s, want 9", sv)
	}
}

func TestCreateSystemDB_Roundtrip(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test_roundtrip.db")

	provA := makeTestProvider("Prov-A", "{GUID-A}", "Application")
	provB := makeTestProvider("Prov-B", "{GUID-B}", "Security")
	groups := []ProviderVariantGroup{
		makeTestVariantGroup("C:\\test.dll", provA, []VariantRow{
			{ProviderName: "Prov-A", EventID: 100, Message: "Event from shared DLL"},
			{ProviderName: "Prov-A", EventID: 101, Message: "Another event"},
		}),
		makeTestVariantGroup("C:\\test.dll", provB, []VariantRow{
			{ProviderName: "Prov-B", EventID: 100, Message: "Event from shared DLL"},
			{ProviderName: "Prov-B", EventID: 101, Message: "Another event"},
		}),
	}

	if err := CreateSystemDB(dbPath, groups, testOsInfo(), []string{"en-US"}); err != nil {
		t.Fatalf("CreateSystemDB failed: %v", err)
	}

	db := openTestDB(t, dbPath)
	defer db.Close()

	// Same DLL path dedups to one entry.
	if n := countRows(db, "dll_entries"); n != 1 {
		t.Errorf("dll_entries count = %d, want 1", n)
	}
	if n := countRows(db, "providers"); n != 2 {
		t.Errorf("providers count = %d, want 2", n)
	}
	// v7: one row per provider variant — 2 providers x 2 events = 4 rows.
	if n := countRows(db, "provider_message_variants"); n != 4 {
		t.Errorf("provider_message_variants count = %d, want 4", n)
	}
	if n := countRows(db, "metadata"); n != 6 {
		t.Errorf("metadata count = %d, want 6", n)
	}

	// Verify DLL entry
	var dllName, dllPath, peVersion string
	err := db.QueryRow("SELECT dll_name, dll_path, pe_version FROM dll_entries WHERE dll_id = 1").
		Scan(&dllName, &dllPath, &peVersion)
	if err != nil {
		t.Fatalf("failed to query dll_entry: %v", err)
	}
	if dllName != "test.dll" {
		t.Errorf("dll_name = %s, want test.dll", dllName)
	}
	if dllPath != "C:\\test.dll" {
		t.Errorf("dll_path = %s", dllPath)
	}
	if peVersion != "10.0.22621.1" {
		t.Errorf("pe_version = %s", peVersion)
	}

	// Verify variant row via provider lookup
	var eid int
	var msg string
	err = db.QueryRow(
		`SELECT v.event_id, v.message FROM provider_message_variants v
		 JOIN providers p ON p.provider_name = v.provider_name
		 WHERE p.provider_name = ? AND v.event_id = 100 AND v.lang = 'en-US'`,
		"Prov-A",
	).Scan(&eid, &msg)
	if err != nil {
		t.Fatalf("failed to query variant via JOIN: %v", err)
	}
	if eid != 100 {
		t.Errorf("event_id = %d, want 100", eid)
	}
	if msg != "Event from shared DLL" {
		t.Errorf("message = %s", msg)
	}

	// Verify schema_version metadata
	var sv string
	err = db.QueryRow("SELECT value FROM metadata WHERE key = 'schema_version'").Scan(&sv)
	if err != nil {
		t.Fatalf("failed to query schema_version: %v", err)
	}
	if sv != "9" {
		t.Errorf("schema_version = %s, want 9", sv)
	}

	// Verify indexes exist (v7 schema indexes)
	indexes := []string{
		"idx_pmv_lookup",
		"idx_pmv_guid",
		"idx_providers_dll",
		"idx_param_value_maps_eid",
	}
	for _, idx := range indexes {
		var name string
		err = db.QueryRow(
			"SELECT name FROM sqlite_master WHERE type='index' AND name = ?", idx,
		).Scan(&name)
		if err != nil {
			t.Errorf("missing index: %s", idx)
		}
	}
}

func TestCreateSystemDB_MultiLang(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test_multilang.db")

	group := ProviderVariantGroup{
		Provider:  makeTestProvider("Prov-A", "{GUID}", "Application"),
		DLLName:   "test.dll",
		DLLPath:   "C:\\test.dll",
		PEVersion: "1.0.0.0",
		VariantsByLang: map[string][]VariantRow{
			"en-US": {{ProviderName: "Prov-A", EventID: 100, Message: "Hello"}},
			"zh-CN": {{ProviderName: "Prov-A", EventID: 100, Message: "你好"}},
		},
	}

	if err := CreateSystemDB(dbPath, []ProviderVariantGroup{group}, testOsInfo(), []string{"en-US", "zh-CN"}); err != nil {
		t.Fatalf("CreateSystemDB failed: %v", err)
	}

	db := openTestDB(t, dbPath)
	defer db.Close()

	// 2 languages = 2 variant rows
	if n := countRows(db, "provider_message_variants"); n != 2 {
		t.Fatalf("provider_message_variants count = %d, want 2", n)
	}

	// Verify English
	var enMsg, enLang string
	err := db.QueryRow(
		"SELECT message, lang FROM provider_message_variants WHERE dll_id = 1 AND lang='en-US'",
	).Scan(&enMsg, &enLang)
	if err != nil {
		t.Fatalf("failed to query en-US: %v", err)
	}
	if enMsg != "Hello" {
		t.Errorf("en-US message = %s, want Hello", enMsg)
	}
	if enLang != "en-US" {
		t.Errorf("en-US lang = %s", enLang)
	}

	// Verify Chinese
	var zhMsg, zhLang string
	err = db.QueryRow(
		"SELECT message, lang FROM provider_message_variants WHERE dll_id = 1 AND lang='zh-CN'",
	).Scan(&zhMsg, &zhLang)
	if err != nil {
		t.Fatalf("failed to query zh-CN: %v", err)
	}
	if zhMsg != "你好" {
		t.Errorf("zh-CN message = %s", zhMsg)
	}
	if zhLang != "zh-CN" {
		t.Errorf("zh-CN lang = %s", zhLang)
	}

	// Verify langs metadata
	var langs string
	err = db.QueryRow("SELECT value FROM metadata WHERE key = 'langs'").Scan(&langs)
	if err != nil {
		t.Fatalf("failed to query langs metadata: %v", err)
	}
	if langs != "en-US,zh-CN" {
		t.Errorf("langs = %s, want en-US,zh-CN", langs)
	}
}

func TestCreateSystemDB_EmptyGroups(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test_empty.db")

	osInfo := OsInfo{
		ProductName:    "EmptyOS",
		DisplayVersion: "1.0",
		BuildNumber:    "0",
	}

	if err := CreateSystemDB(dbPath, []ProviderVariantGroup{}, osInfo, []string{"en-US"}); err != nil {
		t.Fatalf("CreateSystemDB with empty groups failed: %v", err)
	}

	db := openTestDB(t, dbPath)
	defer db.Close()

	if n := countRows(db, "dll_entries"); n != 0 {
		t.Errorf("dll_entries count = %d, want 0", n)
	}
	if n := countRows(db, "provider_message_variants"); n != 0 {
		t.Errorf("provider_message_variants count = %d, want 0", n)
	}
	if n := countRows(db, "metadata"); n != 6 { // no architecture, 6 base rows
		t.Errorf("metadata count = %d, want 6", n)
	}
}

func TestCreateSystemDB_IDempotentDLL(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test_idempotent.db")

	group1 := makeTestVariantGroup(
		"C:\\shared.dll",
		makeTestProvider("Prov-A", "{GUID}", "App"),
		[]VariantRow{{ProviderName: "Prov-A", EventID: 100, Message: "Event A"}},
	)

	group2 := makeTestVariantGroup(
		"C:\\shared.dll", // same path
		makeTestProvider("Prov-B", "{GUID2}", "Sec"),
		[]VariantRow{{ProviderName: "Prov-B", EventID: 101, Message: "Event B"}},
	)

	if err := CreateSystemDB(dbPath, []ProviderVariantGroup{group1, group2}, testOsInfo(), []string{"en-US"}); err != nil {
		t.Fatalf("CreateSystemDB failed: %v", err)
	}

	db := openTestDB(t, dbPath)
	defer db.Close()

	// Same DLL path = 1 dll_entry
	if n := countRows(db, "dll_entries"); n != 1 {
		t.Errorf("dll_entries count = %d, want 1 (same path dedup)", n)
	}
	// 2 providers
	if n := countRows(db, "providers"); n != 2 {
		t.Errorf("providers count = %d, want 2", n)
	}
	// 2 variant rows (one per provider)
	if n := countRows(db, "provider_message_variants"); n != 2 {
		t.Errorf("provider_message_variants count = %d, want 2", n)
	}
}

func TestCreateSystemDB_WithArchitecture(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test_arch.db")

	arch := "AMD64"
	osInfo := OsInfo{
		ProductName:    "TestOS",
		DisplayVersion: "1.0",
		BuildNumber:    "1000",
		Architecture:   &arch,
	}

	if err := CreateSystemDB(dbPath, []ProviderVariantGroup{}, osInfo, []string{"en-US"}); err != nil {
		t.Fatalf("CreateSystemDB failed: %v", err)
	}

	db := openTestDB(t, dbPath)
	defer db.Close()

	var a string
	err := db.QueryRow("SELECT value FROM metadata WHERE key = 'architecture'").Scan(&a)
	if err != nil {
		t.Fatalf("failed to query architecture: %v", err)
	}
	if a != "AMD64" {
		t.Errorf("architecture = %s, want AMD64", a)
	}
}

func TestInsertParameterRef(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("failed to open in-memory db: %v", err)
	}
	defer db.Close()

	db.Exec("CREATE TABLE IF NOT EXISTS parameter_refs (provider_name TEXT, param_id INTEGER, message TEXT, lang TEXT, PRIMARY KEY (provider_name, param_id, lang))")

	err = InsertParameterRef(db, "Test-Provider", 1, "User: %1", "en-US")
	if err != nil {
		t.Fatalf("InsertParameterRef failed: %v", err)
	}

	var msg string
	err = db.QueryRow(
		"SELECT message FROM parameter_refs WHERE provider_name = ? AND param_id = 1",
		"Test-Provider",
	).Scan(&msg)
	if err != nil {
		t.Fatalf("failed to query param_ref: %v", err)
	}
	if msg != "User: %1" {
		t.Errorf("message = %s, want 'User: %%1'", msg)
	}
}

func TestInsertValueMap(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("failed to open in-memory db: %v", err)
	}
	defer db.Close()

	db.Exec("CREATE TABLE IF NOT EXISTS value_maps (provider_name TEXT, map_name TEXT, entry_value TEXT, display_name TEXT, is_bitmap INTEGER, lang TEXT NOT NULL DEFAULT 'en-US', PRIMARY KEY (provider_name, map_name, entry_value, lang))")

	err = InsertValueMap(db, "Test-Provider", "Level", "2", "Error", false, "en-US")
	if err != nil {
		t.Fatalf("InsertValueMap failed: %v", err)
	}

	var displayName string
	var isBitmap int
	err = db.QueryRow(
		"SELECT display_name, is_bitmap FROM value_maps WHERE provider_name = ? AND map_name = 'Level' AND entry_value = '2'",
		"Test-Provider",
	).Scan(&displayName, &isBitmap)
	if err != nil {
		t.Fatalf("failed to query value_map: %v", err)
	}
	if displayName != "Error" {
		t.Errorf("display_name = %s, want Error", displayName)
	}
	if isBitmap != 0 {
		t.Errorf("is_bitmap = %d, want 0", isBitmap)
	}
}

func TestInsertValueMap_Bitmap(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("failed to open in-memory db: %v", err)
	}
	defer db.Close()

	db.Exec("CREATE TABLE IF NOT EXISTS value_maps (provider_name TEXT, map_name TEXT, entry_value TEXT, display_name TEXT, is_bitmap INTEGER, lang TEXT NOT NULL DEFAULT 'en-US', PRIMARY KEY (provider_name, map_name, entry_value, lang))")

	err = InsertValueMap(db, "Test-Provider", "Keywords", "0x8020000000000000", "Audit Success", true, "en-US")
	if err != nil {
		t.Fatalf("InsertValueMap (bitmap) failed: %v", err)
	}

	var isBitmap int
	err = db.QueryRow(
		"SELECT is_bitmap FROM value_maps WHERE provider_name = ? AND map_name = 'Keywords' AND entry_value = '0x8020000000000000'",
		"Test-Provider",
	).Scan(&isBitmap)
	if err != nil {
		t.Fatalf("failed to query bitmap entry: %v", err)
	}
	if isBitmap != 1 {
		t.Errorf("is_bitmap = %d, want 1", isBitmap)
	}
}

func TestEscapeSQL(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"simple", "simple"},
		{"it's", "it''s"},
		{"'single'", "''single''"},
		{"no'quote's", "no''quote''s"},
		{"", ""},
	}
	for _, tt := range tests {
		got := escapeSQL(tt.input)
		if got != tt.want {
			t.Errorf("escapeSQL(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestJoinLangCodes(t *testing.T) {
	tests := []struct {
		input []string
		want  string
	}{
		{[]string{"en-US"}, "en-US"},
		{[]string{"en-US", "zh-CN"}, "en-US,zh-CN"},
		{[]string{}, ""},
		{[]string{"a", "b", "c"}, "a,b,c"},
	}
	for _, tt := range tests {
		got := joinLangCodes(tt.input)
		if got != tt.want {
			t.Errorf("joinLangCodes(%v) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestSQLInjectionPrevention(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test_injection.db")

	// Provider name with SQL injection attempt
	group := makeTestVariantGroup(
		"C:\\test.dll",
		makeTestProvider("Injected'); DROP TABLE providers;--", "", "App"),
		[]VariantRow{{ProviderName: "Injected'); DROP TABLE providers;--", EventID: 100, Message: "Test"}},
	)

	if err := CreateSystemDB(dbPath, []ProviderVariantGroup{group}, testOsInfo(), []string{"en-US"}); err != nil {
		t.Fatalf("CreateSystemDB with injection attempt failed: %v", err)
	}

	db := openTestDB(t, dbPath)
	defer db.Close()

	// Verify providers table still exists and has 1 row
	var tableCount int
	db.QueryRow("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='providers'").Scan(&tableCount)
	if tableCount != 1 {
		t.Error("providers table was dropped — SQL injection succeeded!")
	}

	var provName string
	err := db.QueryRow("SELECT provider_name FROM providers").Scan(&provName)
	if err != nil {
		t.Fatalf("failed to query provider: %v", err)
	}
	if !strings.Contains(provName, "Injected") {
		t.Errorf("provider_name = %s, should contain 'Injected'", provName)
	}
}
