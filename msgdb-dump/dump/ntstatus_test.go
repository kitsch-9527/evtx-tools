package dump

import "testing"

// 冒烟测试：本机 ntdll 必须含 0xC000000E（阶段 E 的靶点，缺表即失败——
// dump 在目标机上运行，机器相关是本仓库的既定口径）。
func TestCollectNtstatusMessages(t *testing.T) {
	msgs, err := collectNtstatusMessages()
	if err != nil {
		t.Fatalf("collectNtstatusMessages: %v", err)
	}
	const statusDeviceDoesNotExist = 0xC000000E
	got, ok := msgs[statusDeviceDoesNotExist]
	if !ok {
		t.Fatalf("0xC000000E missing from ntdll message table (%d entries)", len(msgs))
	}
	if got != "A device which does not exist was specified." {
		t.Fatalf("0xC000000E text = %q", got)
	}
}
