use crate::{
    game::Game, journal::{InputJournal, InputPayload}, state::ContentPack, AdvanceStopReason, RunOutcome, GameMode,
};

#[derive(Debug, PartialEq)]
pub enum ReplayError {
    UnexpectedInterruption,
    MissingInput,
}

#[derive(Debug, PartialEq)]
pub struct ReplayResult {
    pub final_outcome: RunOutcome,
    pub final_snapshot_hash: u64,
    pub final_tick: u64,
}

pub fn replay_to_end(
    content: &ContentPack,
    journal: &InputJournal,
) -> Result<ReplayResult, ReplayError> {
    let mut game = Game::new(journal.seed, content, GameMode::Ironman);
    let mut input_iter = journal.inputs.iter();

    loop {
        let batch = game.advance(100);
        
        match batch.stop_reason {
            AdvanceStopReason::Finished(outcome) => {
                return Ok(ReplayResult {
                    final_outcome: outcome,
                    final_snapshot_hash: game.snapshot_hash(),
                    final_tick: game.current_tick(),
                });
            }
            AdvanceStopReason::Interrupted(_) => {
                if let Some(record) = input_iter.next() {
                    match &record.payload {
                        InputPayload::Choice { prompt_id, choice } => {
                            if game.apply_choice(*prompt_id, choice.clone()).is_err() {
                                return Err(ReplayError::UnexpectedInterruption);
                            }
                        }
                    }
                } else {
                    return Err(ReplayError::MissingInput);
                }
            }
            AdvanceStopReason::PausedAtBoundary { .. } => {
                // Automatically resume since this is headless continuous simulation
            }
            AdvanceStopReason::BudgetExhausted => {
                // Just continue next loop iteration
            }
        }
    }
}
