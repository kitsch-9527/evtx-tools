package schema

// SchemaDDL is the single-file message DB schema (schema_version 7).
//
// Single-file MessageDB (ADR 0003): one file holds all 7 entity tables and no
// views. provider_message_variants is the only message table; the legacy
// dll_messages / provider_messages single-template tables and their compat
// views are deleted. Variant Identity =
// (provider_name, lang, event_id, version, level, opcode, task, keywords,
// channel, message_identifier). See CONTEXT.md: Variant Identity, ADR 0001.
const SchemaDDL = `
CREATE TABLE IF NOT EXISTS dll_entries (
    dll_id      INTEGER PRIMARY KEY AUTOINCREMENT,
    dll_name    TEXT NOT NULL,
    dll_path    TEXT NOT NULL UNIQUE,
    pe_version  TEXT NOT NULL DEFAULT '0.0.0.0'
);

CREATE TABLE IF NOT EXISTS providers (
    provider_name TEXT NOT NULL PRIMARY KEY,
    guid          TEXT,
    channel       TEXT,
    dll_id        INTEGER NOT NULL REFERENCES dll_entries(dll_id)
);

CREATE TABLE IF NOT EXISTS provider_message_variants (
    provider_name      TEXT NOT NULL,
    provider_guid      TEXT NOT NULL,
    lang               TEXT NOT NULL DEFAULT 'en-US',
    event_id           INTEGER NOT NULL,
    version            INTEGER NOT NULL DEFAULT 0,
    level              INTEGER NOT NULL DEFAULT 0,
    opcode             INTEGER NOT NULL DEFAULT 0,
    task               INTEGER NOT NULL DEFAULT 0,
    keywords           TEXT NOT NULL DEFAULT '0',
    channel            INTEGER NOT NULL DEFAULT 0,
    message_identifier INTEGER NOT NULL,
    dll_id             INTEGER NOT NULL REFERENCES dll_entries(dll_id),
    message            TEXT NOT NULL,
    parameters         TEXT,
    level_name         TEXT,
    keywords_name      TEXT,
    PRIMARY KEY (provider_name, lang, event_id, version, level, opcode, task, keywords, channel, message_identifier, dll_id)
);

CREATE TABLE IF NOT EXISTS metadata (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS parameter_refs (
    provider_name TEXT NOT NULL,
    param_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    lang          TEXT NOT NULL DEFAULT 'en-US',
    PRIMARY KEY (provider_name, param_id, lang)
);

CREATE TABLE IF NOT EXISTS value_maps (
    provider_name TEXT NOT NULL,
    map_name      TEXT NOT NULL,
    entry_value   TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    is_bitmap     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (provider_name, map_name, entry_value)
);

CREATE TABLE IF NOT EXISTS param_value_maps (
    provider_name TEXT NOT NULL,
    event_id      INTEGER NOT NULL,
    param_index   TEXT NOT NULL,
    param_name    TEXT NOT NULL DEFAULT '',
    map_name      TEXT NOT NULL,
    is_bitmap     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (provider_name, event_id, param_index)
);

-- Provenance table (R12, ADR 0004): registered event source names
-- (classic EventLog sources union WINEVT publisher names). Part of the v8
-- schema: readers hard-require it. An empty table means collection failed —
-- readers then treat the registry state as unknown (feature off).
CREATE TABLE IF NOT EXISTS eventlog_sources (
    source_name TEXT NOT NULL PRIMARY KEY,
    source_type TEXT NOT NULL DEFAULT 'classic'
);

-- Typed-render table (R10, ADR 0005): per-parameter TDH InType and
-- OutType bytes from the CRIM TEMP item descriptor +4 field. Part of the
-- v8 schema: readers hard-require it. No rows for a cohort = passthrough
-- rendering for that cohort.
CREATE TABLE IF NOT EXISTS param_types (
    provider_name TEXT NOT NULL,
    event_id      INTEGER NOT NULL,
    param_index   TEXT NOT NULL,
    param_name    TEXT NOT NULL DEFAULT '',
    in_type       INTEGER NOT NULL,
    out_type      INTEGER NOT NULL,
    PRIMARY KEY (provider_name, event_id, param_index)
);
`

// BulkInsertPragmas speeds up bulk writes (applied before BEGIN). Matches
// the expectations in schema_test.go.
const BulkInsertPragmas = `
PRAGMA synchronous=OFF;
PRAGMA journal_mode=MEMORY;
PRAGMA cache_size=-64000;
PRAGMA page_size=65536;
`

// SchemaIndexes — covering lookup indexes.
const SchemaIndexes = `
CREATE INDEX IF NOT EXISTS idx_pmv_lookup ON provider_message_variants(provider_name, lang, event_id);
CREATE INDEX IF NOT EXISTS idx_pmv_guid ON provider_message_variants(provider_guid, lang, event_id);
CREATE INDEX IF NOT EXISTS idx_providers_dll ON providers(dll_id);
CREATE INDEX IF NOT EXISTS idx_param_value_maps_eid ON param_value_maps(provider_name, event_id);
`
