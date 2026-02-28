//! Choice prompt dispatch and decision entry points.
//! This module routes prompt/choice pairs to focused choice handlers.
//! It does not own combat math, blink movement, policy mutation, or target ordering details.

use super::*;
use crate::game::prompts::PendingPromptKind;

mod avoidance;
mod combat;
mod floor_transition;
mod loot;
mod policy;
mod targeting;

impl Game {
    pub fn apply_choice(
        &mut self,
        prompt_id: ChoicePromptId,
        choice: Choice,
    ) -> Result<(), GameError> {
        let Some(prompt) = self.pending_prompt.clone() else {
            return Err(GameError::PromptMismatch);
        };
        if prompt.id != prompt_id {
            return Err(GameError::PromptMismatch);
        }

        let handled = match (prompt.kind, choice) {
            (PendingPromptKind::Loot { item }, Choice::KeepLoot) => {
                self.resolve_keep_loot_choice(item);
                true
            }
            (PendingPromptKind::Loot { item }, Choice::DiscardLoot) => {
                self.resolve_discard_loot_choice(item);
                true
            }
            (PendingPromptKind::EnemyEncounter { primary_enemy, .. }, Choice::Fight) => {
                self.resolve_fight_choice(primary_enemy);
                true
            }
            (PendingPromptKind::EnemyEncounter { primary_enemy, .. }, Choice::Avoid) => {
                self.resolve_avoid_choice(primary_enemy);
                true
            }
            (PendingPromptKind::DoorBlocked { pos }, Choice::OpenDoor) => {
                self.state.map.set_tile(pos, TileKind::Floor);
                let radius = self.get_fov_radius();
                compute_fov(
                    &mut self.state.map,
                    self.state.actors[self.state.player_id].pos,
                    radius,
                );
                true
            }
            (
                PendingPromptKind::FloorTransition {
                    current_floor,
                    next_floor,
                    requires_branch_god_choice,
                },
                floor_choice,
            ) if Self::is_floor_transition_choice(&floor_choice) => {
                self.resolve_floor_transition_choice(
                    current_floor,
                    next_floor,
                    requires_branch_god_choice,
                    floor_choice,
                )?;
                true
            }
            _ => false,
        };

        if !handled {
            return Err(GameError::InvalidChoice);
        }

        self.pending_prompt = None;
        self.next_input_seq += 1;
        self.no_progress_ticks = 0;
        Ok(())
    }
}
