//go:build windows
package dump

import (
	"fmt"
	"sort"

	"github.com/kitsch-9527/msgdb-dump/pe"
)

// BuildProviderVariantGroups builds Provider-center variant groups (Q2=A).
// Each Provider gets isolated Variants per language via per-GUID WEVT extraction.
func BuildProviderVariantGroups(dllResults []*DLLResult, langCodes []string) ([]ProviderVariantGroup, map[string]map[string]string, []ParamMapRef, []ParamTypeRef) {
	var groups []ProviderVariantGroup
	allValueMaps := make(map[string]map[string]string)
	var allParamRefs []ParamMapRef
	var allParamTypes []ParamTypeRef

	for _, res := range dllResults {
		for _, prov := range res.Providers {
			variantsByLang, vmaps, pRefs, tRefs, err := ExtractProviderVariants(res.DLLPath, prov, nil, langCodes)
			if err != nil || len(variantsByLang) == 0 {
				// Fallback: try raw RT_MESSAGETABLE path for providers without CRIM
				variantsByLang, vmaps, pRefs, tRefs, _ = extractVariantsFallback(res, prov, langCodes)
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
	return groups, allValueMaps, allParamRefs, allParamTypes
}

func extractVariantsFallback(res *DLLResult, prov ProviderInfo, langCodes []string) (map[string][]VariantRow, map[string]map[string]string, []ParamMapRef, []ParamTypeRef, error) {
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
			sort.Slice(msgs, func(i, j int) bool { return msgs[i].EventID < msgs[j].EventID })
		}
		if len(msgs) == 0 {
			continue
		}
		var rows []VariantRow
		for _, m := range msgs {
			rows = append(rows, VariantRow{
				ProviderName: prov.Name,
				Lang:         code,
				EventID:      m.EventID,
				Message:      m.Message,
			})
			if prov.GUID != nil {
				rows[len(rows)-1].ProviderGUID = *prov.GUID
			}
		}
		byLang[code] = rows
	}
	if len(byLang) == 0 {
		return nil, nil, nil, nil, fmt.Errorf("no fallback messages")
	}
	return byLang, nil, nil, nil, nil
}

// injectWellKnownVariants — v6 variant-aware well-known injection (stub)
func injectWellKnownVariants(groups []ProviderVariantGroup, langCodes []string) []ProviderVariantGroup {
	return groups
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
func extractAndBuildVariantGroups(dllResults []*DLLResult, langCodes []string) ([]ProviderVariantGroup, map[string]map[string]string, []ParamMapRef, []ParamTypeRef) {
	return BuildProviderVariantGroups(dllResults, langCodes)
}
