package dump

import (
	"encoding/binary"
	"fmt"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/kitsch-9527/msgdb-dump/crim"
	"github.com/kitsch-9527/msgdb-dump/pe"
)

// ---------------------------------------------------------------------------
// ExtractSingleDLL — per-DLL extraction
// ---------------------------------------------------------------------------

// ExtractSingleDLL extracts all data from a single message DLL.
//
// Flow (matching Rust cmd_dump.rs extract_single_dll):
//  1. Parameter messages from ParameterMessageFile (%%N expansion)
//  2. Main PE extraction via pe.ExtractAllFromPE
//  3. RT_MESSAGETABLE branch: build PeMessage objects, enrich from CRIM, try MUI
//  4. CRIM+MUI fallback: when DLL has only WEVT_TEMPLATE (no RT_MESSAGETABLE)
func ExtractSingleDLL(dllPath string, providers []ProviderInfo, langLCIDs, langCodes []string) *DLLResult {
	result := &DLLResult{
		DLLName:        filepath.Base(dllPath),
		DLLPath:        dllPath,
		Version:        "0.0.0.0",
		MessagesByLang: make(map[string][]PeMessage),
		Providers:      providers,
		ParamMessages:  make(map[uint32]string),
		ValueMaps:      make(map[string]map[string]string),
	}

	// 1. Extract parameter messages from ParameterMessageFile (%%N expansion)
	// v9（2026-09-29）：参数文件同样按语言萃取——基础文件为 en-US 层，非 en 语言
	// 走 MUI companion。此前只读基础文件单语，zh 渲染时 %%1833 等永远是英文
	// （Security-Auditing 9733 条 D 的根因；schema 的 PK 本就含 lang，无需升版）。
	seenParamPaths := make(map[string]bool)
	for _, p := range providers {
		if p.ParameterFile != nil && *p.ParameterFile != "" {
			paramPath := *p.ParameterFile
			if !seenParamPaths[paramPath] {
				seenParamPaths[paramPath] = true
				result.ParamMessagesByLang = mergeParamMessagesByLang(
					result.ParamMessagesByLang,
					extractParamMessagesByLang(paramPath, langCodes),
				)
			}
		}
	}
	if en := result.ParamMessagesByLang["en-US"]; len(en) > 0 {
		result.ParamMessages = en
	}

	// 2. Main PE extraction
	combined, err := pe.ExtractAllFromPE(dllPath)
	if err != nil {
		return result
	}

	result.Version = combined.Version
	result.CrimBlobs = combined.CrimBlobs

	// Collect the 0xD2 string table (RT_MESSAGETABLE entries with IDs
	// 0xD200xxxx) from the main DLL and all MUI companions. CRIM VMAP/BMAP
	// entry references use 0xD2-encoded indexes into this table.
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

	// v9 (2026-09-30): per-language string tables for VMAP/BMAP display
	// extraction. Base = main DLL RTStrings+D2Strings; each language's MUI
	// D2Strings fully overlay it (same-key override — mirrors MUI whole-family
	// localization; the old merge only filled missing keys and left the en
	// string occupying the key, the Ntfs 98 %3 root cause).
	stringTablesByLang := buildStringTablesByLang(dllPath, combined, d2Strings, langCodes)

	// 3. RT_MESSAGETABLE branch: messages found in DLL
	if len(combined.Messages) > 0 {
		// Build PeMessage objects from flat message map
		primaryMsgs := buildPeMessages(combined.Messages, nil, nil)

		// Apply CRIM message_id -> event_id mapping
		// resource IDs to manifest-defined event IDs.
		msgIDMap := buildMessageIDMap(combined.CrimBlobs, providers)
		if len(msgIDMap) > 0 {
			primaryMsgs = applyCRIMMessageMapping(primaryMsgs, msgIDMap)
		}

		result.MessagesByLang["en-US"] = primaryMsgs

		// Try MUI companions for additional languages
		for _, code := range langCodes {
			if code == "en-US" {
				continue
			}
			muiPath := FindMUICompanion(dllPath, code)
			if muiPath == nil {
				continue
			}
			if muiCombined, err := pe.ExtractAllFromPE(*muiPath); err == nil {
				if len(muiCombined.Messages) > 0 {
					muiMsgs := buildPeMessages(muiCombined.Messages, nil, nil)
					if len(msgIDMap) > 0 {
						muiMsgs = applyCRIMMessageMapping(muiMsgs, msgIDMap)
					}
					result.MessagesByLang[code] = muiMsgs
				}
			}
		}

		// LEVL/KEYW enrichment from CRIM blobs; string table = RT_STRING +
		// D2 msgtable strings for VMAP/BMAP 0xD2 reference resolution.
		stringTable := make(map[uint32]string)
		for k, v := range combined.RTStrings {
			stringTable[k] = v
		}
		for k, v := range d2Strings {
			stringTable[k] = v
		}
		enrichFromCRIM(result, providers, stringTable)

		// v9: per-language value map extraction (MAPS blocks + provider data)
		enrichValueMapsByLang(result, providers, stringTablesByLang)
	}

	// 4. CRIM+MUI fallback: no messages, but CRIM blobs exist
	if len(result.MessagesByLang) == 0 && len(combined.CrimBlobs) > 0 {
		for i, code := range langCodes {
			lcid := langLCIDs[i]
			var allMsgs []PeMessage
			for _, p := range providers {
				crimResult, err := extractMUICRIMFromBlobs(
					dllPath, combined.CrimBlobs, p.Name, p.GUID, lcid,
				)
				if err == nil && crimResult != nil && len(crimResult.Messages) > 0 {
					allMsgs = append(allMsgs, crimResult.Messages...)
					// Collect per-parameter value map references
					if len(crimResult.ParamMapRefs) > 0 {
						result.ParamMapRefs = append(result.ParamMapRefs, crimResult.ParamMapRefs...)
					}
					// Collect ValueMaps from CRIM+MUI fallback.
					// v9: extractMUICRIMFromBlobs parses with THIS language's
					// MUI D2 strings, so its displays land under their own lang;
					// en-US also fills the legacy flat layer.
					if len(crimResult.ValueMaps) > 0 {
						if result.ValueMapsByLang == nil {
							result.ValueMapsByLang = make(map[string]map[string]map[string]string)
						}
						byMap := result.ValueMapsByLang[code]
						if byMap == nil {
							byMap = make(map[string]map[string]string)
						}
						for mapName, entries := range crimResult.ValueMaps {
							vm := make(map[string]string)
							for val, display := range entries {
								vm[strconv.FormatUint(uint64(val), 10)] = display
							}
							byMap[p.Name+"."+mapName] = vm
							if code == "en-US" {
								result.ValueMaps[p.Name+"."+mapName] = vm
							}
						}
						result.ValueMapsByLang[code] = byMap
					}
					if len(crimResult.BitMaps) > 0 {
						if result.ValueMapsByLang == nil {
							result.ValueMapsByLang = make(map[string]map[string]map[string]string)
						}
						byMap := result.ValueMapsByLang[code]
						if byMap == nil {
							byMap = make(map[string]map[string]string)
						}
						for mapName, entries := range crimResult.BitMaps {
							vm := make(map[string]string)
							for bitPos, display := range entries {
								vm[strconv.FormatUint(bitPos, 10)] = display
							}
							byMap[p.Name+"."+mapName+"_bits"] = vm
							if code == "en-US" {
								result.ValueMaps[p.Name+"."+mapName+"_bits"] = vm
							}
						}
						result.ValueMapsByLang[code] = byMap
					}
				}
			}
			// Dedup by event_id (first match wins)
			deduped := dedupPeMessages(allMsgs)
			if len(deduped) > 0 {
				result.MessagesByLang[code] = deduped
			}
		}
	}


	// 5. MUI-only fallback: LN DLL has no messages at all
	// Try extracting from MUI companion directly (e.g. IoLogMsg.dll, esent.dll).
	if len(result.MessagesByLang) == 0 && len(combined.CrimBlobs) == 0 {
		// Try each requested language
		for _, code := range langCodes {
			if code == "en-US" {
				continue
			}
			muiPath := FindMUICompanion(dllPath, code)
			if muiPath == nil {
				continue
			}
			if muiCombined, err := pe.ExtractAllFromPE(*muiPath); err == nil {
				if len(muiCombined.Messages) > 0 {
					muiMsgs := buildPeMessages(muiCombined.Messages, nil, nil)
					msgIDMap := buildMessageIDMap(combined.CrimBlobs, providers)
					if len(msgIDMap) > 0 {
						muiMsgs = applyCRIMMessageMapping(muiMsgs, msgIDMap)
					}
					result.MessagesByLang[code] = muiMsgs
				}
			}
		}
		// Also try en-US MUI companion
		muiPath := FindMUICompanion(dllPath, "en-US")
		if muiPath != nil {
			if muiCombined, err := pe.ExtractAllFromPE(*muiPath); err == nil {
				if len(muiCombined.Messages) > 0 {
					muiMsgs := buildPeMessages(muiCombined.Messages, nil, nil)
					msgIDMap := buildMessageIDMap(combined.CrimBlobs, providers)
					if len(msgIDMap) > 0 {
						muiMsgs = applyCRIMMessageMapping(muiMsgs, msgIDMap)
					}
					result.MessagesByLang["en-US"] = muiMsgs
				}
			}
		}
	}
	return result
}

// ---------------------------------------------------------------------------
// extractMessages — thin wrapper to extract message table from a DLL
// ---------------------------------------------------------------------------

func extractMessages(dllPath string) map[uint32]string {
	combined, err := pe.ExtractAllFromPE(dllPath)
	if err != nil {
		return nil
	}
	return combined.Messages
}

// extractParamMessagesByLang extracts %%N parameter messages for a parameter
// file across languages (v9): the base file is the en-US layer; each non-en
// language comes from its MUI companion. When the base file has no message
// table (modern MUI-only layout), the en-US MUI companion is used so param
// refs do not silently disappear.
func extractParamMessagesByLang(paramPath string, langCodes []string) map[string]map[uint32]string {
	byLang := make(map[string]map[uint32]string)
	// 注意 len 判断而非 nil：MUI-only 布局的基础文件（如 netevent.dll 28KB 无表）
	// 会返回空非 nil map，若按 nil 判断会挡住 en-US MUI 回退臂（09-29 实测失衡根因）。
	if msgs := extractMessages(paramPath); len(msgs) > 0 {
		byLang["en-US"] = cloneParamMessages(msgs)
	}
	for _, code := range langCodes {
		if code == "en-US" {
			continue
		}
		muiPath := FindMUICompanion(paramPath, code)
		if muiPath == nil {
			continue
		}
		if msgs := extractMessages(*muiPath); len(msgs) > 0 {
			byLang[code] = cloneParamMessages(msgs)
		}
	}
	if len(byLang["en-US"]) == 0 {
		if muiPath := FindMUICompanion(paramPath, "en-US"); muiPath != nil {
			if msgs := extractMessages(*muiPath); len(msgs) > 0 {
				byLang["en-US"] = cloneParamMessages(msgs)
			}
		}
	}
	return byLang
}

func cloneParamMessages(msgs map[uint32]string) map[uint32]string {
	out := make(map[uint32]string, len(msgs))
	for id, text := range msgs {
		out[id] = text
	}
	return out
}

// mergeParamMessagesByLang merges src into dst (per-language flat maps);
// returns dst (created when nil). Later param files override earlier rows on
// id collision, matching the legacy flat-map semantics.
func mergeParamMessagesByLang(dst, src map[string]map[uint32]string) map[string]map[uint32]string {
	if dst == nil {
		dst = make(map[string]map[uint32]string)
	}
	for lang, msgs := range src {
		if dst[lang] == nil {
			dst[lang] = make(map[uint32]string, len(msgs))
		}
		for id, text := range msgs {
			dst[lang][id] = text
		}
	}
	return dst
}

// ---------------------------------------------------------------------------
// buildPeMessages — convert flat message map to PeMessage objects
// ---------------------------------------------------------------------------

// buildPeMessages converts a flat map[uint32]string (from ParseMessageTable)
// to a sorted []PeMessage slice, attaching any WEVT template parameters
// and CRIM-provided metadata if available.
//
// When wevtParams or eventMap is nil, PeMessage fields beyond EventID/Message
// remain nil (caller enriches later via enrichFromCRIM).
func buildPeMessages(msgs map[uint32]string, wevtParams map[string]map[string]string, eventMap map[uint32]crim.EventEntry) []PeMessage {
	if len(msgs) == 0 {
		return nil
	}

	// Sort event IDs for deterministic output
	eids := make([]uint32, 0, len(msgs))
	for eid := range msgs {
		eids = append(eids, eid)
	}
	sort.Slice(eids, func(i, j int) bool { return eids[i] < eids[j] })

	pm := make([]PeMessage, 0, len(eids))
	for _, eid := range eids {
		p := PeMessage{
			EventID: eid,
			Message: msgs[eid],
		}

		// Attach WEVT template parameter names if available
		if wevtParams != nil {
			eidStr := strconv.FormatUint(uint64(eid), 10)
			if params, ok := wevtParams[eidStr]; ok && len(params) > 0 {
				p.Parameters = &params
			}
		}

		// Attach CRIM event entry metadata if available
		if eventMap != nil {
			if ev, ok := eventMap[eid]; ok {
				// Level
				if ev.Level != 0 {
					lvl := ev.Level
					p.Level = &lvl
				}
				// Task
				// (task name resolution happens in enrichFromCRIM)
				_ = ev
			}
		}

		pm = append(pm, p)
	}
	return pm
}

// ---------------------------------------------------------------------------
// buildMessageIDMap — build message_id → EventEntry mapping from CRIM blobs
// ---------------------------------------------------------------------------

// buildMessageIDMap parses CRIM blobs for the given providers and builds a
// map of message_identifier → EventEntry. This enables remapping messages
// from RT_MESSAGETABLE resource IDs to manifest-defined event IDs.
//
// Some providers (e.g. Microsoft-Windows-Servicing) store messages in their
// DLL's RT_MESSAGETABLE under resource IDs while the CRIM manifest defines
// event IDs that reference those resource IDs via EventEntry.MessageID.
func buildMessageIDMap(crimBlobs [][]byte, providers []ProviderInfo) map[uint32]crim.EventEntry {
	if len(crimBlobs) == 0 || len(providers) == 0 {
		return nil
	}
	msgIDMap := make(map[uint32]crim.EventEntry)
	for _, p := range providers {
		if p.GUID == nil {
			continue
		}
		for _, blob := range crimBlobs {
			pd := crim.ParseProviderData(blob, *p.GUID)
			if pd == nil || len(pd.Events) == 0 {
				continue
			}
			for _, ev := range pd.Events {
				if ev.MessageID == 0xFFFFFFFF || ev.MessageID == 0 {
					continue
				}
				curr, exists := msgIDMap[ev.MessageID]
				if !exists {
					msgIDMap[ev.MessageID] = ev
				} else if ev.Version > curr.Version || (ev.Version == curr.Version && ev.TemplateOffset != nil && curr.TemplateOffset == nil) {
					msgIDMap[ev.MessageID] = ev
				}
			}
		}
	}
	if len(msgIDMap) == 0 {
		return nil
	}
	return msgIDMap
}

// applyCRIMMessageMapping remaps PeMessage event IDs from resource IDs to
// manifest-defined event IDs using a message_id → EventEntry map.
//
// The returned slice is a new allocation; the input is not modified.
func applyCRIMMessageMapping(msgs []PeMessage, msgIDMap map[uint32]crim.EventEntry) []PeMessage {
	if len(msgs) == 0 {
		return nil
	}
	result := make([]PeMessage, 0, len(msgs))
	seenEIDs := make(map[uint16]bool)
	for _, m := range msgs {
		ev, ok := msgIDMap[m.EventID]
		if !ok {
			result = append(result, m)
			continue
		}
		if seenEIDs[ev.EventID] {
			continue
		}
		seenEIDs[ev.EventID] = true
		p := PeMessage{
			EventID:      uint32(ev.EventID),
			Message:      m.Message,
			ProviderName: m.ProviderName,
		}
		if ev.Level != 0 {
			lvl := ev.Level
			p.Level = &lvl
		}
		result = append(result, p)
	}
	return result
}

// ---------------------------------------------------------------------------
// per-language value map extraction (v9, 2026-09-30)
// ---------------------------------------------------------------------------

// overlayStringTable returns a new table: base entries with every overlay
// entry fully overriding same-key values (MUI localization semantics — the
// MUI resource is authoritative for its language even when the main DLL
// already carries an en string at that 0xD2 key).
func overlayStringTable(base, overlay map[uint32]string) map[uint32]string {
	out := make(map[uint32]string, len(base)+len(overlay))
	for k, v := range base {
		out[k] = v
	}
	for k, v := range overlay {
		out[k] = v
	}
	return out
}

// buildStringTablesByLang builds one string table per language: the main
// DLL's D2 string table as base, each language's MUI D2Strings overlaid.
func buildStringTablesByLang(dllPath string, combined *pe.CombinedPEData, d2Strings map[uint32]string, langCodes []string) map[string]map[uint32]string {
	tables := make(map[string]map[uint32]string, len(langCodes))
	for _, code := range langCodes {
		base := make(map[uint32]string, len(d2Strings))
		for k, v := range d2Strings {
			base[k] = v
		}
		tables[code] = base
		muiPath := FindMUICompanion(dllPath, code)
		if muiPath == nil {
			continue
		}
		if mc, err := pe.ExtractAllFromPE(*muiPath); err == nil && len(mc.D2Strings) > 0 {
			tables[code] = overlayStringTable(base, mc.D2Strings)
		}
	}
	return tables
}

// enrichValueMapsByLang extracts VMAP/BMAP displays per language from the
// same CRIM structures enrichFromCRIM reads (top-level MAPS blocks plus each
// provider's parsed data), landing them in result.ValueMapsByLang[lang]. The
// legacy ValueMaps layer and LEVL/KEYW enrichment are untouched.
func enrichValueMapsByLang(result *DLLResult, providers []ProviderInfo, stringTablesByLang map[string]map[uint32]string) {
	if len(result.CrimBlobs) == 0 || len(stringTablesByLang) == 0 {
		return
	}
	for lang, stringTable := range stringTablesByLang {
		if result.ValueMapsByLang == nil {
			result.ValueMapsByLang = make(map[string]map[string]map[string]string)
		}
		byMap := result.ValueMapsByLang[lang]
		if byMap == nil {
			byMap = make(map[string]map[string]string)
		}
		// Top-level MAPS blocks (same scan as enrichFromCRIM first pass)
		for _, blob := range result.CrimBlobs {
			if len(blob) < 16 || string(blob[:4]) != "CRIM" {
				continue
			}
			providerCount := int(binary.LittleEndian.Uint32(blob[12:16]))
			mapsStart := 16 + providerCount*20
			if mapsStart < len(blob) {
				for pos := mapsStart; pos+12 < len(blob); pos += 4 {
					if string(blob[pos:pos+4]) != "MAPS" {
						continue
					}
					vmaps, bmaps := crim.ParseMAPSBlockWithStrings(blob, pos, stringTable)
					for mapName, entries := range vmaps {
						vm := make(map[string]string)
						for val, display := range entries {
							vm[strconv.FormatUint(uint64(val), 10)] = display
						}
						byMap["MAPS."+mapName] = vm
					}
					for mapName, entries := range bmaps {
						vm := make(map[string]string)
						for bitPos, display := range entries {
							vm[strconv.FormatUint(bitPos, 10)] = display
						}
						byMap["MAPS."+mapName+"_bits"] = vm
					}
				}
			}
		}
		// Per-provider data value maps
		for _, p := range providers {
			if p.GUID == nil {
				continue
			}
			for _, blob := range result.CrimBlobs {
				pd := crim.ParseProviderDataWithStrings(blob, *p.GUID, stringTable)
				if pd == nil {
					continue
				}
				for mapName, entries := range pd.ValueMaps {
					vm := make(map[string]string)
					for val, display := range entries {
						vm[strconv.FormatUint(uint64(val), 10)] = display
					}
					byMap[p.Name+"."+mapName] = vm
				}
				for mapName, entries := range pd.BitMaps {
					vm := make(map[string]string)
					for bitPos, display := range entries {
						vm[strconv.FormatUint(bitPos, 10)] = display
					}
					byMap[p.Name+"."+mapName+"_bits"] = vm
				}
			}
		}
		result.ValueMapsByLang[lang] = byMap
	}
}

// ---------------------------------------------------------------------------
// enrichFromCRIM — apply LEVL/KEYW/TASK/OPCO metadata to all messages
// ---------------------------------------------------------------------------

// enrichFromCRIM iterates over all CRIM blobs in the result, parses provider
// data for each provider, and applies level_name, keywords_name, task, opcode
// to matching PeMessage objects across all languages.
// Also extracts VMAP/BMAP from MAPS blocks at the CRIM top level.
func enrichFromCRIM(result *DLLResult, providers []ProviderInfo, stringTable map[uint32]string) {
	if len(result.CrimBlobs) == 0 {
		return
	}

	// First pass: extract VMAP/BMAP from MAPS blocks at CRIM top level
	for _, blob := range result.CrimBlobs {
		if len(blob) < 16 || string(blob[:4]) != "CRIM" {
			continue
		}
		providerCount := int(binary.LittleEndian.Uint32(blob[12:16]))
		mapsStart := 16 + providerCount*20
		if mapsStart < len(blob) {
			for pos := mapsStart; pos+12 < len(blob); pos += 4 {
				if string(blob[pos:pos+4]) == "MAPS" {
					vmaps, bmaps := crim.ParseMAPSBlockWithStrings(blob, pos, stringTable)
					if len(vmaps) > 0 {
						if result.ValueMaps == nil {
							result.ValueMaps = make(map[string]map[string]string)
						}
						for mapName, entries := range vmaps {
							vm := make(map[string]string)
							for val, display := range entries {
								vm[strconv.FormatUint(uint64(val), 10)] = display
							}
							result.ValueMaps["MAPS."+mapName] = vm
						}
					}
					if len(bmaps) > 0 {
						if result.ValueMaps == nil {
							result.ValueMaps = make(map[string]map[string]string)
						}
						for mapName, entries := range bmaps {
							vm := make(map[string]string)
							for bitPos, display := range entries {
								vm[strconv.FormatUint(bitPos, 10)] = display
							}
							result.ValueMaps["MAPS."+mapName+"_bits"] = vm
						}
					}
				}
			}
		}
	}

	for _, p := range providers {
		if p.GUID == nil {
			continue
		}
		for _, blob := range result.CrimBlobs {
			pd := crim.ParseProviderDataWithStrings(blob, *p.GUID, stringTable)
			if pd == nil {
				continue
			}

			// Extract value maps even if Events is empty
			if len(pd.ValueMaps) > 0 {
				if result.ValueMaps == nil {
					result.ValueMaps = make(map[string]map[string]string)
				}
				for mapName, entries := range pd.ValueMaps {
					vm := make(map[string]string)
					for val, display := range entries {
						vm[strconv.FormatUint(uint64(val), 10)] = display
					}
					result.ValueMaps[p.Name+"."+mapName] = vm
				}
			}
			if len(pd.BitMaps) > 0 {
				if result.ValueMaps == nil {
					result.ValueMaps = make(map[string]map[string]string)
				}
				for mapName, entries := range pd.BitMaps {
					vm := make(map[string]string)
					for bitPos, display := range entries {
						vm[strconv.FormatUint(bitPos, 10)] = display
					}
					result.ValueMaps[p.Name+"."+mapName+"_bits"] = vm
				}
			}

			// R9（2026-09-04）：主分支补收集 ParamMapRefs（TEMP+8 链接主分支同样存在，
			// 此前只在 CRIM+MUI fallback 收集，如 Kernel-Power 566 param2 Reason 缺链接）。
			if refs := collectParamMapRefs(pd, p.Name, pd.Events); len(refs) > 0 {
				result.ParamMapRefs = append(result.ParamMapRefs, refs...)
			}

			// R10：主分支收集参数 InType/OutType（TDH 字节），供 evtx-tools
			// 类型化渲染（hex 大小写 / HRESULT 消息化 / FILETIME 纳秒精度）。
			if trefs := collectParamTypes(pd, p.Name, pd.Events); len(trefs) > 0 {
				result.ParamTypes = append(result.ParamTypes, trefs...)
			}

			// Skip event processing if no events
			if len(pd.Events) == 0 {
				continue
			}

			// Build event_id -> EventEntry map
			eventMap := make(map[uint32]crim.EventEntry)
			for _, ev := range pd.Events {
				eventMap[uint32(ev.EventID)] = ev
			}

			// Apply level_name and keywords_name to messages in all languages
			for lang := range result.MessagesByLang {
				msgs := result.MessagesByLang[lang]
				for i := range msgs {
					ev, ok := eventMap[msgs[i].EventID]
					if !ok {
						continue
					}

					// Level name
					if ev.LevelOffset != nil {
						if name, ok := pd.LevelsByOffset[*ev.LevelOffset]; ok {
							msgs[i].LevelName = &name
						}
					} else if name, ok := pd.Levels[uint32(ev.Level)]; ok {
						msgs[i].LevelName = &name
					}

					// Keywords name
					kws := resolveKeywords(ev.Keywords, pd.KeywordsMap)
					if kws != "" {
						msgs[i].KeywordsName = &kws
					}

					// Task name
					if ev.TaskOffset != nil {
						if name, ok := pd.TasksByOffset[*ev.TaskOffset]; ok {
							msgs[i].Task = &name
						}
					} else if name, ok := pd.Tasks[uint32(ev.Task)]; ok {
						msgs[i].Task = &name
					}

					// Opcode name
					if ev.OpcodeOffset != nil {
						if name, ok := pd.OpcodesByOffset[*ev.OpcodeOffset]; ok {
							msgs[i].Opcode = &name
						}
					} else if name, ok := pd.Opcodes[uint32(ev.Opcode)]; ok {
						msgs[i].Opcode = &name
					}
				}
				result.MessagesByLang[lang] = msgs
			}

		}
	}
}

// resolveKeywords checks which keyword masks apply to a given event's keywords
// value and returns a comma-separated list of keyword names.
func resolveKeywords(eventKW uint64, kwMap map[uint64]string) string {
	if len(kwMap) == 0 {
		return ""
	}
	var names []string
	for mask, name := range kwMap {
		if eventKW&mask == mask {
			names = append(names, name)
		}
	}
	sort.Strings(names)
	return strings.Join(names, ", ")
}

// ---------------------------------------------------------------------------
// extractMUICRIMFromBlobs — CRIM+MUI fallback extraction
// ---------------------------------------------------------------------------

// extractMUICRIMFromBlobs extracts messages from CRIM blobs plus MUI companion
// when the main DLL has no RT_MESSAGETABLE (EventManifest providers).
//
// Flow:
//  1. Parse all CRIM blobs for the target provider
//  2. Find MUI companion for the requested language
//  3. Extract MUI message table
//  4. Match CRIM event entries to MUI messages by message_identifier
//  5. Build PeMessage objects with CRIM metadata
func extractMUICRIMFromBlobs(dllPath string, crimBlobs [][]byte, providerName string, providerGUID *string, langLCID string) (*CRIMMUIResult, error) {
	if len(crimBlobs) == 0 {
		return nil, fmt.Errorf("no CRIM blobs for %s", providerName)
	}

	// 1. Parse CRIM blobs for event entries
	var allEvents []crim.EventEntry
	var allPD *crim.ProviderData

	// Try to obtain the D2 string table from the MUI companion before parsing,
	// so 0xD2-encoded VMAP/BMAP references resolve correctly.
	var d2Strings map[uint32]string
	if muiEarly := FindMUICompanion(dllPath, LCIDToLangCode(langLCID)); muiEarly != nil {
		if mc, err := pe.ExtractAllFromPE(*muiEarly); err == nil && len(mc.D2Strings) > 0 {
			d2Strings = mc.D2Strings
		}
	}

	for _, blob := range crimBlobs {
		if providerGUID != nil {
			pd := crim.ParseProviderDataWithStrings(blob, *providerGUID, d2Strings)
			if pd != nil && len(pd.Events) > 0 {
				if allPD == nil {
					allPD = pd
				} else {
					allPD.Events = append(allPD.Events, pd.Events...)
				}
				allEvents = append(allEvents, pd.Events...)
			} else {
			}
		} else {
			// No GUID — scan for all EVNT blocks unfiltered
			events := parseCRIMUnfilteredEVNT(blob)
			if len(events) > 0 {
				allEvents = append(allEvents, events...)
			}
		}
	}

	if len(allEvents) == 0 {
		return nil, fmt.Errorf("no events found in CRIM blobs for %s", providerName)
	}

	// Collect ValueMaps and BitMaps from allPD
	var valueMaps map[string]map[uint32]string
	var bitMaps map[string]map[uint64]string
	if allPD != nil {
		if len(allPD.ValueMaps) > 0 {
			valueMaps = allPD.ValueMaps
			fmt.Printf("[debug] %s: allPD.ValueMaps has %d maps\n", providerName, len(allPD.ValueMaps))
		}
		if len(allPD.BitMaps) > 0 {
			bitMaps = allPD.BitMaps
			fmt.Printf("[debug] %s: allPD.BitMaps has %d maps\n", providerName, len(allPD.BitMaps))
		}
	} else {
		fmt.Printf("[debug] %s: allPD is nil\n", providerName)
	}

	// 2. Find MUI companion
	langCode := LCIDToLangCode(langLCID)
	muiPath := FindMUICompanion(dllPath, langCode)
	if muiPath == nil {
		return nil, fmt.Errorf("no MUI companion for %s at lang %s", dllPath, langCode)
	}

	// 3. Extract MUI messages
	muiCombined, err := pe.ExtractAllFromPE(*muiPath)
	if err != nil || len(muiCombined.Messages) == 0 {
		return nil, fmt.Errorf("no messages in MUI companion %s: %v", *muiPath, err)
	}

	// 4. Deduplicate by VariantKey (Q3=B): 8-tuple full equality, not single EID.
	// Distinct Variants (e.g. TxR vs AppContainer for EID=11) are all retained.
	dedupedEvents := crim.DeduplicateVariants(allEvents)
	// Filter out entries with no message
	var filteredEvents []crim.EventEntry
	for _, ev := range dedupedEvents {
		if ev.MessageID != 0xFFFFFFFF && ev.MessageID != 0 {
			filteredEvents = append(filteredEvents, ev)
		}
	}
	// Sort by VariantKey for deterministic output — descending Version so that
	// legacy single-template dedup (dedupPeMessages) keeps the highest version
	// (preserves pre-variant semantics for DllGroup path). Variant-aware path
	// (ProviderVariantGroup) retains all rows sorted desc version.
	sort.Slice(filteredEvents, func(i, j int) bool {
		ki, kj := filteredEvents[i].Key(), filteredEvents[j].Key()
		if ki.EventID != kj.EventID {
			return ki.EventID < kj.EventID
		}
		if ki.Version != kj.Version {
			return ki.Version > kj.Version
		}
		if ki.Level != kj.Level {
			return ki.Level < kj.Level
		}
		if ki.Opcode != kj.Opcode {
			return ki.Opcode < kj.Opcode
		}
		if ki.Task != kj.Task {
			return ki.Task < kj.Task
		}
		if ki.Channel != kj.Channel {
			return ki.Channel < kj.Channel
		}
		if ki.Keywords != kj.Keywords {
			return ki.Keywords < kj.Keywords
		}
		return ki.MessageID < kj.MessageID
	})

	// 5. Match MUI messages to CRIM events — one PeMessage per Variant (not per EID)
	var msgs []PeMessage
	var paramMapRefs []ParamMapRef

	for _, ev := range filteredEvents {
		msgText, ok := muiCombined.Messages[ev.MessageID]
		if !ok || msgText == "" {
			continue
		}

		eid := uint32(ev.EventID)
		p := PeMessage{
			EventID:      eid,
			Message:      msgText,
			ProviderName: &providerName,
		}

		// Apply CRIM metadata
		if ev.Level != 0 {
			lvl := ev.Level
			p.Level = &lvl
		}
		if allPD != nil {
			if ev.LevelOffset != nil {
				if name, ok := allPD.LevelsByOffset[*ev.LevelOffset]; ok {
					p.LevelName = &name
				}
			} else if name, ok := allPD.Levels[uint32(ev.Level)]; ok {
				p.LevelName = &name
			}
			if kws := resolveKeywords(ev.Keywords, allPD.KeywordsMap); kws != "" {
				p.KeywordsName = &kws
			}
			if ev.TaskOffset != nil {
				if name, ok := allPD.TasksByOffset[*ev.TaskOffset]; ok {
					p.Task = &name
				}
			} else if name, ok := allPD.Tasks[uint32(ev.Task)]; ok {
				p.Task = &name
			}
			if ev.OpcodeOffset != nil {
				if name, ok := allPD.OpcodesByOffset[*ev.OpcodeOffset]; ok {
					p.Opcode = &name
				}
			} else if name, ok := allPD.Opcodes[uint32(ev.Opcode)]; ok {
				p.Opcode = &name
			}
			// Attach template parameters from TTBL
			if ev.TemplateOffset != nil && allPD.TemplateParams != nil {
				if params, ok := allPD.TemplateParams[*ev.TemplateOffset]; ok && len(params) > 0 {
					paramMap := make(map[string]string, len(params))
					for _, param := range params {
						paramMap[fmt.Sprintf("%d", param.Index)] = param.Name
					}
					p.Parameters = &paramMap

					// Attach per-parameter value map references (TEMP item +8
					// field → VMAP/BMAP block). This is the compiled form of the
					// manifest's <data name="..." map="..."/> association.
					for _, param := range params {
						if param.MapRef == 0 {
							continue
						}
						info, ok := allPD.MapRefsByOffset[param.MapRef]
						if !ok || info.Name == "" {
							continue
						}
						key := providerName + "." + info.Name
						if info.IsBitmap {
							key += "_bits"
						}
						paramMapRefs = append(paramMapRefs, ParamMapRef{
							ProviderName: providerName,
							EventID:      eid,
							ParamIndex:   fmt.Sprintf("%d", param.Index),
							ParamName:    param.Name,
							MapName:      key,
							IsBitmap:     info.IsBitmap,
						})
					}
				}
			}
		}

		msgs = append(msgs, p)
	}

	if len(msgs) == 0 {
		return nil, fmt.Errorf("no matched MUI messages for %s", providerName)
	}
	return &CRIMMUIResult{
		Messages:     msgs,
		ValueMaps:    valueMaps,
		BitMaps:      bitMaps,
		ParamMapRefs: paramMapRefs,
	}, nil
}

// parseCRIMUnfilteredEVNT scans a CRIM blob for all EVNT blocks (unfiltered by
// provider GUID). Used in the CRIM+MUI fallback when no GUID is available.
func parseCRIMUnfilteredEVNT(data []byte) []crim.EventEntry {
	if len(data) < 4 || string(data[:4]) != "CRIM" {
		return nil
	}
	var all []crim.EventEntry
	for i := 0; i <= len(data)-4; i++ {
		if data[i] == 'E' && data[i+1] == 'V' && data[i+2] == 'N' && data[i+3] == 'T' {
			events := crim.ParseEVNTBlock(data, i)
			all = append(all, events...)
		}
	}
	return all
}

// ---------------------------------------------------------------------------
// dedupPeMessages deduplicates PeMessage objects by event_id (first wins).
// ---------------------------------------------------------------------------

func dedupPeMessages(msgs []PeMessage) []PeMessage {
	seen := make(map[uint32]bool)
	var result []PeMessage
	for _, m := range msgs {
		if !seen[m.EventID] {
			seen[m.EventID] = true
			result = append(result, m)
		}
	}
	return result
}

// ---------------------------------------------------------------------------
// GlobMatch
// ---------------------------------------------------------------------------

// GlobMatch checks if name matches a simple glob pattern supporting * and ?.
//
// - '*' matches any sequence of characters (including empty)
// - '?' matches any single character
// - No escape characters
//
// This is a simple recursive/backtracking matcher (not regex-based),
// matching the Rust cmd_dump.rs glob_match behaviour.
// GlobMatch checks if name matches a simple glob pattern supporting * and ?.
//
// - '*' matches any sequence of characters (including empty)
// - '?' matches any single character
// - No escape characters
//
// This is a simple recursive/backtracking matcher (not regex-based),
// matching the Rust cmd_dump.rs glob_match behaviour.
func GlobMatch(pattern, name string) bool {
	// Empty pattern matches everything
	if pattern == "" {
		return true
	}

	px := 0 // pattern index
	nx := 0 // name index
	star := -1
	starNx := 0

	for nx < len(name) {
		if px < len(pattern) && (pattern[px] == '?' || pattern[px] == name[nx]) {
			px++
			nx++
		} else if px < len(pattern) && pattern[px] == '*' {
			star = px
			starNx = nx
			px++
		} else if star >= 0 {
			px = star + 1
			starNx++
			nx = starNx
		} else {
			return false
		}
	}

	// Consume trailing * in pattern
	for px < len(pattern) && pattern[px] == '*' {
		px++
	}
	return px == len(pattern)
}