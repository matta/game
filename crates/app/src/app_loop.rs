use core::{AdvanceStopReason, ChoicePromptId, Game, Interrupt};
use macroquad::prelude::KeyCode;

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
    Finished,
}

#[derive(Default)]
pub struct AppState {
    pub mode: AppMode,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process input and logic for a single frame, returning whether we should continue.
    /// In a real test, we would mock `is_key_pressed`, but for now we can just provide
    /// a list of keys pressed this frame, or a trait/closure.
    pub fn tick(&mut self, game: &mut Game, keys_pressed: &[KeyCode]) {
        let mut advance_result = None;

        let handle_policy = |game: &mut Game| {
            if keys_pressed.contains(&KeyCode::M) {
                let next = match game.state().policy.fight_or_avoid {
                    core::FightMode::Fight => core::FightMode::Avoid,
                    core::FightMode::Avoid => core::FightMode::Fight,
                };
                let _ = game.apply_policy_update(core::PolicyUpdate::FightMode(next));
            }
            if keys_pressed.contains(&KeyCode::T) {
                let next = match game.state().policy.stance {
                    core::Stance::Aggressive => core::Stance::Balanced,
                    core::Stance::Balanced => core::Stance::Defensive,
                    core::Stance::Defensive => core::Stance::Aggressive,
                };
                let _ = game.apply_policy_update(core::PolicyUpdate::Stance(next));
            }
        };

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
                    handle_policy(game);
                }
            }
            AppMode::PendingPrompt { prompt_id, auto_play_suspended, interrupt } => {
                let id = *prompt_id;
                let resume = *auto_play_suspended;
                match interrupt {
                    Interrupt::LootFound { .. } => {
                        if keys_pressed.contains(&KeyCode::L) {
                            game.apply_choice(id, core::Choice::KeepLoot)
                                .expect("Failed to apply pending choice");
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        } else if keys_pressed.contains(&KeyCode::D) {
                            game.apply_choice(id, core::Choice::DiscardLoot)
                                .expect("Failed to apply pending choice");
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        }
                    }
                    Interrupt::EnemyEncounter { .. } => {
                        if keys_pressed.contains(&KeyCode::F) {
                            game.apply_choice(id, core::Choice::Fight)
                                .expect("Failed to apply pending choice");
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        } else if keys_pressed.contains(&KeyCode::A) {
                            game.apply_choice(id, core::Choice::Avoid)
                                .expect("Failed to apply pending choice");
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        }
                    }
                    Interrupt::DoorBlocked { .. } => {
                        if keys_pressed.contains(&KeyCode::O) {
                            game.apply_choice(id, core::Choice::OpenDoor)
                                .expect("Failed to apply pending choice");
                            self.mode = if resume { AppMode::AutoPlay } else { AppMode::Paused };
                        }
                    }
                }

                handle_policy(game);
            }
            AppMode::Finished => {
                // No inputs valid after completion
            }
        }

        // Logic stepping
        if matches!(self.mode, AppMode::AutoPlay) {
            advance_result = Some(game.advance(10)); // Batch step for synchronous iteration
        }

        if let Some(result) = advance_result {
            match result.stop_reason {
                AdvanceStopReason::PausedAtBoundary { .. } => {
                    self.mode = AppMode::Paused;
                }
                AdvanceStopReason::Interrupted(interrupt) => {
                    let prompt_id = match interrupt {
                        Interrupt::LootFound { prompt_id, .. } => prompt_id,
                        Interrupt::EnemyEncounter { prompt_id, .. } => prompt_id,
                        Interrupt::DoorBlocked { prompt_id, .. } => prompt_id,
                    };
                    self.mode = AppMode::PendingPrompt {
                        interrupt,
                        prompt_id,
                        auto_play_suspended: matches!(self.mode, AppMode::AutoPlay),
                    };
                }
                AdvanceStopReason::Finished(_) => {
                    self.mode = AppMode::Finished;
                }
                AdvanceStopReason::BudgetExhausted => {
                    // Continuing auto play on next frame
                }
            }
        }
    }
}
