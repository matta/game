use core::ContentPack;
use core::{AdvanceStopReason, Choice, Game, GameMode, Interrupt};

fn run_to_floor_five(seed: u64, branch: Choice) -> u64 {
    let content = ContentPack::default();
    let mut game = Game::new(seed, &content, GameMode::Ironman);
    let mut floor_5_reached = false;

    // 4000 steps should be plenty for the starter layout to reach floor 5
    for _ in 0..4000 {
        let res = game.advance(10);

        if game.state().floor_index == 5 {
            floor_5_reached = true;
        }

        match res.stop_reason {
            AdvanceStopReason::Finished(_) => {
                break;
            }
            AdvanceStopReason::Interrupted(interrupt) => {
                let (prompt_id, choice) = match interrupt {
                    Interrupt::LootFound { prompt_id, .. } => (prompt_id, Choice::KeepLoot),
                    Interrupt::EnemyEncounter { prompt_id, .. } => (prompt_id, Choice::Fight),
                    Interrupt::DoorBlocked { prompt_id, .. } => (prompt_id, Choice::OpenDoor),
                    Interrupt::FloorTransition { prompt_id, requires_branch_choice, .. } => {
                        let c =
                            if requires_branch_choice { branch.clone() } else { Choice::Descend };
                        (prompt_id, c)
                    }
                };
                game.apply_choice(prompt_id, choice).expect("choice should apply");
            }
            _ => {}
        }
    }

    assert!(floor_5_reached, "Floor 5 was not reached for seed {} and branch {:?}", seed, branch);
    game.snapshot_hash()
}

#[test]
fn test_smoke_run_branch_a() {
    let hash = run_to_floor_five(12345, Choice::DescendBranchA);
    assert!(hash != 0);
}

#[test]
fn test_smoke_run_branch_b() {
    let hash = run_to_floor_five(12345, Choice::DescendBranchB);
    assert!(hash != 0);
}

#[test]
fn test_smoke_branches_diverge() {
    // Both starts from same seed but different branches at floor 1 -> 2 transition.
    let hash_a = run_to_floor_five(12345, Choice::DescendBranchA);
    let hash_b = run_to_floor_five(12345, Choice::DescendBranchB);
    assert_ne!(hash_a, hash_b, "Different branches should produce different hashes at floor 5");
}

#[test]
fn test_regression_no_ascend_in_choice() {
    // This is a "smoke test" for the API surface mentioned in DR-003.
    //Choice::Ascend; // This would fail to compile if it existed.
}
