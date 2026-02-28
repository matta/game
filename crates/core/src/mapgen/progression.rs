//! Floor progression and branch-policy rules used by map generation.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BranchProfile {
    Uncommitted,
    BranchA,
    BranchB,
}

pub const MAX_FLOORS: u8 = 5;
pub const STARTING_FLOOR_INDEX: u8 = 1;

const BASE_ENEMY_COUNT_BY_FLOOR: [usize; MAX_FLOORS as usize] = [3, 4, 4, 4, 4];
const ITEM_SPAWN_ATTEMPTS_BY_FLOOR: [usize; MAX_FLOORS as usize] = [1, 1, 1, 2, 2];

pub(super) fn enemy_spawn_count(floor_index: u8, branch_profile: BranchProfile) -> usize {
    base_enemy_count(floor_index) + branch_enemy_bonus(floor_index, branch_profile)
}

pub(super) fn item_spawn_attempts(floor_index: u8) -> usize {
    let floor_slot = floor_slot(floor_index);
    ITEM_SPAWN_ATTEMPTS_BY_FLOOR[floor_slot.min(ITEM_SPAWN_ATTEMPTS_BY_FLOOR.len() - 1)]
}

pub(super) fn branch_hazard_count(floor_index: u8, branch_profile: BranchProfile) -> usize {
    match branch_profile {
        BranchProfile::BranchB if floor_index > STARTING_FLOOR_INDEX => 3,
        _ => 0,
    }
}

pub(super) fn is_final_floor(floor_index: u8) -> bool {
    floor_index == MAX_FLOORS
}

pub(super) fn branch_code(branch_profile: BranchProfile) -> u64 {
    match branch_profile {
        BranchProfile::Uncommitted => 0,
        BranchProfile::BranchA => 1,
        BranchProfile::BranchB => 2,
    }
}

fn base_enemy_count(floor_index: u8) -> usize {
    let floor_slot = floor_slot(floor_index);
    BASE_ENEMY_COUNT_BY_FLOOR[floor_slot.min(BASE_ENEMY_COUNT_BY_FLOOR.len() - 1)]
}

fn branch_enemy_bonus(floor_index: u8, branch_profile: BranchProfile) -> usize {
    match branch_profile {
        BranchProfile::BranchA if floor_index > STARTING_FLOOR_INDEX => 1,
        _ => 0,
    }
}

fn floor_slot(floor_index: u8) -> usize {
    floor_index.saturating_sub(STARTING_FLOOR_INDEX) as usize
}
