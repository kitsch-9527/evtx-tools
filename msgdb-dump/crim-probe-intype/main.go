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

	pd := findPD(combined.CrimBlobs, guid)
	if pd == nil {
		fmt.Fprintln(os.Stderr, "provider not found in CRIM blobs:", guid)
		os.Exit(1)
	}

	// eid -> template offset (first occurrence wins; variants share templates)
	tplByEid := map[uint32]*uint32{}
	seen := map[uint32]bool{}
	for i := range pd.Events {
		ev := pd.Events[i]
		if ev.TemplateOffset == nil {
			continue
		}
		eid := uint32(ev.EventID)
		if !seen[eid] {
			tplByEid[eid] = ev.TemplateOffset
			seen[eid] = true
		}
	}

	eids := make([]int, 0, len(tplByEid))
	for eid := range tplByEid {
		if len(eidFilter) == 0 || eidFilter[eid] {
			eids = append(eids, int(eid))
		}
	}
	sort.Ints(eids)
	for _, eid := range eids {
		tpl := tplByEid[uint32(eid)]
		params, ok := pd.TemplateParams[*tpl]
		if !ok {
			continue
		}
		fmt.Printf("EID %d (template@%#x):\n", eid, *tpl)
		for _, p := range params {
			fmt.Printf("  [%d] %-30s in=0x%02X out=0x%02X mapRef=%#x\n",
				p.Index, p.Name, p.InType, p.OutType, p.MapRef)
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
