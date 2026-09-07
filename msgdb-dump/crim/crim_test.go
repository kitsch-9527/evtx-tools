package crim

import (
	"testing"
)

func TestParseHeader(t *testing.T) {
	// Synthetic CRIM blob with 1 provider
	data := []byte{
		'C', 'R', 'I', 'M', // signature
		0x00, 0x01, 0x00, 0x00, // size = 256
		0x01, 0x00, // major version = 1
		0x00, 0x00, // minor version = 0
		0x01, 0x00, 0x00, 0x00, // provider_count = 1
	}
	// Fill rest to 256 bytes with zeros
	for len(data) < 256 {
		data = append(data, 0)
	}

	hdr, err := ParseCRIMHeader(data)
	if err != nil {
		t.Fatalf("ParseCRIMHeader() error: %v", err)
	}
	if hdr.Size != 256 {
		t.Errorf("Size = %d, want 256", hdr.Size)
	}
	if hdr.MajorVersion != 1 {
		t.Errorf("MajorVersion = %d, want 1", hdr.MajorVersion)
	}
	if hdr.MinorVersion != 0 {
		t.Errorf("MinorVersion = %d, want 0", hdr.MinorVersion)
	}
	if hdr.ProviderCount != 1 {
		t.Errorf("ProviderCount = %d, want 1", hdr.ProviderCount)
	}
}

func TestParseHeader_invalid(t *testing.T) {
	// Too short
	_, err := ParseCRIMHeader([]byte("CRIM"))
	if err == nil {
		t.Fatal("expected error for short data")
	}
	// Wrong signature
	_, err = ParseCRIMHeader([]byte("XGIM" + "\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"))
	if err == nil {
		t.Fatal("expected error for bad signature")
	}
}

func TestParseTASKBlock(t *testing.T) {
	// Build synthetic TASK block with 2 tasks
	// TASK record: 28 bytes each: id(u32) + msg_id(u32) + mui_guid(16) + name_offset(u32)
	var data []byte
	data = append(data, []byte("TASK")...)
	strOff := 12 + 2*28 // strings start after the 2 records
	totalSz := uint32(strOff + 8 + 16 + 4 + 10 + 4) // approx
	data = append(data, le32(totalSz)...)
	data = append(data, le32(2)...) // count=2

	// Record 0: id=12544, name at strOff
	data = append(data, le32(12544)...)     // id
	data = append(data, le32(0)...)          // msg_id (none)
	data = append(data, make([]byte, 16)...) // mui_guid placeholder
	data = append(data, le32(uint32(strOff))...) // name_offset

	// Record 1: id=12545, name at strOff+22
	data = append(data, le32(12545)...)     // id
	data = append(data, le32(0)...)          // msg_id
	data = append(data, make([]byte, 16)...) // mui_guid
	data = append(data, le32(uint32(strOff+16))...) // name_offset

	// Name strings: 4-byte alloc_size prefix + UTF-16LE data
	// "Logon\0" = 5 chars = 10 bytes UTF-16LE; alloc_size=4+10=14
	// But CRIM format: 4-byte total allocation INCLUDING prefix
	logon := encodeCRIMString("Logon")
	data = append(data, logon...)
	logoff := encodeCRIMString("Logoff")
	data = append(data, logoff...)

	byID, byOff := ParseTASKBlock(data, 0)
	if len(byID) != 2 {
		t.Fatalf("got %d tasks, want 2", len(byID))
	}
	if byID[12544] != "Logon" {
		t.Errorf("tasks[12544] = %q, want %q", byID[12544], "Logon")
	}
	if byID[12545] != "Logoff" {
		t.Errorf("tasks[12545] = %q, want %q", byID[12545], "Logoff")
	}
	if len(byOff) != 2 {
		t.Fatalf("byOff has %d entries, want 2", len(byOff))
	}
}

// encodeCRIMString builds a CRIM-sized UTF-16LE string.
// Format: 4-byte u32 total_alloc (includes the 4-byte prefix itself) + UTF-16LE data.
func encodeCRIMString(s string) []byte {
	utf16 := make([]uint16, 0, len(s)+1)
	for _, r := range s {
		utf16 = append(utf16, uint16(r))
	}
	utf16 = append(utf16, 0) // null terminator
	total := 4 + len(utf16)*2
	var buf []byte
	buf = append(buf, le32(uint32(total))...)
	for _, w := range utf16 {
		buf = append(buf, byte(w), byte(w>>8))
	}
	return buf
}

func TestParseEVNTBlock(t *testing.T) {
	// Build a synthetic EVNT block with 2 events:
	// Event 4624: level=0, opcode=0, task=12544, keywords=0x8020000000000000
	// Event 4625: level=0, opcode=0, task=12544, keywords=0x8010000000000000
	var data []byte
	data = append(data, []byte("EVNT")...)
	data = append(data, le32(48+2*48)...) // block size = 16 + 2*48
	data = append(data, le32(2)...)       // event count = 2
	data = append(data, le32(0)...)       // unknown

	// Event 0: event_id=4624
	data = append(data, le16(4624)...)    // event_id
	data = append(data, 0, 1, 0, 0)        // version, channel(1=Security), level, opcode
	data = append(data, le16(12544)...)    // task
	data = append(data, le64(0x8020000000000000)...)  // keywords
	data = append(data, le32(0x12345678)...)           // message_identifier
	data = append(data, le32(100)...)                  // template_offset
	data = append(data, le32(200)...)                  // opcode_offset
	data = append(data, le32(0)...)                    // level_offset
	data = append(data, le32(300)...)                  // task_offset
	data = append(data, le32(0)...)
		data = append(data, le32(0)...)
		data = append(data, le32(0)...)  // unknown_count/offset/flags

	// Event 1: event_id=4625
	data = append(data, le16(4625)...)    // event_id
	data = append(data, 0, 1, 0, 0)        // version, channel, level, opcode
	data = append(data, le16(12544)...)    // task
	data = append(data, le64(0x8010000000000000)...)  // keywords
	data = append(data, le32(0x87654321)...)           // message_identifier
	data = append(data, le32(0)...)                    // template_offset (none)
	data = append(data, le32(0)...)                    // opcode_offset
	data = append(data, le32(0)...)                    // level_offset
	data = append(data, le32(0)...)                    // task_offset
	data = append(data, le32(0)...)
		data = append(data, le32(0)...)
		data = append(data, le32(0)...)  // trailing zeros

	events := ParseEVNTBlock(data, 0)
	if len(events) != 2 {
		t.Fatalf("got %d events, want 2", len(events))
	}

	// Event 4624
	if events[0].EventID != 4624 {
		t.Errorf("events[0].EventID = %d, want 4624", events[0].EventID)
	}
	if events[0].Keywords != 0x8020000000000000 {
		t.Errorf("events[0].Keywords = 0x%X, want 0x8020000000000000", events[0].Keywords)
	}
	if events[0].MessageID != 0x12345678 {
		t.Errorf("events[0].MessageID = 0x%X, want 0x12345678", events[0].MessageID)
	}
	if events[0].TemplateOffset == nil || *events[0].TemplateOffset != 100 {
		t.Errorf("events[0].TemplateOffset = %v, want 100", events[0].TemplateOffset)
	}
	if events[0].TaskOffset == nil || *events[0].TaskOffset != 300 {
		t.Errorf("events[0].TaskOffset = %v, want 300", events[0].TaskOffset)
	}

	// Event 4625
	if events[1].EventID != 4625 {
		t.Errorf("events[1].EventID = %d, want 4625", events[1].EventID)
	}
	if events[1].Keywords != 0x8010000000000000 {
		t.Errorf("events[1].Keywords = 0x%X, want 0x8010000000000000", events[1].Keywords)
	}
	if events[1].MessageID != 0x87654321 {
		t.Errorf("events[1].MessageID = 0x%X, want 0x87654321", events[1].MessageID)
	}
	if events[1].TemplateOffset != nil {
		t.Errorf("events[1].TemplateOffset should be nil, got %d", *events[1].TemplateOffset)
	}
}

// le16 returns 2-byte little-endian encoding of v.
func le16(v uint16) []byte { return []byte{byte(v), byte(v >> 8)} }

// le32 returns 4-byte little-endian encoding of v.
func le32(v uint32) []byte { return []byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24)} }

// le64 returns 8-byte little-endian encoding of v.
func le64(v uint64) []byte {
	return []byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24), byte(v >> 32), byte(v >> 40), byte(v >> 48), byte(v >> 56)}
}

// ========== ParseTTBLBlock tests ==========

// TestParseTTBLBlock_intype_outtype — R10: the TEMP item descriptor +4:4
// field carries (OutType<<8)|InType with TDH enum values; both bytes must be
// captured on TTBLParam for typed rendering downstream.
func TestParseTTBLBlock_intype_outtype(t *testing.T) {
	enc0 := encodeCRIMString("ErrorCode")

	var data []byte
	data = append(data, []byte("TTBL")...)
	data = append(data, le32(0)...) // size placeholder
	data = append(data, le32(1)...) // temp_count = 1

	tempStart := len(data) // = 12
	data = append(data, []byte("TEMP")...)
	data = append(data, le32(0)...) // temp_size placeholder
	data = append(data, le32(2)...) // item_desc_count = 2
	data = append(data, le32(2)...) // item_name_count
	data = append(data, le32(0)...) // items_offset placeholder
	data = append(data, make([]byte, 20)...)

	itemsOff := len(data)
	copy(data[tempStart+16:tempStart+20], le32(uint32(itemsOff)))

	nameOff := itemsOff + 40
	enc1 := encodeCRIMString("Enabled")
	// Descriptor 0: InType=0x14 (HEXINT32), OutType=0x12 (HEXINT32 out)
	desc0 := make([]byte, 20)
	desc0[4] = 0x14
	desc0[5] = 0x12
	copy(desc0[16:20], le32(uint32(nameOff)))
	data = append(data, desc0...)
	// Descriptor 1: InType=0x0D (BOOLEAN), OutType=0x0D — R10 field evidence
	desc1 := make([]byte, 20)
	desc1[4] = 0x0D
	desc1[5] = 0x0D
	copy(desc1[16:20], le32(uint32(nameOff+len(enc0))))
	data = append(data, desc1...)

	data = append(data, enc0...)
	data = append(data, enc1...)

	tempSize := uint32(len(data) - tempStart)
	copy(data[tempStart+4:tempStart+8], le32(tempSize))
	copy(data[4:8], le32(uint32(len(data))))

	result := ParseTTBLBlock(data, 0)
	params, ok := result[uint32(tempStart)]
	if !ok {
		t.Fatal("missing TEMP entry")
	}
	if len(params) != 2 {
		t.Fatalf("got %d params, want 2", len(params))
	}
	if params[0].InType != 0x14 || params[0].OutType != 0x12 {
		t.Errorf("params[0] in/out = %#x/%#x, want 0x14/0x12", params[0].InType, params[0].OutType)
	}
	if params[1].InType != 0x0D || params[1].OutType != 0x0D {
		t.Errorf("params[1] in/out = %#x/%#x, want 0x0D/0x0D", params[1].InType, params[1].OutType)
	}
}

func TestParseTTBLBlock_basic(t *testing.T) {
	data := buildRawTTBLBlock(0, "SubjectUserSid", "SubjectUserName")

	result := ParseTTBLBlock(data, 0)
	if len(result) != 1 {
		t.Fatalf("got %d templates, want 1", len(result))
	}

	// TEMP entry is at offset 12
	params, ok := result[12]
	if !ok {
		t.Fatal("missing TEMP entry at offset 12")
	}
	if len(params) != 2 {
		t.Fatalf("got %d params, want 2", len(params))
	}
	if params[0].Index != 1 || params[0].Name != "SubjectUserSid" {
		t.Errorf("params[0] = %+v, want {Index:1 Name:SubjectUserSid}", params[0])
	}
	if params[1].Index != 2 || params[1].Name != "SubjectUserName" {
		t.Errorf("params[1] = %+v, want {Index:2 Name:SubjectUserName}", params[1])
	}
}

func TestParseTTBLBlock_empty(t *testing.T) {
	var data []byte
	data = append(data, []byte("TTBL")...)
	data = append(data, le32(12)...) // size = header only
	data = append(data, le32(0)...)  // temp_count = 0

	result := ParseTTBLBlock(data, 0)
	if result != nil {
		t.Fatal("expected nil for empty TTBL")
	}
}

func TestParseTTBLBlock_corrupted(t *testing.T) {
	// Bad signature
	result := ParseTTBLBlock([]byte("XTBL\x00\x00\x00\x00\x01\x00\x00\x00"), 0)
	if result != nil {
		t.Fatal("expected nil for bad sig")
	}
	// Truncated (too short for header)
	result = ParseTTBLBlock([]byte("TTBL\x00\x00"), 0)
	if result != nil {
		t.Fatal("expected nil for truncated data")
	}
	// temp_count > 4096 (safety bound)
	var data []byte
	data = append(data, []byte("TTBL")...)
	data = append(data, le32(0)...)           // size
	data = append(data, le32(5000)...)         // temp_count > 4096
	result = ParseTTBLBlock(data, 0)
	if result != nil {
		t.Fatal("expected nil for temp_count > 4096")
	}
}

func TestParseTTBLBlock_multipleTEMP(t *testing.T) {
	enc0 := encodeCRIMString("ParamA")
	enc1 := encodeCRIMString("ParamB")
	enc2 := encodeCRIMString("ParamC")

	var data []byte
	data = append(data, []byte("TTBL")...)
	data = append(data, le32(0)...) // size placeholder
	data = append(data, le32(2)...) // temp_count = 2

	// ---- TEMP entry 0 at offset 12: 1 parameter ----
	temp0Start := len(data) // = 12
	data = append(data, []byte("TEMP")...)
	data = append(data, le32(0)...) // temp_size placeholder
	data = append(data, le32(1)...) // item_desc_count = 1
	data = append(data, le32(1)...) // item_name_count
	data = append(data, le32(0)...) // items_offset placeholder
	data = append(data, make([]byte, 20)...) // padding

	temp0Items := len(data)
	copy(data[temp0Start+16:temp0Start+20], le32(uint32(temp0Items))) // items_offset

	// Descriptor 0: name_offset points past descriptor
	nameOff0 := temp0Items + 20
	data = append(data, make([]byte, 16)...)
	data = append(data, le32(uint32(nameOff0))...)
	data = append(data, enc0...)

	// Update TEMP 0 size
	temp0Size := uint32(len(data) - temp0Start)
	copy(data[temp0Start+4:temp0Start+8], le32(temp0Size))

	// ---- TEMP entry 1: 2 parameters ----
	temp1Start := len(data)
	data = append(data, []byte("TEMP")...)
	data = append(data, le32(0)...) // temp_size placeholder
	data = append(data, le32(2)...) // item_desc_count = 2
	data = append(data, le32(2)...) // item_name_count
	data = append(data, le32(0)...) // items_offset placeholder
	data = append(data, make([]byte, 20)...) // padding

	temp1Items := len(data)
	copy(data[temp1Start+16:temp1Start+20], le32(uint32(temp1Items)))

	nameOff1 := temp1Items + 40 // after 2 descriptors
	nameOff2 := nameOff1 + len(enc1)

	// Descriptor 0 for TEMP 1
	data = append(data, make([]byte, 16)...)
	data = append(data, le32(uint32(nameOff1))...)
	// Descriptor 1 for TEMP 1
	data = append(data, make([]byte, 16)...)
	data = append(data, le32(uint32(nameOff2))...)

	data = append(data, enc1...)
	data = append(data, enc2...)

	// Update TEMP 1 size
	temp1Size := uint32(len(data) - temp1Start)
	copy(data[temp1Start+4:temp1Start+8], le32(temp1Size))

	// Update TTBL size
	copy(data[4:8], le32(uint32(len(data))))

	result := ParseTTBLBlock(data, 0)
	if len(result) != 2 {
		t.Fatalf("got %d templates, want 2", len(result))
	}

	// Verify TEMP 0
	params0, ok := result[uint32(temp0Start)]
	if !ok {
		t.Fatal("missing TEMP entry at temp0Start")
	}
	if len(params0) != 1 || params0[0].Index != 1 || params0[0].Name != "ParamA" {
		t.Fatalf("TEMP 0 params = %+v, want [{1 ParamA}]", params0)
	}

	// Verify TEMP 1
	params1, ok := result[uint32(temp1Start)]
	if !ok {
		t.Fatal("missing TEMP entry at temp1Start")
	}
	if len(params1) != 2 {
		t.Fatalf("TEMP 1: got %d params, want 2", len(params1))
	}
	if params1[0].Index != 1 || params1[0].Name != "ParamB" {
		t.Errorf("params1[0] = %+v, want {1 ParamB}", params1[0])
	}
	if params1[1].Index != 2 || params1[1].Name != "ParamC" {
		t.Errorf("params1[1] = %+v, want {2 ParamC}", params1[1])
	}
}

func TestParseTTBLBlock_nameFilter(t *testing.T) {
	// "Invalid@Name" should be filtered out; "ValidName" kept
	data := buildRawTTBLBlock(0, "Invalid@Name", "ValidName")

	result := ParseTTBLBlock(data, 0)
	if len(result) != 1 {
		t.Fatalf("got %d templates, want 1", len(result))
	}

	params, ok := result[12]
	if !ok {
		t.Fatal("missing TEMP entry at offset 12")
	}
	if len(params) != 1 {
		t.Fatalf("got %d params, want 1 (after filter)", len(params))
	}
	if params[0].Index != 2 || params[0].Name != "ValidName" {
		t.Errorf("params[0] = %+v, want {Index:2 Name:ValidName}", params[0])
	}
}

// ========== ProviderData + ParseProviderData tests ==========

func TestParseProviderData_full(t *testing.T) {
	data := buildCRIMTestBlob()
	guid := "{F1EF270A-0D32-4352-BA52-DBAB41E1D859}"

	pd := ParseProviderData(data, guid)
	if pd == nil {
		t.Fatal("ParseProviderData returned nil")
	}

	// Verify Events (1 event: 4624)
	if len(pd.Events) != 1 {
		t.Fatalf("got %d events, want 1", len(pd.Events))
	}
	if pd.Events[0].EventID != 4624 {
		t.Errorf("EventID = %d, want 4624", pd.Events[0].EventID)
	}
	if pd.Events[0].Keywords != 0x8020000000000000 {
		t.Errorf("Keywords = 0x%X, want 0x8020000000000000", pd.Events[0].Keywords)
	}

	// Verify Tasks (1 task: 12544 → "Logon")
	if len(pd.Tasks) != 1 {
		t.Fatalf("got %d tasks, want 1", len(pd.Tasks))
	}
	if pd.Tasks[12544] != "Logon" {
		t.Errorf("Tasks[12544] = %q, want %q", pd.Tasks[12544], "Logon")
	}
	if len(pd.TasksByOffset) != 1 {
		t.Fatal("TasksByOffset should have 1 entry")
	}

	// Verify TemplateParams (1 TEMP entry)
	if len(pd.TemplateParams) != 1 {
		t.Fatalf("got %d template entries, want 1", len(pd.TemplateParams))
	}
	for _, params := range pd.TemplateParams {
		if len(params) != 2 {
			t.Fatalf("got %d params per TEMP, want 2", len(params))
		}
		if params[0].Index != 1 || params[0].Name != "SubjectUserSid" {
			t.Errorf("params[0] = %+v", params[0])
		}
		if params[1].Index != 2 || params[1].Name != "SubjectUserName" {
			t.Errorf("params[1] = %+v", params[1])
		}
	}
}

func TestParseProviderData_invalidGUID(t *testing.T) {
	data := buildCRIMTestBlob()
	pd := ParseProviderData(data, "{00000000-0000-0000-0000-000000000000}")
	if pd != nil {
		t.Fatal("expected nil for non-matching GUID")
	}
}

func TestParseProviderData_shortData(t *testing.T) {
	pd := ParseProviderData([]byte("CRIM"), "{F1EF270A-0D32-4352-BA52-DBAB41E1D859}")
	if pd != nil {
		t.Fatal("expected nil for short data")
	}
}

func TestParseProviderData_wrongSig(t *testing.T) {
	data := make([]byte, 24)
	copy(data, "XGIM")
	pd := ParseProviderData(data, "{F1EF270A-0D32-4352-BA52-DBAB41E1D859}")
	if pd != nil {
		t.Fatal("expected nil for wrong signature")
	}
}

// ========== Test helper: buildRawTTBLBlock ==========

// buildRawTTBLBlock constructs a TTBL block with 1 TEMP entry
// containing the given parameter names.
// baseOff is the absolute offset where this block sits in the full CRIM blob.
func buildRawTTBLBlock(baseOff int, names ...string) []byte {
	var data []byte
	data = append(data, []byte("TTBL")...)
	data = append(data, le32(0)...) // size placeholder
	data = append(data, le32(1)...) // temp_count = 1

	// TEMP entry at offset 12
	data = append(data, []byte("TEMP")...)
	data = append(data, le32(0)...) // temp_size placeholder
	data = append(data, le32(uint32(len(names)))...) // item_desc_count
	data = append(data, le32(uint32(len(names)))...) // item_name_count
	data = append(data, le32(0)...) // items_offset placeholder
	data = append(data, make([]byte, 20)...) // padding

	itemsOffset := len(data)
	copy(data[28:32], le32(uint32(baseOff+itemsOffset)))

	// Pre-encode names (need sizes for offset computation)
	encs := make([][]byte, len(names))
	for i, n := range names {
		encs[i] = encodeCRIMString(n)
	}

	// Compute name offsets (descriptors then strings)
	namesStart := itemsOffset + len(names)*20
	nameOffs := make([]int, len(names))
	cur := namesStart
	for i, enc := range encs {
		nameOffs[i] = cur
		cur += len(enc)
	}

	// Write item descriptors (20 bytes each, name_offset at bytes 16-19)
	for i := range names {
		data = append(data, make([]byte, 16)...)
		data = append(data, le32(uint32(baseOff+nameOffs[i]))...)
	}

	// Write CRIM strings
	for _, enc := range encs {
		data = append(data, enc...)
	}

	// Update sizes
	tempSize := uint32(len(data) - 12) // TEMP starts at offset 12
	copy(data[16:20], le32(tempSize))
	copy(data[4:8], le32(uint32(len(data)))) // TTBL size

	return data
}

// ========== Test helper: buildCRIMTestBlob ==========

// buildCRIMTestBlob constructs a synthetic CRIM blob containing
// TASK, EVNT, and TTBL blocks for testing ParseProviderData.
func buildCRIMTestBlob() []byte {
	// Get block sizes by building with baseOff=0 first
	taskSize := uint32(len(buildRawTASKBlock("Logon", 12544, 0)))
	evntSize := uint32(len(buildRawEVNTBlock()))

	// Compute layout offsets
	const headerArea = 16 + 20 + 20 + 24 // CRIM + provider desc + WEVT + 3 elem descs
	taskOff := headerArea
	evntOff := taskOff + int(taskSize)
	ttblOff := evntOff + int(evntSize)
	wevtOff := 16 + 20 // WEVT right after provider descriptor

	// Build final blocks with correct absolute offsets
	taskData := buildRawTASKBlock("Logon", 12544, taskOff)
	evntData := buildRawEVNTBlock()
	ttblData := buildRawTTBLBlock(ttblOff, "SubjectUserSid", "SubjectUserName")

	var data []byte

	// ---- CRIM header ----
	data = append(data, []byte("CRIM")...)
	totalSizeOff := len(data)
	data = append(data, le32(0)...) // size placeholder
	data = append(data, le16(1)...) // major version
	data = append(data, le16(0)...) // minor version
	data = append(data, le32(1)...) // provider_count = 1

	// ---- Provider descriptor (20 bytes) ----
	// GUID: {F1EF270A-0D32-4352-BA52-DBAB41E1D859}
	guidBytes := []byte{
		0x0A, 0x27, 0xEF, 0xF1, // d1 LE
		0x32, 0x0D,             // d2 LE
		0x52, 0x43,             // d3 LE
		0xBA, 0x52, 0xDB, 0xAB, 0x41, 0xE1, 0xD8, 0x59,
	}
	data = append(data, guidBytes...)
	data = append(data, le32(uint32(wevtOff))...) // WEVT offset

	// ---- WEVT block header (20 bytes) ----
	data = append(data, []byte("WEVT")...)
	wevtSizeOff := len(data)
	data = append(data, le32(0)...) // wevt_size placeholder
	data = append(data, le32(0)...) // unknown
	data = append(data, le32(3)...) // desc_count = 3 (TASK, EVNT, TTBL)
	data = append(data, le32(0)...) // unknown

	// ---- Element descriptors (3 * 8 = 24 bytes) ----
	data = append(data, le32(uint32(taskOff))...)
	data = append(data, le32(uint32(len(taskData)))...)
	data = append(data, le32(uint32(evntOff))...)
	data = append(data, le32(uint32(len(evntData)))...)
	data = append(data, le32(uint32(ttblOff))...)
	data = append(data, le32(uint32(len(ttblData)))...)

	// Update WEVT size (header + element descriptors only)
	copy(data[wevtSizeOff:wevtSizeOff+4], le32(uint32(len(data)-wevtSizeOff+4)))

	// ---- Block data ----
	data = append(data, taskData...)
	data = append(data, evntData...)
	data = append(data, ttblData...)

	// Update CRIM total size
	copy(data[totalSizeOff:totalSizeOff+4], le32(uint32(len(data))))

	return data
}

// buildRawTASKBlock constructs a single TASK block with one task entry.
// baseOff is the absolute offset where this block sits in the full CRIM blob.
func buildRawTASKBlock(name string, id uint32, baseOff int) []byte {
	var data []byte
	data = append(data, []byte("TASK")...)
	data = append(data, le32(0)...) // size placeholder
	data = append(data, le32(1)...) // count = 1

	// Record: id(4) + msg_id(4) + mui_guid(16) + name_offset(4) = 28 bytes
	data = append(data, le32(id)...)
	data = append(data, le32(0)...) // msg_id = 0
	data = append(data, make([]byte, 16)...) // mui_guid zeros

	// name_offset placeholder
	nameOffField := len(data)
	data = append(data, le32(0)...) // placeholder

	// CRIM-encoded name string
	nameEnc := encodeCRIMString(name)
	nameOff := len(data) // relative offset within task data
	data = append(data, nameEnc...)

	// name_offset must be ABSOLUTE in the full CRIM blob
	copy(data[nameOffField:nameOffField+4], le32(uint32(baseOff+nameOff)))
	// Update TASK size
	copy(data[4:8], le32(uint32(len(data))))

	return data
}

// buildRawEVNTBlock constructs a single EVNT block with one event (ID 4624).
func buildRawEVNTBlock() []byte {
	var data []byte
	data = append(data, []byte("EVNT")...)
	data = append(data, le32(0)...) // size placeholder
	data = append(data, le32(1)...) // count = 1
	data = append(data, le32(0)...) // unknown

	// Event record (48 bytes): event_id=4624, level=0, channel=1, task=12544
	data = append(data, le16(4624)...)
	data = append(data, 0, 1, 0, 0)                // version, channel(1=Security), level, opcode
	data = append(data, le16(12544)...)              // task
	data = append(data, le64(0x8020000000000000)...) // keywords
	data = append(data, le32(0x12345678)...)         // message_identifier
	data = append(data, le32(0)...)                  // template_offset
	data = append(data, le32(0)...)                  // opcode_offset
	data = append(data, le32(0)...)                  // level_offset
	data = append(data, le32(0)...)                  // task_offset
	data = append(data, le32(0)...)                  // unknown_count
	data = append(data, le32(0)...)                  // unknown_offset
	data = append(data, le32(0)...)                  // flags

	// Update EVNT size
	copy(data[4:8], le32(uint32(len(data))))

	return data
}



