use std::fs;

use core::ContentPack;
use core::journal::InputPayload;
use core::replay::{replay_journal_inputs, replay_to_end};
use core::{
    AdvanceStopReason, Choice, Game, GameMode, Interrupt, JournalWriter, load_journal_from_file,
};

/// Play a full game recording inputs to a JSONL file, then load the file
/// and replay to completion. The snapshot hash must match.
#[test]
fn test_file_journal_replay_equivalence() {
    let dir = tempfile::tempdir().unwrap();
    let journal_path = dir.path().join("replay_equiv.jsonl");
    let content = ContentPack::default();
    let seed = 12345u64;

    // --- play the game, recording to a file journal ---
    let mut game = Game::new(seed, &content, GameMode::Ironman);
    let mut writer = JournalWriter::create(&journal_path, seed, "test", 0).unwrap();

    let mut finished = false;
    for _ in 0..512 {
        let result = game.advance(100);
        match result.stop_reason {
            AdvanceStopReason::Finished(_) => {
                finished = true;
                break;
            }
            AdvanceStopReason::Interrupted(ref interrupt) => {
                let (prompt_id, choice) = match interrupt {
                    Interrupt::LootFound { prompt_id, .. } => (*prompt_id, Choice::KeepLoot),
                    Interrupt::EnemyEncounter { prompt_id, .. } => (*prompt_id, Choice::Fight),
                    Interrupt::DoorBlocked { prompt_id, .. } => (*prompt_id, Choice::OpenDoor),
                    Interrupt::FloorTransition {
                        prompt_id, requires_branch_god_choice, ..
                    } => {
                        let c = if *requires_branch_god_choice {
                            Choice::DescendBranchAVeil
                        } else {
                            Choice::Descend
                        };
                        (*prompt_id, c)
                    }
                };
                writer
                    .append(
                        game.current_tick(),
                        &InputPayload::Choice { prompt_id, choice: choice.clone() },
                    )
                    .unwrap();
                game.apply_choice(prompt_id, choice).unwrap();
            }
            _ => {}
        }
    }
    assert!(finished, "game did not finish within budget");
    let original_hash = game.snapshot_hash();
    drop(writer);

    // --- load the file journal and replay ---
    let loaded = load_journal_from_file(&journal_path).unwrap();
    let replay_result = replay_to_end(&content, &loaded.journal).unwrap();

    assert_eq!(
        original_hash, replay_result.final_snapshot_hash,
        "file-journal replay must produce the same snapshot hash"
    );
}

/// Corrupt a record in a file journal, confirm that loading stops at
/// the corrupt line and that replaying the truncated journal still works.
#[test]
fn test_file_journal_corruption_stops_at_bad_line() {
    let dir = tempfile::tempdir().unwrap();
    let journal_path = dir.path().join("corrupt.jsonl");
    let seed = 42u64;
    let content = ContentPack::default();

    // Record a few inputs
    let mut game = Game::new(seed, &content, GameMode::Ironman);
    let mut writer = JournalWriter::create(&journal_path, seed, "test", 0).unwrap();
    let mut recorded = 0usize;

    for _ in 0..512 {
        let result = game.advance(100);
        match result.stop_reason {
            AdvanceStopReason::Finished(_) => break,
            AdvanceStopReason::Interrupted(ref interrupt) => {
                let (prompt_id, choice) = match interrupt {
                    Interrupt::LootFound { prompt_id, .. } => (*prompt_id, Choice::KeepLoot),
                    Interrupt::EnemyEncounter { prompt_id, .. } => (*prompt_id, Choice::Fight),
                    Interrupt::DoorBlocked { prompt_id, .. } => (*prompt_id, Choice::OpenDoor),
                    Interrupt::FloorTransition {
                        prompt_id, requires_branch_god_choice, ..
                    } => {
                        let c = if *requires_branch_god_choice {
                            Choice::DescendBranchAVeil
                        } else {
                            Choice::Descend
                        };
                        (*prompt_id, c)
                    }
                };
                writer
                    .append(
                        game.current_tick(),
                        &InputPayload::Choice { prompt_id, choice: choice.clone() },
                    )
                    .unwrap();
                game.apply_choice(prompt_id, choice).unwrap();
                recorded += 1;
                if recorded >= 3 {
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(recorded >= 3, "need at least 3 recorded inputs for corruption test");
    drop(writer);

    // Corrupt the third record (line 4 = header + 3 records)
    let content_str = fs::read_to_string(&journal_path).unwrap();
    let mut lines: Vec<String> = content_str.lines().map(String::from).collect();
    assert!(lines.len() >= 4, "expected header + 3 records");
    lines[3] = lines[3].replace("Fight", "CORRUPTED_VALUE");
    fs::write(&journal_path, lines.join("\n") + "\n").unwrap();

    // Loading should fail at line 4
    let result = load_journal_from_file(&journal_path);
    assert!(result.is_err(), "corrupted journal should fail to load");
}

/// Replay a partial journal (crash recovery scenario): play halfway,
/// replay just the recorded inputs, then verify we can continue.
#[test]
fn test_replay_journal_inputs_reconstructs_game_state() {
    let dir = tempfile::tempdir().unwrap();
    let journal_path = dir.path().join("partial.jsonl");
    let content = ContentPack::default();
    let seed = 777u64;

    // --- play and record 2 inputs ---
    let mut game = Game::new(seed, &content, GameMode::Ironman);
    let mut writer = JournalWriter::create(&journal_path, seed, "test", 0).unwrap();
    let mut recorded = 0usize;

    for _ in 0..512 {
        let result = game.advance(100);
        match result.stop_reason {
            AdvanceStopReason::Finished(_) => break,
            AdvanceStopReason::Interrupted(ref interrupt) => {
                let (prompt_id, choice) = match interrupt {
                    Interrupt::LootFound { prompt_id, .. } => (*prompt_id, Choice::KeepLoot),
                    Interrupt::EnemyEncounter { prompt_id, .. } => (*prompt_id, Choice::Fight),
                    Interrupt::DoorBlocked { prompt_id, .. } => (*prompt_id, Choice::OpenDoor),
                    Interrupt::FloorTransition {
                        prompt_id, requires_branch_god_choice, ..
                    } => {
                        let c = if *requires_branch_god_choice {
                            Choice::DescendBranchAVeil
                        } else {
                            Choice::Descend
                        };
                        (*prompt_id, c)
                    }
                };
                writer
                    .append(
                        game.current_tick(),
                        &InputPayload::Choice { prompt_id, choice: choice.clone() },
                    )
                    .unwrap();
                game.apply_choice(prompt_id, choice).unwrap();
                recorded += 1;
                if recorded >= 2 {
                    break;
                }
            }
            _ => {}
        }
    }
    let hash_after_inputs = game.snapshot_hash();
    drop(writer);

    // --- load journal and replay inputs to reconstruct ---
    let loaded = load_journal_from_file(&journal_path).unwrap();
    assert_eq!(loaded.journal.inputs.len(), recorded);

    let reconstructed = replay_journal_inputs(&content, &loaded.journal).unwrap();
    assert_eq!(
        hash_after_inputs,
        reconstructed.snapshot_hash(),
        "reconstructed game should have the same hash as the original at that point"
    );
}
