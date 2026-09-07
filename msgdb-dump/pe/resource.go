package pe

import (
	"encoding/binary"
	"fmt"
	"unicode/utf16"

	wcorefxpe "github.com/kitsch-9527/wcorefx/pe"
)

// ResourceDataEntry represents a leaf PE resource with its raw data and language ID.
type ResourceDataEntry struct {
	Data []byte // raw resource data bytes
	Lang uint16 // language ID (e.g. 1033 for en-US)
}

// imageResourceDirectory represents IMAGE_RESOURCE_DIRECTORY (16 bytes).
type imageResourceDirectory struct {
	Characteristics      uint32
	TimeDateStamp        uint32
	MajorVersion         uint16
	MinorVersion         uint16
	NumberOfNamedEntries uint16
	NumberOfIdEntries    uint16
}

// imageResourceDirectoryEntry represents one IMAGE_RESOURCE_DIRECTORY_ENTRY (8 bytes).
type imageResourceDirectoryEntry struct {
	NameID          uint32
	DataEntryOffset uint32
}

// imageResourceDataEntry represents IMAGE_RESOURCE_DATA_ENTRY (16 bytes).
type imageResourceDataEntry struct {
	DataRVA  uint32
	Size     uint32
	CodePage uint32
	Reserved uint32
}

func (e *imageResourceDirectoryEntry) isNamedEntry() bool {
	return e.NameID&0x80000000 != 0
}

func (e *imageResourceDirectoryEntry) id() uint32 {
	return e.NameID & 0x7FFFFFFF
}

func (e *imageResourceDirectoryEntry) nameStringOffset() uint32 {
	return e.NameID & 0x7FFFFFFF
}

func (e *imageResourceDirectoryEntry) isSubdirectory() bool {
	return e.DataEntryOffset&0x80000000 != 0
}

func (e *imageResourceDirectoryEntry) dataOffset() uint32 {
	return e.DataEntryOffset & 0x7FFFFFFF
}

// readResourceDirectory reads an IMAGE_RESOURCE_DIRECTORY from data at the given offset.
func readResourceDirectory(data []byte, off uint32) (*imageResourceDirectory, error) {
	if uint64(off)+16 > uint64(len(data)) {
		return nil, fmt.Errorf("pe: resource directory at offset %d out of bounds", off)
	}
	return &imageResourceDirectory{
		Characteristics:      binary.LittleEndian.Uint32(data[off : off+4]),
		TimeDateStamp:        binary.LittleEndian.Uint32(data[off+4 : off+8]),
		MajorVersion:         binary.LittleEndian.Uint16(data[off+8 : off+10]),
		MinorVersion:         binary.LittleEndian.Uint16(data[off+10 : off+12]),
		NumberOfNamedEntries: binary.LittleEndian.Uint16(data[off+12 : off+14]),
		NumberOfIdEntries:    binary.LittleEndian.Uint16(data[off+14 : off+16]),
	}, nil
}

// readResourceDirEntries reads the entries following an IMAGE_RESOURCE_DIRECTORY.
func readResourceDirEntries(data []byte, dirOff uint32, dir *imageResourceDirectory) ([]imageResourceDirectoryEntry, error) {
	totalEntries := uint32(dir.NumberOfNamedEntries) + uint32(dir.NumberOfIdEntries)
	entriesOff := dirOff + 16
	if uint64(entriesOff)+uint64(totalEntries)*8 > uint64(len(data)) {
		return nil, fmt.Errorf("pe: resource directory entries at offset %d out of bounds", entriesOff)
	}

	entries := make([]imageResourceDirectoryEntry, totalEntries)
	for i := uint32(0); i < totalEntries; i++ {
		off := entriesOff + i*8
		entries[i].NameID = binary.LittleEndian.Uint32(data[off : off+4])
		entries[i].DataEntryOffset = binary.LittleEndian.Uint32(data[off+4 : off+8])
	}
	return entries, nil
}

// readResourceNameString reads a UTF-16LE resource name string from the resource section.
func readResourceNameString(peData []byte, rsrcOffset uint32, strOffset uint32) (string, error) {
	off := rsrcOffset + strOffset
	if uint64(off)+2 > uint64(len(peData)) {
		return "", fmt.Errorf("pe: resource name string offset %d out of bounds", off)
	}
	length := int(binary.LittleEndian.Uint16(peData[off : off+2]))
	if length == 0 {
		return "", nil
	}
	charOff := off + 2
	if uint64(charOff)+uint64(length)*2 > uint64(len(peData)) {
		return "", fmt.Errorf("pe: resource name string data out of bounds")
	}
	runes := make([]uint16, length)
	for i := 0; i < length; i++ {
		runes[i] = binary.LittleEndian.Uint16(peData[charOff+uint32(i)*2:])
	}
	return string(utf16.Decode(runes)), nil
}

// readResourceDataEntry reads an IMAGE_RESOURCE_DATA_ENTRY at the given offset within rsrc.
func readResourceDataEntry(peData []byte, rsrcOffset uint32, entryOff uint32) (*imageResourceDataEntry, error) {
	off := rsrcOffset + entryOff
	if uint64(off)+16 > uint64(len(peData)) {
		return nil, fmt.Errorf("pe: resource data entry at offset %d out of bounds", off)
	}
	return &imageResourceDataEntry{
		DataRVA:  binary.LittleEndian.Uint32(peData[off : off+4]),
		Size:     binary.LittleEndian.Uint32(peData[off+4 : off+8]),
		CodePage: binary.LittleEndian.Uint32(peData[off+8 : off+12]),
		Reserved: binary.LittleEndian.Uint32(peData[off+12 : off+16]),
	}, nil
}

// enumerateLanguageDir processes a Level 3 (Language) resource directory and collects
// all leaf data entries keyed by language ID.
// dirOff is an offset relative to the resource section start.
func enumerateLanguageDir(peData []byte, rsrcOffset uint32, dirOff uint32, sections []wcorefxpe.ImageSectionHeader) ([]ResourceDataEntry, error) {
	absDirOff := rsrcOffset + dirOff
	dir, err := readResourceDirectory(peData, absDirOff)
	if err != nil {
		return nil, err
	}
	entries, err := readResourceDirEntries(peData, absDirOff, dir)
	if err != nil {
		return nil, err
	}

	var results []ResourceDataEntry
	for _, entry := range entries {
		if entry.isSubdirectory() {
			// Unexpected: language entries should be leaves, but handle gracefully
			sub, err := enumerateLanguageDir(peData, rsrcOffset, entry.dataOffset(), sections)
			if err != nil {
				continue
			}
			results = append(results, sub...)
			continue
		}
		// Leaf: read IMAGE_RESOURCE_DATA_ENTRY
		dataEntry, err := readResourceDataEntry(peData, rsrcOffset, entry.dataOffset())
		if err != nil {
			continue
		}
		// Resolve DataRVA to file offset
		fileOff, err := resolveRVA(sections, dataEntry.DataRVA)
		if err != nil {
			continue
		}
		if uint64(fileOff)+uint64(dataEntry.Size) > uint64(len(peData)) {
			continue
		}
		rawData := make([]byte, dataEntry.Size)
		copy(rawData, peData[fileOff:fileOff+dataEntry.Size])

		results = append(results, ResourceDataEntry{
			Data: rawData,
			Lang: uint16(entry.id()), // entry ID is the language ID
		})
	}
	return results, nil
}

// GetResourceByTypeID enumerates all resources of a given type ID.
// typeID: e.g. 11 = RT_MESSAGETABLE, 16 = RT_VERSION.
// Returns a map keyed by resource name ID (Level 2 entry ID).
func GetResourceByTypeID(peData []byte, rsrcOffset uint32, rsrcSize uint32, sections []wcorefxpe.ImageSectionHeader, typeID uint32) (map[uint32][]ResourceDataEntry, error) {
	if uint64(rsrcOffset)+16 > uint64(len(peData)) {
		return nil, fmt.Errorf("pe: resource data out of bounds")
	}

	// Level 1: Type directory
	rootDir, err := readResourceDirectory(peData, rsrcOffset)
	if err != nil {
		return nil, err
	}
	rootEntries, err := readResourceDirEntries(peData, rsrcOffset, rootDir)
	if err != nil {
		return nil, err
	}

	// Find the type entry matching typeID
	var typeSubdirOffset uint32
	found := false
	for _, entry := range rootEntries {
		if !entry.isNamedEntry() && entry.id() == typeID {
			if !entry.isSubdirectory() {
				return nil, fmt.Errorf("pe: resource type %d is not a directory", typeID)
			}
			typeSubdirOffset = entry.dataOffset()
			found = true
			break
		}
	}
	if !found {
		return nil, fmt.Errorf("pe: resource type %d not found", typeID)
	}

	return enumerateNameDirectory(peData, rsrcOffset, typeSubdirOffset, sections)
}

// enumerateNameDirectory processes a Level 2 (Name) resource directory and collects
// all language-level data entries, keyed by name entry ID.
// dirOff is an offset relative to the resource section start.
func enumerateNameDirectory(peData []byte, rsrcOffset uint32, dirOff uint32, sections []wcorefxpe.ImageSectionHeader) (map[uint32][]ResourceDataEntry, error) {
	absDirOff := rsrcOffset + dirOff
	nameDir, err := readResourceDirectory(peData, absDirOff)
	if err != nil {
		return nil, err
	}
	nameEntries, err := readResourceDirEntries(peData, absDirOff, nameDir)
	if err != nil {
		return nil, err
	}

	results := make(map[uint32][]ResourceDataEntry)
	for _, nameEntry := range nameEntries {
		nameID := nameEntry.id() // numeric name ID; if named entry, this is the string offset
		if !nameEntry.isSubdirectory() {
			// Direct data entry at name level (rare)
			dataEntry, err := readResourceDataEntry(peData, rsrcOffset, nameEntry.dataOffset())
			if err != nil {
				continue
			}
			fileOff, err := resolveRVA(sections, dataEntry.DataRVA)
			if err != nil {
				continue
			}
			if uint64(fileOff)+uint64(dataEntry.Size) > uint64(len(peData)) {
				continue
			}
			rawData := make([]byte, dataEntry.Size)
			copy(rawData, peData[fileOff:fileOff+dataEntry.Size])
			results[nameID] = append(results[nameID], ResourceDataEntry{
				Data: rawData,
				Lang: 0, // no language level
			})
			continue
		}

		// Level 3: Language directory
		langEntries, err := enumerateLanguageDir(peData, rsrcOffset, nameEntry.dataOffset(), sections)
		if err != nil {
			continue
		}
		results[nameID] = append(results[nameID], langEntries...)
	}
	return results, nil
}

// GetResourceByTypeName enumerates all resources of a given named type.
// typeName: e.g. "WEVT_TEMPLATE".
// Returns a map keyed by resource name ID (Level 2 entry ID).
func GetResourceByTypeName(peData []byte, rsrcOffset uint32, rsrcSize uint32, sections []wcorefxpe.ImageSectionHeader, typeName string) (map[uint32][]ResourceDataEntry, error) {
	if uint64(rsrcOffset)+16 > uint64(len(peData)) {
		return nil, fmt.Errorf("pe: resource data out of bounds")
	}

	// Level 1: Type directory
	rootDir, err := readResourceDirectory(peData, rsrcOffset)
	if err != nil {
		return nil, err
	}
	rootEntries, err := readResourceDirEntries(peData, rsrcOffset, rootDir)
	if err != nil {
		return nil, err
	}

	// Find the type entry matching typeName by scanning named entries
	var typeSubdirOffset uint32
	found := false
	for _, entry := range rootEntries {
		if !entry.isNamedEntry() {
			continue
		}
		strOff := entry.nameStringOffset()
		name, err := readResourceNameString(peData, rsrcOffset, strOff)
		if err != nil || name != typeName {
			continue
		}
		if !entry.isSubdirectory() {
			return nil, fmt.Errorf("pe: resource type %q is not a directory", typeName)
		}
		typeSubdirOffset = entry.dataOffset()
		found = true
		break
	}
	if !found {
		return nil, fmt.Errorf("pe: resource type %q not found", typeName)
	}

	return enumerateNameDirectory(peData, rsrcOffset, typeSubdirOffset, sections)
}
