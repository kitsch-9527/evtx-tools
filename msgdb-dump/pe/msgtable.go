package pe

import (
	"bytes"
	"encoding/binary"
	"unicode/utf16"
	"unicode/utf8"

	"golang.org/x/text/encoding/simplifiedchinese"
	"golang.org/x/text/transform"
)

// decodeANSIText decodes ANSI-encoded message text to a Go string.
//
// Message table entries with flags&1==0 use the system ANSI code page
// (typically GBK on Chinese Windows, Windows-1252 on Western systems).
// Since the DLL's message tables are compiled for a specific locale,
// this function tries common encodings in order:
//
//  1. Valid UTF-8 → use as-is
//  2. GBK (simplified Chinese) → decode via golang.org/x/text
//  3. Fallback → raw bytes as string (original behavior)
func decodeANSIText(data []byte) string {
	if len(data) == 0 {
		return ""
	}
	// 1. Valid UTF-8 — no conversion needed
	if utf8.Valid(data) {
		return string(data)
	}
	// 2. Try GBK (common for Chinese Windows message DLLs)
	decoded, err := decodeGBK(data)
	if err == nil && utf8.Valid([]byte(decoded)) {
		return decoded
	}
	// 3. Fallback: raw bytes as string
	return string(data)
}

// decodeGBK decodes GBK-encoded bytes to UTF-8 using golang.org/x/text.
func decodeGBK(data []byte) (string, error) {
	reader := transform.NewReader(bytes.NewReader(data), simplifiedchinese.GBK.NewDecoder())
	var out bytes.Buffer
	if _, err := out.ReadFrom(reader); err != nil {
		return "", err
	}
	return out.String(), nil
}

// Message resource constants.
const (
	messageResourceUnicode = 0x0001
	maxMsgBlocks          = 1024
)

// ParseMessageTable parses MESSAGE_RESOURCE_DATA from raw resource bytes.
// Returns a map of event_id → message template (still has %1, %2 placeholders)
// or nil if the data is invalid.
func ParseMessageTable(data []byte) map[uint32]string {
	if len(data) < 4 {
		return nil
	}

	numberOfBlocks := binary.LittleEndian.Uint32(data[0:4])
	if numberOfBlocks == 0 || numberOfBlocks > maxMsgBlocks {
		return nil
	}

	messages := make(map[uint32]string)
	offset := uint32(4)

	for blockIdx := uint32(0); blockIdx < numberOfBlocks; blockIdx++ {
		// Each MESSAGE_RESOURCE_BLOCK is 12 bytes: LowId(4) + HighId(4) + OffsetToEntries(4)
		if uint64(offset)+12 > uint64(len(data)) {
			break
		}

		lowID := binary.LittleEndian.Uint32(data[offset : offset+4])
		highID := binary.LittleEndian.Uint32(data[offset+4 : offset+8])
		entriesOffset := binary.LittleEndian.Uint32(data[offset+8 : offset+12])
		offset += 12

		// Validate block
		if lowID > highID {
			continue
		}
		if uint64(entriesOffset) >= uint64(len(data)) {
			continue
		}

		// Parse entries within this block
		entryPos := entriesOffset

		for eventID := lowID; eventID <= highID; eventID++ {
			if uint64(entryPos)+4 > uint64(len(data)) {
				break
			}

			entryLength := binary.LittleEndian.Uint16(data[entryPos : entryPos+2])
			if entryLength < 4 {
				break // Malformed or end of block
			}

			flags := binary.LittleEndian.Uint16(data[entryPos+2 : entryPos+4])

			// Text starts after the 4-byte header
			textOffset := entryPos + 4
			textLen := uint32(entryLength) - 4

			var message string
			if textLen > 0 && uint64(textOffset)+uint64(textLen) <= uint64(len(data)) {
				textBytes := data[textOffset : textOffset+textLen]
				if flags&messageResourceUnicode != 0 {
					// UTF-16LE encoding
					wordLen := textLen / 2
					words := make([]uint16, wordLen)
					for i := uint32(0); i < wordLen; i++ {
						words[i] = binary.LittleEndian.Uint16(textBytes[i*2:])
					}
					nullEnd := len(words)
					for i, c := range words {
						if c == 0 {
							nullEnd = i
							break
						}
					}
					message = string(utf16.Decode(words[:nullEnd]))
				} else {
					// ANSI encoding (GBK/Windows-1252 - decoded via decodeANSIText)
					nullEnd := len(textBytes)
					for i, b := range textBytes {
						if b == 0 {
							nullEnd = i
							break
						}
					}
					message = decodeANSIText(textBytes[:nullEnd])
				}
			}

			if message != "" {
				messages[eventID] = message
			}

			// Move to next entry (each entry is padded to 4-byte boundary)
			paddedLength := (uint32(entryLength) + 3) & ^uint32(3)
			entryPos += paddedLength
		}
	}

	if len(messages) == 0 {
		return nil
	}
	return messages
}
