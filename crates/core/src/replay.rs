use crate::{
    AdvanceStopReason, EngineFailureReason, GameMode, RunOutcome,
    content::ContentPack,
    game::Game,
    journal::{InputJournal, InputPayload},
};

#[derive(Debug, PartialEq)]
pub enum ReplayError {
    UnexpectedInterruption,
    MissingInput,
    SimulationStalled,
    EngineFailure(EngineFailureReason),
}

#[derive(Debug, PartialEq)]
pub struct ReplayResult {
    pub final_outcome: RunOutcome,
    pub final_snapshot_hash: u64,
    pub final_tick: u64,
}

const MAX_REPLAY_BATCHES: u32 = 512;

pub fn replay_to_end(
    content: &ContentPack,
    journal: &InputJournal,
) -> Result<ReplayResult, ReplayError> {
    let mut game = Game::new(journal.seed, content, GameMode::Ironman);
    let mut input_iter = journal.inputs.iter();
    let mut replay_batches = 0_u32;

    loop {
        replay_batches += 1;
        if replay_batches > MAX_REPLAY_BATCHES {
            return Err(ReplayError::SimulationStalled);
        }

        let batch = game.advance(100);
        if matches!(batch.stop_reason, AdvanceStopReason::BudgetExhausted)
            && batch.simulated_ticks == 0
        {
            return Err(ReplayError::SimulationStalled);
        }

        match batch.stop_reason {
            AdvanceStopReason::Finished(outcome) => {
                return Ok(ReplayResult {
                    final_outcome: outcome,
                    final_snapshot_hash: game.snapshot_hash(),
                    final_tick: game.current_tick(),
                });
            }
            AdvanceStopReason::Interrupted(_) => {
                let mut record_peek = input_iter.clone();
                if let Some(record) = record_peek.next() {
                    match &record.payload {
                        InputPayload::Choice { prompt_id, choice } => {
                            if game.apply_choice(*prompt_id, choice.clone()).is_err() {
                                return Err(ReplayError::UnexpectedInterruption);
                            }
                            input_iter.next(); // consume
                        }
                        InputPayload::PolicyUpdate { update, .. } => {
                            if game.apply_policy_update(update.clone()).is_err() {
                                return Err(ReplayError::UnexpectedInterruption);
                            }
                            input_iter.next(); // consume
                            // Keep checking since multiple policy updates can happen at a boundary
                            continue;
                        }
                        InputPayload::SwapActiveWeapon { .. } => {
                            if game.apply_swap_weapon().is_err() {
                                return Err(ReplayError::UnexpectedInterruption);
                            }
                            input_iter.next(); // consume
                            continue;
                        }
                    }
                } else {
                    return Err(ReplayError::MissingInput);
                }
            }
            AdvanceStopReason::PausedAtBoundary { .. } => {
                // Peek for policy updates
                let mut record_peek = input_iter.clone();
                if let Some(record) = record_peek.next() {
                    match &record.payload {
                        InputPayload::PolicyUpdate { update, .. } => {
                            if game.apply_policy_update(update.clone()).is_err() {
                                return Err(ReplayError::UnexpectedInterruption);
                            }
                            input_iter.next(); // consume
                            continue; // Multiple updates might exist here
                        }
                        InputPayload::SwapActiveWeapon { .. } => {
                            if game.apply_swap_weapon().is_err() {
                                return Err(ReplayError::UnexpectedInterruption);
                            }
                            input_iter.next();
                            continue;
                        }
                        _ => {}
                    }
                }
                // Automatically resume since this is headless continuous simulation
            }
            AdvanceStopReason::BudgetExhausted => {
                // Just continue next loop iteration
            }
            AdvanceStopReason::EngineFailure(e) => {
                return Err(ReplayError::EngineFailure(e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::InputJournal;
    use crate::types::{Choice, Interrupt, PolicyUpdate, Stance, TargetTag};

    const MAX_TEST_RUN_LOOP_COUNT: usize = 512;

    fn floor_transition_choice(interrupt: &Interrupt) -> Choice {
        match interrupt {
            Interrupt::FloorTransition { requires_branch_choice, .. } => {
                if *requires_branch_choice { Choice::DescendBranchA } else { Choice::Descend }
            }
            _ => panic!("expected FloorTransition"),
        }
    }

    #[test]
    fn test_replay_policy_equivalence() {
        let content = ContentPack::default();
        let mut game1 = Game::new(777, &content, GameMode::Ironman);
        let mut journal = InputJournal::new(777);

        // Policy Update at tick 0
        game1.apply_policy_update(PolicyUpdate::Stance(Stance::Defensive)).unwrap();
        journal.append_policy_update(0, PolicyUpdate::Stance(Stance::Defensive), 0);

        let mut seq = 1;
        let mut finished = false;
        for _ in 0..MAX_TEST_RUN_LOOP_COUNT {
            let res = game1.advance(100);
            match res.stop_reason {
                AdvanceStopReason::Finished(_) => {
                    finished = true;
                    break;
                }
                AdvanceStopReason::Interrupted(interrupt) => match interrupt {
                    Interrupt::DoorBlocked { prompt_id, .. } => {
                        game1.apply_choice(prompt_id, Choice::OpenDoor).unwrap();
                        journal.append_choice(prompt_id, Choice::OpenDoor, seq);
                        seq += 1;
                    }
                    Interrupt::EnemyEncounter { prompt_id, .. } => {
                        game1.apply_choice(prompt_id, Choice::Fight).unwrap();
                        journal.append_choice(prompt_id, Choice::Fight, seq);
                        seq += 1;
                    }
                    Interrupt::LootFound { prompt_id, .. } => {
                        game1.apply_choice(prompt_id, Choice::KeepLoot).unwrap();
                        journal.append_choice(prompt_id, Choice::KeepLoot, seq);
                        seq += 1;
                    }
                    int @ Interrupt::FloorTransition { prompt_id, .. } => {
                        let choice = floor_transition_choice(&int);
                        game1.apply_choice(prompt_id, choice.clone()).unwrap();
                        journal.append_choice(prompt_id, choice, seq);
                        seq += 1;
                    }
                },
                _ => {}
            }
        }
        assert!(finished, "test setup did not terminate within bounded batch budget");

        let hash1 = game1.snapshot_hash();
        let replay_res = replay_to_end(&content, &journal).unwrap();

        assert_eq!(hash1, replay_res.final_snapshot_hash);
    }

    #[test]
    fn test_replay_policy_edit_resume_equivalence() {
        let content = ContentPack::default();
        let mut game1 = Game::new(1234, &content, GameMode::Ironman);
        let mut journal = InputJournal::new(1234);

        let mut seq = 0;
        let mut policy_edited = false;

        let mut finished = false;
        for _ in 0..MAX_TEST_RUN_LOOP_COUNT {
            let res = game1.advance(20);
            match res.stop_reason {
                AdvanceStopReason::Finished(_) => {
                    finished = true;
                    break;
                }
                AdvanceStopReason::Interrupted(interrupt) => {
                    // edit policy during interrupt
                    if !policy_edited {
                        game1
                            .apply_policy_update(PolicyUpdate::TargetPriority(vec![
                                TargetTag::LowestHp,
                            ]))
                            .unwrap();
                        journal.append_policy_update(
                            seq,
                            PolicyUpdate::TargetPriority(vec![TargetTag::LowestHp]),
                            seq,
                        );
                        policy_edited = true;
                    }
                    match interrupt {
                        Interrupt::DoorBlocked { prompt_id, .. } => {
                            game1.apply_choice(prompt_id, Choice::OpenDoor).unwrap();
                            journal.append_choice(prompt_id, Choice::OpenDoor, seq);
                            seq += 1;
                        }
                        Interrupt::EnemyEncounter { prompt_id, .. } => {
                            game1.apply_choice(prompt_id, Choice::Fight).unwrap();
                            journal.append_choice(prompt_id, Choice::Fight, seq);
                            seq += 1;
                        }
                        Interrupt::LootFound { prompt_id, .. } => {
                            game1.apply_choice(prompt_id, Choice::KeepLoot).unwrap();
                            journal.append_choice(prompt_id, Choice::KeepLoot, seq);
                            seq += 1;
                        }
                        int @ Interrupt::FloorTransition { prompt_id, .. } => {
                            let choice = floor_transition_choice(&int);
                            game1.apply_choice(prompt_id, choice.clone()).unwrap();
                            journal.append_choice(prompt_id, choice, seq);
                            seq += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        assert!(finished, "test setup did not terminate within bounded batch budget");

        let hash1 = game1.snapshot_hash();
        let replay_res = replay_to_end(&content, &journal).unwrap();

        assert_eq!(hash1, replay_res.final_snapshot_hash);
    }

    #[test]
    fn test_replay_swap_weapon_equivalence() {
        let content = ContentPack::default();
        let mut game1 = Game::new(777, &content, GameMode::Ironman);
        let mut journal = InputJournal::new(777);

        // Swap weapon at tick 0 (pause boundary)
        game1.apply_swap_weapon().unwrap();
        journal.append_swap_weapon(0, 0);

        let mut seq = 1;

        // Play until end
        let mut finished = false;
        for _ in 0..MAX_TEST_RUN_LOOP_COUNT {
            let res = game1.advance(100);
            match res.stop_reason {
                AdvanceStopReason::Finished(_) => {
                    finished = true;
                    break;
                }
                AdvanceStopReason::Interrupted(interrupt) => match interrupt {
                    Interrupt::DoorBlocked { prompt_id, .. } => {
                        game1.apply_choice(prompt_id, Choice::OpenDoor).unwrap();
                        journal.append_choice(prompt_id, Choice::OpenDoor, seq);
                        seq += 1;
                    }
                    Interrupt::EnemyEncounter { prompt_id, .. } => {
                        game1.apply_choice(prompt_id, Choice::Fight).unwrap();
                        journal.append_choice(prompt_id, Choice::Fight, seq);
                        seq += 1;
                    }
                    Interrupt::LootFound { prompt_id, .. } => {
                        game1.apply_choice(prompt_id, Choice::KeepLoot).unwrap();
                        journal.append_choice(prompt_id, Choice::KeepLoot, seq);
                        seq += 1;
                    }
                    int @ Interrupt::FloorTransition { prompt_id, .. } => {
                        let choice = floor_transition_choice(&int);
                        game1.apply_choice(prompt_id, choice.clone()).unwrap();
                        journal.append_choice(prompt_id, choice, seq);
                        seq += 1;
                    }
                },
                _ => {}
            }
        }
        assert!(finished, "test setup did not terminate within bounded batch budget");

        let hash1 = game1.snapshot_hash();
        let replay_res = replay_to_end(&content, &journal).unwrap();

        assert_eq!(hash1, replay_res.final_snapshot_hash);
    }
}
