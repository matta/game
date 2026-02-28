//! Public map generator entrypoint and configuration.

use super::model::GeneratedFloor;
use super::progression::BranchProfile;

mod hazards;
mod pipeline;

pub struct MapGenerator {
    run_seed: u64,
    branch_profile: BranchProfile,
    width: usize,
    height: usize,
}

impl MapGenerator {
    pub fn new(run_seed: u64, branch_profile: BranchProfile) -> Self {
        Self { run_seed, branch_profile, width: 20, height: 15 }
    }

    pub fn generate(&self, floor_index: u8) -> GeneratedFloor {
        pipeline::generate_floor(
            self.run_seed,
            self.branch_profile,
            self.width,
            self.height,
            floor_index,
        )
    }
}

#[cfg(test)]
mod tests;
