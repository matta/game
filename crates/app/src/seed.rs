use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedChoice {
    Cli(u64),
    Generated(u64),
}

impl SeedChoice {
    pub fn value(self) -> u64 {
        match self {
            Self::Cli(seed) | Self::Generated(seed) => seed,
        }
    }
}

static GENERATED_SEED_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn generate_runtime_seed() -> u64 {
    let now_nanos =
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0_u128, |duration| duration.as_nanos());
    let pid = u64::from(std::process::id());
    let counter = GENERATED_SEED_COUNTER.fetch_add(1, Ordering::Relaxed);

    let entropy = (now_nanos as u64)
        ^ ((now_nanos >> 64) as u64)
        ^ pid.rotate_left(17)
        ^ counter.rotate_left(7);

    mix_seed(entropy)
}

pub fn resolve_seed_from_args(args: &[String], generated_seed: u64) -> Result<SeedChoice, String> {
    let mut selected_seed = None;
    let mut index = 1usize;

    while index < args.len() {
        let argument = args[index].as_str();

        if argument == "--seed" {
            let Some(value) = args.get(index + 1) else {
                return Err("missing value for --seed".to_string());
            };
            if selected_seed.is_some() {
                return Err("seed provided more than once".to_string());
            }
            selected_seed = Some(parse_seed_value(value)?);
            index += 2;
            continue;
        }

        if let Some(value) = argument.strip_prefix("--seed=") {
            if selected_seed.is_some() {
                return Err("seed provided more than once".to_string());
            }
            selected_seed = Some(parse_seed_value(value)?);
        }
        index += 1;
    }

    Ok(match selected_seed {
        Some(seed) => SeedChoice::Cli(seed),
        None => SeedChoice::Generated(generated_seed),
    })
}

fn parse_seed_value(raw_value: &str) -> Result<u64, String> {
    raw_value.parse::<u64>().map_err(|_| format!("seed value '{raw_value}' must be a number"))
}

fn mix_seed(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| part.to_string()).collect()
    }

    #[test]
    fn uses_generated_seed_when_seed_flag_is_absent() {
        let args = as_args(&["game"]);
        let choice =
            resolve_seed_from_args(&args, 9_876_543).expect("seed resolution should not fail");
        assert_eq!(choice, SeedChoice::Generated(9_876_543));
    }

    #[test]
    fn parses_seed_flag_with_separate_value() {
        let args = as_args(&["game", "--seed", "4242"]);
        let choice = resolve_seed_from_args(&args, 1).expect("valid --seed should parse");
        assert_eq!(choice, SeedChoice::Cli(4_242));
    }

    #[test]
    fn parses_seed_flag_with_inline_value() {
        let args = as_args(&["game", "--seed=2026"]);
        let choice = resolve_seed_from_args(&args, 1).expect("valid --seed should parse");
        assert_eq!(choice, SeedChoice::Cli(2_026));
    }

    #[test]
    fn errors_when_seed_flag_has_no_value() {
        let args = as_args(&["game", "--seed"]);
        let err = resolve_seed_from_args(&args, 1).expect_err("missing seed value should error");
        assert!(err.contains("missing"), "error should explain missing value: {err}");
    }

    #[test]
    fn errors_when_seed_value_is_not_a_number() {
        let args = as_args(&["game", "--seed=abc"]);
        let err =
            resolve_seed_from_args(&args, 1).expect_err("non-numeric seed value should error");
        assert!(err.contains("number"), "error should explain numeric requirement: {err}");
    }

    #[test]
    fn errors_when_seed_is_provided_more_than_once() {
        let args = as_args(&["game", "--seed=1", "--seed", "2"]);
        let err =
            resolve_seed_from_args(&args, 1).expect_err("duplicate seed flags should be rejected");
        assert!(err.contains("more than once"), "error should explain duplicate seed: {err}");
    }

    #[test]
    fn generated_seed_changes_between_calls() {
        let first = generate_runtime_seed();
        let second = generate_runtime_seed();
        assert_ne!(first, second, "runtime seed generation should vary per call");
    }
}
