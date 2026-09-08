package pe

import (
	"os"
	"testing"

	"github.com/kitsch-9527/msgdb-dump/crim"
)

// TestExtractD2StringsFromMUI verifies that the 0xD2 string table is extracted
// from a real MUI RT_MESSAGETABLE resource (Kernel-Power).
//
// The 0xD-encoded VMAP string references (full message ID 0xD200xxxx) resolve
// against this table, keyed by the full ID. Without it, ValueMaps stays empty.
func TestExtractD2StringsFromMUI(t *testing.T) {
	const muiPath = `C:\WINDOWS\System32\en-US\microsoft-windows-kernel-power-events.dll.mui`
	if _, err := os.Stat(muiPath); err != nil {
		t.Skipf("MUI not present: %v", err)
	}

	combined, err := ExtractAllFromPE(muiPath)
	if err != nil {
		t.Fatalf("ExtractAllFromPE(%s): %v", muiPath, err)
	}

	if len(combined.D2Strings) < 300 {
		t.Fatalf("D2Strings = %d entries, want >= 300 (0xD2 string table)", len(combined.D2Strings))
	}

	// Spot-check well-known full IDs (MUI message table block 0xD2000001..)
	checks := map[uint32]string{
		0xD2000001: "Button or Lid",
		0xD2000013: "Power Action Sleep",
		0xD200001B: "Unknown",
		0xD2000029: "Sleep Button",
		0xD200002A: "Lid",
		0xD20000AC: "Unknown",
		0xD20000AD: "PowerButton",
		0xD20000AE: "SleepButton",
		0xD200011F: "Active",
		0xD2000120: "Modern Standby",
		0xD2000125: "Sleeping (Idle)",
		0xD2000142: "System initiated boot (display ignored)",
	}
	for id, want := range checks {
		got, ok := combined.D2Strings[id]
		if !ok {
			t.Errorf("D2Strings[%#010x] missing", id)
			continue
		}
		if got != want {
			t.Errorf("D2Strings[%#010x] = %q, want %q", id, got, want)
		}
	}
}

// TestExtractD0D1StringFamilies verifies the other observed string-table
// families (batch 1 residual-zeroing): Ntfs carries 0xD1 (VolumeCorruption-
// ActionState value 0 = "is healthy."), Kernel-Processor-Power carries 0xD0
// (MapIdleImplementation value 1 = "ACPI Idle (C) States") in its MUI.
func TestExtractD0D1StringFamilies(t *testing.T) {
	cases := []struct {
		path string
		min  int
		id   uint32
		want string
	}{
		{
			path: `C:\WINDOWS\system32\drivers\ntfs.sys`,
			min:  100,
			id:   0xD1000001,
			want: "is healthy.  No action is needed.",
		},
		{
			// KPP main DLL carries NO message table (2026-09-08 probe: 0
			// D-strings) — the 0xD0 table lives in the MUI only.
			path: `C:\Windows\System32\en-US\microsoft-windows-kernel-processor-power-events.dll.mui`,
			min:  30,
			id:   0xD0000007,
			want: "ACPI Idle (C) States",
		},
	}
	for _, c := range cases {
		if _, err := os.Stat(c.path); err != nil {
			t.Skipf("%s not present: %v", c.path, err)
		}
		combined, err := ExtractAllFromPE(c.path)
		if err != nil {
			t.Fatalf("ExtractAllFromPE(%s): %v", c.path, err)
		}
		if c.min > 0 && len(combined.D2Strings) < c.min {
			t.Errorf("%s: D2Strings = %d, want >= %d", c.path, len(combined.D2Strings), c.min)
		}
		if got, ok := combined.D2Strings[c.id]; !ok {
			t.Errorf("%s: D2Strings[%#010x] missing", c.path, c.id)
		} else if got != c.want {
			t.Errorf("%s: D2Strings[%#010x] = %q, want %q", c.path, c.id, got, c.want)
		}
	}
}

// TestParseVMAPWithD2StringTable verifies end-to-end: CRIM VMAP references
// resolve to display names once the MUI D2 string table is supplied.
func TestParseVMAPWithD2StringTable(t *testing.T) {
	const (
		dllPath = `C:\WINDOWS\System32\microsoft-windows-kernel-power-events.dll`
		muiPath = `C:\WINDOWS\System32\en-US\microsoft-windows-kernel-power-events.dll.mui`
	)
	if _, err := os.Stat(dllPath); err != nil {
		t.Skipf("DLL not present: %v", err)
	}
	if _, err := os.Stat(muiPath); err != nil {
		t.Skipf("MUI not present: %v", err)
	}

	muiC, err := ExtractAllFromPE(muiPath)
	if err != nil {
		t.Fatalf("MUI extract: %v", err)
	}
	if len(muiC.D2Strings) == 0 {
		t.Fatal("no D2 strings extracted from MUI")
	}

	mainC, err := ExtractAllFromPE(dllPath)
	if err != nil {
		t.Fatalf("DLL extract: %v", err)
	}
	if len(mainC.CrimBlobs) == 0 {
		t.Fatal("no CRIM blobs in DLL")
	}

	guid := "{331C3B3A-2005-44C2-AC5E-77220C37D6B4}"
	var pd *crim.ProviderData
	for _, blob := range mainC.CrimBlobs {
		pd = crim.ParseProviderDataWithStrings(blob, guid, muiC.D2Strings)
		if pd != nil && len(pd.ValueMaps) > 0 {
			break
		}
	}
	if pd == nil {
		t.Fatal("ParseProviderDataWithStrings returned nil")
	}
	if len(pd.ValueMaps) == 0 {
		t.Fatal("ValueMaps empty — 0xD2 references not resolved")
	}
	if len(pd.ValueMaps) < 40 {
		t.Fatalf("ValueMaps = %d maps, want >= 40 (Kernel-Power has 45)", len(pd.ValueMaps))
	}

	// AdaptiveBootTargetState: values 0..6
	boot, ok := pd.ValueMaps["Pop:AdaptiveBootTargetState"]
	if !ok {
		t.Fatal("missing Pop:AdaptiveBootTargetState map")
	}
	if boot[0] == "" || boot[6] == "" {
		t.Errorf("AdaptiveBootTargetState incomplete: %v", boot)
	}
	// MapPowerAction: value 2 = "Power Action Sleep"
	if pa, ok := pd.ValueMaps["Pop:MapPowerAction"]; ok {
		if pa[2] != "Power Action Sleep" {
			t.Errorf("MapPowerAction[2] = %q, want %q", pa[2], "Power Action Sleep")
		}
	} else {
		t.Error("missing Pop:MapPowerAction map")
	}
}

// TestParseVMAPWithD0D1Families — end-to-end for the two new families:
// Ntfs (0xD1, main DLL carries the table) and Kernel-Processor-Power
// (0xD0, table lives in the MUI — the pipeline must merge it in).
func TestParseVMAPWithD0D1Families(t *testing.T) {
	type tc struct {
		name     string
		dllPath  string
		muiPath  string
		guid     string
		eid      uint32
		mapName  string
		val      uint32
		wantText string
	}
	cases := []tc{
		{
			name: "ntfs-98", dllPath: `C:\WINDOWS\system32\drivers\ntfs.sys`,
			guid: "{3ff37a1c-a68d-4d6e-8c9b-f79e8b16c482}", eid: 98,
			mapName: "VolumeCorruptionActionState", val: 0,
			wantText: "is healthy.  No action is needed.",
		},
		{
			name: "kpp-55", dllPath: `C:\WINDOWS\system32\microsoft-windows-kernel-processor-power-events.dll`,
			muiPath: `C:\Windows\System32\en-US\microsoft-windows-kernel-processor-power-events.dll.mui`,
			guid:    "{0f67e49f-fe51-4e9f-b490-6f2948cc6027}", eid: 55,
			mapName: "MapIdleImplementation", val: 1,
			wantText: "ACPI Idle (C) States",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if _, err := os.Stat(c.dllPath); err != nil {
				t.Skipf("DLL not present: %v", err)
			}
			mainC, err := ExtractAllFromPE(c.dllPath)
			if err != nil {
				t.Fatalf("DLL extract: %v", err)
			}
			strs := map[uint32]string{}
			for k, v := range mainC.D2Strings {
				strs[k] = v
			}
			if c.muiPath != "" {
				if _, err := os.Stat(c.muiPath); err == nil {
					if muiC, err := ExtractAllFromPE(c.muiPath); err == nil {
						for k, v := range muiC.D2Strings {
							if _, exists := strs[k]; !exists {
								strs[k] = v
							}
						}
					}
				}
			}

			var pd *crim.ProviderData
			for _, blob := range mainC.CrimBlobs {
				if p := crim.ParseProviderDataWithStrings(blob, c.guid, strs); p != nil && len(p.Events) > 0 {
					pd = p
					break
				}
			}
			if pd == nil {
				t.Fatal("provider data not found")
			}
			vm, ok := pd.ValueMaps[c.mapName]
			if !ok {
				t.Fatalf("map %q missing from ValueMaps (have %d maps)", c.mapName, len(pd.ValueMaps))
			}
			if got := vm[c.val]; got != c.wantText {
				t.Errorf("%s[%d] = %q, want %q", c.mapName, c.val, got, c.wantText)
			}
		})
	}
}
