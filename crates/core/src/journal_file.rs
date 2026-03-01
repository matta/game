//! File-backed JSONL journal with SHA-256 hash chain for crash recovery.
//!
//! The file format is line-delimited JSON (`.jsonl`):
//! - Line 1: header with `format_version`, `build_id`, `content_hash`, `seed`.
//! - Lines 2+: one record per accepted simulation input, each carrying a
//!   SHA-256 hash chain (`prev_sha256_hex`, `sha256_hex`) for corruption detection.
//!
//! Writing flushes each record immediately so the file survives crashes.
//! Loading validates every line's JSON shape and SHA-256 chain, stopping
//! at the first invalid or incomplete line.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::journal::{InputJournal, InputPayload, InputRecord};

// ---------------------------------------------------------------------------
// File format structs
// ---------------------------------------------------------------------------

/// First line of the JSONL journal file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct FileHeader {
    format_version: u16,
    build_id: String,
    content_hash: u64,
    seed: u64,
}

/// Fields used to compute the canonical SHA-256 for a record.
/// Serialized to JSON as the hash input (concatenated with `prev_sha256_hex`).
#[derive(Serialize)]
struct RecordBody<'a> {
    seq: u64,
    tick_boundary: u64,
    payload: &'a InputPayload,
}

/// Full record line written to the JSONL file.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileRecord {
    seq: u64,
    tick_boundary: u64,
    payload: InputPayload,
    prev_sha256_hex: String,
    sha256_hex: String,
}

// ---------------------------------------------------------------------------
// SHA-256 helpers
// ---------------------------------------------------------------------------

/// The initial previous-hash used for the first record in a chain.
const INITIAL_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Compute `hex(SHA-256(body_json || prev_sha256_hex))`.
fn compute_record_sha256(body_json: &str, prev_sha256_hex: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body_json.as_bytes());
    hasher.update(prev_sha256_hex.as_bytes());
    let result = hasher.finalize();
    format!("{result:064x}")
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Appends simulation inputs to a JSONL file with a SHA-256 hash chain.
pub struct JournalWriter {
    writer: BufWriter<File>,
    last_sha256_hex: String,
    next_seq: u64,
}

impl JournalWriter {
    /// Create a new journal file, writing the header line immediately.
    pub fn create(path: &Path, seed: u64, build_id: &str, content_hash: u64) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let header =
            FileHeader { format_version: 1, build_id: build_id.to_string(), content_hash, seed };
        let header_json = serde_json::to_string(&header).map_err(io::Error::other)?;
        writeln!(writer, "{header_json}")?;
        writer.flush()?;

        Ok(Self { writer, last_sha256_hex: INITIAL_HASH.to_string(), next_seq: 0 })
    }

    /// Resume appending to an existing journal after loading it.
    /// `last_sha256_hex` and `next_seq` come from `LoadedJournal`.
    pub fn resume(path: &Path, last_sha256_hex: String, next_seq: u64) -> io::Result<Self> {
        let file = OpenOptions::new().append(true).open(path)?;
        let writer = BufWriter::new(file);
        Ok(Self { writer, last_sha256_hex, next_seq })
    }

    /// Append one accepted input and flush immediately.
    pub fn append(&mut self, tick_boundary: u64, payload: &InputPayload) -> io::Result<()> {
        let body = RecordBody { seq: self.next_seq, tick_boundary, payload };
        let body_json = serde_json::to_string(&body).map_err(io::Error::other)?;
        let sha256_hex = compute_record_sha256(&body_json, &self.last_sha256_hex);

        let record = FileRecord {
            seq: self.next_seq,
            tick_boundary,
            payload: payload.clone(),
            prev_sha256_hex: self.last_sha256_hex.clone(),
            sha256_hex: sha256_hex.clone(),
        };

        let record_json = serde_json::to_string(&record).map_err(io::Error::other)?;
        writeln!(self.writer, "{record_json}")?;
        self.writer.flush()?;

        self.last_sha256_hex = sha256_hex;
        self.next_seq += 1;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// Successfully loaded journal with metadata needed for resuming appends.
#[derive(Debug)]
pub struct LoadedJournal {
    pub journal: InputJournal,
    /// SHA-256 hex of the last valid record (or the initial hash if empty).
    pub last_sha256_hex: String,
    /// Sequence number for the next record to be appended.
    pub next_seq: u64,
}

/// Describes why a journal file could not be fully loaded.
#[derive(Debug)]
pub enum JournalLoadError {
    /// Underlying I/O failure.
    Io(io::Error),
    /// The file contains no lines at all.
    EmptyFile,
    /// The header line could not be parsed as valid JSON.
    InvalidHeader { line: usize, message: String },
    /// A record line could not be parsed or its fields are inconsistent.
    InvalidRecord { line: usize, message: String },
    /// A line is incomplete (for example, file ended without trailing newline).
    IncompleteLine { line: usize },
    /// The SHA-256 chain is broken (prev hash mismatch or recomputed hash
    /// does not match stored hash).
    HashChainBroken { line: usize },
}

impl fmt::Display for JournalLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "journal I/O error: {e}"),
            Self::EmptyFile => write!(f, "journal file is empty"),
            Self::InvalidHeader { line, message } => {
                write!(f, "invalid journal header at line {line}: {message}")
            }
            Self::InvalidRecord { line, message } => {
                write!(f, "invalid journal record at line {line}: {message}")
            }
            Self::IncompleteLine { line } => {
                write!(f, "incomplete journal line at line {line}")
            }
            Self::HashChainBroken { line } => {
                write!(f, "SHA-256 hash chain broken at line {line}")
            }
        }
    }
}

/// Load and validate a JSONL journal file.
///
/// Returns the in-memory journal plus metadata for resuming appends.
/// Stops at the first invalid, incomplete, or hash-broken line and returns
/// an error describing the problem.
pub fn load_journal_from_file(path: &Path) -> Result<LoadedJournal, JournalLoadError> {
    let content = fs::read_to_string(path).map_err(JournalLoadError::Io)?;
    if content.is_empty() {
        return Err(JournalLoadError::EmptyFile);
    }
    let has_trailing_newline = content.ends_with('\n');
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Err(JournalLoadError::EmptyFile);
    }
    if !has_trailing_newline {
        return Err(JournalLoadError::IncompleteLine { line: lines.len() });
    }

    // --- header (line 1) ---
    let header_line = lines[0];

    let header: FileHeader = serde_json::from_str(header_line)
        .map_err(|e| JournalLoadError::InvalidHeader { line: 1, message: e.to_string() })?;

    let mut journal = InputJournal {
        format_version: header.format_version,
        build_id: header.build_id,
        content_hash: header.content_hash,
        seed: header.seed,
        inputs: Vec::new(),
    };

    let mut prev_sha256_hex = INITIAL_HASH.to_string();
    let mut next_seq: u64 = 0;

    // --- records (lines 2+) ---
    for (line_index, line) in lines.iter().skip(1).enumerate() {
        let line_number = line_index + 2; // 1-indexed; header is line 1

        if line.is_empty() {
            return Err(JournalLoadError::InvalidRecord {
                line: line_number,
                message: "empty line".to_string(),
            });
        }

        let record: FileRecord = serde_json::from_str(line).map_err(|e| {
            JournalLoadError::InvalidRecord { line: line_number, message: e.to_string() }
        })?;

        if record.seq != next_seq {
            return Err(JournalLoadError::InvalidRecord {
                line: line_number,
                message: format!("expected seq {next_seq}, found {}", record.seq),
            });
        }

        // Verify prev_sha256 link
        if record.prev_sha256_hex != prev_sha256_hex {
            return Err(JournalLoadError::HashChainBroken { line: line_number });
        }

        // Recompute canonical hash and verify
        let body = RecordBody {
            seq: record.seq,
            tick_boundary: record.tick_boundary,
            payload: &record.payload,
        };
        let body_json = serde_json::to_string(&body).map_err(|e| {
            JournalLoadError::InvalidRecord { line: line_number, message: e.to_string() }
        })?;
        let expected_sha256 = compute_record_sha256(&body_json, &prev_sha256_hex);

        if record.sha256_hex != expected_sha256 {
            return Err(JournalLoadError::HashChainBroken { line: line_number });
        }

        journal.inputs.push(InputRecord { seq: record.seq, payload: record.payload });

        prev_sha256_hex = record.sha256_hex;
        next_seq += 1;
    }

    Ok(LoadedJournal { journal, last_sha256_hex: prev_sha256_hex, next_seq })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
