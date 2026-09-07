//go:build windows

package dump

import (
	"os"
	"strings"

	"golang.org/x/sys/windows/registry"

	"github.com/kitsch-9527/wcorefx/reg"
)

// EnumETWProviders enumerates all event providers from the Windows registry.
// Scans both HKLM\SYSTEM\...\EventLog and HKLM\SOFTWARE\...\WINEVT\Publishers.
// Deduplicates by (name, dll_path).
func EnumETWProviders() ([]ProviderInfo, error) {
	var providers []ProviderInfo

	if p, err := scanEventLog(); err == nil {
		providers = append(providers, p...)
	}
	if p, err := scanWINEVTPublishers(); err == nil {
		providers = append(providers, p...)
	}

	// Dedup by (name, dll_path)
	seen := make(map[string]bool)
	deduped := make([]ProviderInfo, 0, len(providers))
	for _, p := range providers {
		key := p.Name + "|" + p.DLLPath
		if !seen[key] {
			seen[key] = true
			deduped = append(deduped, p)
		}
	}
	return deduped, nil
}

// ---------------------------------------------------------------------------
// EventLog scan: HKLM\SYSTEM\CurrentControlSet\Services\EventLog
// ---------------------------------------------------------------------------

func scanEventLog() ([]ProviderInfo, error) {
	var providers []ProviderInfo

	const eventLogPath = `HKLM\SYSTEM\CurrentControlSet\Services\EventLog`
	k, err := reg.OpenKeyWithFlags(eventLogPath,
		registry.QUERY_VALUE|registry.ENUMERATE_SUB_KEYS|reg.KEY_WOW64_64KEY)
	if err != nil {
		return nil, err
	}
	defer k.Close()

	channels, err := k.ReadSubKeyNames(0)
	if err != nil {
		return nil, err
	}

	for _, channel := range channels {
		chPath := eventLogPath + `\` + channel
		ck, err := reg.OpenKeyWithFlags(chPath,
			registry.QUERY_VALUE|registry.ENUMERATE_SUB_KEYS|reg.KEY_WOW64_64KEY)
		if err != nil {
			continue
		}

		// Channel key itself may have EventMessageFile
		dll := readRegExpandString(ck, "EventMessageFile")
		if dll != "" {
			for _, dllPath := range strings.Split(dll, ";") {
				trimmed := strings.TrimSpace(dllPath)
				if trimmed != "" {
					providers = append(providers, ProviderInfo{
						Name:    channel,
						DLLPath: trimmed,
						Channel: strPtr(channel),
					})
				}
			}
		}

		// Enumerate sources (subkeys of channel)
		sources, err := ck.ReadSubKeyNames(0)
		if err == nil {
			for _, source := range sources {
				srcPath := chPath + `\` + source
				sk, err := reg.OpenKeyWithFlags(srcPath,
					registry.QUERY_VALUE|registry.ENUMERATE_SUB_KEYS|reg.KEY_WOW64_64KEY)
				if err != nil {
					continue
				}

				dll := readRegExpandString(sk, "EventMessageFile")
				guid := readRegString(sk, "ProviderGuid")
				paramFile := readRegExpandString(sk, "ParameterMessageFile")

				if dll != "" {
					for _, dllPath := range strings.Split(dll, ";") {
						trimmed := strings.TrimSpace(dllPath)
						if trimmed != "" {
							providers = append(providers, ProviderInfo{
								Name:          source,
								GUID:          strPtr(guid),
								DLLPath:       trimmed,
								Channel:       strPtr(channel),
								ParameterFile: strPtr(paramFile),
							})
						}
					}
				}

				sk.Close()
			}
		}

		ck.Close()
	}

	return providers, nil
}

// ---------------------------------------------------------------------------
// WINEVT Publishers scan: HKLM\SOFTWARE\...\WINEVT\Publishers
// ---------------------------------------------------------------------------

func scanWINEVTPublishers() ([]ProviderInfo, error) {
	var providers []ProviderInfo

	const pubsPath = `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\WINEVT\Publishers`
	k, err := reg.OpenKeyWithFlags(pubsPath,
		registry.QUERY_VALUE|registry.ENUMERATE_SUB_KEYS|reg.KEY_WOW64_64KEY)
	if err != nil {
		return nil, err
	}
	defer k.Close()

	guids, err := k.ReadSubKeyNames(0)
	if err != nil {
		return nil, err
	}

	for _, guid := range guids {
		pubPath := pubsPath + `\` + guid
		pk, err := reg.OpenKeyWithFlags(pubPath,
			registry.QUERY_VALUE|registry.ENUMERATE_SUB_KEYS|reg.KEY_WOW64_64KEY)
		if err != nil {
			continue
		}

		// FriendlyName is the display name; fall back to ProviderName or GUID
		name := readRegString(pk, "FriendlyName")
		if name == "" {
			name = readRegString(pk, "ProviderName")
		}
		if name == "" {
			// Try the default (unnamed) value of the key.
			name = readRegString(pk, "")
		}
		if name == "" {
			name = guid
		}

		msgFile := readRegExpandString(pk, "MessageFileName")
		resourceFile := readRegExpandString(pk, "ResourceFileName")
		paramFile := readRegExpandString(pk, "ParameterFileName")

		// Use MessageFileName as primary, ResourceFileName as fallback,
		// then ParameterFileName as last resort
		dllFiles := msgFile
		if dllFiles == "" {
			dllFiles = resourceFile
		}
		if dllFiles == "" {
			dllFiles = paramFile
		}

		if dllFiles != "" {
			for _, dllPath := range strings.Split(dllFiles, ";") {
				trimmed := strings.TrimSpace(dllPath)
				if trimmed != "" {
					providers = append(providers, ProviderInfo{
						Name:          name,
						GUID:          strPtr(guid),
						DLLPath:       trimmed,
						ParameterFile: strPtr(paramFile),
					})
				}
			}
		}

		pk.Close()
	}

	return providers, nil
}

// ---------------------------------------------------------------------------
// Registry value helpers
// ---------------------------------------------------------------------------

// readRegExpandString reads a registry string value, expanding %VAR%
// references if the value is REG_EXPAND_SZ.
// Uses manual %VAR% expansion (unlike os.Expand which uses $VAR syntax).
func readRegExpandString(k registry.Key, name string) string {
	s, valType, err := k.GetStringValue(name)
	if err != nil {
		return ""
	}
	if valType == registry.EXPAND_SZ {
		return expandWindowsVars(s)
	}
	return s
}

// expandWindowsVars replaces %VAR% patterns (Windows-style) with environment
// variable values, matching Rust's resolve_env_vars behavior.
func expandWindowsVars(s string) string {
	var result strings.Builder
	result.Grow(len(s))
	for i := 0; i < len(s); {
		if s[i] == '%' {
			end := strings.IndexByte(s[i+1:], '%')
			if end >= 0 {
				varName := s[i+1 : i+1+end]
				if val, ok := os.LookupEnv(varName); ok {
					result.WriteString(val)
				}
				i += 2 + end // skip %VAR%
				continue
			}
		}
		result.WriteByte(s[i])
		i++
	}
	return result.String()
}

// readRegString reads a REG_SZ value without environment expansion.
func readRegString(k registry.Key, name string) string {
	s, _, err := k.GetStringValue(name)
	if err != nil {
		return ""
	}
	return s
}

// strPtr returns a pointer to s, or nil if s is empty.
// Used to convert Go strings to *string for optional fields.
func strPtr(s string) *string {
	if s == "" {
		return nil
	}
	return &s
}
