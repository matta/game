use core::journal::InputJournal;
use core::replay::replay_to_end;
use core::state::ContentPack;
use core::{AdvanceStopReason, Choice, Game, GameMode, Interrupt};

#[test]
fn test_determinism_identical_seeds_produce_same_hash() {
    let content = ContentPack {};
    let seed1 = 12345;
    let mut journal1 = InputJournal::new(seed1);
    journal1.append_choice(core::ChoicePromptId(0), core::Choice::KeepLoot, 0);

    let seed2 = 12345;
    let mut journal2 = InputJournal::new(seed2);
    journal2.append_choice(core::ChoicePromptId(0), core::Choice::KeepLoot, 0);

    let result1 = replay_to_end(&content, &journal1).expect("Replay 1 failed");
    let result2 = replay_to_end(&content, &journal2).expect("Replay 2 failed");

    assert_eq!(
        result1.final_snapshot_hash, result2.final_snapshot_hash,
        "Identical runs must produce identical hashes"
    );
    assert_eq!(result1.final_tick, result2.final_tick);
}

#[test]
fn test_determinism_different_seeds_produce_different_hashes() {
    let content = ContentPack {};
    let mut journal1 = InputJournal::new(123);
    journal1.append_choice(core::ChoicePromptId(0), core::Choice::KeepLoot, 0);

    let mut journal2 = InputJournal::new(456);
    journal2.append_choice(core::ChoicePromptId(0), core::Choice::KeepLoot, 0);

    let result1 = replay_to_end(&content, &journal1).expect("Replay 1 failed");
    let result2 = replay_to_end(&content, &journal2).expect("Replay 2 failed");

    assert_ne!(
        result1.final_snapshot_hash, result2.final_snapshot_hash,
        "Different seeds should probably produce different outcomes or hashes"
    );
}

#[test]
fn test_deterministic_smoke_fixed_seed_stable_intent_and_log_sequence() {
    let content = ContentPack {};

    fn run_trace(seed: u64, content: &ContentPack) -> Vec<String> {
        let mut game = Game::new(seed, content, GameMode::Ironman);
        let mut trace = Vec::new();
        let mut seen_logs = 0usize;

        while game.current_tick() < 40 {
            let result = game.advance(1);
            match result.stop_reason {
                AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::KeepLoot)
                        .expect("loot choice should apply");
                    trace.push("loot".to_string());
                }
                AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::Fight).expect("fight choice should apply");
                    trace.push("fight".to_string());
                }
                _ => {}
            }

            let logs = game.log();
            for event in &logs[seen_logs..] {
                trace.push(format!("{event:?}"));
            }
            seen_logs = logs.len();
        }

        trace
    }

    let left = run_trace(12345, &content);
    let right = run_trace(12345, &content);
    assert_eq!(left, right, "same seed should produce the same intent/log trace");
}
