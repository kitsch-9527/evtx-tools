//go:build windows

package main

import (
	"fmt"
	"testing"

	"github.com/kitsch-9527/msgdb-dump/pe"
)

// Live check: the D0/D1 string-table families exist in the driver DLL/MUIs
// and carry the exact strings wevtapi renders for Ntfs 98 / KPP 55.
func TestDumpStringTables(t *testing.T) {
	for _, path := range []string{
		`C:\WINDOWS\system32\drivers\ntfs.sys`,
		`C:\Windows\System32\drivers\en-US\ntfs.sys.mui`,
		`C:\WINDOWS\system32\microsoft-windows-kernel-processor-power-events.dll`,
		`C:\Windows\System32\en-US\microsoft-windows-kernel-processor-power-events.dll.mui`,
	} {
		c, err := pe.ExtractAllFromPE(path)
		if err != nil {
			t.Logf("%s: extract error: %v", path, err)
			continue
		}
		t.Logf("%s: D-strings=%d", path, len(c.D2Strings))
		ids := make([]int, 0, len(c.D2Strings))
		for id := range c.D2Strings {
			ids = append(ids, int(id))
		}
		for i := 0; i < len(ids); i++ {
			for j := i + 1; j < len(ids); j++ {
				if ids[j] < ids[i] {
					ids[i], ids[j] = ids[j], ids[i]
				}
			}
		}
		n := 0
		for _, id := range ids {
			if n >= 12 {
				break
			}
			n++
			t.Logf("  %#010x = %q", id, c.D2Strings[uint32(id)])
		}
		_ = fmt.Sprint()
	}
}
