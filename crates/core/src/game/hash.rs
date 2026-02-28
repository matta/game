//! Stable snapshot hashing for deterministic verification.
//! This module exists to keep hashing concerns separate from simulation control code.
//! It does not own replay execution or journal persistence policies.

use std::hash::Hasher;

use super::*;
use crate::floor::BranchProfile;
use xxhash_rust::xxh3::Xxh3;

impl Game {
    pub fn snapshot_hash(&self) -> u64 {
        let mut hasher = Xxh3::new();
        hasher.write_u64(self.seed);
        hasher.write_u64(self.tick);
        hasher.write_u64(self.next_input_seq);
        hasher.write_u32(self.no_progress_ticks);
        hasher.write_u8(self.state.floor_index);
        hasher.write_u8(match self.state.branch_profile {
            BranchProfile::Uncommitted => 0,
            BranchProfile::BranchA => 1,
            BranchProfile::BranchB => 2,
        });
        hasher.write_u8(match self.state.active_god {
            None => 0,
            Some(GodId::Veil) => 1,
            Some(GodId::Forge) => 2,
        });
        let player = &self.state.actors[self.state.player_id];
        hasher.write_i32(player.pos.x);
        hasher.write_i32(player.pos.y);
        hasher.write_i32(self.state.sanctuary_tile.x);
        hasher.write_i32(self.state.sanctuary_tile.y);
        hasher.write_u8(u8::from(self.state.sanctuary_active));
        if let Some(intent) = self.state.auto_intent {
            hasher.write_i32(intent.target.x);
            hasher.write_i32(intent.target.y);
            hasher.write_u16(intent.path_len);
            hasher.write_u8(intent.reason as u8);
        }
        hasher.finish()
    }
}
