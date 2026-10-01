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

    /// Read and validate the next chunk. `Ok(None)` = exhausted; `Err` =
    /// fatal I/O error (seek failed).
    pub fn next_chunk(&mut self) -> Option<Result<ShardChunk>> {
        loop {
            match self.next_chunk_detail()? {
                Ok(ChunkReadOutcome::Ready(chunk)) => return Some(Ok(chunk)),
                Ok(ChunkReadOutcome::Skipped(_)) => continue,
                Err(e) => return Some(Err(e)),
            }
        }
    }

    /// Like [`ShardReader::next_chunk`] but also reports skipped (dirty)
    /// chunk ids — the parallel driver uses this to keep the ordered-merge
    /// cursor advancing across dirty files.
    ///
    /// `Err(String)` carries fatal I/O errors (seek failure).
    pub fn next_chunk_detail(&mut self) -> Option<Result<ChunkReadOutcome>> {
        if self.next_chunk >= self.chunk_count {
            return None;
        }
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
            return Some(Ok(ChunkReadOutcome::Skipped(chunk_id)));
        }
        if data.iter().all(|x| *x == 0) {
            return Some(Ok(ChunkReadOutcome::Skipped(chunk_id))); // silent like the crate
        }
        match EvtxChunkData::new(data, false) {
            Ok(parsed) => Some(Ok(ChunkReadOutcome::Ready(ShardChunk {
                chunk_id,
                data: parsed,
                settings: Arc::clone(&self.settings),
            }))),
            Err(e) => {
                eprintln!("[warn] shard: chunk {chunk_id} invalid ({e}), skipped");
                Some(Ok(ChunkReadOutcome::Skipped(chunk_id)))
            }
        }
    }
}

/// One chunk read step: a parse-ready chunk, or a dirty chunk the reader
/// skipped (id reported so the parallel merge cursor can advance). Fatal
/// I/O errors travel as `Err(String)`.
pub enum ChunkReadOutcome {
    Ready(ShardChunk),
    Skipped(u64),
}

/// Per-chunk result handed to the consumer: chunk id + that chunk's records
/// in stream order (empty for chunks the reader skipped as dirty).
pub type ChunkRecords = (u64, Vec<(Value, u64)>);

/// Parallel counterpart of `EvtxParser::records_json_value`: split the file
/// into chunks (partition), parse them on `workers` threads (the evtx crate's
/// JSON tree build is the ~54% serial segment this targets), and reassemble
/// the record stream in strict chunk order (merge).
///
/// Classic shape: static round-robin worker pool (each worker owns its
/// bounded queue — a shared `Mutex<Receiver>` would park `recv` under the
/// lock and block all workers) + ordered merge of out-of-order partitions
/// (per-chunk results buffer in a `BTreeMap` behind a contiguous cursor;
/// out-of-order buffering is bounded by the worker count).
///
/// Returns the receiver of an ordered result stream: `Ok((chunk_id,
/// records))` with strictly increasing chunk ids, plus `Err(String)` for
/// fatal reader/worker errors (consumers skip those like any other record
/// error). Dropping the receiver shuts the pipeline down.
pub fn shard_records_parallel(
    path: &str,
    workers: usize,
) -> Result<std::sync::mpsc::Receiver<Result<ChunkRecords>>> {
    let reader = ShardReader::open(path)?;
    let workers = workers.max(1);

    // Per-worker bounded work queues (static partition, round-robin dispatch).
    let mut work_txs = Vec::with_capacity(workers);
    let mut work_rxs = Vec::with_capacity(workers);
    for _ in 0..workers {
        let (tx, rx) = std::sync::mpsc::sync_channel::<ShardChunk>(1);
        work_txs.push(tx);
        work_rxs.push(rx);
    }
    // Results: cap 4 chunks in flight; the merger's cursor keeps memory flat.
    let (tx_out, rx_out) = std::sync::mpsc::sync_channel::<Result<ChunkRecords>>(4);
    let (tx_ordered, rx_ordered) = std::sync::mpsc::sync_channel::<Result<ChunkRecords>>(4);

    // Workers: parse chunks, ship (chunk_id, records) downstream.
    for rx in work_rxs {
        let tx_out = tx_out.clone();
        std::thread::Builder::new()
            .name("shard-worker".into())
            .spawn(move || {
                while let Ok(chunk) = rx.recv() {
                    let id = chunk.chunk_id;
                    let msg = match chunk.records_json_value() {
                        Ok(records) => Ok((id, records)),
                        Err(e) => Err(format!("chunk {id}: {e}")),
                    };
                    if tx_out.send(msg).is_err() {
                        break; // consumer/merger gone — shut down
                    }
                }
            })
            .map_err(|e| format!("failed to spawn shard worker: {e}"))?;
    }

    // Merger: reassemble out-of-order chunk results in strict chunk order.
    std::thread::Builder::new()
        .name("shard-merge".into())
        .spawn(move || {
            let mut expected: u64 = 0;
            let mut buf: std::collections::BTreeMap<u64, Vec<(Value, u64)>> =
                std::collections::BTreeMap::new();
            'pump: while let Ok(msg) = rx_out.recv() {
                match msg {
                    Ok((id, records)) => {
                        buf.insert(id, records);
                        while let Some(records) = buf.remove(&expected) {
                            if tx_ordered.send(Ok((expected, records))).is_err() {
                                break 'pump;
                            }
                            expected += 1;
                        }
                    }
                    Err(e) => {
                        if tx_ordered.send(Err(e)).is_err() {
                            break;
                        }
                    }
                }
            }
            // All producers done: flush whatever the cursor never reached
            // (ascending — order still preserved) and close the stream.
            for (id, records) in buf {
                let _ = tx_ordered.send(Ok((id, records)));
            }
        })
        .map_err(|e| format!("failed to spawn shard merger: {e}"))?;

    // Dispatcher: walk chunks in file order, feed workers round-robin,
    // emit skip markers so the merge cursor advances across dirty files.
    let tx_out_disp = tx_out.clone();
    std::thread::Builder::new()
        .name("shard-dispatch".into())
        .spawn(move || {
            let mut reader = reader;
            let mut rr = 0usize;
            while let Some(item) = reader.next_chunk_detail() {
                match item {
                    Ok(ChunkReadOutcome::Ready(chunk)) => {
                        if work_txs[rr % workers].send(chunk).is_err() {
                            break;
                        }
                        rr += 1;
                    }
                    Ok(ChunkReadOutcome::Skipped(id)) => {
                        if tx_out_disp.send(Ok((id, Vec::new()))).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx_out_disp.send(Err(e));
                        break;
                    }
                }
            }
        })
        .map_err(|e| format!("failed to spawn shard dispatcher: {e}"))?;
    drop(tx_out);

    Ok(rx_ordered)
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

    // -------------------------------------------------------------------
    // R40-C②: parallel driver + ordered merge
    // -------------------------------------------------------------------

    fn whole_stream_reference() -> Vec<(serde_json::Value, u64)> {
        let mut parser = crate::parser::open_evtx(fixture_path(), 1).unwrap();
        crate::parser::iter_raw_json_values(&mut parser)
            .map(|r| r.expect("record should parse"))
            .collect()
    }

    /// 经典：out-of-order partitions 的 ordered merge。worker 并行解析导致乱序
    /// 完成；输出必须按 chunk_id 严格递增拼接，且每条记录的 JSON 树与全流
    /// （iter_raw_json_values）逐字段相等——归并不得重排/丢失/改写任何记录。
    #[test]
    fn test_shard_records_parallel_matches_whole_stream_in_order() {
        for workers in [1usize, 4] {
            let rx = shard_records_parallel(fixture_path(), workers).unwrap();
            let mut got: Vec<(serde_json::Value, u64)> = Vec::new();
            let mut chunk_seq: Vec<u64> = Vec::new();
            for msg in rx {
                match msg {
                    Ok((chunk_id, records)) => {
                        if chunk_seq.last() != Some(&chunk_id) {
                            chunk_seq.push(chunk_id);
                        }
                        got.extend(records);
                    }
                    Err(e) => panic!("workers={workers}: unexpected chunk error: {e}"),
                }
            }
            let expected = whole_stream_reference();
            assert_eq!(
                got.len(),
                expected.len(),
                "workers={workers}: record count mismatch"
            );
            for (i, ((g_val, g_rid), (e_val, e_rid))) in got.iter().zip(&expected).enumerate() {
                assert_eq!(g_rid, e_rid, "workers={workers}: rid mismatch at {i}");
                assert_eq!(g_val, e_val, "workers={workers}: value mismatch at {i}");
            }
            let sorted = chunk_seq.is_sorted();
            assert!(sorted, "workers={workers}: chunk ids must be strictly increasing");
        }
    }
}
