package crim

import "testing"

// Batch 1 (residual-zeroing plan): VMAP entry string refs come in three
// observed table families — 0xD2 (Kernel-Power), 0xD1 (Ntfs
// VolumeCorruptionActionState), 0xD0 (Kernel-Processor-Power). The resolver
// must accept all three, keyed by the full message ID, and reject refs that
// are plain CRIM string offsets.
func TestResolveVMAPStringFamilies(t *testing.T) {
	table := map[uint32]string{
		0xD2000042: "Power Action Sleep",
		0xD1000001: "is healthy.  No action is needed.",
		0xD0000007: "ACPI Idle (C) States",
		0xDC000001: "disabled",
	}
	cases := []struct {
		ref  uint32
		want string
	}{
		{0xD2000042, "Power Action Sleep"},
		{0xD1000001, "is healthy.  No action is needed."},
		{0xD0000007, "ACPI Idle (C) States"},
		{0xDC000001, "disabled"},
		{0xD00000FF, ""},  // family ok, index missing
		{0x0001_0203, ""}, // not a 0xDn ref
		{0xC0000001, ""},  // not a 0xDn ref
	}
	for _, c := range cases {
		if got := resolveVMAPString(c.ref, table); got != c.want {
			t.Errorf("resolveVMAPString(%#010x) = %q, want %q", c.ref, got, c.want)
		}
	}
	if got := resolveVMAPString(0xD1000001, nil); got != "" {
		t.Errorf("nil table must resolve empty, got %q", got)
	}
}

// TestParseMAPSVMAPBlock_D1Family — synthetic MAPS VMAP sub-block using the
// 0xD1 family (Ntfs layout): count=2, entries (0, ref 0xD1000001), (1, ref
// 0xD1000002). Mirrors the live ntfs.sys VolumeCorruptionActionState block.
func TestParseMAPSVMAPBlock_D1Family(t *testing.T) {
	var data []byte
	appendStr := func(s string) uint32 {
		off := uint32(len(data))
		enc := encodeCRIMString(s)
		data = append(data, enc...)
		return off
	}
	// Offset 0 is treated as "no string" by the parser — pad so real strings
	// start at a non-zero offset.
	data = append(data, 0x00)
	nameOff := appendStr("VolumeCorruptionActionState")
	appendStr("is healthy.  No action is needed.")
	appendStr("requires an Online Scan.")

	sub := []byte("VMAP")
	sub = append(sub, le32(0)...)             // size placeholder
	sub = append(sub, le32(nameOff)...)       // +8 name offset
	sub = append(sub, le32(0)...)             // +12 unknown
	sub = append(sub, le32(2)...)             // +16 entry count
	sub = append(sub, le32(0)...)             // entry: value 0
	sub = append(sub, 0x01, 0x00, 0x00, 0xD1) // ref 0xD1000001
	sub = append(sub, le32(1)...)             // entry: value 1
	sub = append(sub, 0x02, 0x00, 0x00, 0xD1) // ref 0xD1000002
	data = append(data, sub...)
	pos := uint32(len(data) - len(sub))

	table := map[uint32]string{
		0xD1000001: "is healthy.  No action is needed.",
		0xD1000002: "requires an Online Scan.",
	}
	name, entries := ParseMAPSVMAPBlock(data, int(pos), table)
	if name != "VolumeCorruptionActionState" {
		t.Fatalf("name = %q", name)
	}
	if entries[0] != "is healthy.  No action is needed." {
		t.Errorf("entries[0] = %q", entries[0])
	}
	if entries[1] != "requires an Online Scan." {
		t.Errorf("entries[1] = %q", entries[1])
	}
}
