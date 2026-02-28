use core::{
    AdvanceStopReason, Choice, ContentPack, DeathCause, EngineFailureReason, Game, GameMode,
    Interrupt, RunOutcome, TileKind,
};
use proptest::{
    arbitrary::any,
    test_runner::{Config as ProptestConfig, TestCaseError, TestRunner},
};
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};

fn choose<T: Clone>(rng: &mut ChaCha8Rng, slice: &[T]) -> T {
    let p = rng.next_u64() as usize % slice.len();
    slice[p].clone()
}

fn run_fuzz_simulation(map_seed: u64, choice_seed: u64, max_ticks: u32) -> Result<(), String> {
    let content = ContentPack::default();
    let mut game = Game::new(map_seed, &content, GameMode::Ironman);
    let mut rng = ChaCha8Rng::seed_from_u64(choice_seed);

    let mut total_steps = 0;
    while total_steps < max_ticks {
        let result = game.advance(10);
        total_steps += result.simulated_ticks;

        match result.stop_reason {
            AdvanceStopReason::Finished(RunOutcome::Victory) => break,
            AdvanceStopReason::Finished(RunOutcome::Defeat(DeathCause::Damage)) => break,
            AdvanceStopReason::Finished(RunOutcome::Defeat(DeathCause::Poison)) => break,
            AdvanceStopReason::EngineFailure(EngineFailureReason::StalledNoProgress) => {
                return Err(format!(
                    "Invariant failed: StalledNoProgress on map_seed {}",
                    map_seed
                ));
            }
            AdvanceStopReason::Interrupted(interrupt) => {
                let (prompt_id, choice) = match interrupt {
                    Interrupt::EnemyEncounter { prompt_id, .. } => (
                        prompt_id,
                        choose(&mut rng, &[Choice::Fight, Choice::Avoid, Choice::Fight]),
                    ),
                    Interrupt::LootFound { prompt_id, .. } => {
                        (prompt_id, choose(&mut rng, &[Choice::KeepLoot, Choice::DiscardLoot]))
                    }
                    Interrupt::DoorBlocked { prompt_id, .. } => (prompt_id, Choice::OpenDoor),
                    Interrupt::FloorTransition {
                        prompt_id, requires_branch_god_choice, ..
                    } => {
                        if requires_branch_god_choice {
                            (
                                prompt_id,
                                choose(
                                    &mut rng,
                                    &[
                                        Choice::DescendBranchAVeil,
                                        Choice::DescendBranchAForge,
                                        Choice::DescendBranchBVeil,
                                        Choice::DescendBranchBForge,
                                    ],
                                ),
                            )
                        } else {
                            (prompt_id, Choice::Descend)
                        }
                    }
                };
                game.apply_choice(prompt_id, choice).expect("fuzz applied invalid choice");
            }
            AdvanceStopReason::PausedAtBoundary { .. } => {}
            AdvanceStopReason::BudgetExhausted => {}
        }

        let state = game.state();
        for (_, actor) in state.actors.iter() {
            if actor.hp > actor.max_hp {
                return Err(format!("Invariant failed: HP > Max HP on map_seed {}", map_seed));
            }
            let tile = state.map.tile_at(actor.pos);
            if tile == TileKind::Wall {
                return Err(format!(
                    "Invariant failed: Actor inside wall on map_seed {}",
                    map_seed
                ));
            }
        }
    }

    Ok(())
}

#[test]
fn test_fuzz_game_simulation() {
    let mut runner = TestRunner::new(ProptestConfig::with_cases(20));
    let seeds = (any::<u64>(), any::<u64>());

    runner
        .run(&seeds, |(map_seed, choice_seed)| {
            run_fuzz_simulation(map_seed, choice_seed, 2000).map_err(TestCaseError::fail)?;
            Ok(())
        })
        .expect("semantic fuzz simulation should preserve invariants");
}
