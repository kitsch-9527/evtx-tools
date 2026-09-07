//go:build windows

package dump

import (
	"golang.org/x/sys/windows/registry"
)

// CollectEventlogSources snapshots registered event source names from the
// registry (R12, ADR 0004): every classic EventLog channel subkey name plus
// every WINEVT Publishers default-value (provider) name. The parity renderer
// needs only the name set; Type records provenance. Classic side enumerates
// all channels on purpose — over-registering can only make the renderer
// under-fire (legacy fallback), never over-fire (wrong N/A).
func CollectEventlogSources() ([]EventlogSource, error) {
	seen := map[string]string{}

	const evtlogRoot = `SYSTEM\CurrentControlSet\Services\EventLog`
	if k, err := registry.OpenKey(registry.LOCAL_MACHINE,
		evtlogRoot,
		registry.ENUMERATE_SUB_KEYS|registry.WOW64_64KEY); err == nil {
		channels, err2 := k.ReadSubKeyNames(-1)
		k.Close()
		if err2 == nil {
			for _, ch := range channels {
				// Open by full path: the parent handle is already closed, and
				// a closed-handle subkey open fails silently (2026-09-07 bug).
				ck, err3 := registry.OpenKey(registry.LOCAL_MACHINE, evtlogRoot+`\`+ch,
					registry.ENUMERATE_SUB_KEYS|registry.WOW64_64KEY)
				if err3 != nil {
					continue
				}
				names, err4 := ck.ReadSubKeyNames(-1)
				ck.Close()
				if err4 != nil {
					continue
				}
				for _, n := range names {
					if n != "" {
						seen[n] = "classic"
					}
				}
			}
		}
	}

	const publishersRoot = `SOFTWARE\Microsoft\Windows\CurrentVersion\WINEVT\Publishers`
	if pk, err := registry.OpenKey(registry.LOCAL_MACHINE,
		publishersRoot,
		registry.ENUMERATE_SUB_KEYS|registry.WOW64_64KEY); err == nil {
		guids, err2 := pk.ReadSubKeyNames(-1)
		pk.Close()
		if err2 == nil {
			for _, g := range guids {
				sk, err3 := registry.OpenKey(registry.LOCAL_MACHINE, publishersRoot+`\`+g,
					registry.QUERY_VALUE|registry.WOW64_64KEY)
				if err3 != nil {
					continue
				}
				name, _, err4 := sk.GetStringValue("") // default value = provider name
				sk.Close()
				if err4 == nil && name != "" {
					if _, dup := seen[name]; !dup {
						seen[name] = "winevt"
					}
				}
			}
		}
	}

	sources := make([]EventlogSource, 0, len(seen))
	for name, typ := range seen {
		sources = append(sources, EventlogSource{Name: name, Type: typ})
	}
	return sources, nil
}
