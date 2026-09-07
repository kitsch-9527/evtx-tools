package dump

import (
	"testing"
)

// R15（2026-09-04）：SCM 7000/7023/7038 的 %%2/%%8/%%21 需 Win32 系统错误文本。
// systemErrorText 直调 FormatMessage(FROM_SYSTEM)：2/8/21 必须有文本。
func TestSystemErrorText_KnownCodes(t *testing.T) {
	for id, want := range map[uint32]string{
		2:  "cannot find the file",
		8:  "memory resources",
		21: "device is not ready",
	} {
		got := systemErrorText(id, 1033)
		if got == "" {
			t.Errorf("winerror %d: empty text", id)
			continue
		}
		lower := got
		if len(lower) > 200 {
			lower = lower[:200]
		}
		found := false
		for i := 0; i+len(want) <= len(lower); i++ {
			if equalFold(lower[i:i+len(want)], want) {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("winerror %d = %q, want substring %q", id, got, want)
		}
	}
}

func equalFold(a, b string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := 0; i < len(a); i++ {
		ca, cb := a[i], b[i]
		if ca >= 'A' && ca <= 'Z' {
			ca += 'a' - 'A'
		}
		if cb >= 'A' && cb <= 'Z' {
			cb += 'a' - 'A'
		}
		if ca != cb {
			return false
		}
	}
	return true
}
