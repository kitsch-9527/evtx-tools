package dump

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/kitsch-9527/msgdb-dump/pe"
)

// DLLImportResult holds data extracted from a single imported DLL.
type DLLImportResult struct {
	DLLName        string
	DLLPath        string
	ProviderName   string
	MessagesByLang map[string][]PeMessage
}

// importDLLDir scans a directory for DLL files and extracts their messages.
// Returns a slice of DLLImportResult, one per successfully scanned DLL.
func importDLLDir(dir string) []*DLLImportResult {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}

	var results []*DLLImportResult
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		ext := strings.ToLower(filepath.Ext(entry.Name()))
		if ext != ".dll" && ext != ".exe" && ext != ".sys" {
			continue
		}
		fullPath := filepath.Join(dir, entry.Name())
		result := importDLL(fullPath)
		if result != nil && len(result.MessagesByLang["en-US"]) > 0 {
			results = append(results, result)
		}
	}
	return results
}

// importDLL extracts message table data from a single DLL file.
// Returns a DLLImportResult with the provider name derived from the DLL filename.
// If the LN DLL has no messages, tries the en-US MUI companion.
func importDLL(dllPath string) *DLLImportResult {
	if _, err := os.Stat(dllPath); os.IsNotExist(err) {
		return nil
	}

	// Derive provider name from DLL filename (stem only)
	baseName := filepath.Base(dllPath)
	providerName := strings.TrimSuffix(baseName, filepath.Ext(baseName))

	result := &DLLImportResult{
		DLLName:        baseName,
		DLLPath:        dllPath,
		ProviderName:   providerName,
		MessagesByLang: make(map[string][]PeMessage),
	}

	// Extract from the LN DLL first
	combined, err := pe.ExtractAllFromPE(dllPath)
	if err == nil && len(combined.Messages) > 0 {
		msgs := buildPeMessages(combined.Messages, nil, nil)
		result.MessagesByLang["en-US"] = msgs
		return result
	}

	// LN DLL had no messages — try MUI companion
	muiPath := FindMUICompanion(dllPath, "en-US")
	if muiPath != nil {
		muiCombined, err := pe.ExtractAllFromPE(*muiPath)
		if err == nil && len(muiCombined.Messages) > 0 {
			msgs := buildPeMessages(muiCombined.Messages, nil, nil)
			result.MessagesByLang["en-US"] = msgs
			return result
		}
	}

	// No messages found at all
	return result
}

// mergeImportResults merges DLLImportResult entries into the existing DllGroup list.
// Each result creates or reuses a DllGroup, adds a provider entry and stores messages.
func mergeImportResults(groups []DllGroup, imports []*DLLImportResult) []DllGroup {
	if len(imports) == 0 {
		return groups
	}

	// Build lookup: normalized path → existing group index
	existingByPath := make(map[string]int)
	for i, g := range groups {
		normalized := strings.ToLower(filepath.ToSlash(g.DLLPath))
		existingByPath[normalized] = i
	}

	for _, imp := range imports {
		if len(imp.MessagesByLang["en-US"]) == 0 {
			continue
		}

		normalized := strings.ToLower(filepath.ToSlash(imp.DLLPath))
		providerName := imp.ProviderName

		if idx, ok := existingByPath[normalized]; ok {
			// DLL already in DB — merge messages if new
			group := &groups[idx]
			existingEnUS := group.MessagesByLang["en-US"]

			// Check if this provider already exists
			providerExists := false
			for _, p := range group.Providers {
				if p.Name == providerName {
					providerExists = true
					break
				}
			}
			if !providerExists {
				group.Providers = append(group.Providers, ProviderInfo{
					Name:    providerName,
					DLLPath: imp.DLLPath,
				})
			}

			// Merge new messages (dedup by event_id)
			seenEID := make(map[uint32]bool)
			for _, m := range existingEnUS {
				seenEID[m.EventID] = true
			}
			for _, m := range imp.MessagesByLang["en-US"] {
				if !seenEID[m.EventID] {
					existingEnUS = append(existingEnUS, m)
					seenEID[m.EventID] = true
				}
			}
			group.MessagesByLang["en-US"] = existingEnUS
		} else {
			// New DLL — create new group
			group := DllGroup{
				DLLName:   imp.DLLName,
				DLLPath:   imp.DLLPath,
				PEVersion: "0.0.0.0",
				Providers: []ProviderInfo{
					{Name: providerName, DLLPath: imp.DLLPath},
				},
				MessagesByLang: imp.MessagesByLang,
			}
			groups = append(groups, group)
		}
	}

	return groups
}

// formatImportSummary prints a human-readable summary of imported DLLs.
func formatImportSummary(imports []*DLLImportResult) string {
	if len(imports) == 0 {
		return "  (no DLLs imported)"
	}
	var b strings.Builder
	for _, imp := range imports {
		enUS := len(imp.MessagesByLang["en-US"])
		if enUS > 0 {
			fmt.Fprintf(&b, "  %s → provider %q (%d messages)\n",
				imp.DLLName, imp.ProviderName, enUS)
		} else {
			fmt.Fprintf(&b, "  %s → no messages extracted\n", imp.DLLName)
		}
	}
	return b.String()
}
