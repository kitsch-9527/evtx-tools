package pe

import (
	"os"
	"testing"

	"github.com/kitsch-9527/msgdb-dump/crim"
)

// TestExtractD2StringsFromMUI verifies that the 0xD2 string table is extracted
// from a real MUI RT_MESSAGETABLE resource (Kernel-Power).
//
// The 0xD2-encoded VMAP string references (high 16 bits 0xD200, low 16 bits
// index) resolve against this table. Without it, ValueMaps stays empty.
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

	// Spot-check well-known indices (from the MUI message table block 0xD2000001..0xD2000142)
	checks := map[uint32]string{
		1:   "Button or Lid",
		19:  "Power Action Sleep",
		27:  "Unknown",
		41:  "Sleep Button",
		42:  "Lid",
		172: "Unknown",
		173: "PowerButton",
		174: "SleepButton",
		287: "Active",
		288: "Modern Standby",
		293: "Sleeping (Idle)",
		322: "System initiated boot (display ignored)",
	}
	for idx, want := range checks {
		got, ok := combined.D2Strings[idx]
		if !ok {
			// value may be checked loosely below; flag as error
			t.Errorf("D2Strings[%d] missing", idx)
			continue
		}
		if got != want {
			t.Errorf("D2Strings[%d] = %q, want %q", idx, got, want)
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
