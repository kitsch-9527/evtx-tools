package dump

import (
	"testing"
)

// R21（2026-09-29）：经典源 event_id 映射。经典 EventLog 约定下，事件记录携带的
// EventID = 消息表 MessageID & 0xFFFF（severity/facility 在高位：Ntfs 50 的表项
// 是 0x80010032，disk 51 / hcmon 0 / EventLog 6013(0x8000177D) 同理）。此前经典
// 路径把原始全格式 ID 直接存成 event_id，渲染端按事件 eid 查询永远 miss——
// 经典源 B 簇（System B926 / App B189）的总根因。
func TestClassicVariantRowsEIDMapping(t *testing.T) {
	prov := ProviderInfo{Name: "Ntfs"} // guid=nil → classic
	rows := classicVariantRows(prov, "en-US", []PeMessage{
		{EventID: 0x80010032, Message: "{Delayed Write Failed} %2"},
		{EventID: 0x8000177D, Message: "The system uptime is %5 seconds."},
		{EventID: 55, Message: "small id stays"},
	})
	check := func(eid uint32, wantMsg string) {
		t.Helper()
		for _, r := range rows {
			if r.EventID == eid {
				if r.Message != wantMsg {
					t.Errorf("eid=%d message = %q, want %q", eid, r.Message, wantMsg)
				}
				if r.MessageID != 0 {
					t.Errorf("eid=%d message_identifier = %d, want 0 (wildcard per 6013 contract)", eid, r.MessageID)
				}
				return
			}
		}
		t.Errorf("no variant row for eid=%d (classic mapping missing)", eid)
	}
	check(50, "{Delayed Write Failed} %2")
	check(6013, "The system uptime is %5 seconds.")
	check(55, "small id stays")
	for _, r := range rows {
		if r.EventID > 0xFFFF {
			t.Errorf("raw full-format event_id %d leaked (want &0xFFFF mapping)", r.EventID)
		}
	}
}

func TestClassicVariantRowsCollisionDeterministic(t *testing.T) {
	// 同 eid 多原始 ID：升序首个胜出（0x40000032 < 0x80010032 → info 变体），
	// 且输入顺序无关（helper 内部先排序）。
	prov := ProviderInfo{Name: "X"}
	fwd := classicVariantRows(prov, "en-US", []PeMessage{
		{EventID: 0x40000032, Message: "info variant"},
		{EventID: 0x80010032, Message: "warning variant"},
	})
	rev := classicVariantRows(prov, "en-US", []PeMessage{
		{EventID: 0x80010032, Message: "warning variant"},
		{EventID: 0x40000032, Message: "info variant"},
	})
	if len(fwd) != 1 || len(rev) != 1 {
		t.Fatalf("collision should dedup to one row: fwd=%d rev=%d", len(fwd), len(rev))
	}
	if fwd[0].Message != "info variant" || rev[0].Message != "info variant" {
		t.Errorf("first-wins by ascending raw ID: fwd=%q rev=%q", fwd[0].Message, rev[0].Message)
	}
}
