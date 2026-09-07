package crim

import (
	"fmt"
	"strings"
)

// NormalizeGUID converts a registry-style GUID to CRIM-matching format.
// Strips curly braces and uppercases hex digits.
//
// Input:  "{f1ef270a-0d32-4352-ba52-dbab41e1d859}"
// Output: "F1EF270A-0D32-4352-BA52-DBAB41E1D859"
func NormalizeGUID(guid string) string {
	s := strings.Trim(guid, "{}")
	return strings.ToUpper(s)
}

// GUIDBytesToString converts a 16-byte CRIM binary GUID to
// "XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX" (uppercase, no braces).
//
// CRIM binary layout (all LE):
//
//	+0:4  D1 (u32 little-endian)
//	+4:2  D2 (u16 little-endian)
//	+6:2  D3 (u16 little-endian)
//	+8:8  D4..D5 (raw bytes)
func GUIDBytesToString(b [16]byte) string {
	d1 := uint32(b[0]) | uint32(b[1])<<8 | uint32(b[2])<<16 | uint32(b[3])<<24
	d2 := uint16(b[4]) | uint16(b[5])<<8
	d3 := uint16(b[6]) | uint16(b[7])<<8
	return fmt.Sprintf("%08X-%04X-%04X-%02X%02X-%02X%02X%02X%02X%02X%02X",
		d1, d2, d3, b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15])
}
