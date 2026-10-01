//! EVTX chunk-level sharding reader (R40, C①).
//!
//! Classic classification: partition-merge — an EVTX file is natively a
//! sequence of self-contained 64 KiB chunks (records never span chunks), so
//! the serial tree-build segment of the pipeline (≈54% of wall clock, see
//! perf-report.md §8) can be parallelized by handing chunks to workers.
//! This module only implements the *partition* half: iterating a file's
//! chunk byte ranges through the evtx crate's public per-chunk API
//! (`EvtxChunkData::new` → `parse` → `iter` → `into_json_value`), with
//! dirty-file tolerance mirroring `EvtxParser`'s own chunk loop
//! (short read / all-zero / bad magic → skip with a warning).
//!
//! The *merge* (ordered reassembly) and the parallel driver land in R40 C②.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

use evtx::{EvtxChunkData, ParserSettings};
use serde_json::Value;

use crate::Result;

/// EVTX file header size (format constant; `EVTX_FILE_HEADER_SIZE` in the
/// evtx crate is `pub(crate)` so we mirror it here).
pub const EVTX_FILE_HEADER_SIZE: u64 = 4096;
/// EVTX chunk size (format constant; `EVTX_CHUNK_SIZE` in the evtx crate).
pub const EVTX_CHUNK_SIZE: u64 = 65536;

/// A chunk's payload plus its position in file order.
pub struct ShardChunk {
    pub chunk_id: u64,
    pub(crate) data: EvtxChunkData,
    pub(crate) settings: Arc<ParserSettings>,
}

/// Number of full chunks the crate's own `chunks()` iterator would visit for
/// a file of `file_size` bytes: `(file_size - header) / chunk_size`, matching
/// `evtx_parser.rs` (`chunk_data_size / EVTX_CHUNK_SIZE`).
pub fn count_chunks(file_size: u64) -> u64 {
    file_size.saturating_sub(EVTX_FILE_HEADER_SIZE) / EVTX_CHUNK_SIZE
}

/// Sequential chunk reader over an EVTX file.
///
/// Skips dirty chunks the same way `EvtxParser`'s chunk loop tolerates them
/// (each with a `[warn]` on stderr) and yields only parse-ready chunks:
/// short reads (truncated tail), all-zero filler chunks, and bad-magic
/// chunks are all skipped.
pub struct ShardReader {
    file: File,
    chunk_count: u64,
    next_chunk: u64,
    settings: Arc<ParserSettings>,
}

impl ShardReader {
    pub fn open(path: &str) -> Result<Self> {
        let file = File::open(path)
            .map_err(|e| format!("failed to open evtx file '{path}' for sharding: {e}"))?;
        let file_size = file
            .metadata()
            .map_err(|e| format!("failed to stat '{path}': {e}"))?
            .len();
        Ok(Self {
            file,
            chunk_count: count_chunks(file_size),
            next_chunk: 0,
            settings: Arc::new(ParserSettings::default().num_threads(1)),
        })
    }

    /// Replace the per-chunk `ParserSettings` used for `EvtxChunkData::parse`.
    pub fn with_settings(mut self, settings: Arc<ParserSettings>) -> Self {
        self.settings = settings;
        self
    }

    pub fn chunk_count(&self) -> u64 {
        self.chunk_count
    }

    /// Read and validate the next chunk, skipping dirty ones. `Ok(None)` =
    /// exhausted; `Err` = fatal I/O error (seek failed).
    pub fn next_chunk(&mut self) -> Option<Result<ShardChunk>> {
        while self.next_chunk < self.chunk_count {
            let chunk_id = self.next_chunk;
            self.next_chunk += 1;
            let offset = EVTX_FILE_HEADER_SIZE + chunk_id * EVTX_CHUNK_SIZE;
            if self.file.seek(SeekFrom::Start(offset)).is_err() {
                return Some(Err(format!("chunk {chunk_id}: seek to {offset} failed")));
            }
            let mut data = Vec::with_capacity(EVTX_CHUNK_SIZE as usize);
            let amount = Read::take(&mut self.file, EVTX_CHUNK_SIZE)
                .read_to_end(&mut data)
                .unwrap_or(0);
            if amount as u64 != EVTX_CHUNK_SIZE {
                eprintln!("[warn] shard: chunk {chunk_id} truncated ({amount} bytes), skipped");
                continue;
            }
            if data.iter().all(|x| *x == 0) {
                continue; // empty filler chunk in a dirty file — silent like the crate
            }
            match EvtxChunkData::new(data, false) {
                Ok(parsed) => {
                    return Some(Ok(ShardChunk {
                        chunk_id,
                        data: parsed,
                        settings: Arc::clone(&self.settings),
                    }))
                }
                Err(e) => {
                    eprintln!("[warn] shard: chunk {chunk_id} invalid ({e}), skipped");
                    continue;
                }
            }
        }
        None
    }
}

impl Iterator for ShardReader {
    type Item = Result<ShardChunk>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_chunk()
    }
}

impl ShardChunk {
    /// Parse the chunk and return every record as `(json Value,
    /// event_record_id)` in stream order — the same serialization path as
    /// `EvtxParser::records_json_value` (record → `into_json_value`).
    pub fn records_json_value(mut self) -> Result<Vec<(Value, u64)>> {
        let mut chunk = self
            .data
            .parse(Arc::clone(&self.settings))
            .map_err(|e| format!("shard: chunk {} parse failed: {e}", self.chunk_id))?;
        let mut out = Vec::new();
        for rec in chunk.iter() {
            let rec = rec.map_err(|e| format!("shard: record parse failed: {e}"))?;
            let ser = rec
                .into_json_value()
                .map_err(|e| format!("shard: json serialize failed: {e}"))?;
            out.push((ser.data, ser.event_record_id));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path() -> &'static str {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/test-dataset/evtx/DE_WinEventLogSvc_Crash_System_7036.evtx"
        )
    }

    #[test]
    fn test_count_chunks_arithmetic() {
        assert_eq!(count_chunks(4096), 0);
        assert_eq!(count_chunks(4095), 0);
        assert_eq!(count_chunks(4096 + 65_536), 1);
        // partial trailing chunk doesn't count (crate reads it and errors → skip)
        assert_eq!(count_chunks(4096 + 2 * 65_536 + 100), 2);
    }

    #[test]
    fn test_shard_reader_open_missing_file_is_err() {
        assert!(ShardReader::open("Z:/definitely/not/here.evtx").is_err());
    }

    #[test]
    fn test_shard_reader_covers_every_record_of_the_whole_stream() {
        let expected: Vec<u64> = {
            let mut parser = crate::parser::open_evtx(fixture_path(), 1).unwrap();
            crate::parser::iter_raw_json_values(&mut parser)
                .map(|r| r.expect("record should parse"))
                .map(|(_, rid)| rid)
                .collect()
        };
        assert!(!expected.is_empty(), "fixture must yield records");

        let mut reader = ShardReader::open(fixture_path()).unwrap();
        assert!(reader.chunk_count() >= 1, "fixture spans at least one chunk");

        let mut got: Vec<u64> = Vec::new();
        let mut chunk_ids: Vec<u64> = Vec::new();
        for chunk in reader {
            let chunk = chunk.expect("fixture chunks must all parse");
            chunk_ids.push(chunk.chunk_id);
            for (value, rid) in chunk.records_json_value().expect("chunk records") {
                assert!(value.get("Event").is_some(), "record {rid}: not an Event tree");
                got.push(rid);
            }
        }
        assert_eq!(
            chunk_ids,
            (0..chunk_ids.len() as u64).collect::<Vec<_>>(),
            "chunk ids must be contiguous from 0 in file order"
        );
        assert_eq!(
            got, expected,
            "sharded record_id stream must equal the whole-file stream"
        );
    }
}
