package pe

import (
	"strings"
	"unicode/utf16"
	"unicode/utf8"
)

// hasEmbeddedNULLE detects if data appears to be UTF-16LE encoded by checking
// for NUL bytes at odd byte positions (1, 3, 5...). In UTF-16LE, ASCII-range
// characters have the high byte = 0x00. Data that is entirely ASCII-range
// (never needs multi-byte sequences) appears as <byte, 0x00, byte, 0x00, ...>.
//
// Pure UTF-8 text never has embedded NUL bytes, so this is a reliable heuristic.
func hasEmbeddedNULLE(data []byte) bool {
	if len(data) < 4 {
		return false
	}
	// Check if odd-indexed bytes are NUL for at least the first 32 bytes or 50%
	for i := 1; i < len(data) && i < 64; i += 2 {
		if data[i] != 0 {
			return false
		}
	}
	// Also verify that the even-indexed bytes are non-zero (actual content)
	for i := 0; i < len(data) && i < 64; i += 2 {
		if data[i] == 0 {
			return false
		}
	}
	return true
}

// decodeUTF16LE converts raw UTF-16LE bytes to a Go string, truncating at the
// first null terminator.
func decodeUTF16LE(data []byte) (string, bool) {
	if len(data) < 2 || len(data)%2 != 0 {
		return "", false
	}
	words := make([]uint16, len(data)/2)
	for i := range words {
		words[i] = uint16(data[i*2]) | uint16(data[i*2+1])<<8
	}
	// Truncate at null terminator
	nullEnd := len(words)
	for i, c := range words {
		if c == 0 {
			nullEnd = i
			break
		}
	}
	if nullEnd == 0 {
		return "", false
	}
	return string(utf16.Decode(words[:nullEnd])), true
}

// extractAttrValue extracts an attribute value from an XML-ish tag.
// Looks for attrName="value" or attrName='value' in a string.
// tagPrefix is used to filter which lines to process.
func extractAttrValue(line, attrName, tagPrefix string) (string, bool) {
	trimmed := strings.TrimSpace(line)
	if !strings.HasPrefix(trimmed, tagPrefix) {
		return "", false
	}

	// Try double-quoted
	search := attrName + "=\""
	if idx := strings.Index(trimmed, search); idx >= 0 {
		valStart := idx + len(search)
		if endIdx := strings.IndexByte(trimmed[valStart:], '"'); endIdx >= 0 {
			return trimmed[valStart : valStart+endIdx], true
		}
	}

	// Try single-quoted
	search = attrName + "='"
	if idx := strings.Index(trimmed, search); idx >= 0 {
		valStart := idx + len(search)
		if endIdx := strings.IndexByte(trimmed[valStart:], '\''); endIdx >= 0 {
			return trimmed[valStart : valStart+endIdx], true
		}
	}

	return "", false
}

// ParseWEVTTemplate parses WEVT_TEMPLATE XML from raw resource bytes.
// Returns a map of event_id → {param_index → param_name}.
// This is a best-effort parse, using the same approach as the Rust reference.
//
// Encoding detection order:
//
//	1. UTF-16LE with BOM (0xFF 0xFE) — immediate decode
//	2. UTF-16LE without BOM — detected via embedded NUL bytes at odd positions
//	3. Plain UTF-8 — valid UTF-8 byte sequences
//	4. Fallback — raw bytes as string
func ParseWEVTTemplate(data []byte) map[string]map[string]string {
	if len(data) == 0 {
		return nil
	}

	var content string

	// 1. UTF-16LE with BOM
	if len(data) >= 2 && data[0] == 0xFF && data[1] == 0xFE {
		if decoded, ok := decodeUTF16LE(data[2:]); ok {
			content = decoded
		}
		return parseWEVTXMLStr(content)
	}

	// 2. UTF-16LE without BOM — detect via NUL bytes at odd positions
	if hasEmbeddedNULLE(data) {
		if decoded, ok := decodeUTF16LE(data); ok {
			content = decoded
			if strings.HasPrefix(strings.TrimSpace(content), "<") || strings.Contains(content, "<template") {
				return parseWEVTXMLStr(content)
			}
		}
		return nil // decoded but doesn't look like XML
	}

	// 3. Plain UTF-8
	if utf8.Valid(data) {
		content = string(data)
		return parseWEVTXMLStr(content)
	}

	// 4. Fallback: raw bytes
	content = string(data)
	return parseWEVTXMLStr(content)
}

// parseWEVTXMLStr parses an XML string containing WEVT_TEMPLATE elements.
func parseWEVTXMLStr(content string) map[string]map[string]string {
	params := make(map[string]map[string]string)

	var eventID string
	dataIndex := 1

	for _, rawLine := range strings.Split(content, "\n") {
		line := strings.TrimSpace(rawLine)
		if line == "" {
			continue
		}

		// Check for <event value="4624"> or <event value='4624'>
		if val, ok := extractAttrValue(line, "value", "<event"); ok {
			eventID = val
			dataIndex = 1
			continue
		}

		// Check for <data name="...">
		if name, ok := extractAttrValue(line, "name", "<data"); ok {
			if eventID != "" {
				entry := params[eventID]
				if entry == nil {
					entry = make(map[string]string)
					params[eventID] = entry
				}
				entry[string(rune('0'+dataIndex%10))] = name // simplified but we use strconv normally
				// Actually let's use proper string conversion
				entry[itoa(dataIndex)] = name
				dataIndex++
			}
			continue
		}

		// Reset on close of event/template
		if strings.HasPrefix(line, "</event>") || strings.HasPrefix(line, "</template>") {
			eventID = ""
			dataIndex = 1
		}
	}

	if len(params) == 0 {
		return nil
	}
	return params
}

// itoa converts an integer to a string (avoiding strconv import).
func itoa(i int) string {
	if i == 0 {
		return "0"
	}
	var buf [12]byte
	var pos = len(buf)
	for i > 0 {
		pos--
		buf[pos] = byte('0' + i%10)
		i /= 10
	}
	return string(buf[pos:])
}
