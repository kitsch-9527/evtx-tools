package dump

import (
	"testing"

	"github.com/kitsch-9527/msgdb-dump/crim"
)

// R9（2026-09-04）：Kernel-Power 566 param2 Reason → Pop:PowerStateEvent 同构回归。
// CRIM 源头 MapRef=3728 存在（探针实证），但 enrichFromCRIM（主分支）不收集 ParamMapRefs，
// 只在 CRIM+MUI fallback 收集，导致 param_value_maps 缺 (566, 2) 链接，wevtapi 展开
// SessionUnlock 而我们输出裸 12。collectParamMapRefs 必须从 TemplateParams+MapRefsByOffset
// 还原出该链接。
func TestCollectParamMapRefs_566Reason(t *testing.T) {
	pd := &crim.ProviderData{
		TemplateParams: map[uint32][]crim.TTBLParam{
			0x100: {
				{Index: 1, Name: "BootId", MapRef: 0},
				{Index: 2, Name: "Reason", MapRef: 3728},
				{Index: 4, Name: "PreviousSessionType", MapRef: 4832},
			},
		},
		MapRefsByOffset: map[uint32]crim.MapRefInfo{
			3728: {Name: "Pop:PowerStateEvent", IsBitmap: false},
			4832: {Name: "Pop:SleepstudySessionType", IsBitmap: false},
		},
	}
	events := []crim.EventEntry{{EventID: 566, TemplateOffset: u32ptr(0x100)}}
	got := collectParamMapRefs(pd, "Microsoft-Windows-Kernel-Power", events)
	want := map[string]string{"2": "Microsoft-Windows-Kernel-Power.Pop:PowerStateEvent", "4": "Microsoft-Windows-Kernel-Power.Pop:SleepstudySessionType"}
	if len(got) != len(want) {
		t.Fatalf("got %d refs %+v, want %d", len(got), got, len(want))
	}
	for _, r := range got {
		if want[r.ParamIndex] != r.MapName || r.EventID != 566 {
			t.Errorf("ref %+v mismatch, want %v", r, want)
		}
	}
}

func u32ptr(v uint32) *uint32 { return &v }
