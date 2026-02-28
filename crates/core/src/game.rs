//! Core game state and orchestration entry point.
//! This module composes focused submodules into the public `Game` API.
//! It does not own low-level navigation, visibility, item, or prompt implementation details.

use rand_chacha::ChaCha8Rng;

use crate::content::keys;
use crate::state::GameState;
use crate::types::*;

mod auto_explore;
mod bootstrap;
mod choices;
mod engine;
mod floor_transition;
mod hash;
mod items;
mod pathfinding;
mod prompts;
mod threat;
mod visibility;

#[cfg(test)]
mod test_support;

use auto_explore::{
    choose_frontier_intent, is_frontier_candidate, is_intent_target_still_valid, path_for_intent,
};
use pathfinding::{
    astar_path, astar_path_allow_hazards, enemy_path_to_player, manhattan, neighbors,
    reachable_discovered_walkable_tiles,
};
use prompts::PendingPrompt;
use visibility::compute_fov;

pub(super) const FOV_RADIUS: i32 = 10;
pub(super) const MAX_NO_PROGRESS_TICKS: u32 = 64;

pub struct Game {
    seed: u64,
    tick: u64,
    #[expect(dead_code)]
    rng: ChaCha8Rng,
    state: GameState,
    log: Vec<LogEvent>,
    next_input_seq: u64,
    pending_prompt: Option<PendingPrompt>,
    suppressed_enemy: Option<EntityId>,
    pause_requested: bool,
    at_pause_boundary: bool,
    finished_outcome: Option<RunOutcome>,
    no_progress_ticks: u32,
}

impl Game {
    pub fn get_fov_radius(&self) -> i32 {
        if self.state.active_perks.contains(&keys::PERK_SCOUT) {
            FOV_RADIUS + 2
        } else {
            FOV_RADIUS
        }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    pub fn push_log(&mut self, event: LogEvent) {
        self.log.push(event);
    }

    pub fn request_pause(&mut self) {
        self.pause_requested = true;
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn log(&self) -> &[LogEvent] {
        &self.log
    }
}
