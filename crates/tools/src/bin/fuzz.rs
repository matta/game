use anyhow::Result;
use clap::Parser;
use core::{AdvanceStopReason, Choice, ContentPack, Game, GameMode, Interrupt, TileKind};
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 42)]
    seed: u64,
    #[arg(short, long, default_value_t = 1000)]
    ticks: u32,
}

fn choose<T: Clone>(rng: &mut ChaCha8Rng, slice: &[T]) -> T {
    let p = rng.next_u64() as usize % slice.len();
    slice[p].clone()
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("Starting Fuzz harness on seed {} for max {} steps...", args.seed, args.ticks);
    let content = ContentPack::default();
    let mut game = Game::new(args.seed, &content, GameMode::Ironman);
    let mut rng = ChaCha8Rng::seed_from_u64(args.seed);

    let mut total_steps = 0;
    while total_steps < args.ticks {
        let result = game.advance(10);
        total_steps += result.simulated_ticks;

        match result.stop_reason {
            AdvanceStopReason::Finished(outcome) => {
                println!("Finished with outcome {:?} after {} ticks", outcome, total_steps);
                break;
            }
            AdvanceStopReason::Interrupted(interrupt) => {
                let (prompt_id, choice) = match interrupt {
                    Interrupt::EnemyEncounter { prompt_id, .. } => {
                        (
                            prompt_id,
                            choose(&mut rng, &[Choice::Fight, Choice::Avoid, Choice::Fight]),
                        ) // Bias to fight
                    }
                    Interrupt::LootFound { prompt_id, .. } => {
                        (prompt_id, choose(&mut rng, &[Choice::KeepLoot, Choice::DiscardLoot]))
                    }
                    Interrupt::DoorBlocked { prompt_id, .. } => {
                        // Must open door or we get stuck
                        (prompt_id, Choice::OpenDoor)
                    }
                    Interrupt::FloorTransition { prompt_id, requires_branch_choice, .. } => {
                        if requires_branch_choice {
                            (
                                prompt_id,
                                choose(&mut rng, &[Choice::DescendBranchA, Choice::DescendBranchB]),
                            )
                        } else {
                            (prompt_id, Choice::Descend)
                        }
                    }
                };
                game.apply_choice(prompt_id, choice).expect("fuzz applied invalid choice");
            }
            AdvanceStopReason::PausedAtBoundary { .. } => {
                // Not using manual pauses in fuzz
            }
            AdvanceStopReason::BudgetExhausted => {}
        }

        // Assert invariants
        let state = game.state();
        for (_, actor) in state.actors.iter() {
            assert!(actor.hp <= actor.max_hp, "Invariant failed: HP > Max HP");
            let tile = state.map.tile_at(actor.pos);
            assert!(tile != TileKind::Wall, "Invariant failed: Actor inside wall");
        }
    }

    println!("Fuzzing completed successfully.");
    Ok(())
}
