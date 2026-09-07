package dump

import (
	"database/sql"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/kitsch-9527/msgdb-dump/crim"
	"github.com/kitsch-9527/msgdb-dump/pe"
	"github.com/kitsch-9527/msgdb-dump/schema"
)

// ---------------------------------------------------------------------------
// GlobMatch tests
// ---------------------------------------------------------------------------

func TestGlobMatch(t *testing.T) {
	tests := []struct {
		pattern string
		name    string
		want    bool
	}{
		{"", "anything", true},
		{"Security", "Security", true},
		{"Security", "Security-Auditing", false},
		{"Security*", "Security-Auditing", true},
		{"Microsoft-*", "Microsoft-Windows-Security-Auditing", true},
		{"*Security*", "Microsoft-Windows-Security-Auditing", true},
		{"????", "Secu", true},
		{"????", "Security", false},
		{"*a*b*", "abc", true},
		{"*a*b*", "xyz", false},
		{"*-Auditing", "Security-Auditing", true},
		{"*-Auditing", "Security-Other", false},
		{"*", "anything", true},
		{"a", "b", false},
		{"*a", "xyza", true},
		{"a*", "abc", true},
		{"a*b*c", "axbyc", true},
		{"a*b*c", "axbycz", false},
	}
	for _, tt := range tests {
		got := GlobMatch(tt.pattern, tt.name)
		if got != tt.want {
			t.Errorf("GlobMatch(%q, %q) = %v, want %v", tt.pattern, tt.name, got, tt.want)
		}
	}
}

// ---------------------------------------------------------------------------
// dedupPeMessages tests
// ---------------------------------------------------------------------------

func TestDedupPeMessages(t *testing.T) {
	msgs := []PeMessage{
		{EventID: 100, Message: "first"},
		{EventID: 100, Message: "second"},
		{EventID: 200, Message: "third"},
		{EventID: 100, Message: "fourth"},
	}
	deduped := dedupPeMessages(msgs)
	if len(deduped) != 2 {
		t.Fatalf("expected 2 messages, got %d", len(deduped))
	}
	if deduped[0].EventID != 100 || deduped[0].Message != "first" {
		t.Fatalf("expected event 100 with 'first', got %d: %q", deduped[0].EventID, deduped[0].Message)
	}
	if deduped[1].EventID != 200 {
		t.Fatalf("expected event 200, got %d", deduped[1].EventID)
	}
}

func TestDedupPeMessages_empty(t *testing.T) {
	if result := dedupPeMessages(nil); len(result) != 0 {
		t.Fatal("expected empty result for nil input")
	}
	if result := dedupPeMessages([]PeMessage{}); len(result) != 0 {
		t.Fatal("expected empty result for empty input")
	}
}

// ---------------------------------------------------------------------------
// resolveKeywords tests
// ---------------------------------------------------------------------------

func TestResolveKeywords(t *testing.T) {
	kwMap := map[uint64]string{
		0x8020000000000000: "AuditSuccess",
		0x8010000000000000: "AuditFailure",
		0x8000000000000000: "Classic",
	}

	tests := []struct {
		eventKW uint64
		want    string
	}{
		{0x8020000000000000, "AuditSuccess, Classic"},
		{0x8010000000000000, "AuditFailure, Classic"},
		{0x8020000000000000 | 0x8000000000000000, "AuditSuccess, Classic"},
		{0, ""},
		{0x0000000000000001, ""},
	}
	for _, tt := range tests {
		got := resolveKeywords(tt.eventKW, kwMap)
		if got != tt.want {
			t.Errorf("resolveKeywords(0x%X) = %q, want one of: %s", tt.eventKW, got, tt.want)
		}
	}
}

func TestResolveKeywords_emptyMap(t *testing.T) {
	if got := resolveKeywords(0x8020000000000000, nil); got != "" {
		t.Errorf("expected empty string for nil map, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// buildPeMessages tests
// ---------------------------------------------------------------------------

func TestBuildPeMessages_empty(t *testing.T) {
	result := buildPeMessages(nil, nil, nil)
	if result != nil {
		t.Fatal("expected nil for nil input")
	}
	result = buildPeMessages(map[uint32]string{}, nil, nil)
	if result != nil {
		t.Fatal("expected nil for empty map")
	}
}

func TestBuildPeMessages_simple(t *testing.T) {
	msgs := map[uint32]string{
		100: "Message one",
		200: "Message two",
	}
	result := buildPeMessages(msgs, nil, nil)
	if len(result) != 2 {
		t.Fatalf("expected 2 messages, got %d", len(result))
	}
	// Should be sorted by event ID
	if result[0].EventID != 100 || result[0].Message != "Message one" {
		t.Fatalf("result[0] = %d: %q", result[0].EventID, result[0].Message)
	}
	if result[1].EventID != 200 || result[1].Message != "Message two" {
		t.Fatalf("result[1] = %d: %q", result[1].EventID, result[1].Message)
	}
}

func TestBuildPeMessages_withParams(t *testing.T) {
	msgs := map[uint32]string{
		4624: "{{Some message}}",
	}
	wevtParams := map[string]map[string]string{
		"4624": {
			"1": "SubjectUserSid",
			"2": "SubjectUserName",
		},
	}
	result := buildPeMessages(msgs, wevtParams, nil)
	if len(result) != 1 {
		t.Fatalf("expected 1 message, got %d", len(result))
	}
	if result[0].Parameters == nil {
		t.Fatal("expected Parameters to be set")
	}
	if (*result[0].Parameters)["1"] != "SubjectUserSid" {
		t.Fatalf("params[1] = %q, want %q", (*result[0].Parameters)["1"], "SubjectUserSid")
	}
}

// ---------------------------------------------------------------------------
// parseCRIMUnfilteredEVNT tests
// ---------------------------------------------------------------------------

func TestParseCRIMUnfilteredEVNT_noCRIM(t *testing.T) {
	result := parseCRIMUnfilteredEVNT([]byte("not crim"))
	if len(result) != 0 {
		t.Fatal("expected empty for non-CRIM data")
	}
	result = parseCRIMUnfilteredEVNT(nil)
	if len(result) != 0 {
		t.Fatal("expected empty for nil data")
	}
}

func TestParseCRIMUnfilteredEVNT_shortData(t *testing.T) {
	result := parseCRIMUnfilteredEVNT([]byte("CRIM"))
	if len(result) != 0 {
		t.Fatal("expected empty for short data")
	}
}

// ---------------------------------------------------------------------------
// ExtractSingleDLL tests (synthetic data)
// ---------------------------------------------------------------------------

func TestExtractSingleDLL_nonexistent(t *testing.T) {
	result := ExtractSingleDLL(
		filepath.Join(t.TempDir(), "nonexistent.dll"),
		[]ProviderInfo{{Name: "Test"}},
		[]string{"1033"},
		[]string{"en-US"},
	)
	if result == nil {
		t.Fatal("ExtractSingleDLL returned nil")
	}
	// Should have basic fields even on failure
	if result.DLLName != "nonexistent.dll" {
		t.Fatalf("DLLName = %q, want %q", result.DLLName, "nonexistent.dll")
	}
}

// ---------------------------------------------------------------------------
// Real DLL extraction tests (Windows only)
// ---------------------------------------------------------------------------

func TestExtractSingleDLL_adtschema(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	dllPath := `C:\Windows\System32\adtschema.dll`
	if _, err := os.Stat(dllPath); os.IsNotExist(err) {
		t.Skipf("%s not found", dllPath)
	}

	result := ExtractSingleDLL(
		dllPath,
		[]ProviderInfo{{
			Name:    "Microsoft-Windows-Security-Auditing",
			GUID:    strPtr("{54849625-5478-4994-A5BA-3E3B0328C30D}"),
			DLLPath: dllPath,
		}},
		[]string{"1033"},
		[]string{"en-US"},
	)
	if result == nil {
		t.Fatal("ExtractSingleDLL returned nil")
	}
	if result.Version == "0.0.0.0" {
		t.Log("Note: PE version is 0.0.0.0 (may not have version resource)")
	}
	msgs := result.MessagesByLang["en-US"]
	t.Logf("adtschema.dll: %d messages extracted", len(msgs))
	if len(msgs) == 0 {
		t.Log("Note: no messages extracted from adtschema.dll (may be CRIM-only)")
	}
}

func TestExtractSingleDLL_systemEvents(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	dllPath := `C:\Windows\System32\Microsoft-Windows-System-Events.dll`
	if _, err := os.Stat(dllPath); os.IsNotExist(err) {
		t.Skipf("%s not found", dllPath)
	}

	// System-Events.dll has only CRIM blobs (no RT_MESSAGETABLE)
	// so it tests the CRIM+MUI fallback path.
	result := ExtractSingleDLL(
		dllPath,
		[]ProviderInfo{{
			Name:    "Microsoft-Windows-System-Events",
			GUID:    strPtr("{F1EF270A-0D32-4352-BA52-DBAB41E1D859}"),
			DLLPath: dllPath,
		}},
		[]string{"1033"},
		[]string{"en-US"},
	)
	if result == nil {
		t.Fatal("ExtractSingleDLL returned nil")
	}
	if len(result.CrimBlobs) == 0 {
		t.Fatal("expected CRIM blobs from system-events.dll")
	}
	t.Logf("CRIM blobs: %d", len(result.CrimBlobs))
	msgs := result.MessagesByLang["en-US"]
	t.Logf("Messages via CRIM+MUI: %d", len(msgs))
	if len(msgs) > 0 {
		t.Logf("First message: event=%d text=%q", msgs[0].EventID, truncate(msgs[0].Message, 60))
	}
}

// ---------------------------------------------------------------------------
// applyCRIMMessageMapping tests
// ---------------------------------------------------------------------------

func TestApplyCRIMMessageMapping_basic(t *testing.T) {
	// Simulate cbsmsg.dll scenario:
	// RT_MESSAGETABLE → messages at resource IDs 600, 601, 608
	// CRIM EventEntry → EventID=1 MessageID=600; EventID=2 MessageID=601; EventID=4 MessageID=608
	msgs := []PeMessage{
		{EventID: 600, Message: "Operation started"},
		{EventID: 601, Message: "Operation completed"},
		{EventID: 608, Message: "Operation failed"},
	}
	msgIDMap := map[uint32]crim.EventEntry{
		600: {EventID: 1, MessageID: 600, Level: 4},
		601: {EventID: 2, MessageID: 601, Level: 0},
		608: {EventID: 4, MessageID: 608, Level: 2},
	}

	result := applyCRIMMessageMapping(msgs, msgIDMap)
	if len(result) != 3 {
		t.Fatalf("expected 3 messages, got %d", len(result))
	}

	// Check remapped event IDs
	expectedIDs := []uint32{1, 2, 4}
	for i, eid := range expectedIDs {
		if result[i].EventID != eid {
			t.Errorf("result[%d].EventID = %d, want %d", i, result[i].EventID, eid)
		}
	}

	// Check original messages preserved
	if result[0].Message != "Operation started" {
		t.Errorf("result[0].Message = %q, want %q", result[0].Message, "Operation started")
	}
}

func TestApplyCRIMMessageMapping_noMatch(t *testing.T) {
	// Messages with no matching CRIM entry should keep their resource ID unchanged
	msgs := []PeMessage{
		{EventID: 100, Message: "Generic event"},
	}
	msgIDMap := map[uint32]crim.EventEntry{
		600: {EventID: 1, MessageID: 600},
	}

	result := applyCRIMMessageMapping(msgs, msgIDMap)
	if len(result) != 1 {
		t.Fatalf("expected 1 message, got %d", len(result))
	}
	if result[0].EventID != 100 {
		t.Errorf("result[0].EventID = %d, want 100 (unchanged)", result[0].EventID)
	}
}

func TestApplyCRIMMessageMapping_nilInputs(t *testing.T) {
	result := applyCRIMMessageMapping(nil, nil)
	if len(result) != 0 {
		t.Errorf("expected empty result for nil input, got %d", len(result))
	}

	result = applyCRIMMessageMapping([]PeMessage{}, nil)
	if len(result) != 0 {
		t.Errorf("expected empty result for empty input, got %d", len(result))
	}
}

// ---------------------------------------------------------------------------
// buildMessageIDMap tests
// ---------------------------------------------------------------------------

func TestBuildMessageIDMap_nilBlobs(t *testing.T) {
	result := buildMessageIDMap(nil, nil)
	if len(result) != 0 {
		t.Errorf("expected empty map for nil input, got %d entries", len(result))
	}

	result = buildMessageIDMap([][]byte{}, []ProviderInfo{})
	if len(result) != 0 {
		t.Errorf("expected empty map for empty input, got %d entries", len(result))
	}
}

func TestBuildMessageIDMap_cbsmsg_dumpDebug(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	dllPath := `C:\Windows\servicing\cbsmsg.dll`
	if _, err := os.Stat(dllPath); os.IsNotExist(err) {
		t.Skipf("%s not found", dllPath)
	}

	combined, err := pe.ExtractAllFromPE(dllPath)
	if err != nil {
		t.Fatalf("PE extraction failed: %v", err)
	}
	t.Logf("Messages: %d, CRIM blobs: %d", len(combined.Messages), len(combined.CrimBlobs))

	guid := "{bd12f3b8-fc40-4a61-a307-b7a013a069c1}"
	for i, blob := range combined.CrimBlobs {
		pd := crim.ParseProviderData(blob, guid)
		if pd == nil {
			t.Logf("CRIM blob %d: no provider data", i)
			continue
		}
		t.Logf("CRIM blob %d: %d events", i, len(pd.Events))
		for j, ev := range pd.Events {
			if j >= 5 {
				break
			}
			t.Logf("  EventID=%d MessageID=%d Level=%d", ev.EventID, ev.MessageID, ev.Level)
		}
	}

	// Show a few RT_MESSAGETABLE keys
	count := 0
	t.Log("RT_MESSAGETABLE resource IDs (first 5):")
	for eid := range combined.Messages {
		t.Logf("  ResourceID=%d", eid)
		count++
		if count >= 5 {
			break
		}
	}
}

func TestBuildMessageIDMap_cbsmsg(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	dllPath := `C:\Windows\servicing\cbsmsg.dll`
	if _, err := os.Stat(dllPath); os.IsNotExist(err) {
		t.Skipf("%s not found", dllPath)
	}

	// Extract CRIM blobs from cbsmsg.dll
	combined, err := pe.ExtractAllFromPE(dllPath)
	if err != nil || len(combined.CrimBlobs) == 0 {
		t.Fatalf("no CRIM blobs in %s: %v", dllPath, err)
	}

	providers := []ProviderInfo{{
		Name:    "Microsoft-Windows-Servicing",
		GUID:    strPtr("{bd12f3b8-fc40-4a61-a307-b7a013a069c1}"),
		DLLPath: dllPath,
	}}

	msgIDMap := buildMessageIDMap(combined.CrimBlobs, providers)
	if len(msgIDMap) == 0 {
		t.Fatal("expected non-empty message ID map from cbsmsg.dll CRIM blobs")
	}

	// The CRIM EventEntry maps EventID→MessageID in the 0xB0000000 range.
	// Verify a few known mappings from the event table.
	found := 0
	for msgID, ev := range msgIDMap {
		// Check that the first few event IDs (1-5) all have valid message entries
		if ev.EventID >= 1 && ev.EventID <= 5 {
			found++
			if msgID == 0 {
				t.Errorf("EventID %d has MessageID=0 (invalid)", ev.EventID)
			}
		}
	}
	if found == 0 {
		t.Error("expected at least one EventID in range 1-5 with valid MessageID, found none")
	}
	if found < 5 {
		t.Logf("warn: only found %d/5 EventIDs (1-5) in msgIDMap (some may be filtered)", found)
	}

	// Verify the actual mapping: EventID=1 → MessageID=2952790017 (0xB0000001)
	ev, ok := msgIDMap[2952790017]
	if !ok {
		// Fallback: check if EventID=1 exists at all
		var foundEvent1 bool
		for _, entry := range msgIDMap {
			if entry.EventID == 1 {
				foundEvent1 = true
				t.Logf("EventID=1 maps to MessageID=%d", entry.MessageID)
				break
			}
		}
		if !foundEvent1 {
			t.Error("expected EventID=1 in msgIDMap, not found")
		}
	} else {
		if ev.EventID != 1 {
			t.Errorf("msgIDMap[2952790017].EventID = %d, want 1", ev.EventID)
		}
	}

	t.Logf("cbsmsg.dll: %d message_id → EventEntry mappings", len(msgIDMap))
}

func TestExtractMUICRIMFromBlobs_bestVersion(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	dllPath := `C:\Windows\System32\qmgr.dll`
	if _, err := os.Stat(dllPath); os.IsNotExist(err) {
		t.Skipf("%s not found", dllPath)
	}

	combined, err := pe.ExtractAllFromPE(dllPath)
	if err != nil || len(combined.CrimBlobs) == 0 {
		t.Fatalf("PE extraction failed: %v", err)
	}

	guid := "{EF1CC15B-46C1-414E-BB95-E76B077BD51E}"
	prov := ProviderInfo{
		Name: "Microsoft-Windows-Bits-Client",
		GUID: &guid,
	}

	res := ExtractSingleDLL(dllPath, []ProviderInfo{prov}, []string{"1033"}, []string{"en-US"})
	if res == nil {
		t.Fatal("ExtractSingleDLL returned nil")
	}

	msgs := res.MessagesByLang["en-US"]
	if len(msgs) == 0 {
		t.Fatal("expected non-empty messages for Bits-Client")
	}

	// Verify EID 3 picks the latest version (ver=3/2 with Transfer job / Job ID / Owner template)
	var eid3 *PeMessage
	for i := range msgs {
		if msgs[i].EventID == 3 {
			eid3 = &msgs[i]
			break
		}
	}
	if eid3 == nil {
		t.Fatal("EID 3 message not found")
	}
	if !strings.Contains(eid3.Message, "Transfer job:") {
		t.Errorf("EID 3 message should be latest version containing 'Transfer job:', got %q", eid3.Message)
	}
	if eid3.Parameters == nil {
		t.Fatal("EID 3 should have template parameters")
	}
	// Verify parameters have jobTitle
	if (*eid3.Parameters)["1"] != "jobTitle" {
		t.Errorf("EID 3 param 1 should be 'jobTitle', got %q", (*eid3.Parameters)["1"])
	}
}

// ---------------------------------------------------------------------------
// importDLL tests
// ---------------------------------------------------------------------------

func TestImportDLL_basic(t *testing.T) {
	// Create a temp directory with a minimal DLL-like PE file
	// Since we can't easily create a real PE file, test the error path
	// and the validation logic.
	tmpDir := t.TempDir()

	// Non-existent DLL (pass absolute path, aliases can be nil)
	nonexistent := filepath.Join(tmpDir, "nonexistent.dll")
	result := importDLL(nonexistent)
	if result != nil {
		t.Fatal("expected nil for non-existent file")
	}

	// Empty directory
	results := importDLLDir(tmpDir)
	if len(results) != 0 {
		t.Errorf("expected 0 results for empty dir, got %d", len(results))
	}
}

func TestImportDLL_withSystemDLL(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	dllPath := `C:\Windows\System32\IoLogMsg.dll`
	if _, err := os.Stat(dllPath); os.IsNotExist(err) {
		t.Skipf("%s not found", dllPath)
	}

	// Test importing IoLogMsg.dll - a real LN DLL (0 messages, but has MUI companion)
	result := importDLL(dllPath)
	if result == nil {
		t.Fatal("importDLL returned nil for existing file")
	}
	t.Logf("DLL: %s, msgs: %d (en-US), providers: %d",
		result.DLLName,
		len(result.MessagesByLang["en-US"]),
		len(result.ProviderName))

	// Check that provider name was derived from filename
	expectedName := "IoLogMsg"
	if result.ProviderName != expectedName {
		t.Errorf("provider name = %q, want %q", result.ProviderName, expectedName)
	}
}

func TestImportDLL_esentMUI(t *testing.T) {
	if runtime.GOOS != "windows" {
		t.Skip("Windows-only test")
	}

	// Test importing the MUI DLL which should have actual messages
	dllPath := `C:\Windows\System32\en-US\esent.dll.mui`
	if _, err := os.Stat(dllPath); os.IsNotExist(err) {
		t.Skipf("%s not found", dllPath)
	}

	result := importDLL(dllPath)
	if result == nil {
		t.Fatal("importDLL returned nil")
	}
	t.Logf("DLL: %s, msgs: %d (en-US), providers: %d",
		result.DLLName,
		len(result.MessagesByLang["en-US"]),
		len(result.ProviderName))

	// Should have extracted messages
	if len(result.MessagesByLang["en-US"]) == 0 {
		t.Error("expected messages from esent.dll.mui, got 0")
	}

	// Should have messages typically used by ESENT provider (event IDs like 100, 300 series)
	msgIDs := make(map[uint32]bool)
	for _, m := range result.MessagesByLang["en-US"] {
		msgIDs[m.EventID] = true
	}
	for _, eid := range []uint32{100, 222, 300, 450} {
		if msgIDs[eid] {
			t.Logf("  Found expected ESENT event ID %d", eid)
		}
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// createTestDB builds a test database with the single-file v7 schema.
func createTestDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open :memory: %v", err)
	}
	if _, err := db.Exec(schema.SchemaDDL); err != nil {
		t.Fatalf("create v7 schema: %v", err)
	}
	if _, err := db.Exec(schema.SchemaIndexes); err != nil {
		t.Fatalf("create indexes: %v", err)
	}
	return db
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
