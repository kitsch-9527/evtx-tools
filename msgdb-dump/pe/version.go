package pe

import (
	"encoding/binary"
	"fmt"

	wcorefxpe "github.com/kitsch-9527/wcorefx/pe"
)

// fixedFileInfo is VS_FIXEDFILEINFO (52 bytes).
type fixedFileInfo struct {
	Signature        uint32
	StrucVersion     uint32
	FileVersionMS    uint32
	FileVersionLS    uint32
	ProductVersionMS uint32
	ProductVersionLS uint32
	FileFlagsMask    uint32
	FileFlags        uint32
	FileOS           uint32
	FileType         uint32
	FileSubtype      uint32
	FileDateMS       uint32
	FileDateLS       uint32
}

// readFixedFileInfo reads a VS_FIXEDFILEINFO from data.
// Returns nil if the signature doesn't match.
func readFixedFileInfo(data []byte) *fixedFileInfo {
	if len(data) < 52 {
		return nil
	}
	sig := binary.LittleEndian.Uint32(data[0:4])
	if sig != 0xFEEF04BD {
		return nil
	}
	return &fixedFileInfo{
		Signature:        sig,
		StrucVersion:     binary.LittleEndian.Uint32(data[4:8]),
		FileVersionMS:    binary.LittleEndian.Uint32(data[8:12]),
		FileVersionLS:    binary.LittleEndian.Uint32(data[12:16]),
		ProductVersionMS: binary.LittleEndian.Uint32(data[16:20]),
		ProductVersionLS: binary.LittleEndian.Uint32(data[20:24]),
		FileFlagsMask:    binary.LittleEndian.Uint32(data[24:28]),
		FileFlags:        binary.LittleEndian.Uint32(data[28:32]),
		FileOS:           binary.LittleEndian.Uint32(data[32:36]),
		FileType:         binary.LittleEndian.Uint32(data[36:40]),
		FileSubtype:      binary.LittleEndian.Uint32(data[40:44]),
		FileDateMS:       binary.LittleEndian.Uint32(data[44:48]),
		FileDateLS:       binary.LittleEndian.Uint32(data[48:52]),
	}
}

// findFixedFileInfo scans version resource data for the VS_FIXEDFILEINFO signature.
// It scans at 4-byte intervals to find 0xFEEF04BD.
func findFixedFileInfo(data []byte) *fixedFileInfo {
	for i := 0; i <= len(data)-52; i += 4 {
		if binary.LittleEndian.Uint32(data[i:i+4]) == 0xFEEF04BD {
			return readFixedFileInfo(data[i:])
		}
	}
	return nil
}

// PEVersion extracts version string from PE version info resource.
// Returns formatted version (e.g. "10.0.22621.1") or "0.0.0.0".
func PEVersion(peData []byte, rsrcOffset uint32, rsrcSize uint32, sections []wcorefxpe.ImageSectionHeader) string {
	entries, err := GetResourceByTypeID(peData, rsrcOffset, rsrcSize, sections, 16) // RT_VERSION
	if err != nil || len(entries) == 0 {
		return "0.0.0.0"
	}

	for _, dataEntries := range entries {
		for _, de := range dataEntries {
			fixed := findFixedFileInfo(de.Data)
			if fixed != nil {
				ms := fixed.FileVersionMS
				ls := fixed.FileVersionLS
				return fmt.Sprintf("%d.%d.%d.%d",
					ms>>16, ms&0xFFFF, ls>>16, ls&0xFFFF)
			}
		}
	}

	return "0.0.0.0"
}
