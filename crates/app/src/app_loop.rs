use core::journal::InputPayload;
use core::{AdvanceStopReason, ChoicePromptId, EngineFailureReason, Game, Interrupt, RunOutcome};
use macroquad::prelude::KeyCode;

/// How a run ended — either a normal game outcome or an engine-level failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCompletion {
    Outcome(RunOutcome),
    EngineFailure(EngineFailureReason),
}

/// An input that was accepted by the simulation this frame.
pub struct AcceptedInput {
    pub tick_boundary: u64,
    pub payload: InputPayload,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Paused,
    AutoPlay,
    PendingPrompt {
        interrupt: Interrupt,
        prompt_id: ChoicePromptId,
        auto_play_suspended: bool,
    },
    Finished(AppCompletion),
}

#[derive(Default)]
pub struct AppState {
    pub mode: AppMode,
    /// Inputs accepted during the current frame's `tick()` call.
    /// Drained by the caller after each tick to persist to the journal file.
    pub accepted_inputs: Vec<AcceptedInput>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process input and logic for a single frame, returning whether we should continue.
    /// In a real test, we would mock `is_key_pressed`, but for now we can just provide
    /// a list of keys pressed this frame, or a trait/closure.
    pub fn tick(&mut self, game: &mut Game, keys_pressed: &[KeyCode]) {
        self.accepted_inputs.clear();
        let mut advance_result = None;

        // Input handling
        match &self.mode {
            AppMode::Paused | AppMode::AutoPlay => {
                if keys_pressed.contains(&KeyCode::Space) {
                    self.mode = match self.mode {
                        AppMode::Paused => AppMode::AutoPlay,
                        AppMode::AutoPlay => {
                            game.request_pause();
                            AppMode::Paused
                        }
                        _ => AppMode::Paused,
                    };
                }

                if keys_pressed.contains(&KeyCode::Right) && matches!(self.mode, AppMode::Paused) {
                    advance_result = Some(game.advance(1));
                }

                if matches!(self.mode, AppMode::Paused) {
                    self.handle_policy_keys(game, keys_pressed);
                }
            }
            AppMode::PendingPrompt { prompt_id, auto_play_suspended, interrupt } => {
                let id = *prompt_id;
                let resume = *auto_play_suspended;
                match interrupt {
                    Interrupt::LootFound { .. } => {
                        if keys_pressed.contains(&KeyCode::L) {
                            self.apply_and_record_choice(game, id, core::Choice::KeepLoot);
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        } else if keys_pressed.contains(&KeyCode::D) {
                            self.apply_and_record_choice(game, id, core::Choice::DiscardLoot);
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        }
                    }
                    Interrupt::EnemyEncounter { .. } => {
                        if keys_pressed.contains(&KeyCode::F) {
                            self.apply_and_record_choice(game, id, core::Choice::Fight);
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        } else if keys_pressed.contains(&KeyCode::A) {
                            self.apply_and_record_choice(game, id, core::Choice::Avoid);
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        }
                    }
                    Interrupt::DoorBlocked { .. } => {
                        if keys_pressed.contains(&KeyCode::O) {
                            self.apply_and_record_choice(game, id, core::Choice::OpenDoor);
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        }
                    }
                    Interrupt::FloorTransition { requires_branch_god_choice, .. } => {
                        if *requires_branch_god_choice {
                            if keys_pressed.contains(&KeyCode::Key1) {
                                self.apply_and_record_choice(
                                    game,
                                    id,
                                    core::Choice::DescendBranchAVeil,
                                );
                                self.mode =
                                    if resume { AppMode::AutoPlay } else { AppMode::Paused };
                            } else if keys_pressed.contains(&KeyCode::Key2) {
                                self.apply_and_record_choice(
                                    game,
                                    id,
                                    core::Choice::DescendBranchAForge,
                                );
                                self.mode =
                                    if resume { AppMode::AutoPlay } else { AppMode::Paused };
                            } else if keys_pressed.contains(&KeyCode::Key3) {
                                self.apply_and_record_choice(
                                    game,
                                    id,
                                    core::Choice::DescendBranchBVeil,
                                );
                                self.mode =
                                    if resume { AppMode::AutoPlay } else { AppMode::Paused };
                            } else if keys_pressed.contains(&KeyCode::Key4) {
                                self.apply_and_record_choice(
                                    game,
                                    id,
                                    core::Choice::DescendBranchBForge,
                                );
                                self.mode =
                                    if resume { AppMode::AutoPlay } else { AppMode::Paused };
                            }
                        } else if keys_pressed.contains(&KeyCode::C) {
                            self.apply_and_record_choice(game, id, core::Choice::Descend);
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        }
                    }
                }

                self.handle_policy_keys(game, keys_pressed);
            }
            AppMode::Finished(_) => {
                // No inputs valid after completion
            }
        }

        // Logic stepping
        if matches!(self.mode, AppMode::AutoPlay) {
            advance_result = Some(game.advance(10)); // Batch step for synchronous iteration
        }

        if let Some(result) = advance_result {
            let auto_play_suspended = matches!(self.mode, AppMode::AutoPlay);
            self.apply_stop_reason(result.stop_reason, auto_play_suspended);
        }
    }

    pub fn apply_stop_reason(&mut self, stop_reason: AdvanceStopReason, auto_play_suspended: bool) {
        match stop_reason {
            AdvanceStopReason::PausedAtBoundary { .. } => {
                self.mode = AppMode::Paused;
            }
            AdvanceStopReason::Interrupted(interrupt) => {
                let prompt_id = match interrupt {
                    Interrupt::LootFound { prompt_id, .. } => prompt_id,
                    Interrupt::EnemyEncounter { prompt_id, .. } => prompt_id,
                    Interrupt::DoorBlocked { prompt_id, .. } => prompt_id,
                    Interrupt::FloorTransition { prompt_id, .. } => prompt_id,
                };
                self.mode = AppMode::PendingPrompt { interrupt, prompt_id, auto_play_suspended };
            }
            AdvanceStopReason::Finished(outcome) => {
                self.mode = AppMode::Finished(AppCompletion::Outcome(outcome));
            }
            AdvanceStopReason::BudgetExhausted => {
                // Continuing auto play on next frame
            }
            AdvanceStopReason::EngineFailure(reason) => {
                self.mode = AppMode::Finished(AppCompletion::EngineFailure(reason));
            }
        }
    }

    /// Apply a choice to the game and record it in `accepted_inputs`.
    fn apply_and_record_choice(
        &mut self,
        game: &mut Game,
        prompt_id: core::ChoicePromptId,
        choice: core::Choice,
    ) {
        let tick = game.current_tick();
        game.apply_choice(prompt_id, choice.clone()).expect("Failed to apply pending choice");
        self.accepted_inputs.push(AcceptedInput {
            tick_boundary: tick,
            payload: InputPayload::Choice { prompt_id, choice },
        });
    }

    /// Apply a policy update to the game and record it if accepted.
    fn apply_and_record_policy(&mut self, game: &mut Game, update: core::PolicyUpdate) {
        let tick = game.current_tick();
        if game.apply_policy_update(update.clone()).is_ok() {
            self.accepted_inputs.push(AcceptedInput {
                tick_boundary: tick,
                payload: InputPayload::PolicyUpdate { tick_boundary: tick, update },
            });
        }
    }

    /// Process policy-related key presses while paused.
    fn handle_policy_keys(&mut self, game: &mut Game, keys_pressed: &[KeyCode]) {
        if keys_pressed.contains(&KeyCode::M) {
            let next = match game.state().policy.fight_or_avoid {
                core::FightMode::Fight => core::FightMode::Avoid,
                core::FightMode::Avoid => core::FightMode::Fight,
            };
            self.apply_and_record_policy(game, core::PolicyUpdate::FightMode(next));
        }
        if keys_pressed.contains(&KeyCode::T) {
            let next = match game.state().policy.stance {
                core::Stance::Aggressive => core::Stance::Balanced,
                core::Stance::Balanced => core::Stance::Defensive,
                core::Stance::Defensive => core::Stance::Aggressive,
            };
            self.apply_and_record_policy(game, core::PolicyUpdate::Stance(next));
        }
        if keys_pressed.contains(&KeyCode::P) {
            let next = match game.state().policy.target_priority.first() {
                Some(core::TargetTag::Nearest) => vec![core::TargetTag::LowestHp],
                _ => vec![core::TargetTag::Nearest, core::TargetTag::LowestHp],
            };
            self.apply_and_record_policy(game, core::PolicyUpdate::TargetPriority(next));
        }
        if keys_pressed.contains(&KeyCode::R) {
            let current = game.state().policy.retreat_hp_threshold;
            let next = if current < 90 { current + 10 } else { 0 };
            self.apply_and_record_policy(game, core::PolicyUpdate::RetreatHpThreshold(next));
        }
        if keys_pressed.contains(&KeyCode::H) {
            let next = match game.state().policy.auto_heal_if_below_threshold {
                None => Some(30),
                Some(80) => None,
                Some(v) => Some(v + 10),
            };
            self.apply_and_record_policy(game, core::PolicyUpdate::AutoHealIfBelowThreshold(next));
        }
        if keys_pressed.contains(&KeyCode::I) {
            let next = match game.state().policy.position_intent {
                core::PositionIntent::HoldGround => core::PositionIntent::AdvanceToMelee,
                core::PositionIntent::AdvanceToMelee => {
                    core::PositionIntent::FleeToNearestExploredTile
                }
                core::PositionIntent::FleeToNearestExploredTile => core::PositionIntent::HoldGround,
            };
            self.apply_and_record_policy(game, core::PolicyUpdate::PositionIntent(next));
        }
        if keys_pressed.contains(&KeyCode::E) {
            self.apply_and_record_policy(
                game,
                core::PolicyUpdate::ExplorationMode(core::ExploreMode::Thorough),
            );
        }
        if keys_pressed.contains(&KeyCode::G) {
            self.apply_and_record_policy(
                game,
                core::PolicyUpdate::ResourceAggression(core::Aggro::Conserve),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppCompletion, AppMode, AppState};
    use core::{AdvanceStopReason, DeathCause, EngineFailureReason, RunOutcome};

    #[test]
    fn finished_outcome_maps_to_finished_mode() {
        let mut app = AppState::new();
        app.apply_stop_reason(
            AdvanceStopReason::Finished(RunOutcome::Defeat(DeathCause::Damage)),
            false,
        );
        assert_eq!(
            app.mode,
            AppMode::Finished(AppCompletion::Outcome(RunOutcome::Defeat(DeathCause::Damage,)))
        );
    }

    #[test]
    fn engine_failure_maps_to_finished_mode_without_panic() {
        let mut app = AppState::new();
        app.apply_stop_reason(
            AdvanceStopReason::EngineFailure(EngineFailureReason::StalledNoProgress),
            true,
        );
        assert_eq!(
            app.mode,
            AppMode::Finished(
                AppCompletion::EngineFailure(EngineFailureReason::StalledNoProgress,)
            )
        );
    }
}
