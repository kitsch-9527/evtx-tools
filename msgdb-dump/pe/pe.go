// Package pe implements PE (Portable Executable) resource directory traversal
// and message table extraction. All offline binary parsing — no Windows API.
//
// Basic PE file parsing (DOS header, optional header, section headers) is
// delegated to the wcorefx/pe package to avoid duplicating low-level PE
// parsing code across the project.
package pe

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"os"
	"strings"
	"unicode/utf16"

	wcorefxpe "github.com/kitsch-9527/wcorefx/pe"
)

// CombinedPEData is the result of extracting all data from a PE file in one pass.
type CombinedPEData struct {
	Messages  map[uint32]string // event_id → message template (with %N placeholders)
	Version   string            // PE version string like "10.0.22621.1"
	CrimBlobs [][]byte          // raw WEVT_TEMPLATE CRIM blob bytes
	RTStrings map[uint32]string // RT_STRING resources (string ID → string value)
	D2Strings map[uint32]string // RT_MESSAGETABLE entries with ID 0xD200xxxx (index → display string)
	// MessagesPerLang carries the RT_MESSAGETABLE content per resource language
	// ID (R27, 2026-09-30). Multi-language files (DriverStore e1dmsg.dll 等)内嵌
	// 多语言块，merged Messages 的 last-write-wins 会让随机一个语言胜出并被存到
	// 所有 lang 键下；消费端按目标语言选择 + 0x0409 回退。
	MessagesPerLang map[uint16]map[uint32]string
}

// SelectMessagesForLangLCID returns the message table for the requested
// resource language (R27): exact language block → 0x0409 (en) block → the
// merged map (legacy single-language semantics).
func (c *CombinedPEData) SelectMessagesForLangLCID(lcid uint16) map[uint32]string {
	if c.MessagesPerLang != nil {
		if tbl := c.MessagesPerLang[lcid]; len(tbl) > 0 {
			return tbl
		}
		if tbl := c.MessagesPerLang[0x0409]; len(tbl) > 0 {
			return tbl
		}
	}
	return c.Messages
}

// resolveRVA converts an RVA to a file offset using PE section headers from
// wcorefx/pe. Replaces the old standalone RVAtoOffset that parsed sections
// from raw bytes.
func resolveRVA(sections []wcorefxpe.ImageSectionHeader, rva uint32) (uint32, error) {
	for _, sec := range sections {
		vaStart := sec.VirtualAddress
		span := sec.SizeOfRawData
		if sec.VirtualSize > span {
			span = sec.VirtualSize
		}
		if rva >= vaStart && rva < vaStart+span {
			return sec.PointerToRawData + (rva - vaStart), nil
		}
	}
	return 0, fmt.Errorf("pe: RVA 0x%X not in any section", rva)
}

// u32max returns the larger of two uint32 values (avoids importing math).
func u32max(a, b uint32) uint32 {
	if a > b {
		return a
	}
	return b
}

// openPEWithSections reads a PE file and returns its raw bytes, wcorefx PEFile
// (for RVA resolution and section access), and the .rsrc section offset + size.
func openPEWithSections(dllPath string) (peData []byte, sections []wcorefxpe.ImageSectionHeader, rsrcOffset uint32, rsrcSize uint32, err error) {
	peData, err = os.ReadFile(dllPath)
	if err != nil {
		return nil, nil, 0, 0, fmt.Errorf("pe: failed to read %q: %w", dllPath, err)
	}

	peFile, err := wcorefxpe.OpenReader(bytes.NewReader(peData))
	if err != nil {
		return nil, nil, 0, 0, fmt.Errorf("pe: %q: %w", dllPath, err)
	}

	sections = peFile.Sections

	// Find .rsrc section
	for _, sec := range sections {
		name := string(bytes.TrimRight(sec.Name[:], "\x00"))
		if name == ".rsrc" {
			rsrcOffset = sec.PointerToRawData
			rsrcSize = sec.SizeOfRawData
			if rsrcOffset+rsrcSize > uint32(len(peData)) {
				return nil, nil, 0, 0, fmt.Errorf("pe: %q: .rsrc section extends beyond file", dllPath)
			}
			return peData, sections, rsrcOffset, rsrcSize, nil
		}
	}

	return nil, nil, 0, 0, fmt.Errorf("pe: %q: no .rsrc section found", dllPath)
}

// ExtractAllFromPE opens a PE file once and extracts messages, version, and CRIM blobs.
func ExtractAllFromPE(dllPath string) (*CombinedPEData, error) {
	peData, sections, rsrcOffset, rsrcSize, err := openPEWithSections(dllPath)
	if err != nil {
		return nil, err
	}

	result := &CombinedPEData{
		Messages:  make(map[uint32]string),
		Version:   "0.0.0.0",
		CrimBlobs: nil,
	}

	// Extract RT_MESSAGETABLE (type ID = 11)
	// Message table entries with IDs in the 0xDn000000 families are CRIM
	// string tables referenced by VMAP entry string offsets (ref = 0xDn00xxxx,
	// low bits = index). Observed families: 0xD2 (Kernel-Power et al.), 0xD1
	// (Ntfs VolumeCorruptionActionState), 0xD0 (Kernel-Processor-Power),
	// 0xDC (System-Events EnableDisableReasonMap). They must NOT be treated
	// as event messages. Keyed by the full message ID so same-index entries
	// across families never collide.
	msgEntries, err := GetResourceByTypeID(peData, rsrcOffset, rsrcSize, sections, 11)
	if err == nil {
		for _, dataEntries := range msgEntries {
			for _, de := range dataEntries {
				msgs := ParseMessageTable(de.Data)
				if len(msgs) > 0 {
					// R27: per-language bookkeeping — merged map 之外按资源语言
					// ID 分桶，供多语言单文件（e1dmsg.dll 型）按语言选择。
					if result.MessagesPerLang == nil {
						result.MessagesPerLang = make(map[uint16]map[uint32]string)
					}
					if result.MessagesPerLang[de.Lang] == nil {
						result.MessagesPerLang[de.Lang] = make(map[uint32]string, len(msgs))
					}
					for id, msg := range msgs {
						result.MessagesPerLang[de.Lang][id] = msg
					}
				}
				for eid, msg := range msgs {
					// 0xDn-encoded string table entries: high nibble 0xD
					if h := eid >> 24; h&0xF0 == 0xD0 {
						if result.D2Strings == nil {
							result.D2Strings = make(map[uint32]string)
						}
						result.D2Strings[eid] = strings.TrimRight(msg, "\r\n\x00 ")
						continue
					}
					// Last write wins on duplicates
					result.Messages[eid] = msg
				}
			}
		}
	}

	// Extract PE version
	result.Version = PEVersion(peData, rsrcOffset, rsrcSize, sections)

	// Extract WEVT_TEMPLATE CRIM blobs
	crimEntries, err := GetResourceByTypeName(peData, rsrcOffset, rsrcSize, sections, "WEVT_TEMPLATE")
	if err == nil {
		for _, dataEntries := range crimEntries {
			for _, de := range dataEntries {
				blob := make([]byte, len(de.Data))
				copy(blob, de.Data)
				result.CrimBlobs = append(result.CrimBlobs, blob)
			}
		}
	}

	// Extract RT_STRING resources (type 6) for 0xD2 VMAP string references
	rtEntries, err := GetResourceByTypeID(peData, rsrcOffset, rsrcSize, sections, 6)
	if err == nil {
		result.RTStrings = make(map[uint32]string)
		for resID, dataEntries := range rtEntries {
			for _, de := range dataEntries {
				// RT_STRING blocks contain 16 strings each:
				// each string is (WORD length, UTF-16 chars)
				// string ID = (block_index * 16) + string_index
				blockIndex := resID
				data := de.Data
				pos := 0
				for si := 0; si < 16; si++ {
					if pos+2 > len(data) {
						break
					}
					length := int(binary.LittleEndian.Uint16(data[pos : pos+2]))
					pos += 2
					if length == 0 {
						continue
					}
					if pos+length*2 > len(data) {
						break
					}
					var words []uint16
					for i := 0; i < length; i++ {
						words = append(words, uint16(data[pos+i*2])|uint16(data[pos+i*2+1])<<8)
					}
					pos += length * 2
					strID := uint32(blockIndex)*16 + uint32(si)
					result.RTStrings[strID] = string(utf16.Decode(words))
				}
			}
		}
	}

	if len(result.Messages) == 0 && len(result.CrimBlobs) == 0 {
		// Return what we have anyway; caller decides
	}

	return result, nil
}
