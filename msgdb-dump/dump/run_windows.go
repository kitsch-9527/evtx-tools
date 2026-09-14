//go:build windows

package dump

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"

	"github.com/kitsch-9527/msgdb-dump/schema"
	wcorefxevtx "github.com/kitsch-9527/wcorefx/evtx"
	"github.com/kitsch-9527/wcorefx/fs"
	"github.com/kitsch-9527/wcorefx/reg"
	"golang.org/x/sys/windows/registry"

	_ "modernc.org/sqlite"
)

// paramRefEntry groups a provider name with its %%N parameter messages.
type paramRefEntry struct {
	ProviderName string
	Messages     map[uint32]string
}

// valueMapEntry groups a provider name with its value maps (VMAP/BMAP).
type valueMapEntry struct {
	ProviderName string
	ValueMaps    map[string]map[string]string
}

// ---------------------------------------------------------------------------
// Windows API fallback — extract messages via EvtFormatMessage
// ---------------------------------------------------------------------------

// extractMessagesViaWinAPI uses Windows Event Log API to extract message templates.
//
// When PE extraction fails (DLL has no RT_MESSAGETABLE/WEVT_TEMPLATE resources),
// this function calls EvtFormatMessage API to get message templates.
//
// Parameters:
//   - providerName: Provider name (e.g. "ESENT")
//   - langLCID: Language code (e.g. "zh-CN")
//
// Returns:
//   - map[uint32]string: EventID → message template
//   - nil: if extraction fails or no messages found
func extractMessagesViaWinAPI(providerName string, langLCID string) map[uint32]string {
	// Convert lang code to LCID
	lcid := LangCodeToLCID(langLCID)
	lcidInt, err := strconv.ParseUint(lcid, 10, 32)
	if err != nil {
		return nil
	}

	msgs := make(map[uint32]string)
	consecutiveFails := 0
	const maxConsecutiveFails = 50

	// Try common event ID ranges (1-1999)
	// Most Windows events are in the 1-999 range
	for eid := 1; eid <= 1999; eid++ {
		msg, err := wcorefxevtx.FormatMessageById(providerName, uint32(eid), uint32(lcidInt))
		if err != nil {
			consecutiveFails++
			if consecutiveFails >= maxConsecutiveFails {
				break // Too many consecutive failures, stop trying
			}
			continue
		}
		if msg != "" {
			msgs[uint32(eid)] = msg
			consecutiveFails = 0 // Reset on success
		}
	}

	if len(msgs) == 0 {
		return nil
	}
	return msgs
}

// RunDump is the main dump pipeline.
//
// Pipeline (matching Rust cmd_dump.rs run_dump):
//  1. Validate non-Windows at compile time
//  2. Create output directory
//  3. Parse language list (LCID -> language code)
//  4. Enumerate ETW providers from registry
//  5. Filter by --providers glob patterns
//  6. Group by unique DLL path (skip non-existent)
//  7. Extract messages from each DLL (sequential — KISS, no parallelism)
//  8. Build DllGroup array, collect param_refs and value_maps
//  9. CRIM+MUI fallback for event-manifest-only DLLs
//  10. Get OS info, create system DB
//  11. Import WELM seed data
//  12. Write parameter_refs for %%N expansion
//  13. Write value_maps for VMAP/BMAP resolution
//  14. Optionally write per-DLL databases
func RunDump(args *DumpArgs) error {
	// 1. Platform check (runtime guard for build-tag safety)
	if runtime.GOOS != "windows" {
		return fmt.Errorf("the dump command requires Windows registry access and is not available on %s", runtime.GOOS)
	}

	// 2. Create output directory
	if err := os.MkdirAll(args.OutDir, 0755); err != nil {
		return fmt.Errorf("failed to create output dir '%s': %w", args.OutDir, err)
	}

	// 3. Parse language list
	langParts := strings.Split(args.Lang, ",")
	langs := make([]string, 0, len(langParts))
	for _, l := range langParts {
		l = strings.TrimSpace(l)
		if l != "" {
			langs = append(langs, l)
		}
	}
	if len(langs) == 0 {
		return fmt.Errorf("--lang must specify at least one LCID")
	}
	langCodes := make([]string, len(langs))
	for i, l := range langs {
		langCodes[i] = LCIDToLangCode(l)
	}
	primaryCode := langCodes[0]

	fmt.Printf("[info] languages (LCID→code): ")
	for i := range langs {
		if i > 0 {
			fmt.Printf(", ")
		}
		fmt.Printf("%s→%s", langs[i], langCodes[i])
	}
	fmt.Println()

	// 4. Enumerate providers
	fmt.Println("[info] scanning registry for ETW providers...")
	allProviders, err := EnumETWProviders()
	if err != nil {
		return fmt.Errorf("failed to enumerate providers: %w", err)
	}
	fmt.Printf("[info] found %d total providers\n", len(allProviders))

	// 5. Filter providers by glob patterns
	var filtered []ProviderInfo
	if args.Providers != "" {
		patterns := strings.Split(args.Providers, ",")
		for i := range patterns {
			patterns[i] = strings.TrimSpace(patterns[i])
		}
		fmt.Printf("[info] filtering by %d pattern(s): %v\n", len(patterns), patterns)
		for _, p := range allProviders {
			for _, pat := range patterns {
				if pat != "" && GlobMatch(pat, p.Name) {
					filtered = append(filtered, p)
					break
				}
			}
		}
	} else {
		filtered = allProviders
	}
	if len(filtered) == 0 {
		return fmt.Errorf("no providers matched the filter criteria")
	}
	fmt.Printf("[info] matched %d providers\n", len(filtered))

	// 6. Group by unique DLL (skip non-existent paths)
	dllMap := make(map[string][]ProviderInfo)
	for _, p := range filtered {

		dllPath := fs.NormalizePath(p.DLLPath)
		if _, err := os.Stat(dllPath); os.IsNotExist(err) {
			continue
		}
		dllMap[dllPath] = append(dllMap[dllPath], p)
	}
	totalDLLs := len(dllMap)
	fmt.Printf("[info] unique DLLs: %d\n", totalDLLs)

	// 7. Sequential extraction from unique DLLs
	fmt.Printf("[info] extracting messages from %d unique DLLs...\n", totalDLLs)
	var dllResults []*DLLResult
	for dllPath, providers := range dllMap {
		result := ExtractSingleDLL(dllPath, providers, langs, langCodes)
		if result != nil {
			// Step 7.5: Windows API fallback
			// If PE extraction found no messages, try Windows Event Log API
			if len(result.MessagesByLang) == 0 {
				fmt.Printf("[info] PE extraction empty for '%s', trying Windows API fallback...\n", result.DLLName)
				for _, provider := range providers {
					for _, langCode := range langCodes {
						msgs := extractMessagesViaWinAPI(provider.Name, langCode)
						if len(msgs) > 0 {
							// Convert to PeMessage format
							var peMsgs []PeMessage
							for eid, msg := range msgs {
								peMsgs = append(peMsgs, PeMessage{
									EventID: eid,
									Message: msg,
								})
							}
							result.MessagesByLang[langCode] = append(
								result.MessagesByLang[langCode], peMsgs...)
							fmt.Printf("[info]   WinAPI: %d messages for '%s' (lang=%s)\n",
								len(peMsgs), provider.Name, langCode)
						}
					}
				}
			}
			dllResults = append(dllResults, result)
		}
	}
	if failed := totalDLLs - len(dllResults); failed > 0 {
		fmt.Printf("[warn] %d DLL(s) failed extraction\n", failed)
	}

	// 8. Build DllGroup array, collect param_refs and value_maps
	var dllGroups []DllGroup
	var allParamRefs []paramRefEntry
	var allValueMaps []valueMapEntry
	var allParamValueMaps []ParamMapRef
	var allParamTypes []ParamTypeRef

	for _, result := range dllResults {
		group := DllGroup{
			DLLName:        result.DLLName,
			DLLPath:        result.DLLPath,
			PEVersion:      result.Version,
			Providers:      result.Providers,
			MessagesByLang: result.MessagesByLang,
		}

		// Collect param messages for %%N expansion
		if len(result.ParamMessages) > 0 {
			for _, p := range result.Providers {
				allParamRefs = append(allParamRefs, paramRefEntry{
					ProviderName: p.Name,
					Messages:     result.ParamMessages,
				})
			}
		}

		// Collect value maps for VMAP/BMAP resolution
		if len(result.ValueMaps) > 0 {
			for _, p := range result.Providers {
				allValueMaps = append(allValueMaps, valueMapEntry{
					ProviderName: p.Name,
					ValueMaps:    result.ValueMaps,
				})
			}
		}

		// Collect per-parameter value map references (TEMP +8 → map)
		if len(result.ParamMapRefs) > 0 {
			allParamValueMaps = append(allParamValueMaps, result.ParamMapRefs...)
		}

		// Collect per-parameter TDH InType/OutType (R10 typed rendering)
		if len(result.ParamTypes) > 0 {
			allParamTypes = append(allParamTypes, result.ParamTypes...)
		}

		// 9. CRIM+MUI fallback: add messages to this group
		// Extract for ALL languages (not just primary)
		if len(group.MessagesByLang) == 0 && len(result.CrimBlobs) > 0 {
			fmt.Printf("[info] CRIM+MUI fallback for '%s' — %d provider(s)\n",
				result.DLLName, len(result.Providers))
			for _, provider := range result.Providers {
				for _, langCode := range langCodes {
					crimResult, err := extractMUICRIMFromBlobs(
						result.DLLPath,
						result.CrimBlobs,
						provider.Name,
						provider.GUID,
						langCode,
					)
					if err == nil && crimResult != nil && len(crimResult.Messages) > 0 {
						fmt.Printf("[info]   MUI+CRIM: %d messages for '%s' (lang=%s)\n",
							len(crimResult.Messages), provider.Name, langCode)
						group.MessagesByLang[langCode] = append(
							group.MessagesByLang[langCode], crimResult.Messages...)
						// Collect per-parameter value map references (first lang only,
						// matching the ValueMaps collection below).
						if langCode == langCodes[0] && len(crimResult.ParamMapRefs) > 0 {
							allParamValueMaps = append(allParamValueMaps, crimResult.ParamMapRefs...)
						}
						// Collect ValueMaps from CRIM+MUI fallback (only once)
						if langCode == langCodes[0] {
							if len(crimResult.ValueMaps) > 0 {
								for mapName, entries := range crimResult.ValueMaps {
									vm := make(map[string]string)
									for val, display := range entries {
										vm[strconv.FormatUint(uint64(val), 10)] = display
									}
									result.ValueMaps[provider.Name+"."+mapName] = vm
								}
							}
							if len(crimResult.BitMaps) > 0 {
								for mapName, entries := range crimResult.BitMaps {
									vm := make(map[string]string)
									for bitPos, display := range entries {
										vm[strconv.FormatUint(bitPos, 10)] = display
									}
									result.ValueMaps[provider.Name+"."+mapName+"_bits"] = vm
								}
							}
						}
					}
				}
			}
		}

		// Collect ValueMaps added during CRIM+MUI fallback
		if len(result.ValueMaps) > 0 {
			for _, p := range result.Providers {
				allValueMaps = append(allValueMaps, valueMapEntry{
					ProviderName: p.Name,
					ValueMaps:    result.ValueMaps,
				})
			}
		}

		// Collect ValueMaps from CRIM+MUI fallback (after the loop)
		if len(result.ValueMaps) > 0 {
			for _, p := range result.Providers {
				allValueMaps = append(allValueMaps, valueMapEntry{
					ProviderName: p.Name,
					ValueMaps:    result.ValueMaps,
				})
			}
		}

		dllGroups = append(dllGroups, group)
	}

	// Count total messages
	totalMsgs := 0
	langSet := make(map[string]bool)
	for _, g := range dllGroups {
		for lang, msgs := range g.MessagesByLang {
			totalMsgs += len(msgs)
			langSet[lang] = true
		}
	}
	if totalMsgs == 0 {
		return fmt.Errorf("no messages could be extracted from any provider DLL")
	}
	var langList []string
	for l := range langSet {
		langList = append(langList, l)
	}
	fmt.Printf("[info] extracted %d total messages across %d language(s): %v\n",
		totalMsgs, len(langList), langList)

	// 9.5 Inject well-known classic messages not covered by DLL extraction
	dllGroups = injectWellKnownMessages(dllGroups, langCodes)

	// 9.6 Import DLLs from remote machines
	if args.ImportDLL != "" {
		imports := importDLLDir(args.ImportDLL)
		if len(imports) > 0 {
			// Merge into dllGroups
			dllGroups = mergeImportResults(dllGroups, imports)
			fmt.Printf("[info] imported %d DLL(s) from %s\n", len(imports), args.ImportDLL)
		}
	}

	// 10. Get OS info and create the single-file system DB (schema_version 7,
	// ADR 0003). Provider-center variant extraction; cross-DLL same-name
	// providers are merged into one group (F1 orphan transplant).
	osInfo := GetOSInfo()
	fmt.Printf("[info] OS: %s %s (build %s)\n",
		osInfo.ProductName, osInfo.DisplayVersion, osInfo.BuildNumber)

	sysDBName := SystemDBFilename(osInfo)
	sysDBPath := filepath.Join(args.OutDir, sysDBName)

	variantGroups, variantValueMaps, variantParamRefs, variantParamTypes := BuildProviderVariantGroups(dllResults, langCodes)
	if len(variantGroups) == 0 {
		return fmt.Errorf("no provider variant groups could be built")
	}
	variantGroups = mergeDuplicateProvidersVariants(variantGroups)
	// Phase D (2026-09-14): inject well-known classic messages into variant
	// groups — currently EventLog 6013 (netevent.dll MUI RT_MESSAGETABLE
	// 0x8000177D), unreachable by the default extraction path.
	variantGroups = injectWellKnownVariants(variantGroups, langCodes)
	// Fresh build: remove any previous DB first. CREATE TABLE IF NOT EXISTS
	// would otherwise keep a stale layout (2026-09-08: param_name column
	// silently missing because the old file survived).
	if err := os.Remove(sysDBPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to remove old system db: %w", err)
	}
	if err := CreateSystemDB(sysDBPath, variantGroups, osInfo, langCodes); err != nil {
		return fmt.Errorf("failed to create system DB: %w", err)
	}
	fmt.Printf("[info] created single-file DB: %s (%d providers, %d variant rows)\n",
		sysDBPath, len(variantGroups), countVariantRows(variantGroups))

	// Fold variant-extracted maps into the single file via steps 12/13/13b.
	// variantValueMaps keys are already "Provider.MapName"; split off the
	// provider prefix for the entry's ProviderName.
	for fullKey, v := range variantValueMaps {
		providerName := fullKey
		if idx := strings.Index(fullKey, "."); idx >= 0 {
			providerName = fullKey[:idx]
		}
		allValueMaps = append(allValueMaps, valueMapEntry{ProviderName: providerName, ValueMaps: map[string]map[string]string{fullKey: v}})
	}
	if len(variantParamRefs) > 0 {
		allParamValueMaps = append(allParamValueMaps, variantParamRefs...)
	}
	if len(variantParamTypes) > 0 {
		allParamTypes = append(allParamTypes, variantParamTypes...)
	}

	// 11. Import WELM seed data
	if args.WithWELM != "" {
		if _, err := os.Stat(args.WithWELM); os.IsNotExist(err) {
			fmt.Printf("[warn] WELM seed file not found: %s\n", args.WithWELM)
		} else {
			fmt.Printf("[info] importing WELM seed from: %s\n", args.WithWELM)
			db, err := sql.Open("sqlite", sysDBPath)
			if err != nil {
				return fmt.Errorf("failed to open system db for welm import: %w", err)
			}
			imported, err := importWELMSeed(db, args.WithWELM)
			db.Close()
			if err != nil {
				fmt.Printf("[warn] WELM import: %v\n", err)
			} else {
				fmt.Printf("[info] imported %d WELM seed messages\n", imported)
			}
		}
	}

	// 11b. Win32 系统错误码（R15）：SCM 7000/7023/7038 的 %%2/%%8/%%21 等参数引用
	// 无 provider ParameterFile（SCM 注册项实测缺失），wevtapi 回退
	// FormatMessage(FROM_SYSTEM)。parameter_refs 现只有 en-US（与现有 5732 行口径一致），
	// 全局 provider 名写入（查询不分 provider，load_param_refs 全表加载）。
	if sysErrs := collectSystemErrorMessages(16000, 1033); len(sysErrs) > 0 {
		fmt.Printf("[info] Win32 system errors for %%N fallback (en-US): %d entries\n", len(sysErrs))
		allParamRefs = append(allParamRefs, paramRefEntry{
			ProviderName: "Win32-System-Error",
			Messages:     sysErrs,
		})
	}

	// 12. Write parameter_refs for %%N expansion
	if len(allParamRefs) > 0 {
		if err := writeParamRefs(sysDBPath, allParamRefs, primaryCode); err != nil {
			return fmt.Errorf("failed to write parameter references: %w", err)
		}
	}

	// 13. Write value_maps for VMAP/BMAP resolution
	if len(allValueMaps) > 0 {
		if err := writeValueMaps(sysDBPath, allValueMaps); err != nil {
			return fmt.Errorf("failed to write value maps: %w", err)
		}
	}

	// 13b. Write param_value_maps linking event parameters to their maps
	// (param_name carried for template-scoped verification at render time).
	if len(allParamValueMaps) > 0 {
		if err := writeParamValueMaps(sysDBPath, allParamValueMaps); err != nil {
			return fmt.Errorf("failed to write param value maps: %w", err)
		}
	}

	// 13c. Write param_types (R10 / ADR 0005): TDH InType/OutType per
	// (provider, event_id, param_index, param_name) for typed rendering.
	if len(allParamTypes) > 0 {
		if err := writeParamTypes(sysDBPath, allParamTypes); err != nil {
			return fmt.Errorf("failed to write param types: %w", err)
		}
	}

	// 14. Registry event source provenance (R12 / ADR 0004): classic EventLog
	// sources union WINEVT publisher names. Optional table; readers treat an
	// empty snapshot as "unknown" and keep legacy fallback rendering.
	if sources, err := CollectEventlogSources(); err != nil {
		fmt.Printf("[warn] eventlog source collection: %v\n", err)
	} else if err := WriteEventlogSources(sysDBPath, sources); err != nil {
		return fmt.Errorf("failed to write eventlog sources: %w", err)
	} else {
		fmt.Printf("[info] wrote %d registered eventlog sources\n", len(sources))
	}

	fmt.Printf("[done] dump complete: %s\n", sysDBPath)
	return nil
}

func countVariantRows(groups []ProviderVariantGroup) int {
	n := 0
	for _, g := range groups {
		for _, rows := range g.VariantsByLang {
			n += len(rows)
		}
	}
	return n
}


// ---------------------------------------------------------------------------
// OS info
// ---------------------------------------------------------------------------

// GetOSInfo reads OS identification from the Windows registry.
//
// Registry path: HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion
//   - ProductName   -> OsInfo.ProductName
//   - DisplayVersion -> OsInfo.DisplayVersion
//   - CurrentBuild   -> base build number
//   - UBR (DWORD)    -> append as .UBR suffix
//   - CurrentArchitecture -> OsInfo.Architecture
func GetOSInfo() OsInfo {
	info := OsInfo{
		ProductName:    "Windows",
		DisplayVersion: "0.0",
		BuildNumber:    "0",
	}
	arch := runtime.GOARCH
	info.Architecture = &arch

	const ntvPath = `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`
	k, err := reg.OpenKeyWithFlags(ntvPath, registry.QUERY_VALUE|reg.KEY_WOW64_64KEY)
	if err != nil {
		return info
	}
	defer k.Close()

	if v, _, err := k.GetStringValue("ProductName"); err == nil && v != "" {
		info.ProductName = v
	}
	if v, _, err := k.GetStringValue("DisplayVersion"); err == nil && v != "" {
		info.DisplayVersion = v
	}
	if v, _, err := k.GetStringValue("CurrentBuild"); err == nil && v != "" {
		info.BuildNumber = v
		if ubr, _, err := k.GetIntegerValue("UBR"); err == nil && ubr > 0 {
			info.BuildNumber = fmt.Sprintf("%s.%d", v, ubr)
		}
	}
	if v, _, err := k.GetStringValue("CurrentArchitecture"); err == nil && v != "" {
		info.Architecture = &v
	}

	return info
}

// ---------------------------------------------------------------------------
// WELM seed import
// ---------------------------------------------------------------------------

// importWELMSeed imports data from a WELM seed database into the system DB.
//
// Only seeds with metadata schema_version=7 are accepted (ADR 0003
// single-file). Copies the 7 entity tables with INSERT OR IGNORE, resolving
// dll_id by dll_path via the existing JOIN logic.
//
// Uses ATTACH DATABASE to copy from the seed into the already-open system DB.
func importWELMSeed(db *sql.DB, welmPath string) (uint64, error) {
	// ATTACH the seed database
	_, err := db.Exec(fmt.Sprintf("ATTACH DATABASE '%s' AS seed", escapeSQL(welmPath)))
	if err != nil {
		return 0, fmt.Errorf("attach seed db: %w", err)
	}
	defer db.Exec("DETACH DATABASE seed")

	// Only schema_version=7 seeds are accepted.
	var schemaVersion string
	err = db.QueryRow("SELECT value FROM seed.metadata WHERE key = 'schema_version'").Scan(&schemaVersion)
	if err != nil {
		return 0, fmt.Errorf("welm seed has no schema_version: %w", err)
	}
	if schemaVersion != "7" {
		return 0, fmt.Errorf("welm seed schema_version=%s not supported (want 7)", schemaVersion)
	}

	// Count rows in seed
	var seedCount int
	err = db.QueryRow("SELECT COUNT(*) FROM seed.provider_message_variants").Scan(&seedCount)
	if err != nil || seedCount == 0 {
		return 0, nil
	}

	// Import in a transaction
	if _, err := db.Exec("BEGIN"); err != nil {
		return 0, fmt.Errorf("begin welm txn: %w", err)
	}

	// Step 1: Insert dll_entries (IGNORE keeps existing dll_id on path match)
	if _, err := db.Exec(`INSERT OR IGNORE INTO dll_entries
		(dll_name, dll_path, pe_version)
		SELECT dll_name, dll_path, pe_version FROM seed.dll_entries`); err != nil {
		db.Exec("ROLLBACK")
		return 0, fmt.Errorf("import dll_entries: %w", err)
	}

	// Step 2: Insert providers, resolving dll_id to match existing entries
	if _, err := db.Exec(`INSERT OR IGNORE INTO providers
		(provider_name, guid, channel, dll_id)
		SELECT s.provider_name, s.guid, s.channel,
			COALESCE(e.dll_id, s.dll_id)
		FROM seed.providers s
		LEFT JOIN dll_entries e ON e.dll_path = (
			SELECT dll_path FROM seed.dll_entries WHERE dll_id = s.dll_id
		)`); err != nil {
		db.Exec("ROLLBACK")
		return 0, fmt.Errorf("import providers: %w", err)
	}

	// Step 3: Insert message variants, resolving dll_id the same way
	if _, err := db.Exec(`INSERT OR IGNORE INTO provider_message_variants
		(provider_name, provider_guid, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id, message, parameters, level_name, keywords_name)
		SELECT s.provider_name, s.provider_guid, s.lang, s.event_id, s.version, s.level, s.opcode, s.task, s.keywords, s.channel, s.message_identifier,
			COALESCE(e.dll_id, s.dll_id), s.message, s.parameters, s.level_name, s.keywords_name
		FROM seed.provider_message_variants s
		LEFT JOIN dll_entries e ON e.dll_path = (
			SELECT dll_path FROM seed.dll_entries WHERE dll_id = s.dll_id
		)`); err != nil {
		db.Exec("ROLLBACK")
		return 0, fmt.Errorf("import provider_message_variants: %w", err)
	}

	// Step 4: metadata (keep local identity; seeds only contribute new keys)
	if _, err := db.Exec(`INSERT OR IGNORE INTO metadata (key, value)
		SELECT key, value FROM seed.metadata WHERE key NOT IN ('schema_version', 'created_at', 'langs', 'product_name', 'display_version', 'build_number', 'architecture')`); err != nil {
		db.Exec("ROLLBACK")
		return 0, fmt.Errorf("import metadata: %w", err)
	}

	// Step 5: parameter_refs
	if _, err := db.Exec(`INSERT OR IGNORE INTO parameter_refs
		(provider_name, param_id, message, lang)
		SELECT provider_name, param_id, message, lang FROM seed.parameter_refs`); err != nil {
		db.Exec("ROLLBACK")
		return 0, fmt.Errorf("import parameter_refs: %w", err)
	}

	// Step 6: value_maps
	if _, err := db.Exec(`INSERT OR IGNORE INTO value_maps
		(provider_name, map_name, entry_value, display_name, is_bitmap)
		SELECT provider_name, map_name, entry_value, display_name, is_bitmap FROM seed.value_maps`); err != nil {
		db.Exec("ROLLBACK")
		return 0, fmt.Errorf("import value_maps: %w", err)
	}

	// Step 7: param_value_maps
	if _, err := db.Exec(`INSERT OR IGNORE INTO param_value_maps
		(provider_name, event_id, param_index, map_name, is_bitmap)
		SELECT provider_name, event_id, param_index, map_name, is_bitmap FROM seed.param_value_maps`); err != nil {
		db.Exec("ROLLBACK")
		return 0, fmt.Errorf("import param_value_maps: %w", err)
	}

	if _, err := db.Exec("COMMIT"); err != nil {
		return 0, fmt.Errorf("commit welm import: %w", err)
	}

	return uint64(seedCount), nil
}

// ---------------------------------------------------------------------------
// Sub-steps: write param_refs, value_maps, param_value_maps
// ---------------------------------------------------------------------------

// writeParamRefs writes parameter references into the system DB.
func writeParamRefs(dbPath string, refs []paramRefEntry, primaryCode string) error {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return fmt.Errorf("open db for param_refs: %w", err)
	}
	defer db.Close()

	if _, err := db.Exec(schema.BulkInsertPragmas); err != nil {
		return fmt.Errorf("pragmas param_refs: %w", err)
	}
	if _, err := db.Exec(`CREATE TABLE IF NOT EXISTS parameter_refs (
		provider_name TEXT NOT NULL,
		param_id      INTEGER NOT NULL,
		message       TEXT NOT NULL,
		lang          TEXT NOT NULL DEFAULT 'en-US',
		PRIMARY KEY (provider_name, param_id, lang)
	)`); err != nil {
		return fmt.Errorf("create parameter_refs table: %w", err)
	}
	if _, err := db.Exec("BEGIN"); err != nil {
		return fmt.Errorf("begin param_refs txn: %w", err)
	}
	written := 0
	for _, ref := range refs {
		for paramID, message := range ref.Messages {
			if err := InsertParameterRef(db, ref.ProviderName, int(paramID), message, primaryCode); err != nil {
				fmt.Printf("[warn] param_ref '%s/%%%d': %v\n", ref.ProviderName, paramID, err)
			} else {
				written++
			}
		}
	}
	if _, err := db.Exec("COMMIT"); err != nil {
		return fmt.Errorf("commit param_refs: %w", err)
	}
	fmt.Printf("[info] wrote %d parameter references for %%N expansion\n", written)
	return nil
}

// writeValueMaps writes value map entries into the system DB.
func writeValueMaps(dbPath string, maps []valueMapEntry) error {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return fmt.Errorf("open db for value_maps: %w", err)
	}
	defer db.Close()

	if _, err := db.Exec(schema.BulkInsertPragmas); err != nil {
		return fmt.Errorf("pragmas value_maps: %w", err)
	}
	if _, err := db.Exec(`CREATE TABLE IF NOT EXISTS value_maps (
		provider_name TEXT NOT NULL,
		map_name      TEXT NOT NULL,
		entry_value   TEXT NOT NULL,
		display_name  TEXT NOT NULL,
		is_bitmap     INTEGER NOT NULL DEFAULT 0,
		PRIMARY KEY (provider_name, map_name, entry_value)
	)`); err != nil {
		return fmt.Errorf("create value_maps table: %w", err)
	}
	if _, err := db.Exec("BEGIN"); err != nil {
		return fmt.Errorf("begin value_maps txn: %w", err)
	}
	written := 0
	for _, vm := range maps {
		for mapName, entries := range vm.ValueMaps {
			isBitmap := strings.HasSuffix(mapName, "_bits")
			for entryValue, displayName := range entries {
				if err := InsertValueMap(db, vm.ProviderName, mapName, entryValue, displayName, isBitmap); err != nil {
					fmt.Printf("[warn] value_map '%s' for '%s': %v\n", mapName, vm.ProviderName, err)
				} else {
					written++
				}
			}
		}
	}
	if _, err := db.Exec("COMMIT"); err != nil {
		return fmt.Errorf("commit value_maps: %w", err)
	}
	fmt.Printf("[info] wrote %d value map entries for VMAP/BMAP resolution\n", written)
	return nil
}

// writeParamTypes writes the per-parameter TDH InType/OutType hints (R10,
// ADR 0005) into the optional param_types table.
func writeParamTypes(dbPath string, refs []ParamTypeRef) error {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return fmt.Errorf("open db for param_types: %w", err)
	}
	defer db.Close()

	if _, err := db.Exec(schema.BulkInsertPragmas); err != nil {
		return fmt.Errorf("pragmas param_types: %w", err)
	}
	if _, err := db.Exec(`CREATE TABLE IF NOT EXISTS param_types (
		provider_name TEXT NOT NULL,
		event_id      INTEGER NOT NULL,
		param_index   TEXT NOT NULL,
		in_type       INTEGER NOT NULL,
		out_type      INTEGER NOT NULL,
		PRIMARY KEY (provider_name, event_id, param_index)
	)`); err != nil {
		return fmt.Errorf("create param_types table: %w", err)
	}
	if _, err := db.Exec("BEGIN"); err != nil {
		return fmt.Errorf("begin param_types txn: %w", err)
	}
	written := 0
	for _, ref := range refs {
		if _, err := db.Exec(
			"INSERT OR IGNORE INTO param_types(provider_name, event_id, param_index, param_name, in_type, out_type) VALUES(?,?,?,?,?,?)",
			ref.ProviderName, ref.EventID, ref.ParamIndex, ref.ParamName, ref.InType, ref.OutType,
		); err != nil {
			fmt.Printf("[warn] param_type eid=%d idx=%s: %v\n", ref.EventID, ref.ParamIndex, err)
			continue
		}
		written++
	}
	if _, err := db.Exec("COMMIT"); err != nil {
		return fmt.Errorf("commit param_types: %w", err)
	}
	fmt.Printf("[info] wrote %d param type hints for typed rendering\n", written)
	return nil
}

// writeParamValueMaps writes the parameter → value map association table.
//
// Each row links (provider_name, event_id, param_index) to the value_maps key
// holding that parameter's display entries. This is the compiled form of the
// manifest's <data name="..." map="..."/> association, extracted from the
// CRIM TEMP item descriptor +8 field.
func writeParamValueMaps(dbPath string, refs []ParamMapRef) error {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return fmt.Errorf("open db for param_value_maps: %w", err)
	}
	defer db.Close()

	if _, err := db.Exec(schema.BulkInsertPragmas); err != nil {
		return fmt.Errorf("pragmas param_value_maps: %w", err)
	}
	if _, err := db.Exec(`CREATE TABLE IF NOT EXISTS param_value_maps (
		provider_name TEXT NOT NULL,
		event_id      INTEGER NOT NULL,
		param_index   TEXT NOT NULL,
		map_name      TEXT NOT NULL,
		is_bitmap     INTEGER NOT NULL DEFAULT 0,
		PRIMARY KEY (provider_name, event_id, param_index)
	)`); err != nil {
		return fmt.Errorf("create param_value_maps table: %w", err)
	}
	if _, err := db.Exec("BEGIN"); err != nil {
		return fmt.Errorf("begin param_value_maps txn: %w", err)
	}
	written := 0
	for _, ref := range refs {
		bitmap := 0
		if ref.IsBitmap {
			bitmap = 1
		}
		if _, err := db.Exec(
			"INSERT OR IGNORE INTO param_value_maps(provider_name, event_id, param_index, param_name, map_name, is_bitmap) VALUES(?,?,?,?,?,?)",
			ref.ProviderName, ref.EventID, ref.ParamIndex, ref.ParamName, ref.MapName, bitmap,
		); err != nil {
			fmt.Printf("[warn] param_value_map eid=%d idx=%s: %v\n", ref.EventID, ref.ParamIndex, err)
			continue
		}
		written++
	}
	if _, err := db.Exec("COMMIT"); err != nil {
		return fmt.Errorf("commit param_value_maps: %w", err)
	}
	fmt.Printf("[info] wrote %d param value map references\n", written)
	return nil
}

