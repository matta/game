use core::ContentPack;
use core::journal::InputJournal;
use core::replay::replay_to_end;
use core::{AdvanceStopReason, Choice, Game, GameMode, Interrupt};

fn build_scripted_journal(seed: u64, content: &ContentPack) -> InputJournal {
    let mut game = Game::new(seed, content, GameMode::Ironman);
    let mut journal = InputJournal::new(seed);
    let mut seq = 0u64;

    loop {
        let result = game.advance(100);
        match result.stop_reason {
            AdvanceStopReason::Finished(_) => return journal,
            AdvanceStopReason::Interrupted(interrupt) => {
                let (prompt_id, choice) = match interrupt {
                    Interrupt::LootFound { prompt_id, .. } => (prompt_id, Choice::KeepLoot),
                    Interrupt::EnemyEncounter { prompt_id, .. } => (prompt_id, Choice::Fight),
                    Interrupt::DoorBlocked { prompt_id, .. } => (prompt_id, Choice::OpenDoor),
                };
                journal.append_choice(prompt_id, choice.clone(), seq);
                seq += 1;
                game.apply_choice(prompt_id, choice)
                    .expect("scripted choice should apply while building journal");
            }
            AdvanceStopReason::PausedAtBoundary { .. } | AdvanceStopReason::BudgetExhausted => {}
        }
    }
}

#[test]
fn test_determinism_identical_seeds_produce_same_hash() {
    let content = ContentPack::default();
    let journal1 = build_scripted_journal(12345, &content);
    let journal2 = build_scripted_journal(12345, &content);

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
    let content = ContentPack::default();
    let journal1 = build_scripted_journal(123, &content);
    let journal2 = build_scripted_journal(456, &content);

    let result1 = replay_to_end(&content, &journal1).expect("Replay 1 failed");
    let result2 = replay_to_end(&content, &journal2).expect("Replay 2 failed");

    assert_ne!(
        result1.final_snapshot_hash, result2.final_snapshot_hash,
        "Different seeds should probably produce different outcomes or hashes"
    );
}

#[test]
fn test_deterministic_smoke_fixed_seed_stable_intent_and_log_sequence() {
    let content = ContentPack::default();

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
                AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::OpenDoor).expect("open door should apply");
                    trace.push("door".to_string());
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

#[test]
fn test_starter_layout_auto_run_hits_door_and_threat_avoidance_within_250_ticks() {
    let content = ContentPack::default();
    let mut game = Game::new(12345, &content, GameMode::Ironman);

    let mut saw_door_blocked = false;
    let mut saw_threat_avoidance = false;
    let mut seen_logs = 0usize;

    while game.current_tick() <= 250 && !(saw_door_blocked && saw_threat_avoidance) {
        let result = game.advance(1);
        match result.stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) => {
                game.apply_choice(prompt_id, Choice::KeepLoot)
                    .expect("loot choice should apply during smoke run");
            }
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { prompt_id, .. }) => {
                game.apply_choice(prompt_id, Choice::Fight)
                    .expect("fight choice should apply during smoke run");
            }
            AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => {
                saw_door_blocked = true;
                game.apply_choice(prompt_id, Choice::OpenDoor)
                    .expect("open door should apply during smoke run");
            }
            _ => {}
        }

        let logs = game.log();
        for event in &logs[seen_logs..] {
            if matches!(
                event,
                core::LogEvent::AutoReasonChanged { reason: core::AutoReason::ThreatAvoidance, .. }
            ) {
                saw_threat_avoidance = true;
            }
        }
        seen_logs = logs.len();
    }

    assert!(saw_door_blocked, "expected DoorBlocked interrupt within 250 ticks");
    assert!(
        saw_threat_avoidance,
        "expected AutoReason::ThreatAvoidance log event within 250 ticks"
    );
}
