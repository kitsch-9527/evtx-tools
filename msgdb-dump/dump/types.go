package dump

// VariantRow is the 8-tuple Message Variant (CONTEXT.md: Variant Identity).
// One row per (provider, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier).
type VariantRow struct {
	ProviderName      string             `json:"provider_name"`
	ProviderGUID      string             `json:"provider_guid"`
	Lang              string             `json:"lang"`
	EventID           uint32             `json:"event_id"`
	Version           uint8              `json:"version"`
	Level             uint8              `json:"level"`
	Opcode            uint8              `json:"opcode"`
	Task              uint16             `json:"task"`
	Keywords          uint64             `json:"keywords"`
	Channel           uint8              `json:"channel"`
	MessageID         uint32             `json:"message_identifier"`
	DLLName           string             `json:"dll_name"`
	DLLPath           string             `json:"dll_path"`
	PEVersion         string             `json:"pe_version"`
	Message           string             `json:"message"`
	Parameters        *map[string]string `json:"parameters,omitempty"`
	LevelName         *string            `json:"level_name,omitempty"`
	KeywordsName      *string            `json:"keywords_name,omitempty"`
	TaskName          *string            `json:"task_name,omitempty"`
	OpcodeName        *string            `json:"opcode_name,omitempty"`
}

// PeMessage represents a single message extracted from a DLL's RT_MESSAGETABLE
// or CRIM blob. All optional fields use *T to match Rust's Option<T>.
// Deprecated for variant-aware paths — VariantRow is the new seam (ADR 0001).
type PeMessage struct {
	EventID    uint32             `json:"event_id"`
	Message    string             `json:"message"`
	Level      *uint8             `json:"level,omitempty"`
	Task       *string            `json:"task,omitempty"`
	Opcode     *string            `json:"opcode,omitempty"`
	Keywords   *string            `json:"keywords,omitempty"`
	Parameters *map[string]string `json:"parameters,omitempty"`
	// ProviderName is set by the caller to associate this message with a provider.
	ProviderName *string `json:"provider_name,omitempty"`
	LevelName    *string `json:"level_name,omitempty"`
	KeywordsName *string `json:"keywords_name,omitempty"`
}

// ProviderInfo describes one event provider from a registry scan.
type ProviderInfo struct {
	Name          string  `json:"name"`
	GUID          *string `json:"guid,omitempty"`
	DLLPath       string  `json:"dll_path"`
	Channel       *string `json:"channel,omitempty"`
	ParameterFile *string `json:"parameter_file,omitempty"`
}

// DllGroup groups providers that share the same DLL, with messages per language.
// Deprecated for variant-aware pipeline — ProviderVariantGroup is the new seam (ADR 0001/Q2=A).
type DllGroup struct {
	DLLName        string                `json:"dll_name"`
	DLLPath        string                `json:"dll_path"`
	PEVersion      string                `json:"pe_version"`
	Providers      []ProviderInfo        `json:"providers"`
	MessagesByLang map[string][]PeMessage `json:"messages_by_lang"`
}

// ProviderVariantGroup is the Provider-center grouping (Q2=A): one Provider
// with its 8-tuple VariantRows per language, isolated per GUID (no DllGroup contamination).
type ProviderVariantGroup struct {
	Provider       ProviderInfo `json:"provider"`
	DLLName        string       `json:"dll_name"`
	DLLPath        string       `json:"dll_path"`
	PEVersion      string       `json:"pe_version"`
	VariantsByLang map[string][]VariantRow `json:"variants_by_lang"`
}

// OsInfo identifies the operating system for DB metadata.
type OsInfo struct {
	ProductName    string  `json:"product_name"`
	DisplayVersion string  `json:"display_version"`
	BuildNumber    string  `json:"build_number"`
	Architecture   *string `json:"architecture,omitempty"`
}

// DLLResult holds all data extracted from a single DLL.
type DLLResult struct {
	DLLName        string                      `json:"dll_name"`
	DLLPath        string                      `json:"dll_path"`
	Version        string                      `json:"version"`
	MessagesByLang map[string][]PeMessage      `json:"messages_by_lang"`
	Providers      []ProviderInfo              `json:"providers"`
	CrimBlobs      [][]byte                    `json:"crim_blobs"`
	ParamMessages  map[uint32]string           `json:"param_messages"`
	// ParamMessagesByLang carries per-language %%N parameter messages (v9,
	// 2026-09-29): lang → param_id → text. The base parameter file is the
	// en-US layer; non-en languages come from MUI companions. Legacy
	// ParamMessages keeps the en-US layer for existing consumers.
	ParamMessagesByLang map[string]map[uint32]string `json:"param_messages_by_lang,omitempty"`
	ValueMaps      map[string]map[string]string `json:"value_maps"`
	// ParamMapRefs links event parameters to their value maps, collected
	// from the CRIM+MUI fallback (TEMP item +8 → VMAP/BMAP association).
	ParamMapRefs []ParamMapRef `json:"param_map_refs,omitempty"`
	// ParamTypes carries per-parameter TDH InType/OutType bytes (R10) for
	// typed rendering, collected from the CRIM TEMP descriptors.
	ParamTypes []ParamTypeRef `json:"param_types,omitempty"`
}

// CRIMMUIResult holds the result of extractMUICRIMFromBlobs including ValueMaps.
type CRIMMUIResult struct {
	Messages  []PeMessage
	ValueMaps map[string]map[uint32]string
	BitMaps   map[string]map[uint64]string
	// ParamMapRefs links event parameters to their value maps, derived from
	// the TEMP item descriptor +8 field (compiled form of the manifest's
	// <data name="..." map="..."/> association).
	ParamMapRefs []ParamMapRef
}

// ParamMapRef links one event parameter to its value map.
//
// MapName uses the DB key form: "Provider.MapName" (or "Provider.MapName_bits"
// for bitmap maps), matching the keys written to the value_maps table.
type ParamMapRef struct {
	ProviderName string
	EventID      uint32
	ParamIndex   string // "1", "2", ... matching the parameters JSON keys
	ParamName    string // TTBL param name at this index (template-scoped)
	MapName      string
	IsBitmap     bool
}

// ParamTypeRef links one event parameter to its TDH InType/OutType bytes
// (R10): the typed-render hint for wevtapi-parity formatting. Captured from
// the CRIM TEMP item descriptor +4 field ((OutType<<8)|InType).
type ParamTypeRef struct {
	ProviderName string
	EventID      uint32
	ParamIndex   string // "1", "2", ... matching the parameters JSON keys
	ParamName    string // TTBL param name at this index (template-scoped)
	InType       uint8
	OutType      uint8
}