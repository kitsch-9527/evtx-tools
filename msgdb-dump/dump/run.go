package dump

import (
	"fmt"
	"strings"
)

// DumpArgs holds all CLI arguments for the dump pipeline.
type DumpArgs struct {
	OutDir    string
	Lang      string
	Providers string
	WithWELM  string
	Update    bool
	ImportDLL string
}

// SystemDBFilename generates the system DB filename from OS info.
//
//	"Windows 11 Pro" + "24H2" + "22631.1" -> "hotech-msg-Windows_11_Pro-24H2-22631.1.db"
func SystemDBFilename(info OsInfo) string {
	name := sanitizeFilename(info.ProductName)
	version := sanitizeFilename(info.DisplayVersion)
	build := sanitizeFilename(info.BuildNumber)
	return fmt.Sprintf("hotech-msg-%s-%s-%s.db", name, version, build)
}

// sanitizeFilename replaces characters that are not alphanumeric, '.', or '-'
// with underscores for safe filename generation.
func sanitizeFilename(s string) string {
	var b strings.Builder
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') ||
			(r >= '0' && r <= '9') || r == '.' || r == '-' {
			b.WriteRune(r)
		} else {
			b.WriteRune('_')
		}
	}
	return b.String()
}
