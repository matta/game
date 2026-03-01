use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::*;
use crate::journal::InputPayload;
use crate::types::{Choice, ChoicePromptId, PolicyUpdate, Stance};

fn make_test_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

// -- 7a tests --

#[test]
fn schema_roundtrip_header_and_records() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "roundtrip.jsonl");

    // Write
    let mut writer = JournalWriter::create(&path, 42, "test-build", 99).unwrap();
    writer
        .append(0, &InputPayload::Choice { prompt_id: ChoicePromptId(1), choice: Choice::Fight })
        .unwrap();
    writer
        .append(
            5,
            &InputPayload::PolicyUpdate {
                tick_boundary: 5,
                update: PolicyUpdate::Stance(Stance::Defensive),
            },
        )
        .unwrap();
    writer.append(10, &InputPayload::SwapActiveWeapon { tick_boundary: 10 }).unwrap();

    // Read back
    let loaded = load_journal_from_file(&path).unwrap();
    assert_eq!(loaded.journal.format_version, 1);
    assert_eq!(loaded.journal.build_id, "test-build");
    assert_eq!(loaded.journal.content_hash, 99);
    assert_eq!(loaded.journal.seed, 42);
    assert_eq!(loaded.journal.inputs.len(), 3);

    // Verify payload types round-tripped correctly
    assert!(matches!(loaded.journal.inputs[0].payload, InputPayload::Choice { .. }));
    assert!(matches!(loaded.journal.inputs[1].payload, InputPayload::PolicyUpdate { .. }));
    assert!(matches!(loaded.journal.inputs[2].payload, InputPayload::SwapActiveWeapon { .. }));

    // Verify sequence numbers
    assert_eq!(loaded.journal.inputs[0].seq, 0);
    assert_eq!(loaded.journal.inputs[1].seq, 1);
    assert_eq!(loaded.journal.inputs[2].seq, 2);

    // Verify resume metadata
    assert_eq!(loaded.next_seq, 3);
    assert_ne!(loaded.last_sha256_hex, INITIAL_HASH);
}

#[test]
fn hash_chain_detects_tampered_record() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "tampered.jsonl");

    // Write two records
    let mut writer = JournalWriter::create(&path, 1, "dev", 0).unwrap();
    writer
        .append(0, &InputPayload::Choice { prompt_id: ChoicePromptId(1), choice: Choice::Fight })
        .unwrap();
    writer
        .append(5, &InputPayload::Choice { prompt_id: ChoicePromptId(2), choice: Choice::KeepLoot })
        .unwrap();

    // Tamper with the second record's payload in the file
    let content = fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    assert!(lines.len() >= 3, "expected header + 2 records");

    // Modify the second record (line index 2) by replacing the choice
    lines[2] = lines[2].replace("KeepLoot", "DiscardLoot");
    fs::write(&path, lines.join("\n") + "\n").unwrap();

    // Load should detect the tamper
    let result = load_journal_from_file(&path);
    assert!(
        matches!(result, Err(JournalLoadError::HashChainBroken { line: 3 })),
        "expected hash chain broken at line 3, got: {result:?}"
    );
}

#[test]
fn hash_chain_detects_deleted_record() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "deleted.jsonl");

    // Write three records
    let mut writer = JournalWriter::create(&path, 1, "dev", 0).unwrap();
    for i in 0..3 {
        writer
            .append(
                i * 5,
                &InputPayload::Choice { prompt_id: ChoicePromptId(i + 1), choice: Choice::Fight },
            )
            .unwrap();
    }

    // Delete the second record (line index 2)
    let content = fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 4); // header + 3 records
    let tampered = format!("{}\n{}\n{}\n", lines[0], lines[1], lines[3]);
    fs::write(&path, tampered).unwrap();

    // Load should detect the chain break at the third record
    let result = load_journal_from_file(&path);
    assert!(
        matches!(
            result,
            Err(JournalLoadError::HashChainBroken { .. })
                | Err(JournalLoadError::InvalidRecord { .. })
        ),
        "expected chain corruption error, got: {result:?}"
    );
}

#[test]
fn truncated_last_line_returns_error() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "truncated.jsonl");

    // Write one valid record
    let mut writer = JournalWriter::create(&path, 1, "dev", 0).unwrap();
    writer
        .append(0, &InputPayload::Choice { prompt_id: ChoicePromptId(1), choice: Choice::Fight })
        .unwrap();

    // Append a truncated (invalid JSON) line
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    write!(file, "{{\"seq\":1,\"tick").unwrap(); // no newline, truncated JSON

    let result = load_journal_from_file(&path);
    assert!(
        matches!(result, Err(JournalLoadError::IncompleteLine { line: 3 })),
        "expected incomplete line at line 3, got: {result:?}"
    );
}

#[test]
fn missing_trailing_newline_on_valid_json_line_is_incomplete() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "no_newline.jsonl");

    // Header line intentionally written without trailing newline.
    fs::write(&path, "{\"format_version\":1,\"build_id\":\"dev\",\"content_hash\":0,\"seed\":123}")
        .unwrap();

    let result = load_journal_from_file(&path);
    assert!(
        matches!(result, Err(JournalLoadError::IncompleteLine { line: 1 })),
        "expected incomplete line at line 1, got: {result:?}"
    );
}

#[test]
fn empty_file_returns_error() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "empty.jsonl");
    fs::write(&path, "").unwrap();

    let result = load_journal_from_file(&path);
    assert!(
        matches!(result, Err(JournalLoadError::EmptyFile)),
        "expected EmptyFile error, got: {result:?}"
    );
}

#[test]
fn header_only_file_loads_empty_journal() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "header_only.jsonl");

    let _writer = JournalWriter::create(&path, 555, "dev", 0).unwrap();
    // Don't write any records

    let loaded = load_journal_from_file(&path).unwrap();
    assert_eq!(loaded.journal.seed, 555);
    assert!(loaded.journal.inputs.is_empty());
    assert_eq!(loaded.next_seq, 0);
    assert_eq!(loaded.last_sha256_hex, INITIAL_HASH);
}

#[test]
fn resume_appends_continue_hash_chain() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "resume.jsonl");

    // Write initial records
    let mut writer = JournalWriter::create(&path, 1, "dev", 0).unwrap();
    writer
        .append(0, &InputPayload::Choice { prompt_id: ChoicePromptId(1), choice: Choice::Fight })
        .unwrap();
    drop(writer);

    // Load to get resume metadata
    let loaded = load_journal_from_file(&path).unwrap();
    assert_eq!(loaded.journal.inputs.len(), 1);

    // Resume and append more
    let mut writer = JournalWriter::resume(&path, loaded.last_sha256_hex, loaded.next_seq).unwrap();
    writer
        .append(5, &InputPayload::Choice { prompt_id: ChoicePromptId(2), choice: Choice::OpenDoor })
        .unwrap();
    drop(writer);

    // Load again and verify the full chain
    let reloaded = load_journal_from_file(&path).unwrap();
    assert_eq!(reloaded.journal.inputs.len(), 2);
    assert_eq!(reloaded.journal.inputs[0].seq, 0);
    assert_eq!(reloaded.journal.inputs[1].seq, 1);
    assert_eq!(reloaded.next_seq, 2);
}

#[test]
fn invalid_header_returns_error() {
    let dir = tempdir().unwrap();
    let path = make_test_path(dir.path(), "bad_header.jsonl");
    fs::write(&path, "not valid json\n").unwrap();

    let result = load_journal_from_file(&path);
    assert!(
        matches!(result, Err(JournalLoadError::InvalidHeader { line: 1, .. })),
        "expected invalid header error, got: {result:?}"
    );
}
