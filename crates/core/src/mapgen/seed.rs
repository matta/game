//! Deterministic seed mixing and pseudo-random stream helpers for map generation.

use super::progression::{BranchProfile, branch_code};

pub(super) fn random_usize(seed: u64, stream: u64, min_value: usize, max_value: usize) -> usize {
    debug_assert!(min_value <= max_value);
    let range_size = max_value - min_value + 1;
    min_value + (mix_seed_stream(seed, stream) as usize % range_size)
}

pub(super) fn mix_seed_stream(seed: u64, stream: u64) -> u64 {
    let mut mixed = seed ^ stream.wrapping_mul(0xD6E8_FD9A_5B89_7A4D);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    mixed ^ (mixed >> 33)
}

pub(super) fn derive_floor_seed(
    run_seed: u64,
    floor_index: u8,
    branch_profile: BranchProfile,
) -> u64 {
    let mut mixed = run_seed ^ 0x9E37_79B9_7F4A_7C15;
    mixed ^= (floor_index as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed ^= branch_code(branch_profile).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^= mixed >> 30;
    mixed = mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_usize_stays_inside_requested_bounds() {
        for stream in 0..100 {
            let value = random_usize(12_345, stream, 7, 13);
            assert!((7..=13).contains(&value));
        }
    }

    #[test]
    fn floor_seed_changes_when_inputs_change() {
        let baseline = derive_floor_seed(99, 2, BranchProfile::Uncommitted);
        assert_ne!(baseline, derive_floor_seed(98, 2, BranchProfile::Uncommitted));
        assert_ne!(baseline, derive_floor_seed(99, 3, BranchProfile::Uncommitted));
        assert_ne!(baseline, derive_floor_seed(99, 2, BranchProfile::BranchA));
        assert_eq!(baseline, derive_floor_seed(99, 2, BranchProfile::Uncommitted));
    }
}
