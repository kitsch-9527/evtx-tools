package dump

import (
	"fmt"
	"sort"

	"github.com/kitsch-9527/msgdb-dump/crim"
	"github.com/kitsch-9527/msgdb-dump/pe"
)

// ExtractProviderVariants extracts 8-tuple variants for a single provider (Q2=A).
// It parses CRIM per-GUID WEVT block and maps message_identifier -> MUI RT_MESSAGETABLE.
// No DllGroup bulk copy; each Provider isolated.
func ExtractProviderVariants(dllPath string, provider ProviderInfo, langs, langCodes []string) (map[string][]VariantRow, map[string]map[string]string, map[string]map[string]map[string]string, []ParamMapRef, []ParamTypeRef, error) {
	combined, err := pe.ExtractAllFromPE(dllPath)
	if err != nil {
		return nil, nil, nil, nil, nil, err
	}

	// Collect D2 strings from main DLL + MUIs
	d2Strings := make(map[uint32]string)
	for k, v := range combined.D2Strings {
		d2Strings[k] = v
	}
	for _, code := range langCodes {
		muiPath := FindMUICompanion(dllPath, code)
		if muiPath == nil {
			continue
		}
		if mc, err := pe.ExtractAllFromPE(*muiPath); err == nil {
			for k, v := range mc.D2Strings {
				if _, exists := d2Strings[k]; !exists {
					d2Strings[k] = v
				}
			}
		}
	}

	// RT_MESSAGETABLE path: provider's messages via message_id remapping
	// If no CRIM, fall back to flat RT_MESSAGETABLE (legacy single-template providers)
	variantsByLang := make(map[string][]VariantRow)
	valueMaps := make(map[string]map[string]string)
	// v9 (2026-09-30): per-language value maps — each language's MUI D2
	// strings fully overlay the main-DLL base before the MAPS parse, so the
	// same map key can carry zh-CN and en-US displays side by side.
	valueMapsByLang := make(map[string]map[string]map[string]string)
	var paramMapRefs []ParamMapRef
	var paramTypes []ParamTypeRef

	if len(combined.CrimBlobs) > 0 && provider.GUID != nil {
		// CRIM provider-center path
		allEvents := collectProviderEvents(combined.CrimBlobs, *provider.GUID, d2Strings)
		if len(allEvents) > 0 {
			deduped := crim.DeduplicateVariants(allEvents)
			// Need ProviderData for template params / level names
			pd := findProviderData(combined.CrimBlobs, *provider.GUID, d2Strings)
			for _, langCode := range langCodes {
				muiPath := dllPath // default: main DLL messages
				muiMsgs := combined.Messages
				if mp := FindMUICompanion(dllPath, langCode); mp != nil {
					if mc, err := pe.ExtractAllFromPE(*mp); err == nil && len(mc.Messages) > 0 {
						muiMsgs = mc.Messages
						muiPath = *mp
					}
				}
				_ = muiPath
				var rows []VariantRow
				for _, ev := range deduped {
					if ev.MessageID == 0xFFFFFFFF || ev.MessageID == 0 {
						continue
					}
					msgText, ok := muiMsgs[ev.MessageID]
					if !ok || msgText == "" {
						continue
					}
					row := VariantRow{
						ProviderName: provider.Name,
						ProviderGUID: *provider.GUID,
						Lang:         langCode,
						EventID:      uint32(ev.EventID),
						Version:      ev.Version,
						Level:        ev.Level,
						Opcode:       ev.Opcode,
						Task:         ev.Task,
						Keywords:     ev.Keywords,
						Channel:      ev.Channel,
						MessageID:    ev.MessageID,
						DLLName:      provider.DLLPath, // will be overwritten by caller with dll name
						Message:      msgText,
					}
					// Attach template params from TTBL
					if pd != nil && ev.TemplateOffset != nil && pd.TemplateParams != nil {
						if params, ok := pd.TemplateParams[*ev.TemplateOffset]; ok && len(params) > 0 {
							pm := make(map[string]string, len(params))
							for _, p := range params {
								pm[fmt.Sprintf("%d", p.Index)] = p.Name
							}
							row.Parameters = &pm
							// Param value map refs
							for _, p := range params {
								if p.MapRef == 0 {
									continue
								}
								if info, ok := pd.MapRefsByOffset[p.MapRef]; ok && info.Name != "" {
									key := provider.Name + "." + info.Name
									if info.IsBitmap {
										key += "_bits"
									}
									paramMapRefs = append(paramMapRefs, ParamMapRef{
										ProviderName: provider.Name,
										EventID:      uint32(ev.EventID),
										ParamIndex:   fmt.Sprintf("%d", p.Index),
										ParamName:    p.Name,
										MapName:      key,
										IsBitmap:     info.IsBitmap,
									})
								}
							}
							// R10: per-parameter TDH InType/OutType for typed rendering
							for _, p := range params {
								if (p.InType == 0x01 && p.OutType == 0x01) || (p.InType == 0 && p.OutType == 0) {
									continue
								}
								paramTypes = append(paramTypes, ParamTypeRef{
									ProviderName: provider.Name,
									EventID:      uint32(ev.EventID),
									ParamIndex:   fmt.Sprintf("%d", p.Index),
									ParamName:    p.Name,
									InType:       p.InType,
									OutType:      p.OutType,
								})
							}
						}
					}
					// Level/keyword names
					if pd != nil {
						if ev.LevelOffset != nil {
							if name, ok := pd.LevelsByOffset[*ev.LevelOffset]; ok {
								row.LevelName = &name
							}
						} else if name, ok := pd.Levels[uint32(ev.Level)]; ok {
							row.LevelName = &name
						}
						if kws := resolveKeywords(ev.Keywords, pd.KeywordsMap); kws != "" {
							row.KeywordsName = &kws
						}
						if ev.TaskOffset != nil {
							if name, ok := pd.TasksByOffset[*ev.TaskOffset]; ok {
								row.TaskName = &name
							}
						} else if name, ok := pd.Tasks[uint32(ev.Task)]; ok {
							row.TaskName = &name
						}
						if ev.OpcodeOffset != nil {
							if name, ok := pd.OpcodesByOffset[*ev.OpcodeOffset]; ok {
								row.OpcodeName = &name
							}
						} else if name, ok := pd.Opcodes[uint32(ev.Opcode)]; ok {
							row.OpcodeName = &name
						}
					}
					rows = append(rows, row)
				}
				sort.Slice(rows, func(i, j int) bool {
					if rows[i].EventID != rows[j].EventID {
						return rows[i].EventID < rows[j].EventID
					}
					if rows[i].Version != rows[j].Version {
						return rows[i].Version > rows[j].Version
					}
					return rows[i].MessageID < rows[j].MessageID
				})
				if len(rows) > 0 {
					variantsByLang[langCode] = rows
				}
			}
			// ValueMaps per provider
			if pd != nil {
				for name, entries := range pd.ValueMaps {
					vm := make(map[string]string)
					for v, d := range entries {
						vm[fmt.Sprintf("%d", v)] = d
					}
					valueMaps[provider.Name+"."+name] = vm
				}
				for name, entries := range pd.BitMaps {
					vm := make(map[string]string)
					for b, d := range entries {
						vm[fmt.Sprintf("%d", b)] = d
					}
					valueMaps[provider.Name+"."+name+"_bits"] = vm
				}
			}
			// v9: re-parse the provider data per language with that language's
			// MUI D2 overlay so its VMAP/BMAP displays are extracted too.
			for _, langCode := range langCodes {
				langD2 := make(map[uint32]string, len(d2Strings))
				for k, v := range d2Strings {
					langD2[k] = v
				}
				if mp := FindMUICompanion(dllPath, langCode); mp != nil {
					if mc, err := pe.ExtractAllFromPE(*mp); err == nil && len(mc.D2Strings) > 0 {
						langD2 = overlayStringTable(langD2, mc.D2Strings)
					}
				}
				pdLang := findProviderData(combined.CrimBlobs, *provider.GUID, langD2)
				if pdLang == nil {
					continue
				}
				byMap := valueMapsByLang[langCode]
				if byMap == nil {
					byMap = make(map[string]map[string]string)
				}
				for name, entries := range pdLang.ValueMaps {
					vm := make(map[string]string)
					for v, d := range entries {
						vm[fmt.Sprintf("%d", v)] = d
					}
					byMap[provider.Name+"."+name] = vm
				}
				for name, entries := range pdLang.BitMaps {
					vm := make(map[string]string)
					for b, d := range entries {
						vm[fmt.Sprintf("%d", b)] = d
					}
					byMap[provider.Name+"."+name+"_bits"] = vm
				}
				valueMapsByLang[langCode] = byMap
			}
			return variantsByLang, valueMaps, valueMapsByLang, paramMapRefs, paramTypes, nil
		}
	}

	// Fallback: flat RT_MESSAGETABLE (no CRIM or no events)
	// One VariantRow per message (single EID, defaults for 8-tuple)
	// R16（2026-09-04）：en-US 也尝试 MUI companion。经典 provider（无 GUID）的主 DLL
	// 可能没有 RT_MESSAGETABLE（消息全在 MUI，如 msimsg.dll msgs=0 / en-US mui 435），
	// 原逻辑 en-US 只读主 DLL 导致 MsiInstaller/VSS 的 en-US 0 行（634 行全 zh-CN）。
	for _, langCode := range langCodes {
		muiMsgs := combined.Messages
		if mp := FindMUICompanion(dllPath, langCode); mp != nil {
			if mc, err := pe.ExtractAllFromPE(*mp); err == nil && len(mc.Messages) > 0 {
				muiMsgs = mc.Messages
			}
		}
		if len(muiMsgs) == 0 {
			continue
		}
		// R21（2026-09-29）：经典源 eid 映射（rawID & 0xFFFF）+ 确定性去重，
		// 见 classicVariantRows —— 原始全格式 ID 直接落库导致经典 B 簇全量 miss。
		var msgs []PeMessage
		for eid, msg := range muiMsgs {
			msgs = append(msgs, PeMessage{EventID: eid, Message: msg})
		}
		variantsByLang[langCode] = classicVariantRows(provider, langCode, msgs)
	}
	return variantsByLang, valueMaps, valueMapsByLang, paramMapRefs, paramTypes, nil
}

func collectProviderEvents(blobs [][]byte, guid string, d2 map[uint32]string) []crim.EventEntry {
	var all []crim.EventEntry
	for _, blob := range blobs {
		pd := crim.ParseProviderDataWithStrings(blob, guid, d2)
		if pd != nil && len(pd.Events) > 0 {
			all = append(all, pd.Events...)
		}
	}
	return all
}

func findProviderData(blobs [][]byte, guid string, d2 map[uint32]string) *crim.ProviderData {
	for _, blob := range blobs {
		pd := crim.ParseProviderDataWithStrings(blob, guid, d2)
		if pd != nil && len(pd.Events) > 0 {
			return pd
		}
	}
	return nil
}
