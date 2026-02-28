use core::{
    AdvanceStopReason, Choice, ContentPack, EngineFailureReason, Game, GameMode, Interrupt,
};

fn percentile(sorted_values: &[u32], percentile: f32) -> u32 {
    let index = ((sorted_values.len() - 1) as f32 * percentile).round() as usize;
    sorted_values[index]
}

fn scripted_interrupt_count(seed: u64) -> Result<u32, String> {
    let content = ContentPack::default();
    let mut game = Game::new(seed, &content, GameMode::Ironman);
    let mut interrupts = 0_u32;

    for _ in 0..5000 {
        let result = game.advance(10);
        match result.stop_reason {
            AdvanceStopReason::Finished(_) => return Ok(interrupts),
            AdvanceStopReason::Interrupted(interrupt) => {
                interrupts += 1;
                let (prompt_id, choice) = match interrupt {
                    Interrupt::LootFound { prompt_id, .. } => (prompt_id, Choice::KeepLoot),
                    Interrupt::EnemyEncounter { prompt_id, .. } => (prompt_id, Choice::Fight),
                    Interrupt::DoorBlocked { prompt_id, .. } => (prompt_id, Choice::OpenDoor),
                    Interrupt::FloorTransition {
                        prompt_id, requires_branch_god_choice, ..
                    } => {
                        let choice = if requires_branch_god_choice {
                            Choice::DescendBranchAVeil
                        } else {
                            Choice::Descend
                        };
                        (prompt_id, choice)
                    }
                };
                game.apply_choice(prompt_id, choice).map_err(|err| format!("{err:?}"))?;
            }
            AdvanceStopReason::PausedAtBoundary { .. } | AdvanceStopReason::BudgetExhausted => {}
            AdvanceStopReason::EngineFailure(EngineFailureReason::StalledNoProgress) => {
                return Err(format!("stalled on seed={seed}"));
            }
        }
    }
    Err(format!("run exceeded tick budget on seed={seed}"))
}

#[test]
fn interrupt_count_seed_sweep_stays_in_target_band() {
    let seeds = [
        17_u64, 42, 99, 123, 321, 777, 1024, 2025, 4096, 9001, 12034, 22222, 33333, 44444, 55555,
        98765,
    ];

    let mut counts = Vec::new();
    for seed in seeds {
        let count = scripted_interrupt_count(seed).expect("seed sweep should complete cleanly");
        counts.push(count);
    }
    counts.sort_unstable();

    let p10 = percentile(&counts, 0.10);
    let p50 = percentile(&counts, 0.50);
    let p90 = percentile(&counts, 0.90);

    assert!(
        p10 >= 60,
        "interrupt density too low for session pacing: p10={p10}, p50={p50}, p90={p90}, counts={counts:?}"
    );
    assert!(
        (120..=190).contains(&p50),
        "median interrupt count out of range: p10={p10}, p50={p50}, p90={p90}, counts={counts:?}"
    );
    assert!(
        p90 <= 230,
        "interrupt density too high for session pacing: p10={p10}, p50={p50}, p90={p90}, counts={counts:?}"
    );
}
