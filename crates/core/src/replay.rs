use std::fmt;

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

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedInterruption => write!(f, "unexpected interruption during replay"),
            Self::MissingInput => write!(f, "journal is missing an expected input"),
            Self::SimulationStalled => write!(f, "simulation stalled during replay"),
            Self::EngineFailure(reason) => {
                write!(f, "engine failure during replay: {reason:?}")
            }
        }
    }
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

/// Replay all inputs from a journal and return the reconstructed `Game`.
///
/// Unlike `replay_to_end`, this function stops as soon as all journal inputs
/// have been consumed, returning the game in whatever state it is at that
/// point (possibly mid-run with an active interrupt or at a pause boundary).
/// This is the primary mechanism for crash recovery.
const MAX_REPLAY_INPUT_BATCHES: u32 = 1024;

pub fn replay_journal_inputs(
    content: &ContentPack,
    journal: &InputJournal,
) -> Result<Game, ReplayError> {
    let mut game = Game::new(journal.seed, content, GameMode::Ironman);
    let inputs = &journal.inputs;

    if inputs.is_empty() {
        return Ok(game);
    }

    let mut cursor = 0;
    let mut batches = 0u32;

    while cursor < inputs.len() {
        batches += 1;
        if batches > MAX_REPLAY_INPUT_BATCHES {
            return Err(ReplayError::SimulationStalled);
        }

        let batch = game.advance(100);
        if matches!(batch.stop_reason, AdvanceStopReason::BudgetExhausted)
            && batch.simulated_ticks == 0
        {
            return Err(ReplayError::SimulationStalled);
        }

        match batch.stop_reason {
            AdvanceStopReason::Finished(_) => return Ok(game),
            AdvanceStopReason::Interrupted(_) => {
                // Apply as many inputs as possible at this boundary.
                while cursor < inputs.len() {
                    let record = &inputs[cursor];
                    match &record.payload {
                        InputPayload::Choice { prompt_id, choice } => {
                            game.apply_choice(*prompt_id, choice.clone())
                                .map_err(|_| ReplayError::UnexpectedInterruption)?;
                            cursor += 1;
                            break; // Advance again after consuming a choice
                        }
                        InputPayload::PolicyUpdate { update, .. } => {
                            game.apply_policy_update(update.clone())
                                .map_err(|_| ReplayError::UnexpectedInterruption)?;
                            cursor += 1;
                        }
                        InputPayload::SwapActiveWeapon { .. } => {
                            game.apply_swap_weapon()
                                .map_err(|_| ReplayError::UnexpectedInterruption)?;
                            cursor += 1;
                        }
                    }
                }
            }
            AdvanceStopReason::PausedAtBoundary { .. } => {
                while cursor < inputs.len() {
                    let record = &inputs[cursor];
                    match &record.payload {
                        InputPayload::PolicyUpdate { update, .. } => {
                            game.apply_policy_update(update.clone())
                                .map_err(|_| ReplayError::UnexpectedInterruption)?;
                            cursor += 1;
                        }
                        InputPayload::SwapActiveWeapon { .. } => {
                            game.apply_swap_weapon()
                                .map_err(|_| ReplayError::UnexpectedInterruption)?;
                            cursor += 1;
                        }
                        _ => break,
                    }
                }
            }
            AdvanceStopReason::BudgetExhausted => {}
            AdvanceStopReason::EngineFailure(e) => {
                return Err(ReplayError::EngineFailure(e));
            }
        }
    }

    Ok(game)
}

#[cfg(test)]
mod tests;
