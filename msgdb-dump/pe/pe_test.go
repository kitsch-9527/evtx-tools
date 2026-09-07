package pe

import (
	"bytes"
	"encoding/binary"
	"os"
	"path/filepath"
	"testing"

	wcorefxpe "github.com/kitsch-9527/wcorefx/pe"
)

// ---------------------------------------------------------------------------
// Helpers for building synthetic PE data
// ---------------------------------------------------------------------------

func le16(v uint16) []byte {
	b := make([]byte, 2)
	binary.LittleEndian.PutUint16(b, v)
	return b
}

func le32(v uint32) []byte {
	b := make([]byte, 4)
	binary.LittleEndian.PutUint32(b, v)
	return b
}

func alignUp(v, align uint32) uint32 {
	return (v + align - 1) & ^(align - 1)
}

// utf16le encodes a string as UTF-16LE bytes (no BOM).
func utf16le(s string) []byte {
	var b []byte
	for _, r := range s {
		u16 := uint16(r)
		b = append(b, byte(u16), byte(u16>>8))
	}
	return b
}

// getSections parses PE sections from raw bytes using wcorefx.
func getSections(peData []byte) []wcorefxpe.ImageSectionHeader {
	pe, err := wcorefxpe.OpenReader(bytes.NewReader(peData))
	if err != nil {
		panic(err)
	}
	return pe.Sections
}

// buildMinimalPE constructs a synthetic PE32 file with a single .rsrc section.
func buildMinimalPE(rsrcData []byte, rsrcRVA uint32) []byte {
	const fileAlign = 0x200
	const sectionAlign = 0x1000
	const eLfanew = 0x80
	const secHdrSize = 40

	hdrSize := uint32(eLfanew) + 4 + 20 + 0xE0 + secHdrSize
	rsrcRawOffset := alignUp(hdrSize, fileAlign)

	totalSize := rsrcRawOffset + uint32(len(rsrcData))
	pe := make([]byte, totalSize)

	pe[0] = 'M'
	pe[1] = 'Z'
	binary.LittleEndian.PutUint32(pe[0x3C:0x40], eLfanew)

	copy(pe[eLfanew:eLfanew+4], "PE\x00\x00")

	fhOff := eLfanew + 4
	binary.LittleEndian.PutUint16(pe[fhOff:], 0x14C)
	binary.LittleEndian.PutUint16(pe[fhOff+2:], 1)
	binary.LittleEndian.PutUint16(pe[fhOff+16:], 0xE0)

	ohOff := eLfanew + 24
	binary.LittleEndian.PutUint16(pe[ohOff:], 0x10B)
	binary.LittleEndian.PutUint32(pe[ohOff+0x1C:], 0x00400000)
	binary.LittleEndian.PutUint32(pe[ohOff+0x20:], sectionAlign)
	binary.LittleEndian.PutUint32(pe[ohOff+0x24:], fileAlign)
	binary.LittleEndian.PutUint16(pe[ohOff+0x30:], 5)
	binary.LittleEndian.PutUint32(pe[ohOff+0x3C:], rsrcRawOffset)
	binary.LittleEndian.PutUint16(pe[ohOff+0x44:], 2)
	binary.LittleEndian.PutUint32(pe[ohOff+0x5C:], 16)

	binary.LittleEndian.PutUint32(pe[ohOff+0x70:], rsrcRVA)
	binary.LittleEndian.PutUint32(pe[ohOff+0x74:], uint32(len(rsrcData)))

	paddedRsrcSize := alignUp(uint32(len(rsrcData)), sectionAlign)
	binary.LittleEndian.PutUint32(pe[ohOff+0x38:], rsrcRVA+paddedRsrcSize)

	shOff := eLfanew + 24 + 0xE0
	copy(pe[shOff:shOff+8], ".rsrc\x00\x00\x00")
	binary.LittleEndian.PutUint32(pe[shOff+8:], uint32(len(rsrcData)))
	binary.LittleEndian.PutUint32(pe[shOff+12:], rsrcRVA)
	binary.LittleEndian.PutUint32(pe[shOff+16:], uint32(len(rsrcData)))
	binary.LittleEndian.PutUint32(pe[shOff+20:], rsrcRawOffset)
	binary.LittleEndian.PutUint32(pe[shOff+36:], 0x40000040)

	copy(pe[rsrcRawOffset:], rsrcData)

	return pe
}

// buildResourceDir builds a simple 3-level resource directory.
func buildResourceDir(typeID uint32, lang uint16, messageData []byte) []byte {
	var d []byte

	// Level 1: Type directory (offset 0x00)
	d = append(d, le32(0)...)
	d = append(d, le32(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(1)...)
	entry1 := typeID & 0x7FFFFFFF
	d = append(d, le32(entry1)...)
	d = append(d, le32(0x80000018)...)

	// Level 2: Name directory (offset 0x18)
	d = append(d, le32(0)...)
	d = append(d, le32(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(1)...)
	d = append(d, le32(1)...)
	d = append(d, le32(0x80000030)...)

	// Level 3: Language directory (offset 0x30)
	d = append(d, le32(0)...)
	d = append(d, le32(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(1)...)
	d = append(d, le32(uint32(lang))...)
	d = append(d, le32(0x48)...)

	// IMAGE_RESOURCE_DATA_ENTRY (offset 0x48)
	d = append(d, le32(0)...)
	d = append(d, le32(uint32(len(messageData)))...)
	d = append(d, le32(0)...)
	d = append(d, le32(0)...)

	// Resource data (offset 0x58)
	d = append(d, messageData...)

	return d
}

// buildVersionResource builds VS_VERSIONINFO + VS_FIXEDFILEINFO.
func buildVersionResource(major, minor, patch, build uint16) []byte {
	fixed := make([]byte, 52)
	binary.LittleEndian.PutUint32(fixed[0:4], 0xFEEF04BD)
	binary.LittleEndian.PutUint32(fixed[8:12], uint32(major)<<16|uint32(minor))
	binary.LittleEndian.PutUint32(fixed[12:16], uint32(patch)<<16|uint32(build))

	key := utf16le("VS_VERSION_INFO")
	key = append(key, 0, 0)

	hdrLen := 6 + len(key)
	paddedHdrLen := (hdrLen + 3) & ^3
	for len(key) < paddedHdrLen-6 {
		key = append(key, 0)
	}

	totalLen := paddedHdrLen + 52
	var d []byte
	d = append(d, le16(uint16(totalLen))...)
	d = append(d, le16(52)...)
	d = append(d, le16(0)...)
	d = append(d, key...)
	d = append(d, fixed...)
	return d
}

// buildMsgTableOneBlock builds a single-block MESSAGE_RESOURCE_DATA.
func buildMsgTableOneBlock(unicode bool, entries map[uint32]string) []byte {
	if len(entries) == 0 {
		return le32(0)
	}
	var low, high uint32
	first := true
	for eid := range entries {
		if first {
			low, high = eid, eid
			first = false
		} else {
			if eid < low {
				low = eid
			}
			if eid > high {
				high = eid
			}
		}
	}

	var blockBody []byte
	for eid := low; eid <= high; eid++ {
		msg, ok := entries[eid]
		if !ok {
			blockBody = append(blockBody, le16(4)...)
			blockBody = append(blockBody, le16(0)...)
			continue
		}
		var textBytes []byte
		if unicode {
			textBytes = utf16le(msg)
			textBytes = append(textBytes, 0, 0)
		} else {
			textBytes = append([]byte(msg), 0)
		}
		totalLen := 4 + len(textBytes)
		paddedLen := (totalLen + 3) & ^3
		entry := make([]byte, paddedLen)
		binary.LittleEndian.PutUint16(entry[0:2], uint16(totalLen))
		entry[3] = 0 // flags byte
		if unicode {
			entry[3] = 1 // set flags in high byte: little-endian uint16 = low byte=0 high byte=1
			binary.LittleEndian.PutUint16(entry[2:4], 1)
		}
		copy(entry[4:], textBytes)
		blockBody = append(blockBody, entry...)
	}

	offsetToEntries := uint32(4 + 12)
	var data []byte
	data = append(data, le32(1)...)
	data = append(data, le32(low)...)
	data = append(data, le32(high)...)
	data = append(data, le32(offsetToEntries)...)
	data = append(data, blockBody...)
	return data
}

// patchDataRVA patches DataRVA at offset 0x48.
func patchDataRVA(rsrcData []byte, rva uint32) {
	binary.LittleEndian.PutUint32(rsrcData[0x48:0x4C], rva)
}

// buildNamedTypeResourceDir builds a resource directory for a named type.
func buildNamedTypeResourceDir(typeName string, lang uint16, messageData []byte) []byte {
	nameStrOffset := uint32(0x58)

	var d []byte
	// Level 1: Type directory (offset 0x00)
	d = append(d, le32(0)...)
	d = append(d, le32(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(1)...)
	d = append(d, le16(0)...)
	d = append(d, le32(0x80000000|nameStrOffset)...)
	d = append(d, le32(0x80000018)...)

	// Level 2: Name directory (offset 0x18)
	d = append(d, le32(0)...)
	d = append(d, le32(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(1)...)
	d = append(d, le32(1)...)
	d = append(d, le32(0x80000030)...)

	// Level 3: Language directory (offset 0x30)
	d = append(d, le32(0)...)
	d = append(d, le32(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(0)...)
	d = append(d, le16(1)...)
	d = append(d, le32(uint32(lang))...)
	d = append(d, le32(0x48)...)

	// IMAGE_RESOURCE_DATA_ENTRY (offset 0x48)
	d = append(d, le32(0)...)
	d = append(d, le32(uint32(len(messageData)))...)
	d = append(d, le32(0)...)
	d = append(d, le32(0)...)

	// Name string (offset 0x58)
	nameChars := utf16le(typeName)
	d = append(d, le16(uint16(len(nameChars)/2))...)
	d = append(d, nameChars...)
	d = append(d, 0, 0)

	d = append(d, messageData...)
	return d
}

// ---------------------------------------------------------------------------
// Tests: openPEWithSections
// ---------------------------------------------------------------------------

func TestOpenPE_nonexistent(t *testing.T) {
	_, _, _, _, err := openPEWithSections(filepath.Join(t.TempDir(), "nonexistent.dll"))
	if err == nil {
		t.Fatal("expected error for nonexistent path")
	}
}

func TestOpenPE_invalidFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "invalid.dll")
	if err := os.WriteFile(path, []byte("not a PE file"), 0644); err != nil {
		t.Fatal(err)
	}
	_, _, _, _, err := openPEWithSections(path)
	if err == nil {
		t.Fatal("expected error for invalid PE file")
	}
}

func TestOpenPE_valid(t *testing.T) {
	pe := buildMinimalPE([]byte{'X'}, 0x1000)
	path := filepath.Join(t.TempDir(), "test.dll")
	if err := os.WriteFile(path, pe, 0644); err != nil {
		t.Fatal(err)
	}

	data, sections, rsrcOff, rsrcSize, err := openPEWithSections(path)
	if err != nil {
		t.Fatalf("openPEWithSections() error: %v", err)
	}
	if len(data) == 0 {
		t.Fatal("openPEWithSections() returned empty data")
	}
	if rsrcSize == 0 {
		t.Fatal("openPEWithSections() returned rsrcSize = 0")
	}
	if int(rsrcOff) >= len(data) || data[rsrcOff] != 'X' {
		t.Fatalf("rsrc data at offset %d doesn't contain expected data", rsrcOff)
	}
	if len(sections) == 0 {
		t.Fatal("openPEWithSections() returned no sections")
	}
}

func TestOpenPE_noRsrcSection(t *testing.T) {
	const eLfanew = 0x80
	const fileAlign = 0x200
	const secHdrSize = 40

	hdrSize := uint32(eLfanew) + 4 + 20 + 0xE0 + secHdrSize
	rawOffset := alignUp(hdrSize, fileAlign)

	pe := make([]byte, rawOffset+4)
	pe[0] = 'M'
	pe[1] = 'Z'
	binary.LittleEndian.PutUint32(pe[0x3C:0x40], eLfanew)
	copy(pe[eLfanew:eLfanew+4], "PE\x00\x00")
	fhOff := eLfanew + 4
	binary.LittleEndian.PutUint16(pe[fhOff:], 0x14C)
	binary.LittleEndian.PutUint16(pe[fhOff+2:], 1)
	binary.LittleEndian.PutUint16(pe[fhOff+16:], 0xE0)
	ohOff := eLfanew + 24
	binary.LittleEndian.PutUint16(pe[ohOff:], 0x10B)
	binary.LittleEndian.PutUint32(pe[ohOff+0x1C:], 0x00400000)
	binary.LittleEndian.PutUint32(pe[ohOff+0x20:], 0x1000)
	binary.LittleEndian.PutUint32(pe[ohOff+0x24:], fileAlign)
	binary.LittleEndian.PutUint32(pe[ohOff+0x3C:], rawOffset)
	binary.LittleEndian.PutUint32(pe[ohOff+0x5C:], 16)
	binary.LittleEndian.PutUint32(pe[ohOff+0x70:], 0)
	binary.LittleEndian.PutUint32(pe[ohOff+0x74:], 0)

	shOff := eLfanew + 24 + 0xE0
	copy(pe[shOff:shOff+8], ".text\x00\x00\x00")
	binary.LittleEndian.PutUint32(pe[shOff+8:], 0x100)
	binary.LittleEndian.PutUint32(pe[shOff+12:], 0x1000)
	binary.LittleEndian.PutUint32(pe[shOff+16:], 0x100)
	binary.LittleEndian.PutUint32(pe[shOff+20:], rawOffset)

	path := filepath.Join(t.TempDir(), "norsrc.dll")
	if err := os.WriteFile(path, pe, 0644); err != nil {
		t.Fatal(err)
	}

	_, _, _, _, err := openPEWithSections(path)
	if err == nil {
		t.Fatal("expected error for PE with no .rsrc section")
	}
}

// ---------------------------------------------------------------------------
// Tests: resolveRVA (uses wcorefx sections)
// ---------------------------------------------------------------------------

func TestRVAtoOffset(t *testing.T) {
	pe := buildMinimalPE(make([]byte, 0x2000), 0x1000)
	sections := getSections(pe)

	off, err := resolveRVA(sections, 0x1000)
	if err != nil {
		t.Fatalf("resolveRVA(0x1000) error: %v", err)
	}
	if off != 0x200 {
		t.Fatalf("resolveRVA(0x1000) = %d, want 512 (0x200)", off)
	}

	off, err = resolveRVA(sections, 0x1005)
	if err != nil {
		t.Fatalf("resolveRVA(0x1005) error: %v", err)
	}
	if off != 0x205 {
		t.Fatalf("resolveRVA(0x1005) = %d, want 517 (0x205)", off)
	}

	_, err = resolveRVA(sections, 0x9999)
	if err == nil {
		t.Fatal("expected error for RVA not in any section")
	}
}

// ---------------------------------------------------------------------------
// Tests: GetResourceByTypeID
// ---------------------------------------------------------------------------

func TestGetResourceByTypeID_singleEntry(t *testing.T) {
	msgData := buildMsgTableOneBlock(true, map[uint32]string{100: "Hi"})
	rsrc := buildResourceDir(11, 1033, msgData)

	const rsrcRVA = uint32(0x1000)
	pe := buildMinimalPE(rsrc, rsrcRVA)
	patchDataRVA(rsrc, rsrcRVA+0x58)
	pe = buildMinimalPE(rsrc, rsrcRVA)

	rsrcOff := uint32(0x200)
	rsrcSize := uint32(len(rsrc))
	sections := getSections(pe)

	result, err := GetResourceByTypeID(pe, rsrcOff, rsrcSize, sections, 11)
	if err != nil {
		t.Fatalf("GetResourceByTypeID() error: %v", err)
	}
	if len(result) == 0 {
		t.Fatal("GetResourceByTypeID() returned empty map")
	}
	entries, ok := result[1]
	if !ok {
		t.Fatal("expected resource name ID 1")
	}
	if len(entries) != 1 {
		t.Fatalf("expected 1 language entry, got %d", len(entries))
	}
	if entries[0].Lang != 1033 {
		t.Fatalf("expected lang 1033, got %d", entries[0].Lang)
	}
	if len(entries[0].Data) == 0 {
		t.Fatal("resource data is empty")
	}
}

func TestGetResourceByTypeID_typeNotFound(t *testing.T) {
	rsrc := buildResourceDir(11, 1033, []byte{0})
	pe := buildMinimalPE(rsrc, 0x1000)

	sections := getSections(pe)
	_, err := GetResourceByTypeID(pe, 0x200, uint32(len(rsrc)), sections, 99)
	if err == nil {
		t.Fatal("expected error for type not found")
	}
}

func TestGetResourceByTypeID_invalidPE(t *testing.T) {
	_, err := GetResourceByTypeID([]byte{0, 1, 2, 3}, 0, 100, nil, 11)
	if err == nil {
		t.Fatal("expected error for invalid PE data")
	}
}

// ---------------------------------------------------------------------------
// Tests: GetResourceByTypeName
// ---------------------------------------------------------------------------

func TestGetResourceByTypeName_wevtTemplate(t *testing.T) {
	crimData := []byte("CRIM" + string(make([]byte, 20)))
	rsrc := buildNamedTypeResourceDir("WEVT_TEMPLATE", 1033, crimData)

	pe := buildMinimalPE(rsrc, 0x1000)

	msgOffset := uint32(0x58 + 2 + 26 + 2)
	patchDataRVA(rsrc, 0x1000+msgOffset)
	pe = buildMinimalPE(rsrc, 0x1000)

	rsrcOff := uint32(0x200)
	rsrcSize := uint32(len(rsrc))
	sections := getSections(pe)

	result, err := GetResourceByTypeName(pe, rsrcOff, rsrcSize, sections, "WEVT_TEMPLATE")
	if err != nil {
		t.Fatalf("GetResourceByTypeName() error: %v", err)
	}
	if len(result) == 0 {
		t.Fatal("expected at least one entry")
	}
	entries, ok := result[1]
	if !ok {
		t.Fatal("expected resource name ID 1")
	}
	if len(entries) == 0 || len(entries[0].Data) == 0 {
		t.Fatal("expected non-empty data")
	}
}

func TestGetResourceByTypeName_typeNotFound(t *testing.T) {
	rsrc := buildResourceDir(11, 1033, []byte{0})
	pe := buildMinimalPE(rsrc, 0x1000)

	sections := getSections(pe)
	_, err := GetResourceByTypeName(pe, 0x200, uint32(len(rsrc)), sections, "NONEXISTENT")
	if err == nil {
		t.Fatal("expected error for type not found")
	}
}

func TestGetResourceByTypeName_wevtCrimContent(t *testing.T) {
	crimData := []byte("CRIM\x30\x00\x00\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00\x00")
	rsrc := buildNamedTypeResourceDir("WEVT_TEMPLATE", 1033, crimData)
	pe := buildMinimalPE(rsrc, 0x1000)

	msgOffset := uint32(0x58 + 2 + 26 + 2)
	patchDataRVA(rsrc, 0x1000+msgOffset)
	pe = buildMinimalPE(rsrc, 0x1000)

	rsrcOff := uint32(0x200)
	rsrcSize := uint32(len(rsrc))
	sections := getSections(pe)

	result, err := GetResourceByTypeName(pe, rsrcOff, rsrcSize, sections, "WEVT_TEMPLATE")
	if err != nil {
		t.Fatalf("GetResourceByTypeName() error: %v", err)
	}
	entries := result[1]
	if len(entries) == 0 {
		t.Fatal("expected at least one entry")
	}
	if len(entries[0].Data) < 4 || string(entries[0].Data[:4]) != "CRIM" {
		t.Fatalf("expected CRIM data, got %q", string(entries[0].Data[:4]))
	}
}

// ---------------------------------------------------------------------------
// Tests: ParseMessageTable
// ---------------------------------------------------------------------------

func TestParseMessageTable_singleEntryUTF16(t *testing.T) {
	data := buildMsgTableOneBlock(true, map[uint32]string{100: "Hi"})
	result := ParseMessageTable(data)
	if result == nil {
		t.Fatal("ParseMessageTable() returned nil")
	}
	if len(result) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(result))
	}
	if result[100] != "Hi" {
		t.Fatalf("result[100] = %q, want %q", result[100], "Hi")
	}
}

func TestParseMessageTable_ansiEntry(t *testing.T) {
	data := buildMsgTableOneBlock(false, map[uint32]string{200: "Hello"})
	result := ParseMessageTable(data)
	if result == nil {
		t.Fatal("ParseMessageTable() returned nil")
	}
	if result[200] != "Hello" {
		t.Fatalf("result[200] = %q, want %q", result[200], "Hello")
	}
}

func TestParseMessageTable_emptyData(t *testing.T) {
	result := ParseMessageTable(nil)
	if result != nil {
		t.Fatal("expected nil for nil data")
	}
	result = ParseMessageTable([]byte{})
	if result != nil {
		t.Fatal("expected nil for empty data")
	}
}

func TestParseMessageTable_zeroBlocks(t *testing.T) {
	data := le32(0)
	result := ParseMessageTable(data)
	if result != nil {
		t.Fatal("expected nil for NumberOfBlocks = 0")
	}
}

func TestParseMessageTable_lowHighMismatch(t *testing.T) {
	var data []byte
	data = append(data, le32(1)...)
	data = append(data, le32(200)...)
	data = append(data, le32(100)...)
	data = append(data, le32(16)...)

	result := ParseMessageTable(data)
	if result != nil {
		t.Fatal("expected nil when LowId > HighId")
	}
}

func TestParseMessageTable_numberOfBlocksBound(t *testing.T) {
	data := le32(1025)
	result := ParseMessageTable(data)
	if result != nil {
		t.Fatal("expected nil for NumberOfBlocks > 1024")
	}
}

func TestParseMessageTable_emptyMessage(t *testing.T) {
	var data []byte
	data = append(data, le32(1)...)
	data = append(data, le32(100)...)
	data = append(data, le32(100)...)
	data = append(data, le32(16)...)

	entry := make([]byte, 4)
	binary.LittleEndian.PutUint16(entry[0:2], 4)
	binary.LittleEndian.PutUint16(entry[2:4], 1)
	data = append(data, entry...)

	result := ParseMessageTable(data)
	if result != nil {
		t.Fatal("expected nil for empty message")
	}
}

func TestParseMessageTable_offsetOutOfBounds(t *testing.T) {
	var data []byte
	data = append(data, le32(1)...)
	data = append(data, le32(100)...)
	data = append(data, le32(100)...)
	data = append(data, le32(999)...)

	result := ParseMessageTable(data)
	if result != nil {
		t.Fatal("expected nil for offset out of bounds")
	}
}

// ---------------------------------------------------------------------------
// Tests: PEVersion
// ---------------------------------------------------------------------------

func TestPEVersion_synthetic(t *testing.T) {
	verData := buildVersionResource(10, 0, 22621, 1)
	rsrc := buildResourceDir(16, 1033, verData)
	pe := buildMinimalPE(rsrc, 0x1000)

	patchDataRVA(rsrc, 0x1000+0x58)
	pe = buildMinimalPE(rsrc, 0x1000)

	sections := getSections(pe)
	ver := PEVersion(pe, 0x200, uint32(len(rsrc)), sections)
	if ver != "10.0.22621.1" {
		t.Fatalf("PEVersion() = %q, want %q", ver, "10.0.22621.1")
	}
}

func TestPEVersion_noVersion(t *testing.T) {
	pe := buildMinimalPE([]byte{0}, 0x1000)
	sections := getSections(pe)
	ver := PEVersion(pe, 0x200, 1, sections)
	if ver != "0.0.0.0" {
		t.Fatalf("PEVersion() = %q, want %q", ver, "0.0.0.0")
	}
}

func TestPEVersion_invalidPE(t *testing.T) {
	ver := PEVersion([]byte{0, 1, 2}, 0, 10, nil)
	if ver != "0.0.0.0" {
		t.Fatalf("PEVersion() = %q, want %q", ver, "0.0.0.0")
	}
}

// ---------------------------------------------------------------------------
// Tests: ParseWEVTTemplate
// ---------------------------------------------------------------------------

func TestParseWEVTTemplate_simple(t *testing.T) {
	xml := []byte(`<template>
  <event value="4624">
    <data name="SubjectUserSid" inType="win:SID" outType="xs:string"/>
    <data name="SubjectUserName" inType="win:UnicodeString"/>
  </event>
  <event value="4625">
    <data name="SubjectUserSid" inType="win:SID"/>
  </event>
</template>`)

	result := ParseWEVTTemplate(xml)
	if result == nil {
		t.Fatal("ParseWEVTTemplate() returned nil")
	}

	e4624, ok := result["4624"]
	if !ok {
		t.Fatal("expected event 4624")
	}
	if e4624["1"] != "SubjectUserSid" {
		t.Fatalf(`e4624["1"] = %q, want "SubjectUserSid"`, e4624["1"])
	}
	if e4624["2"] != "SubjectUserName" {
		t.Fatalf(`e4624["2"] = %q, want "SubjectUserName"`, e4624["2"])
	}

	e4625, ok := result["4625"]
	if !ok {
		t.Fatal("expected event 4625")
	}
	if e4625["1"] != "SubjectUserSid" {
		t.Fatalf(`e4625["1"] = %q, want "SubjectUserSid"`, e4625["1"])
	}
}

func TestParseWEVTTemplate_utf16le(t *testing.T) {
	xml := `<template>
  <event value="100">
    <data name="Param1"/>
  </event>
</template>`
	bom := []byte{0xFF, 0xFE}
	utf16Data := append(bom, utf16le(xml)...)

	result := ParseWEVTTemplate(utf16Data)
	if result == nil {
		t.Fatal("ParseWEVTTemplate(UTF16) returned nil")
	}
	e100, ok := result["100"]
	if !ok {
		t.Fatal("expected event 100")
	}
	if e100["1"] != "Param1" {
		t.Fatalf(`e100["1"] = %q, want "Param1"`, e100["1"])
	}
}

func TestParseWEVTTemplate_empty(t *testing.T) {
	result := ParseWEVTTemplate(nil)
	if result != nil {
		t.Fatal("expected nil for nil input")
	}
	result = ParseWEVTTemplate([]byte{})
	if result != nil {
		t.Fatal("expected nil for empty input")
	}
}

func TestParseWEVTTemplate_malformed(t *testing.T) {
	result := ParseWEVTTemplate([]byte("<event value=4624>"))
	_ = result
}

// ---------------------------------------------------------------------------
// Tests: ExtractAllFromPE
// ---------------------------------------------------------------------------

func TestExtractAllFromPE(t *testing.T) {
	msgData := buildMsgTableOneBlock(true, map[uint32]string{100: "TestMsg"})
	rsrc := buildResourceDir(11, 1033, msgData)
	pe := buildMinimalPE(rsrc, 0x1000)
	patchDataRVA(rsrc, 0x1000+0x58)
	pe = buildMinimalPE(rsrc, 0x1000)

	path := filepath.Join(t.TempDir(), "test.dll")
	if err := os.WriteFile(path, pe, 0644); err != nil {
		t.Fatal(err)
	}

	result, err := ExtractAllFromPE(path)
	if err != nil {
		t.Fatalf("ExtractAllFromPE() error: %v", err)
	}
	if result == nil {
		t.Fatal("ExtractAllFromPE() returned nil")
	}
	if len(result.Messages) == 0 {
		t.Fatal("expected non-empty messages")
	}
	msg, ok := result.Messages[100]
	if !ok {
		t.Fatal("expected message for event 100")
	}
	if msg != "TestMsg" {
		t.Fatalf("Messages[100] = %q, want %q", msg, "TestMsg")
	}
	if len(result.CrimBlobs) != 0 {
		t.Fatal("expected empty crim blobs")
	}
}

func TestExtractAllFromPE_nonexistent(t *testing.T) {
	_, err := ExtractAllFromPE(filepath.Join(t.TempDir(), "nonexistent.dll"))
	if err == nil {
		t.Fatal("expected error")
	}
}
