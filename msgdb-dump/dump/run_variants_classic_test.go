package dump

import (
	"testing"
)

// R21（2026-09-29）/R26（2026-09-30）：经典源 event_id 映射 + 全格式 msgid 保留。
// 经典 EventLog 约定下，事件记录携带的 EventID = 消息表 MessageID & 0xFFFF
// （severity/facility 在高位：Ntfs 50 的表项是 0x80010032，disk 51 / hcmon 0 /
// EventLog 6013(0x8000177D) 同理）。此前经典路径把原始全格式 ID 直接存成
// event_id，渲染端按事件 eid 查询永远 miss——经典源 B 簇（System B926 /
// App B189）的总根因。R26 起 message_identifier 保留原始全格式 ID，供渲染端
// 以 (Qualifiers<<16)|eid 精确探针区分同 eid 姊妹条目。
func TestClassicVariantRowsEIDMapping(t *testing.T) {
	prov := ProviderInfo{Name: "Ntfs"} // guid=nil → classic
	rows := classicVariantRows(prov, "en-US", []PeMessage{
		{EventID: 0x80010032, Message: "{Delayed Write Failed} %2"},
		{EventID: 0x8000177D, Message: "The system uptime is %5 seconds."},
		{EventID: 55, Message: "small id stays"},
	})
	check := func(eid uint32, wantMsg string, wantMsgID uint32) {
		t.Helper()
		for _, r := range rows {
			if r.EventID == eid && r.Message == wantMsg {
				if r.MessageID != wantMsgID {
					t.Errorf("eid=%d message_identifier = %d, want %d", eid, r.MessageID, wantMsgID)
				}
				return
			}
		}
		t.Errorf("no variant row for eid=%d msg=%q (classic mapping missing)", eid, wantMsg)
	}
	check(50, "{Delayed Write Failed} %2", 0x80010032)
	check(6013, "The system uptime is %5 seconds.", 0x8000177D)
	check(55, "small id stays", 0)
	for _, r := range rows {
		if r.EventID > 0xFFFF {
			t.Errorf("raw full-format event_id %d leaked (want &0xFFFF mapping)", r.EventID)
		}
	}
}

func TestClassicVariantRowsCollisionDeterministic(t *testing.T) {
	// R26：同 eid 的姊妹条目必须全部保留（各带自己的全格式 message_identifier，
	// PK 不再相撞），且输入顺序无关（helper 内部先排序）。渲染端以 Qualifier
	// 拼全键精确选择。
	prov := ProviderInfo{Name: "X"}
	fwd := classicVariantRows(prov, "en-US", []PeMessage{
		{EventID: 0x40000024, Message: "info variant"},
		{EventID: 0x40080024, Message: "starting variant"},
	})
	rev := classicVariantRows(prov, "en-US", []PeMessage{
		{EventID: 0x40080024, Message: "starting variant"},
		{EventID: 0x40000024, Message: "info variant"},
	})
	if len(fwd) != 2 || len(rev) != 2 {
		t.Fatalf("siblings must be preserved: fwd=%d rev=%d", len(fwd), len(rev))
	}
	if fwd[0].MessageID != 0x40000024 || fwd[1].MessageID != 0x40080024 {
		t.Errorf("fwd msgids = %X,%X", fwd[0].MessageID, fwd[1].MessageID)
	}
	if rev[0].MessageID != 0x40000024 || rev[1].MessageID != 0x40080024 {
		t.Errorf("rev msgids = %X,%X (input order must not matter)", rev[0].MessageID, rev[1].MessageID)
	}
}
