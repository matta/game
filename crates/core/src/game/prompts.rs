//! Prompt state and interrupt conversion for player-facing decisions.
//! This module exists to isolate prompt lifecycle and ID stability logic.
//! It does not own the gameplay consequences of accepted choices.

use super::*;
use crate::floor::{BranchProfile, MAX_FLOORS, STARTING_FLOOR_INDEX};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PendingPromptKind {
    Loot {
        item: ItemId,
    },
    EnemyEncounter {
        enemies: Vec<EntityId>,
        primary_enemy: EntityId,
        retreat_eligible: bool,
        threat: ThreatSummary,
    },
    DoorBlocked {
        pos: Pos,
    },
    FloorTransition {
        current_floor: u8,
        next_floor: Option<u8>,
        requires_branch_god_choice: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingPrompt {
    pub(super) id: ChoicePromptId,
    pub(super) kind: PendingPromptKind,
}

impl Game {
    pub(super) fn interrupt_loot(&mut self, item: ItemId, steps: u32) -> AdvanceResult {
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::Loot { item },
        };
        self.pending_prompt = Some(prompt.clone());
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }

    pub(super) fn interrupt_enemy(
        &mut self,
        enemies: Vec<EntityId>,
        primary_enemy: EntityId,
        steps: u32,
    ) -> AdvanceResult {
        let player = &self.state.actors[self.state.player_id];
        let player_pos = player.pos;
        let hp_percent = (player.hp * 100) / player.max_hp;
        let retreat_eligible = hp_percent <= (self.state.policy.retreat_hp_threshold as i32);

        let mut tags = Vec::new();
        for &enemy_id in &enemies {
            if let Some(actor) = self.state.actors.get(enemy_id) {
                tags.extend(super::threat::danger_tags_for_kind(actor.kind));
            }
        }
        tags.sort();
        tags.dedup();

        let visible_enemy_count = self
            .state
            .actors
            .iter()
            .filter(|(id, actor)| {
                *id != self.state.player_id && self.state.map.is_visible(actor.pos)
            })
            .count();
        let nearest_enemy_distance = self
            .state
            .actors
            .iter()
            .filter_map(|(id, actor)| {
                if id != self.state.player_id && self.state.map.is_visible(actor.pos) {
                    Some(manhattan(player_pos, actor.pos))
                } else {
                    None
                }
            })
            .min();
        let primary_enemy_kind = self.state.actors[primary_enemy].kind;

        let threat = ThreatSummary {
            danger_tags: tags,
            visible_enemy_count,
            nearest_enemy_distance,
            primary_enemy_kind,
        };

        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::EnemyEncounter {
                enemies,
                primary_enemy,
                retreat_eligible,
                threat: threat.clone(),
            },
        };
        self.pending_prompt = Some(prompt.clone());
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }

    pub(super) fn interrupt_door(&mut self, pos: Pos, steps: u32) -> AdvanceResult {
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::DoorBlocked { pos },
        };
        self.pending_prompt = Some(prompt.clone());
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }

    pub(super) fn interrupt_floor_transition(&mut self, steps: u32) -> AdvanceResult {
        let next_floor = if self.state.floor_index < MAX_FLOORS {
            Some(self.state.floor_index + 1)
        } else {
            None
        };
        let requires_branch_god_choice = self.state.floor_index == STARTING_FLOOR_INDEX
            && self.state.branch_profile == BranchProfile::Uncommitted
            && self.state.active_god.is_none()
            && next_floor.is_some();
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::FloorTransition {
                current_floor: self.state.floor_index,
                next_floor,
                requires_branch_god_choice,
            },
        };
        self.pending_prompt = Some(prompt.clone());
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }

    pub(super) fn prompt_to_interrupt(&self, prompt: PendingPrompt) -> Interrupt {
        match prompt.kind {
            PendingPromptKind::Loot { item } => Interrupt::LootFound {
                prompt_id: prompt.id,
                item,
                kind: self.state.items[item].kind,
            },
            PendingPromptKind::EnemyEncounter {
                enemies,
                primary_enemy,
                retreat_eligible,
                threat,
            } => Interrupt::EnemyEncounter {
                prompt_id: prompt.id,
                enemies,
                primary_enemy,
                retreat_eligible,
                threat,
            },
            PendingPromptKind::DoorBlocked { pos } => {
                Interrupt::DoorBlocked { prompt_id: prompt.id, pos }
            }
            PendingPromptKind::FloorTransition {
                current_floor,
                next_floor,
                requires_branch_god_choice,
            } => Interrupt::FloorTransition {
                prompt_id: prompt.id,
                current_floor,
                next_floor,
                requires_branch_god_choice,
            },
        }
    }
}
