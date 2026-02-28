//! Procedural map generation domain split into coherent submodules.

pub mod model;
pub mod progression;

mod generator;
mod grid;
mod layout;
mod seed;
mod spawns;
mod vaults;

pub use generator::MapGenerator;
pub use model::{EnemySpawn, GeneratedFloor, ItemSpawn};
pub use progression::{BranchProfile, MAX_FLOORS, STARTING_FLOOR_INDEX};

pub fn generate_floor(
    run_seed: u64,
    floor_index: u8,
    branch_profile: BranchProfile,
) -> GeneratedFloor {
    MapGenerator::new(run_seed, branch_profile).generate(floor_index)
}

#[cfg(test)]
mod tests {
    use super::{BranchProfile, MapGenerator};

    #[test]
    fn generate_floor_matches_map_generator_output() {
        let seed = 123_u64;
        let floor_index = 2_u8;
        let branch = BranchProfile::BranchA;

        let from_helper = super::generate_floor(seed, floor_index, branch);
        let from_generator = MapGenerator::new(seed, branch).generate(floor_index);

        assert_eq!(from_helper, from_generator);
    }
}
