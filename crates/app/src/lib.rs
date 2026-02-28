pub mod app_loop;
pub mod run_state_file;
pub mod seed;

/// Format a seed as an exact decimal string with no prefix or suffix.
pub fn format_seed(seed: u64) -> String {
    seed.to_string()
}

/// Format a snapshot hash as `0x` followed by exactly 16 lowercase hex digits.
pub fn format_snapshot_hash(hash: u64) -> String {
    format!("0x{hash:016x}")
}

/// Map a `RunOutcome` to its reason code string.
pub fn reason_code(outcome: &core::RunOutcome) -> &'static str {
    match outcome {
        core::RunOutcome::Victory => "WIN_CLEAR",
        core::RunOutcome::Defeat(core::DeathCause::Damage) => "DMG_HP_ZERO",
        core::RunOutcome::Defeat(core::DeathCause::Poison) => "PSN_HP_ZERO",
    }
}

/// Map an `EngineFailureReason` to its reason code string.
pub fn engine_failure_code(reason: &core::EngineFailureReason) -> &'static str {
    match reason {
        core::EngineFailureReason::StalledNoProgress => "ENG_STALLED_NO_PROGRESS",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_seed_is_exact_decimal() {
        assert_eq!(format_seed(0), "0");
        assert_eq!(format_seed(12345), "12345");
        assert_eq!(format_seed(u64::MAX), "18446744073709551615");
    }

    #[test]
    fn format_snapshot_hash_is_16_hex_digits() {
        assert_eq!(format_snapshot_hash(0), "0x0000000000000000");
        assert_eq!(format_snapshot_hash(255), "0x00000000000000ff");
        assert_eq!(format_snapshot_hash(u64::MAX), "0xffffffffffffffff");
        assert_eq!(format_snapshot_hash(0xDEADBEEF), "0x00000000deadbeef");
    }

    #[test]
    fn reason_codes_are_correct() {
        assert_eq!(reason_code(&core::RunOutcome::Victory), "WIN_CLEAR");
        assert_eq!(reason_code(&core::RunOutcome::Defeat(core::DeathCause::Damage)), "DMG_HP_ZERO");
        assert_eq!(reason_code(&core::RunOutcome::Defeat(core::DeathCause::Poison)), "PSN_HP_ZERO");
    }

    #[test]
    fn engine_failure_codes_are_correct() {
        assert_eq!(
            engine_failure_code(&core::EngineFailureReason::StalledNoProgress),
            "ENG_STALLED_NO_PROGRESS"
        );
    }
}
