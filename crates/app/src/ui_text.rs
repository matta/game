//! Text formatting for prompts, status lines, recaps, and event log entries.

use app::app_loop::{AppCompletion, AppMode};
use app::{engine_failure_code, format_snapshot_hash, reason_code};
use core::{
    AutoExploreIntent, AutoReason, BranchProfile, Game, GodId, Interrupt, LogEvent, Policy,
    WeaponSlot,
};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerHudSnapshot {
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: u32,
    pub active_weapon_slot: WeaponSlot,
    pub equipped_weapon: Option<&'static str>,
    pub reserve_weapon: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HudSnapshot {
    pub tick: u64,
    pub run_seed: u64,
    pub floor_index: u8,
    pub branch_profile: BranchProfile,
    pub active_god: Option<GodId>,
    pub snapshot_hash: u64,
    pub auto_intent: Option<AutoExploreIntent>,
    pub player: PlayerHudSnapshot,
    pub active_perks: Vec<&'static str>,
    pub kills_this_floor: u32,
    pub policy: Policy,
}

pub fn gather_hud_snapshot(game: &Game, run_seed: u64) -> HudSnapshot {
    let state = game.state();
    let player = &state.actors[state.player_id];
    HudSnapshot {
        tick: game.current_tick(),
        run_seed,
        floor_index: state.floor_index,
        branch_profile: state.branch_profile,
        active_god: state.active_god,
        snapshot_hash: game.snapshot_hash(),
        auto_intent: state.auto_intent,
        player: PlayerHudSnapshot {
            hp: player.hp,
            max_hp: player.max_hp,
            attack: player.attack,
            defense: player.defense,
            speed: player.speed,
            active_weapon_slot: player.active_weapon_slot,
            equipped_weapon: player.equipped_weapon,
            reserve_weapon: player.reserve_weapon,
        },
        active_perks: state.active_perks.clone(),
        kills_this_floor: state.kills_this_floor,
        policy: state.policy.clone(),
    }
}

pub fn stats_panel_lines_from_snapshot(snapshot: &HudSnapshot) -> Vec<String> {
    let mut lines = vec![
        format!("Tick: {}", snapshot.tick),
        format!("Seed: {}", snapshot.run_seed),
        format!("Floor: {} / 5", snapshot.floor_index),
        format!("Branch: {:?}", snapshot.branch_profile),
        format!("God: {:?}", snapshot.active_god),
        format!("Hash: {}", format_snapshot_hash(snapshot.snapshot_hash)),
    ];

    let intent_text = if let Some(intent) = snapshot.auto_intent {
        format!(
            "Intent: {:?} target=({}, {}) path_len={}",
            intent.reason, intent.target.x, intent.target.y, intent.path_len
        )
    } else {
        "Intent: none".to_string()
    };
    lines.push(intent_text);
    lines.push("Level: not tracked yet (planned with XP milestone)".to_string());

    let p = &snapshot.player;
    lines.push(format!(
        "HP: {}/{}  ATK: {}  DEF: {}  SPD: {}",
        p.hp, p.max_hp, p.attack, p.defense, p.speed
    ));
    lines.push(format!(
        "Active Slot: {:?}  Weapon: {}",
        p.active_weapon_slot,
        p.equipped_weapon.unwrap_or("None")
    ));
    lines.push(format!("Reserve: {}", p.reserve_weapon.unwrap_or("None")));

    let perks = if snapshot.active_perks.is_empty() {
        "None".to_string()
    } else {
        snapshot.active_perks.join(", ")
    };
    lines.push(format!("Perks: {}", perks));
    lines.push(format!("Kills this floor: {}", snapshot.kills_this_floor));

    let policy = &snapshot.policy;
    let auto_heal_text = policy
        .auto_heal_if_below_threshold
        .map(|v| format!("{v}%"))
        .unwrap_or_else(|| "off".to_string());
    lines.push(format!(
        "Policy: stance={:?} retreat_if_hp<= {}% auto_heal={}",
        policy.stance, policy.retreat_hp_threshold, auto_heal_text
    ));

    lines
}

pub fn stats_panel_lines(game: &Game, run_seed: u64) -> Vec<String> {
    let snapshot = gather_hud_snapshot(game, run_seed);
    stats_panel_lines_from_snapshot(&snapshot)
}

#[cfg(test)]
mod tests {
    use super::{
        HudSnapshot, PlayerHudSnapshot, auto_reason_text, completion_reason_code, event_log_line,
        prompt_text, stats_panel_lines, stats_panel_lines_from_snapshot, status_text,
    };
    use app::app_loop::{AppCompletion, AppMode};
    use core::{
        AutoReason, ChoicePromptId, DeathCause, EngineFailureReason, Interrupt, LogEvent, Policy,
        Pos, WeaponSlot, content::ContentPack, mapgen::BranchProfile,
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

    #[test]
    fn stats_panel_lines_cover_player_and_policy_data() {
        let content = ContentPack::build_default();
        let game = core::Game::new(7, &content, core::GameMode::Ironman);
        let lines = stats_panel_lines(&game, 7);

        assert!(lines.iter().any(|l| l.contains("HP: 20/20")), "expected player HP line");
        assert!(
            lines.iter().any(|l| l.contains("Active Slot")),
            "expected active weapon slot line"
        );
        assert!(lines.iter().any(|l| l.contains("Policy:")), "expected policy summary line");
        assert!(
            lines.iter().any(|l| l.contains("Level: not tracked yet")),
            "expected level placeholder line"
        );
    }

    #[test]
    fn stats_panel_lines_update_when_snapshot_changes() {
        let snapshot = HudSnapshot {
            tick: 12,
            run_seed: 99,
            floor_index: 2,
            branch_profile: BranchProfile::BranchA,
            active_god: Some(core::GodId::Veil),
            snapshot_hash: 12345,
            auto_intent: None,
            player: PlayerHudSnapshot {
                hp: 5,
                max_hp: 25,
                attack: 9,
                defense: 3,
                speed: 11,
                active_weapon_slot: WeaponSlot::Reserve,
                equipped_weapon: Some("weapon_phase_dagger"),
                reserve_weapon: Some("weapon_rusty_sword"),
            },
            active_perks: vec!["perk_scout"],
            kills_this_floor: 4,
            policy: Policy {
                fight_or_avoid: core::FightMode::Fight,
                stance: core::Stance::Aggressive,
                target_priority: vec![core::TargetTag::LowestHp],
                retreat_hp_threshold: 15,
                auto_heal_if_below_threshold: Some(30),
                position_intent: core::PositionIntent::AdvanceToMelee,
                resource_aggression: core::Aggro::Conserve,
                exploration_mode: core::ExploreMode::Thorough,
            },
        };

        let lines = stats_panel_lines_from_snapshot(&snapshot);
        assert!(
            lines.iter().any(|l| l.contains("HP: 5/25")),
            "expected updated hp values to render"
        );
        assert!(
            lines.iter().any(|l| l.contains("weapon_phase_dagger")),
            "expected equipped weapon id to render"
        );
        assert!(lines.iter().any(|l| l.contains("perk_scout")), "expected perk list to render");
        assert!(
            lines.iter().any(|l| l.contains("Kills this floor: 4")),
            "expected kill count to render"
        );
        assert!(
            lines.iter().any(|l| l.contains("retreat_if_hp<= 15%")),
            "expected policy thresholds to render"
        );
    }
}
