// crim-probe-intype — R10 前置取证：dump 指定 provider 每个事件的参数
// InType/OutType（TDH 枚举字节，TEMP 描述符 +4 字段）。
// 用法: crim-probe-intype <dll-path> <provider-guid-with-braces> [eid ...]
// 无 eid 过滤时打印全部事件。只读，不写库。
package main

import (
	"fmt"
	"os"
	"sort"
	"strconv"

	"github.com/kitsch-9527/msgdb-dump/crim"
	"github.com/kitsch-9527/msgdb-dump/pe"
)

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "usage: crim-probe-intype <dll> <guid> [eid ...]")
		os.Exit(2)
	}
	dll, guid := os.Args[1], os.Args[2]
	eidFilter := map[uint32]bool{}
	for _, a := range os.Args[3:] {
		v, err := strconv.ParseUint(a, 10, 32)
		if err == nil {
			eidFilter[uint32(v)] = true
		}
	}

	combined, err := pe.ExtractAllFromPE(dll)
	if err != nil {
		fmt.Fprintln(os.Stderr, "extract:", err)
		os.Exit(1)
	}

	fmt.Printf("[probe] D-strings in file: %d\n", len(combined.D2Strings))

	// -mode=strings: dump all D-string-table entries (works on any PE, incl. MUI)
	if os.Getenv("DUMP_STRINGS") != "" {
		ids := make([]int, 0, len(combined.D2Strings))
		for id := range combined.D2Strings {
			ids = append(ids, int(id))
		}
		sort.Ints(ids)
		for _, id := range ids {
			fmt.Printf("%#010x = %q\n", id, combined.D2Strings[uint32(id)])
		}
		return
	}

	pd := findPD(combined.CrimBlobs, guid)
	if pd2 := findPDWithStrings(combined.CrimBlobs, guid, combined.D2Strings); pd2 != nil {
		pd = pd2
		fmt.Printf("[probe] re-parsed with D-strings: VMAP maps=%d BMAP maps=%d\n", len(pd.ValueMaps), len(pd.BitMaps))
	}
	if pd == nil {
		fmt.Fprintln(os.Stderr, "provider not found in CRIM blobs:", guid)
		os.Exit(1)
	}

	// -mode=alltpl: dump every template containing param "TimeDeltaInMs"
	if os.Getenv("ALL_TPL") != "" {
		dumpAllTemplatesWithParam(combined.CrimBlobs, guid, "TimeDeltaInMs", combined.D2Strings)
		return
	}

	// -dump-map <hexoff>: hexdump a map sub-block region instead of templates
	if dumpHex := os.Getenv("DUMP_MAP_OFF"); dumpHex != "" {
		off, err := strconv.ParseUint(dumpHex, 0, 32)
		if err == nil && int(off) < len(combined.CrimBlobs[0]) {
			b := combined.CrimBlobs[0][off:]
			end := 128
			if end > len(b) {
				end = len(b)
			}
			fmt.Printf("blob0 @ %#x (sig %q):\n", off, string(b[0:4]))
			for i := 0; i+16 <= end; i += 16 {
				row := ""
				for _, c := range b[i : i+16] {
					if c >= 0x20 && c < 0x7f {
						row += string(rune(c))
					} else {
						row += "."
					}
				}
				fmt.Printf("  +%03d: % x  |%s|\n", i, b[i:i+16], row)
			}
		}
		return
	}

	// eid -> ALL distinct template offsets (variants may use different templates)
	tplByEid := map[uint32][]*uint32{}
	for i := range pd.Events {
		ev := pd.Events[i]
		if ev.TemplateOffset == nil {
			continue
		}
		eid := uint32(ev.EventID)
		tplByEid[eid] = append(tplByEid[eid], ev.TemplateOffset)
	}

	eids := make([]int, 0, len(tplByEid))
	for eid := range tplByEid {
		if len(eidFilter) == 0 || eidFilter[eid] {
			eids = append(eids, int(eid))
		}
	}
	sort.Ints(eids)
	for _, eid := range eids {
		seenTpl := map[uint32]bool{}
		for _, tpl := range tplByEid[uint32(eid)] {
			if seenTpl[*tpl] {
				continue
			}
			seenTpl[*tpl] = true
			params, ok := pd.TemplateParams[*tpl]
			if !ok {
				continue
			}
			fmt.Printf("EID %d (template@%#x):\n", eid, *tpl)
			for _, p := range params {
				line := fmt.Sprintf("  [%d] %-30s in=0x%02X out=0x%02X mapRef=%#x",
					p.Index, p.Name, p.InType, p.OutType, p.MapRef)
			if p.MapRef != 0 {
				if info, ok := pd.MapRefsByOffset[p.MapRef]; ok {
					line += fmt.Sprintf(" -> map %q bitmap=%v", info.Name, info.IsBitmap)
					vm, hasVm := pd.ValueMaps[info.Name]
					bm, hasBm := pd.BitMaps[info.Name]
					if hasVm {
						line += fmt.Sprintf(" VMAP(%d):", len(vm))
						vals := make([]int, 0, len(vm))
						for v := range vm {
							vals = append(vals, int(v))
						}
						sort.Ints(vals)
						for _, v := range vals {
							line += fmt.Sprintf(" %d=%q", v, vm[uint32(v)])
						}
					} else if hasBm {
						line += fmt.Sprintf(" BMAP(%d)", len(bm))
					} else {
						line += " [NO ENTRIES in ProviderData]"
					}
				} else {
					line += " [mapRef unresolved]"
				}
			}
			fmt.Println(line)
		}
		}
	}
}

// dumpAllTemplatesWithParam walks every blob, every template, and prints any
// template containing the named parameter — with per-param mapRef resolution.
func dumpAllTemplatesWithParam(blobs [][]byte, guid, paramName string, strTable map[uint32]string) {
	for bi, b := range blobs {
		pd := crim.ParseProviderDataWithStrings(b, guid, strTable)
		if pd == nil || pd.TemplateParams == nil {
			continue
		}
		for off, params := range pd.TemplateParams {
			has := false
			for _, p := range params {
				if p.Name == paramName {
					has = true
					break
				}
			}
			if !has {
				continue
			}
			fmt.Printf("blob[%d] template@%#x:\n", bi, off)
			for _, p := range params {
				line := fmt.Sprintf("  [%d] %-28s in=0x%02X out=0x%02X mapRef=%#x",
					p.Index, p.Name, p.InType, p.OutType, p.MapRef)
				if p.MapRef != 0 {
					if info, ok := pd.MapRefsByOffset[p.MapRef]; ok {
						line += fmt.Sprintf(" -> %q", info.Name)
						if vm, ok2 := pd.ValueMaps[info.Name]; ok2 {
							line += fmt.Sprintf(" VMAP(%d)", len(vm))
						} else {
							line += " [NO ENTRIES]"
						}
					} else {
						line += " [unresolved]"
					}
				}
				fmt.Println(line)
			}
		}
	}
}

func findPD(blobs [][]byte, guid string) *crim.ProviderData {
	for _, b := range blobs {
		if pd := crim.ParseProviderDataWithStrings(b, guid, nil); pd != nil && len(pd.Events) > 0 {
			return pd
		}
	}
	return nil
}

// findPDWithStrings re-parses with the D2 string table so MAPS sub-block
// entries (0xD2-encoded refs) resolve — mirrors the production pipeline.
func findPDWithStrings(blobs [][]byte, guid string, d2 map[uint32]string) *crim.ProviderData {
	for _, b := range blobs {
		if pd := crim.ParseProviderDataWithStrings(b, guid, d2); pd != nil && len(pd.Events) > 0 {
			return pd
		}
	}
	return nil
}
