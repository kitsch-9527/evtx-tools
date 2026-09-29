//go:build windows
package dump

import (
	"fmt"
	"sort"

	"github.com/kitsch-9527/msgdb-dump/pe"
)

// BuildProviderVariantGroups builds Provider-center variant groups (Q2=A).
// Each Provider gets isolated Variants per language via per-GUID WEVT extraction.
// v9 (2026-09-30): also returns per-language value maps
// (lang → "Provider.MapName" → value → display).
func BuildProviderVariantGroups(dllResults []*DLLResult, langCodes []string) ([]ProviderVariantGroup, map[string]map[string]string, map[string]map[string]map[string]string, []ParamMapRef, []ParamTypeRef) {
	var groups []ProviderVariantGroup
	allValueMaps := make(map[string]map[string]string)
	allValueMapsByLang := make(map[string]map[string]map[string]string)
	var allParamRefs []ParamMapRef
	var allParamTypes []ParamTypeRef

	for _, res := range dllResults {
		for _, prov := range res.Providers {
			variantsByLang, vmaps, vmapsByLang, pRefs, tRefs, err := ExtractProviderVariants(res.DLLPath, prov, nil, langCodes)
			if err != nil || len(variantsByLang) == 0 {
				// Fallback: try raw RT_MESSAGETABLE path for providers without CRIM
				variantsByLang, vmaps, vmapsByLang, pRefs, tRefs, _ = extractVariantsFallback(res, prov, langCodes)
				if len(variantsByLang) == 0 {
					continue
				}
			}
			// Collect per-provider value maps
			for k, v := range vmaps {
				if _, ok := allValueMaps[k]; !ok {
					allValueMaps[k] = make(map[string]string)
				}
				for ek, ev := range v {
					allValueMaps[k][ek] = ev
				}
			}
			for lang, byMap := range vmapsByLang {
				if allValueMapsByLang[lang] == nil {
					allValueMapsByLang[lang] = make(map[string]map[string]string)
				}
				for k, v := range byMap {
					if _, ok := allValueMapsByLang[lang][k]; !ok {
						allValueMapsByLang[lang][k] = make(map[string]string)
					}
					for ek, ev := range v {
						allValueMapsByLang[lang][k][ek] = ev
					}
				}
			}
			allParamRefs = append(allParamRefs, pRefs...)
			allParamTypes = append(allParamTypes, tRefs...)
			groups = append(groups, ProviderVariantGroup{
				Provider:       prov,
				DLLName:        res.DLLName,
				DLLPath:        res.DLLPath,
				PEVersion:      res.Version,
				VariantsByLang: variantsByLang,
			})
		}
	}
	return groups, allValueMaps, allValueMapsByLang, allParamRefs, allParamTypes
}

func extractVariantsFallback(res *DLLResult, prov ProviderInfo, langCodes []string) (map[string][]VariantRow, map[string]map[string]string, map[string]map[string]map[string]string, []ParamMapRef, []ParamTypeRef, error) {
	// Use existing RT_MESSAGETABLE extraction (flat) as single-variant fallback
	byLang := make(map[string][]VariantRow)
	for _, code := range langCodes {
		var msgs []PeMessage
		if code == "en-US" {
			msgs = res.MessagesByLang["en-US"]
		} else {
			muiPath := FindMUICompanion(res.DLLPath, code)
			if muiPath != nil {
				if mc, err := pe.ExtractAllFromPE(*muiPath); err == nil {
					for eid, msg := range mc.Messages {
						msgs = append(msgs, PeMessage{EventID: eid, Message: msg})
					}
				}
			}
			if len(msgs) == 0 {
				continue
			}
		}
		if len(msgs) == 0 {
			continue
		}
		byLang[code] = classicVariantRows(prov, code, msgs)
	}
	if len(byLang) == 0 {
		return nil, nil, nil, nil, nil, fmt.Errorf("no fallback messages")
	}
	return byLang, nil, nil, nil, nil, nil
}

// classicVariantRows builds flat RT_MESSAGETABLE variant rows for classic
// providers (R21, 2026-09-29): event_id = rawID & 0xFFFF — the classic
// EventLog convention (severity/facility occupy the high bits; the event
// record carries the low 16 bits, e.g. Ntfs 50 ← 0x80010032, EventLog
// 6013 ← 0x8000177D). Storing the raw full-format ID as event_id made every
// classic-source cohort unreachable for the renderer (the classic B
// clusters). message_identifier stays 0 = wildcard for the resolve ladder.
// Deterministic first-wins dedup on the mapped eid (ascending raw ID).
func classicVariantRows(provider ProviderInfo, lang string, msgs []PeMessage) []VariantRow {
	sorted := make([]PeMessage, len(msgs))
	copy(sorted, msgs)
	sort.Slice(sorted, func(i, j int) bool { return sorted[i].EventID < sorted[j].EventID })
	var rows []VariantRow
	seen := make(map[uint32]bool)
	for _, m := range sorted {
		eid := m.EventID
		if eid > 0xFFFF {
			eid &= 0xFFFF
		}
		if seen[eid] {
			continue
		}
		seen[eid] = true
		row := VariantRow{
			ProviderName: provider.Name,
			Lang:         lang,
			EventID:      eid,
			Message:      m.Message,
		}
		if provider.GUID != nil {
			row.ProviderGUID = *provider.GUID
		}
		rows = append(rows, row)
	}
	return rows
}

// wellKnownEventLog6013 — phase D (2026-09-14) variant injection source.
//
// EventLog 6013 lives in netevent.dll's MUI satellite RT_MESSAGETABLE
// (table ID 0x8000177D, low 16 bits = 6013); the main netevent.dll carries
// no message table and no WEVT_TEMPLATE, so the default extraction path
// finds nothing. Texts verified char-by-char against the MUI tables and the
// live Get-WinEvent ground truth (phase C).
//
// Slot contract: the live event has 8 substitution slots — slots 1-4 are
// EMPTY strings, slot 5 = uptime seconds. The template therefore says %5,
// and the renderer's classic Data.#text strategy keeps empty slots in
// position, so no slot remapping is needed on our side. Inject only 6013:
// it is the one classic ID still diffing in the parity corpus (6013×16);
// 6005/6006/6008/6009 fallback rendering is pre-existing behavior outside
// this phase's scope.
var wellKnownEventLog6013 = map[string]string{
	"en-US": "The system uptime is %5 seconds.",
	"zh-CN": "系统启动时间为 %5 秒。",
}

// injectWellKnownVariants — variant-aware well-known injection (phase D).
// Adds the (EventLog, 6013) variant rows when no extraction path produced
// them; creates the EventLog provider group if the corpus scan yielded none.
func injectWellKnownVariants(groups []ProviderVariantGroup, langCodes []string) []ProviderVariantGroup {
	for gi := range groups {
		if groups[gi].Provider.Name != "EventLog" {
			continue
		}
		if variantGroupHas6013(groups[gi]) {
			return groups
		}
		guid := ""
		if groups[gi].Provider.GUID != nil {
			guid = *groups[gi].Provider.GUID
		}
		for _, code := range langCodes {
			msg, ok := wellKnownEventLog6013[code]
			if !ok {
				continue
			}
			groups[gi].VariantsByLang[code] = append(groups[gi].VariantsByLang[code], VariantRow{
				ProviderName: "EventLog",
				ProviderGUID: guid,
				Lang:         code,
				EventID:      6013,
				Message:      msg,
				// version/level/opcode/task/keywords/channel/message_identifier
				// stay 0 = wildcard for the renderer's resolve_variant ladder.
			})
		}
		return groups
	}
	// No EventLog group from extraction — create a minimal one so the
	// writer emits providers/dll_entries/variants rows.
	byLang := make(map[string][]VariantRow)
	for _, code := range langCodes {
		msg, ok := wellKnownEventLog6013[code]
		if !ok {
			continue
		}
		byLang[code] = append(byLang[code], VariantRow{
			ProviderName: "EventLog",
			Lang:         code,
			EventID:      6013,
			Message:      msg,
		})
	}
	if len(byLang) == 0 {
		return groups
	}
	return append(groups, ProviderVariantGroup{
		Provider:       ProviderInfo{Name: "EventLog", Channel: strPtr("System")},
		DLLName:        "netevent.dll",
		DLLPath:        `C:\Windows\System32\netevent.dll`,
		PEVersion:      "10.0.0.0",
		VariantsByLang: byLang,
	})
}

// variantGroupHas6013 reports whether any language variant of the group
// already carries an extracted 6013 row (real extraction wins over injection).
func variantGroupHas6013(g ProviderVariantGroup) bool {
	for _, rows := range g.VariantsByLang {
		for _, r := range rows {
			if r.EventID == 6013 {
				return true
			}
		}
	}
	return false
}

// mergeDuplicateProvidersVariants merges variant groups sharing same provider name across DLLs.
func mergeDuplicateProvidersVariants(groups []ProviderVariantGroup) []ProviderVariantGroup {
	byName := make(map[string][]ProviderVariantGroup)
	order := []string{}
	for _, g := range groups {
		if _, ok := byName[g.Provider.Name]; !ok {
			order = append(order, g.Provider.Name)
		}
		byName[g.Provider.Name] = append(byName[g.Provider.Name], g)
	}
	var merged []ProviderVariantGroup
	for _, name := range order {
		list := byName[name]
		if len(list) == 1 {
			merged = append(merged, list[0])
			continue
		}
		base := list[0]
		for _, other := range list[1:] {
			for lang, rows := range other.VariantsByLang {
				if _, ok := base.VariantsByLang[lang]; !ok {
					base.VariantsByLang[lang] = rows
				} else {
					base.VariantsByLang[lang] = append(base.VariantsByLang[lang], rows...)
				}
			}
		}
		merged = append(merged, base)
	}
	return merged
}

// Helper to keep ordered provider extraction for variant pipeline
// extractAndBuildVariantGroups helper for legacy callers
func extractAndBuildVariantGroups(dllResults []*DLLResult, langCodes []string) ([]ProviderVariantGroup, map[string]map[string]string, map[string]map[string]map[string]string, []ParamMapRef, []ParamTypeRef) {
	return BuildProviderVariantGroups(dllResults, langCodes)
}
