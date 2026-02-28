//! Text formatting for prompts, status lines, recaps, and event log entries.

use app::app_loop::{AppCompletion, AppMode};
use app::{engine_failure_code, format_snapshot_hash, reason_code};
use core::{AutoReason, Game, Interrupt, LogEvent};

pub fn status_text(mode: &AppMode) -> String {
    match mode {
        AppMode::PendingPrompt { interrupt, .. } => prompt_text(interrupt),
        AppMode::Finished(completion) => {
            format!("Finished: {}", completion_reason_code(completion))
        }
        AppMode::AutoPlay => "Auto-Explore ON (Space to pause)".to_string(),
        AppMode::Paused => "Paused (Space to Auto-Explore, Right to step)".to_string(),
    }
}

pub fn prompt_text(interrupt: &Interrupt) -> String {
    match interrupt {
        Interrupt::LootFound { .. } => "INTERRUPT: Loot found (L=keep, D=discard)".to_string(),
        Interrupt::EnemyEncounter { threat, .. } => {
            let dist_text = match threat.nearest_enemy_distance {
                Some(distance) => distance.to_string(),
                None => "?".to_string(),
            };
            format!(
                "INTERRUPT: {:?} sighted (F=fight, A=avoid) {} visible, nearest={}, Tags: {:?}",
                threat.primary_enemy_kind,
                threat.visible_enemy_count,
                dist_text,
                threat.danger_tags
            )
        }
        Interrupt::DoorBlocked { .. } => "INTERRUPT: Door blocked (O=open)".to_string(),
        Interrupt::FloorTransition { next_floor, requires_branch_god_choice, .. } => {
            if *requires_branch_god_choice {
                "INTERRUPT: Choose pact (1=A+Veil, 2=A+Forge, 3=B+Veil, 4=B+Forge)".to_string()
            } else {
                match next_floor {
                    Some(floor) => {
                        format!("INTERRUPT: Stairs reached (C=descend to floor {floor})")
                    }
                    None => "INTERRUPT: Final stairs reached (C=finish run)".to_string(),
                }
            }
        }
    }
}

pub fn auto_reason_text(reason: AutoReason) -> &'static str {
    match reason {
        AutoReason::Frontier => "Exploring the unknown...",
        AutoReason::Loot => "Moving to collect loot...",
        AutoReason::ThreatAvoidance => "Pathing around threats...",
        AutoReason::Stuck => "Auto-explore is stuck.",
        AutoReason::Door => "Moving to open a door...",
    }
}

pub fn completion_reason_code(completion: &AppCompletion) -> &'static str {
    match completion {
        AppCompletion::Outcome(outcome) => reason_code(outcome),
        AppCompletion::EngineFailure(reason) => engine_failure_code(reason),
    }
}

pub fn finished_recap_lines(game: &Game, run_seed: u64, completion: &AppCompletion) -> Vec<String> {
    let mut lines = vec![
        "Run recap:".to_string(),
        format!("Reason: {}", completion_reason_code(completion)),
        format!("Seed: {run_seed}"),
        format!("Snapshot: {}", format_snapshot_hash(game.snapshot_hash())),
        format!(
            "Floor/Branch/God: {}/{:?}/{:?}",
            game.state().floor_index,
            game.state().branch_profile,
            game.state().active_god
        ),
        format!("Tick: {}", game.current_tick()),
        "Threat trace (latest 5):".to_string(),
    ];

    for trace in game.state().threat_trace.iter().take(5) {
        lines.push(format!(
            "T{} vis={} min_dist={:?} retreat={}",
            trace.tick,
            trace.visible_enemy_count,
            trace.min_enemy_distance,
            trace.retreat_triggered
        ));
    }

    lines
}

pub fn event_log_line(event: &LogEvent) -> String {
    match event {
        LogEvent::AutoReasonChanged { reason, .. } => auto_reason_text(*reason).to_string(),
        LogEvent::EnemyEncountered { enemy } => format!("enemy encountered {:?}", enemy),
        LogEvent::ItemPickedUp { kind: _ } => "picked up item".to_string(),
        LogEvent::ItemDiscarded { kind: _ } => "discarded item".to_string(),
        LogEvent::EncounterResolved { enemy, fought } => {
            format!("encounter {:?} resolved fought={}", enemy, fought)
        }
        LogEvent::RecoveryHint { seed, hash_hex } => {
            format!("Recovered last run: seed={} hash={}", seed, hash_hex)
        }
        LogEvent::Notice(message) => message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        auto_reason_text, completion_reason_code, event_log_line, prompt_text, status_text,
    };
    use app::app_loop::{AppCompletion, AppMode};
    use core::{
        AutoReason, ChoicePromptId, DeathCause, EngineFailureReason, Interrupt, LogEvent, Pos,
    };

    #[test]
    fn status_text_reports_finished_reason_code() {
        let mode = AppMode::Finished(AppCompletion::Outcome(core::RunOutcome::Victory));
        assert_eq!(status_text(&mode), "Finished: WIN_CLEAR");
    }

    #[test]
    fn completion_reason_covers_engine_failures() {
        let completion = AppCompletion::EngineFailure(EngineFailureReason::StalledNoProgress);
        assert_eq!(completion_reason_code(&completion), "ENG_STALLED_NO_PROGRESS");
    }

    #[test]
    fn prompt_text_covers_branch_floor_transition() {
        let interrupt = Interrupt::FloorTransition {
            prompt_id: ChoicePromptId(17),
            current_floor: 3,
            next_floor: Some(4),
            requires_branch_god_choice: true,
        };

        assert_eq!(
            prompt_text(&interrupt),
            "INTERRUPT: Choose pact (1=A+Veil, 2=A+Forge, 3=B+Veil, 4=B+Forge)"
        );
    }

    #[test]
    fn prompt_text_covers_final_floor_transition() {
        let interrupt = Interrupt::FloorTransition {
            prompt_id: ChoicePromptId(31),
            current_floor: 5,
            next_floor: None,
            requires_branch_god_choice: false,
        };

        assert_eq!(prompt_text(&interrupt), "INTERRUPT: Final stairs reached (C=finish run)");
    }

    #[test]
    fn event_log_line_formats_recovery_hint() {
        let event = LogEvent::RecoveryHint { seed: 42, hash_hex: "0xabc".to_string() };
        assert_eq!(event_log_line(&event), "Recovered last run: seed=42 hash=0xabc");
    }

    #[test]
    fn auto_reason_text_formats_frontier_reason() {
        assert_eq!(auto_reason_text(AutoReason::Frontier), "Exploring the unknown...");
    }

    #[test]
    fn prompt_text_covers_door_blocked_interrupt() {
        let interrupt =
            Interrupt::DoorBlocked { prompt_id: ChoicePromptId(9), pos: Pos { x: 3, y: 5 } };

        assert_eq!(prompt_text(&interrupt), "INTERRUPT: Door blocked (O=open)");
    }

    #[test]
    fn status_text_reports_paused_mode() {
        assert_eq!(status_text(&AppMode::Paused), "Paused (Space to Auto-Explore, Right to step)");
    }

    #[test]
    fn completion_reason_reports_damage_death() {
        let completion = AppCompletion::Outcome(core::RunOutcome::Defeat(DeathCause::Damage));
        assert_eq!(completion_reason_code(&completion), "DMG_HP_ZERO");
    }
}
