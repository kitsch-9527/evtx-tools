package crim

import (
	"encoding/binary"
	"fmt"
)

// CRIMHeader is the top-level CRIM blob header.
//
// Layout:
//
//	+0:4  signature "CRIM"
//	+4:4  total size (including header)
//	+8:2  major version
//	+10:2 minor version
//	+12:4 provider count
//	+16:   provider descriptor table (20 bytes each)
type CRIMHeader struct {
	Size          uint32
	MajorVersion  uint16
	MinorVersion  uint16
	ProviderCount uint32
}

const (
	// CRIMHeaderSize is the fixed header byte count (offset to provider table).
	CRIMHeaderSize = 16
	// ProviderDescSize is the size of each provider descriptor entry.
	ProviderDescSize = 20 // 16 GUID bytes + 4 WEVT offset
	// EventRecordSize is the size of each EVNT record.
	EventRecordSize = 48
)

// ParseCRIMHeader reads the CRIM blob header. Returns an error if the
// signature is wrong or the data is too short.
func ParseCRIMHeader(data []byte) (*CRIMHeader, error) {
	if len(data) < CRIMHeaderSize {
		return nil, fmt.Errorf("crim: data too short (%d bytes, need %d)", len(data), CRIMHeaderSize)
	}
	if string(data[:4]) != "CRIM" {
		return nil, fmt.Errorf("crim: bad signature %q", string(data[:4]))
	}
	return &CRIMHeader{
		Size:          binary.LittleEndian.Uint32(data[4:8]),
		MajorVersion:  binary.LittleEndian.Uint16(data[8:10]),
		MinorVersion:  binary.LittleEndian.Uint16(data[10:12]),
		ProviderCount: binary.LittleEndian.Uint32(data[12:16]),
	}, nil
}

// EventEntry is one event definition from an EVNT block.
// Layout (48 bytes each):
//
//	+0:2  event_id (u16)
//	+2:1  version (u8)
//	+3:1  channel (u8)
//	+4:1  level (u8)
//	+5:1  opcode (u8)
//	+6:2  task (u16)
//	+8:8  keywords (u64)
//	+16:4 message_identifier (u32)
//	+20:4 template_offset (u32, 0 = none)
//	+24:4 opcode_offset (u32, 0 = none)
//	+28:4 level_offset (u32, 0 = none)
//	+32:4 task_offset (u32, 0 = none)
//	+36:4 unknown_count (skip)
//	+40:4 unknown_offset (skip)
//	+44:4 flags (skip)
// VariantKey is the 8-tuple identity for a Message Variant (CONTEXT.md: Variant Identity).
// Two EventEntry records with the same Key are the same Variant (redundant WEVT duplication);
// otherwise they are distinct Variants that must both be retained (e.g. TxR vs AppContainer for EID=11).
type VariantKey struct {
	EventID   uint16
	Version   uint8
	Channel   uint8
	Level     uint8
	Opcode    uint8
	Task      uint16
	Keywords  uint64
	MessageID uint32
}

// Key returns the Variant Identity for this EventEntry.
func (e EventEntry) Key() VariantKey {
	return VariantKey{
		EventID:   e.EventID,
		Version:   e.Version,
		Channel:   e.Channel,
		Level:     e.Level,
		Opcode:    e.Opcode,
		Task:      e.Task,
		Keywords:  e.Keywords,
		MessageID: e.MessageID,
	}
}

// DeduplicateVariants collapses entries whose VariantKey is identical, preserving
// the first occurrence. Distinct Keys (e.g. TxR vs AppContainer) are all retained.
func DeduplicateVariants(entries []EventEntry) []EventEntry {
	seen := make(map[VariantKey]struct{}, len(entries))
	result := make([]EventEntry, 0, len(entries))
	for _, e := range entries {
		k := e.Key()
		if _, ok := seen[k]; ok {
			continue
		}
		seen[k] = struct{}{}
		result = append(result, e)
	}
	return result
}

type EventEntry struct {
	EventID     uint16
	Version     uint8
	Channel     uint8
	Level       uint8
	Opcode      uint8
	Task        uint16
	Keywords    uint64
	MessageID   uint32
	TemplateOffset *uint32
	OpcodeOffset   *uint32
	LevelOffset    *uint32
	TaskOffset     *uint32
}

// ParseEVNTBlock parses EVNT event definition records from data at offset pos.
func ParseEVNTBlock(data []byte, pos int) []EventEntry {
	if pos+16 > len(data) || string(data[pos:pos+4]) != "EVNT" {
		return nil
	}
	count := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if count <= 0 || count > 65536 {
		return nil
	}
	minSize := 16 + count*EventRecordSize
	if pos+minSize > len(data) {
		return nil
	}

	events := make([]EventEntry, 0, count)
	start := pos + 16
	for i := 0; i < count; i++ {
		eo := start + i*EventRecordSize
		if eo+EventRecordSize > len(data) {
			break
		}
		msgID := binary.LittleEndian.Uint32(data[eo+16 : eo+20])
		// Skip entries with no message
		if msgID == 0xFFFFFFFF || msgID == 0 {
			continue
		}
		ev := EventEntry{
			EventID:   binary.LittleEndian.Uint16(data[eo : eo+2]),
			Version:   data[eo+2],
			Channel:   data[eo+3],
			Level:     data[eo+4],
			Opcode:    data[eo+5],
			Task:      binary.LittleEndian.Uint16(data[eo+6 : eo+8]),
			Keywords:  binary.LittleEndian.Uint64(data[eo+8 : eo+16]),
			MessageID: msgID,
		}
		if to := binary.LittleEndian.Uint32(data[eo+20 : eo+24]); to != 0 {
			ev.TemplateOffset = &to
		}
		if oo := binary.LittleEndian.Uint32(data[eo+24 : eo+28]); oo != 0 {
			ev.OpcodeOffset = &oo
		}
		if lo := binary.LittleEndian.Uint32(data[eo+28 : eo+32]); lo != 0 {
			ev.LevelOffset = &lo
		}
		if to := binary.LittleEndian.Uint32(data[eo+32 : eo+36]); to != 0 {
			ev.TaskOffset = &to
		}
		events = append(events, ev)
	}
	return events
}


// readCRIMString reads a CRIM-sized UTF-16LE string at offset.
func readCRIMString(data []byte, offset int) string {
	if offset+4 > len(data) {
		return ""
	}
	alloc := int(binary.LittleEndian.Uint32(data[offset : offset+4]))
	if alloc < 4 || alloc > 65536 {
		return ""
	}
	strEnd := offset + alloc
	if strEnd > len(data) {
		strEnd = len(data)
	}
	start := offset + 4
	words := make([]uint16, 0, (strEnd-start)/2)
	for i := start; i+1 < strEnd; i += 2 {
		c := uint16(data[i]) | uint16(data[i+1])<<8
		if c == 0 {
			break
		}
		words = append(words, c)
	}
	if len(words) == 0 {
		return ""
	}
	runes := make([]rune, 0, len(words))
	for _, w := range words {
		runes = append(runes, rune(w))
	}
	return string(runes)
}

// ParseTASKBlock parses a TASK block, returns (id→name, offset→name).
func ParseTASKBlock(data []byte, pos int) (map[uint32]string, map[uint32]string) {
	if pos+12 > len(data) || string(data[pos:pos+4]) != "TASK" {
		return nil, nil
	}
	count := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if count <= 0 || count > 4096 {
		return nil, nil
	}
	byID := make(map[uint32]string, count)
	byOff := make(map[uint32]string, count)
	for i := 0; i < count; i++ {
		recOff := pos + 12 + i*28
		if recOff+28 > len(data) {
			break
		}
		id := binary.LittleEndian.Uint32(data[recOff : recOff+4])
		nameOff := int(binary.LittleEndian.Uint32(data[recOff+24 : recOff+28]))
		if nameOff > 0 {
			if name := readCRIMString(data, nameOff); name != "" {
				byID[id] = name
				byOff[uint32(recOff)] = name
			}
		}
	}
	return byID, byOff
}

// ParseOPCOBlock parses an OPCO block, returns (id→name, offset→name).
func ParseOPCOBlock(data []byte, pos int) (map[uint32]string, map[uint32]string) {
	if pos+12 > len(data) || string(data[pos:pos+4]) != "OPCO" {
		return nil, nil
	}
	count := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if count <= 0 || count > 4096 {
		return nil, nil
	}
	byID := make(map[uint32]string, count)
	byOff := make(map[uint32]string, count)
	for i := 0; i < count; i++ {
		recOff := pos + 12 + i*12
		if recOff+12 > len(data) {
			break
		}
		id := binary.LittleEndian.Uint32(data[recOff : recOff+4])
		nameOff := int(binary.LittleEndian.Uint32(data[recOff+8 : recOff+12]))
		if nameOff > 0 {
			if name := readCRIMString(data, nameOff); name != "" {
				byID[id] = name
				byOff[uint32(recOff)] = name
			}
		}
	}
	return byID, byOff
}

// ParseLEVLBlock parses a LEVL block, returns (id→name, offset→name).
func ParseLEVLBlock(data []byte, pos int) (map[uint32]string, map[uint32]string) {
	if pos+12 > len(data) || string(data[pos:pos+4]) != "LEVL" {
		return nil, nil
	}
	count := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if count <= 0 || count > 4096 {
		return nil, nil
	}
	byID := make(map[uint32]string, count)
	byOff := make(map[uint32]string, count)
	for i := 0; i < count; i++ {
		recOff := pos + 12 + i*12
		if recOff+12 > len(data) {
			break
		}
		id := binary.LittleEndian.Uint32(data[recOff : recOff+4])
		nameOff := int(binary.LittleEndian.Uint32(data[recOff+8 : recOff+12]))
		if nameOff > 0 {
			if name := readCRIMString(data, nameOff); name != "" {
				byID[id] = name
				byOff[uint32(recOff)] = name
			}
		}
	}
	return byID, byOff
}

// ParseKEYWBlock parses a KEYW block, returns (mask→name, offset→name).
func ParseKEYWBlock(data []byte, pos int) (map[uint64]string, map[uint32]string) {
	if pos+12 > len(data) || string(data[pos:pos+4]) != "KEYW" {
		return nil, nil
	}
	count := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if count <= 0 || count > 4096 {
		return nil, nil
	}
	byMask := make(map[uint64]string, count)
	byOff := make(map[uint32]string, count)
	for i := 0; i < count; i++ {
		recOff := pos + 12 + i*16
		if recOff+16 > len(data) {
			break
		}
		mask := binary.LittleEndian.Uint64(data[recOff : recOff+8])
		nameOff := int(binary.LittleEndian.Uint32(data[recOff+12 : recOff+16]))
		if nameOff > 0 {
			if name := readCRIMString(data, nameOff); name != "" {
				byMask[mask] = name
				byOff[uint32(recOff)] = name
			}
		}
	}
	return byMask, byOff
}

// ParseVMAPBlock parses a VMAP block, returns (map_name, value→display_name).
func ParseVMAPBlock(data []byte, pos int) (string, map[uint32]string) {
	if pos+20 > len(data) || string(data[pos:pos+4]) != "VMAP" {
		return "", nil
	}
	count := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if count <= 0 || count > 4096 {
		return "", nil
	}
	nameOff := int(binary.LittleEndian.Uint32(data[pos+12 : pos+16]))
	mapName := readCRIMString(data, nameOff)
	if mapName == "" {
		return "", nil
	}
	entries := make(map[uint32]string, count)
	for i := 0; i < count; i++ {
		entryOff := pos + 16 + i*8
		if entryOff+8 > len(data) {
			break
		}
		val := binary.LittleEndian.Uint32(data[entryOff : entryOff+4])
		valNameOff := int(binary.LittleEndian.Uint32(data[entryOff+4 : entryOff+8]))
		if valNameOff > 0 {
			if valName := readCRIMString(data, valNameOff); valName != "" {
				entries[val] = valName
			}
		}
	}
	return mapName, entries
}

// resolveVMAPString resolves an encoded string offset (0xD200xxxx format) using
// a string table. The encoded offset contains:
// - High 16 bits (0xD200): string table type identifier
// - Low 16 bits: string table index
//
// Returns the resolved string or an empty string if resolution fails.
func resolveVMAPString(encodedOffset uint32, stringTable map[uint32]string) string {
	if stringTable == nil {
		return ""
	}
	// Check if this is an encoded string offset
	highBits := (encodedOffset >> 16) & 0xFFFF
	if highBits != 0xD200 {
		return ""
	}
	// Extract the string table index
	stringIndex := encodedOffset & 0xFFFF
	// Look up in the string table
	if str, ok := stringTable[stringIndex]; ok {
		return str
	}
	return ""
}

// ParseMAPSVMAPBlock parses a VMAP sub-block inside a MAPS block.
//
// MAPS VMAP sub-block layout (relative to the CRIM blob start):
//
//	+0:4  "VMAP" signature
//	+4:4  block size
//	+8:4  map name string offset (alloc-prefixed UTF-16, e.g. "Pop:MapSleepReason")
//	+12:4 unknown (usually 0)
//	+16:4 entryCount
//	+20:  entries, each 8 bytes: (u32 value, u32 string reference)
//
// The per-entry string reference is a 0xD2-encoded index into the provider's
// RT_MESSAGETABLE string table (high 16 bits 0xD200, low 16 bits = index).
func ParseMAPSVMAPBlock(data []byte, pos int, stringTable map[uint32]string) (string, map[uint32]string) {
	if pos+20 > len(data) || string(data[pos:pos+4]) != "VMAP" {
		return "", nil
	}
	entryCount := int(binary.LittleEndian.Uint32(data[pos+16 : pos+20]))
	if entryCount <= 0 || entryCount > 4096 {
		return "", nil
	}

	// Map name at +8: alloc-prefixed UTF-16 string
	mapName := ""
	if nameOff := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12])); nameOff > 0 {
		mapName = readCRIMString(data, nameOff)
	}

	entries := make(map[uint32]string, entryCount)
	for i := 0; i < entryCount; i++ {
		entryOff := pos + 20 + i*8
		if entryOff+8 > len(data) {
			break
		}
		val := binary.LittleEndian.Uint32(data[entryOff : entryOff+4])
		ref := binary.LittleEndian.Uint32(data[entryOff+4 : entryOff+8])
		name := ""
		// 0xD2-encoded reference: high 16 bits == 0xD200 → index into stringTable
		if ref>>16 == 0xD200 {
			name = resolveVMAPString(ref, stringTable)
		} else if ref > 0 {
			// Fallback: treat as a direct CRIM string offset
			name = readCRIMString(data, int(ref))
		}
		if name != "" {
			entries[val] = name
		}
	}

	return mapName, entries
}

// ParseMAPSBMAPBlock parses a BMAP sub-block inside a MAPS block.
//
// Layout mirrors VMAP: +8 = map name offset, +16 = entryCount, +20 = entries.
func ParseMAPSBMAPBlock(data []byte, pos int, stringTable map[uint32]string) (string, map[uint64]string) {
	if pos+20 > len(data) || string(data[pos:pos+4]) != "BMAP" {
		return "", nil
	}
	entryCount := int(binary.LittleEndian.Uint32(data[pos+16 : pos+20]))
	if entryCount <= 0 || entryCount > 4096 {
		return "", nil
	}

	mapName := ""
	if nameOff := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12])); nameOff > 0 {
		mapName = readCRIMString(data, nameOff)
	}

	entries := make(map[uint64]string, entryCount)
	for i := 0; i < entryCount; i++ {
		entryOff := pos + 20 + i*8
		if entryOff+8 > len(data) {
			break
		}
		bitPos := uint64(binary.LittleEndian.Uint32(data[entryOff : entryOff+4]))
		ref := binary.LittleEndian.Uint32(data[entryOff+4 : entryOff+8])
		name := ""
		if ref>>16 == 0xD200 {
			name = resolveVMAPString(ref, stringTable)
		} else if ref > 0 {
			name = readCRIMString(data, int(ref))
		}
		if name != "" {
			entries[bitPos] = name
		}
	}

	return mapName, entries
}

// ParseMAPSBlock parses a MAPS container block, returning VMAP and BMAP maps.
// The MAPS block contains sub-block offsets relative to CRIM resource start.
func ParseMAPSBlock(data []byte, pos int) (map[string]map[uint32]string, map[string]map[uint64]string) {
	return ParseMAPSBlockWithStrings(data, pos, nil)
}

// ParseMAPSBlockWithStrings parses a MAPS container block with a string table
// for resolving 0xD2-encoded string references in VMAP entries.
func ParseMAPSBlockWithStrings(data []byte, pos int, stringTable map[uint32]string) (map[string]map[uint32]string, map[string]map[uint64]string) {
	if pos+12 > len(data) || string(data[pos:pos+4]) != "MAPS" {
		return nil, nil
	}
	totalSize := int(binary.LittleEndian.Uint32(data[pos+4 : pos+8]))
	subCount := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if subCount <= 0 || subCount > 256 || pos+totalSize > len(data) {
		return nil, nil
	}

	valueMaps := make(map[string]map[uint32]string)
	bitMaps := make(map[string]map[uint64]string)

	for i := 0; i < subCount; i++ {
		off := pos + 12 + i*4
		if off+4 > len(data) {
			break
		}
		subOff := int(binary.LittleEndian.Uint32(data[off : off+4]))
		if subOff+4 > len(data) {
			continue
		}
		sig := string(data[subOff : subOff+4])
		switch sig {
		case "VMAP":
			name, entries := ParseMAPSVMAPBlock(data, subOff, stringTable)
			if name != "" && len(entries) > 0 {
				valueMaps[name] = entries
			}
		case "BMAP":
			name, entries := ParseMAPSBMAPBlock(data, subOff, stringTable)
			if name != "" && len(entries) > 0 {
				bitMaps[name] = entries
			}
		}
	}
	return valueMaps, bitMaps
}

// ParseBMAPBlock parses a BMAP block, returns (map_name, bit_position→display_name).
// TTBLParam is one parameter from a TEMP descriptor.
//
// Index is 1-based (corresponds to %1, %2, ... in the message template).
// Name is the parameter name from the CRIM string table.
// MapRef is the CRIM-relative offset of the VMAP/BMAP block this parameter
// uses for value display resolution (0 = no map). This is the compiled form
// of the manifest's <data name="..." map="..."/> association.
type TTBLParam struct {
	Index  int
	Name   string
	MapRef uint32
	// InType/OutType are the TDH enum bytes from the descriptor +4:4 field
	// ((OutType<<8)|InType): e.g. InType 0x14=HEXINT32, 0x0D=BOOLEAN,
	// 0x0A=UINT64, 0x12=HEXINT32-out, 0x20=HRESULT-out. Captured for typed
	// rendering (R10).
	InType  uint8
	OutType uint8
}

// ParseTTBLBlock parses a TTBL (template table) block from CRIM data.
//
// Returns a map of TEMP-entry-offset -> list of parameter descriptors,
// or nil if the block is invalid/empty.
func ParseTTBLBlock(data []byte, pos int) map[uint32][]TTBLParam {
	if pos+12 > len(data) || string(data[pos:pos+4]) != "TTBL" {
		return nil
	}
	tempCount := binary.LittleEndian.Uint32(data[pos+8 : pos+12])
	if tempCount == 0 || tempCount > 4096 {
		return nil
	}

	templates := make(map[uint32][]TTBLParam)
	cur := pos + 12

	for i := 0; i < int(tempCount); i++ {
		if cur+40 > len(data) || string(data[cur:cur+4]) != "TEMP" {
			break
		}
		tempSize := int(binary.LittleEndian.Uint32(data[cur+4 : cur+8]))
		itemDescCount := int(binary.LittleEndian.Uint32(data[cur+8 : cur+12]))
		itemsOffset := int(binary.LittleEndian.Uint32(data[cur+16 : cur+20]))

		// Skip TEMP entries with no descriptors or too many
		if itemDescCount == 0 || itemDescCount > 256 {
			if tempSize > 0 {
				cur += tempSize
			} else {
				cur += 40
			}
			continue
		}

		// Validate item descriptor bounds
		descEnd := itemsOffset + itemDescCount*20
		if itemsOffset+20 > len(data) || descEnd > len(data) {
			if tempSize > 0 {
				cur += tempSize
			} else {
				cur += 40
			}
			continue
		}

		// Parse item descriptors (20 bytes each):
		//   +0:4  flags
		//   +4:4  (OutType<<8)|InType (0x0808=u32, 0x0A0A=u64, 0x0D0D=bool)
		//   +8:4  map reference: CRIM-relative offset of VMAP/BMAP block (0 = none)
		//   +12:4 unknown (observed 0 or 1)
		//   +16:4 parameter name string offset
		params := make([]TTBLParam, 0, itemDescCount)
		for idx := 0; idx < itemDescCount; idx++ {
			dOff := itemsOffset + idx*20
			nameOff := int(binary.LittleEndian.Uint32(data[dOff+16 : dOff+20]))
			mapRef := binary.LittleEndian.Uint32(data[dOff+8 : dOff+12])
			if mapRef != 0 && !isMapBlockSig(data, mapRef) {
				// Not a recognized map block — ignore the reference.
				mapRef = 0
			}
			if nameOff > 0 && nameOff < len(data) {
				if name := readCRIMString(data, nameOff); name != "" {
					params = append(params, TTBLParam{
						Index:   idx + 1,
						Name:    name,
						MapRef:  mapRef,
						InType:  data[dOff+4],
						OutType: data[dOff+5],
					})
				}
			}
		}

		// Filter names: must be non-empty and contain only valid chars
		validParams := make([]TTBLParam, 0, len(params))
		for _, p := range params {
			if p.Name != "" && isAlphaNumDashSpace(p.Name) {
				validParams = append(validParams, p)
			}
		}
		if len(validParams) > 0 {
			templates[uint32(cur)] = validParams
		}

		if tempSize > 0 {
			cur += tempSize
		} else {
			cur += 40
		}
	}

	return templates
}

// isAlphaNumDashSpace checks if a string contains only alphanumeric
// characters, underscores, hyphens, or spaces (matching Rust's filter).
func isAlphaNumDashSpace(s string) bool {
	for _, c := range s {
		if !((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
			(c >= '0' && c <= '9') || c == '_' || c == '-' || c == ' ') {
			return false
		}
	}
	return true
}

// isMapBlockSig reports whether the 4 bytes at the given CRIM-relative offset
// are a VMAP or BMAP block signature (valid map reference targets).
func isMapBlockSig(data []byte, off uint32) bool {
	o := int(off)
	if o+4 > len(data) {
		return false
	}
	sig := string(data[o : o+4])
	return sig == "VMAP" || sig == "BMAP"
}

// ProviderData holds all extracted data for one provider from a CRIM blob.
//
// Mirrors the Rust CrimProviderData struct exactly, including value_maps
// and bit_maps which are populated from VMAP and BMAP blocks.
type ProviderData struct {
	Events           []EventEntry
	Tasks            map[uint32]string
	TasksByOffset    map[uint32]string
	Opcodes          map[uint32]string
	OpcodesByOffset  map[uint32]string
	Levels           map[uint32]string
	LevelsByOffset   map[uint32]string
	KeywordsMap      map[uint64]string
	KeywordsByOffset map[uint32]string
	TemplateParams   map[uint32][]TTBLParam
	ValueMaps        map[string]map[uint32]string
	BitMaps          map[string]map[uint64]string
	// MapRefsByOffset maps a CRIM-relative VMAP/BMAP block offset to its
	// parsed name and kind. Used to resolve TTBLParam.MapRef references to
	// the named map (e.g. param "Reason" → VMAP "Pop:PowerStateEvent").
	MapRefsByOffset map[uint32]MapRefInfo
}

// MapRefInfo describes one VMAP/BMAP block referenced from a template item.
type MapRefInfo struct {
	Name     string
	IsBitmap bool
}

// ParseProviderData parses all provider data from a CRIM blob for a
// specific GUID.
//
// Returns nil if the GUID is not found or data is invalid.
// ParseProviderData parses provider data from a CRIM blob for a specific GUID.
func ParseProviderData(data []byte, guid string) *ProviderData {
	return ParseProviderDataWithStrings(data, guid, nil)
}

// ParseProviderDataWithStrings parses provider data with a string table for
// resolving 0xD2-encoded references in VMAP blocks.
func ParseProviderDataWithStrings(data []byte, guid string, stringTable map[uint32]string) *ProviderData {
	if len(data) < 16 || string(data[:4]) != "CRIM" {
		return nil
	}

	target := NormalizeGUID(guid)
	providerCount := int(binary.LittleEndian.Uint32(data[12:16]))
	maxProviders := (len(data) - 16) / 20

	for i := 0; i < providerCount && i < maxProviders; i++ {
		descOff := 16 + i*20
		if descOff+20 > len(data) {
			break
		}

		// Extract GUID from descriptor and compare
		var guidBytes [16]byte
		copy(guidBytes[:], data[descOff:descOff+16])
		guidStr := GUIDBytesToString(guidBytes)
		if guidStr != target {
			continue
		}

		// Found matching provider — navigate its WEVT block
		provOff := int(binary.LittleEndian.Uint32(data[descOff+16 : descOff+20]))
		if provOff+20 > len(data) || string(data[provOff:provOff+4]) != "WEVT" {
			return nil
		}

		descCount := int(binary.LittleEndian.Uint32(data[provOff+12 : provOff+16]))

		result := &ProviderData{}

		// Iterate element descriptors and dispatch by block signature
		for j := 0; j < descCount; j++ {
			eo := provOff + 20 + j*8
			if eo+8 > len(data) {
				break
			}
			elemOff := int(binary.LittleEndian.Uint32(data[eo : eo+4]))
			if elemOff+4 > len(data) {
				continue
			}
			sig := string(data[elemOff : elemOff+4])

			switch sig {
			case "EVNT":
				result.Events = ParseEVNTBlock(data, elemOff)
			case "TASK":
				result.Tasks, result.TasksByOffset = ParseTASKBlock(data, elemOff)
			case "OPCO":
				result.Opcodes, result.OpcodesByOffset = ParseOPCOBlock(data, elemOff)
			case "LEVL":
				result.Levels, result.LevelsByOffset = ParseLEVLBlock(data, elemOff)
			case "KEYW":
				result.KeywordsMap, result.KeywordsByOffset = ParseKEYWBlock(data, elemOff)
			case "TTBL":
				result.TemplateParams = ParseTTBLBlock(data, elemOff)
			case "VMAP":
				name, entries := ParseVMAPBlock(data, elemOff)
				if name != "" && len(entries) > 0 {
					if result.ValueMaps == nil {
						result.ValueMaps = make(map[string]map[uint32]string)
					}
					result.ValueMaps[name] = entries
					result.recordMapRef(uint32(elemOff), name, false)
				}
			case "BMAP":
				name, entries := ParseBMAPBlock(data, elemOff)
				if name != "" && len(entries) > 0 {
					if result.BitMaps == nil {
						result.BitMaps = make(map[string]map[uint64]string)
					}
					result.BitMaps[name] = entries
					result.recordMapRef(uint32(elemOff), name, true)
				}
			case "MAPS":
				// Parse MAPS block using the special MAPS sub-block format
				vmaps, bmaps := ParseMAPSBlockWithStrings(data, elemOff, stringTable)
				for name, entries := range vmaps {
					if result.ValueMaps == nil {
						result.ValueMaps = make(map[string]map[uint32]string)
					}
					result.ValueMaps[name] = entries
				}
				for name, entries := range bmaps {
					if result.BitMaps == nil {
						result.BitMaps = make(map[string]map[uint64]string)
					}
					result.BitMaps[name] = entries
				}
				// Record sub-block offsets so TTBLParam.MapRef can resolve names.
				for off, info := range ParseMAPSNames(data, elemOff) {
					result.recordMapRef(off, info.Name, info.IsBitmap)
				}
			}
		}

		return result
	}

	return nil
}

// recordMapRef stores a VMAP/BMAP block offset → name mapping (first wins).
func (pd *ProviderData) recordMapRef(off uint32, name string, isBitmap bool) {
	if name == "" {
		return
	}
	if pd.MapRefsByOffset == nil {
		pd.MapRefsByOffset = make(map[uint32]MapRefInfo)
	}
	if _, exists := pd.MapRefsByOffset[off]; !exists {
		pd.MapRefsByOffset[off] = MapRefInfo{Name: name, IsBitmap: isBitmap}
	}
}

// ParseMAPSNames parses a MAPS container and returns each sub-block's
// CRIM-relative offset mapped to its name and kind. Used to resolve
// TTBLParam.MapRef references (TEMP item +8 field).
func ParseMAPSNames(data []byte, pos int) map[uint32]MapRefInfo {
	result := make(map[uint32]MapRefInfo)
	if pos+12 > len(data) || string(data[pos:pos+4]) != "MAPS" {
		return result
	}
	subCount := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if subCount <= 0 || subCount > 65536 {
		return result
	}
	for i := 0; i < subCount; i++ {
		offPos := pos + 12 + i*4
		if offPos+4 > len(data) {
			break
		}
		subOff := binary.LittleEndian.Uint32(data[offPos : offPos+4])
		o := int(subOff)
		if o+20 > len(data) {
			continue
		}
		sig := string(data[o : o+4])
		var isBitmap bool
		switch sig {
		case "VMAP":
			isBitmap = false
		case "BMAP":
			isBitmap = true
		default:
			continue
		}
		nameOff := int(binary.LittleEndian.Uint32(data[o+8 : o+12]))
		name := readCRIMString(data, nameOff)
		if name == "" {
			continue
		}
		result[subOff] = MapRefInfo{Name: name, IsBitmap: isBitmap}
	}
	return result
}

func ParseBMAPBlock(data []byte, pos int) (string, map[uint64]string) {
	if pos+20 > len(data) || string(data[pos:pos+4]) != "BMAP" {
		return "", nil
	}
	count := int(binary.LittleEndian.Uint32(data[pos+8 : pos+12]))
	if count <= 0 || count > 4096 {
		return "", nil
	}
	nameOff := int(binary.LittleEndian.Uint32(data[pos+12 : pos+16]))
	mapName := readCRIMString(data, nameOff)
	if mapName == "" {
		return "", nil
	}
	entries := make(map[uint64]string, count)
	for i := 0; i < count; i++ {
		entryOff := pos + 16 + i*8
		if entryOff+8 > len(data) {
			break
		}
		bitPos := uint64(binary.LittleEndian.Uint32(data[entryOff : entryOff+4]))
		valNameOff := int(binary.LittleEndian.Uint32(data[entryOff+4 : entryOff+8]))
		if valNameOff > 0 {
			if valName := readCRIMString(data, valNameOff); valName != "" {
				entries[bitPos] = valName
			}
		}
	}
	return mapName, entries
}