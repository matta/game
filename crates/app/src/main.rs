use app::{
    app_loop::{AppMode, AppState},
    seed::{generate_runtime_seed, resolve_seed_from_args},
};
use core::{ContentPack, Game, GameMode, Interrupt, LogEvent, Pos, TileKind};
use macroquad::prelude::*;
use std::{env, process::exit};

#[macroquad::main("Roguelike")]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let generated_seed = generate_runtime_seed();
    let selected_seed = match resolve_seed_from_args(&args, generated_seed) {
        Ok(seed_choice) => seed_choice,
        Err(message) => {
            let program_name = args.first().map_or("game", String::as_str);
            eprintln!("Error: {message}");
            eprintln!("Usage: {program_name} [--seed <u64>]");
            exit(2);
        }
    };
    let run_seed = selected_seed.value();

    let content = ContentPack::default();
    let mut game = Game::new(run_seed, &content, GameMode::Ironman);

    let mut app_state = AppState::default();

    loop {
        clear_background(BLACK);

        let mut keys_pressed = vec![];
        if is_key_pressed(KeyCode::Space) {
            keys_pressed.push(KeyCode::Space);
        }
        if is_key_pressed(KeyCode::Right) {
            keys_pressed.push(KeyCode::Right);
        }
        for key in [
            KeyCode::L,
            KeyCode::D,
            KeyCode::F,
            KeyCode::A,
            KeyCode::B,
            KeyCode::O,
            KeyCode::C,
            KeyCode::M,
            KeyCode::T,
            KeyCode::P,
            KeyCode::R,
            KeyCode::H,
            KeyCode::I,
            KeyCode::E,
            KeyCode::G,
        ] {
            if is_key_pressed(key) {
                keys_pressed.push(key);
            }
        }

        app_state.tick(&mut game, &keys_pressed);

        let map_top = 50.0;
        let map_left = 20.0;
        let line_height = 18.0;

        draw_ascii_map(&game, map_left, map_top, line_height);
        draw_event_log(&game, 430.0, map_top, line_height);

        let status = match app_state.mode {
            AppMode::PendingPrompt { ref interrupt, .. } => prompt_text(interrupt),
            AppMode::Finished(outcome) => format!("Finished ({:?})", outcome),
            AppMode::AutoPlay => "Auto-Explore ON (Space to pause)".to_string(),
            AppMode::Paused => "Paused (Space to Auto-Explore, Right to step)".to_string(),
        };
        draw_text(&status, 20.0, 30.0, 20.0, WHITE);
        draw_text(&format!("Tick: {}", game.current_tick()), 20.0, 350.0, 20.0, WHITE);
        draw_text(&format!("Seed: {run_seed}"), 20.0, 330.0, 20.0, WHITE);
        draw_text(&format!("Floor: {} / 3", game.state().floor_index), 20.0, 310.0, 20.0, WHITE);
        draw_text(&format!("Branch: {:?}", game.state().branch_profile), 20.0, 290.0, 20.0, WHITE);

        let intent_text = if let Some(intent) = game.state().auto_intent {
            format!(
                "Intent: {:?} target=({}, {}) path_len={}",
                intent.reason, intent.target.x, intent.target.y, intent.path_len
            )
        } else {
            "Intent: none".to_string()
        };
        draw_text(&intent_text, 20.0, 380.0, 20.0, WHITE);

        let policy = &game.state().policy;
        draw_text("Policy: ", 20.0, 420.0, 20.0, YELLOW);
        draw_text(&format!("[M]ode: {:?}", policy.fight_or_avoid), 20.0, 440.0, 18.0, LIGHTGRAY);
        draw_text(&format!("s[T]ance: {:?}", policy.stance), 20.0, 460.0, 18.0, LIGHTGRAY);
        draw_text(
            &format!("[P]riority: {:?}", policy.target_priority),
            20.0,
            480.0,
            18.0,
            LIGHTGRAY,
        );
        draw_text(
            &format!("[R]etreat HP: {}%", policy.retreat_hp_threshold),
            20.0,
            500.0,
            18.0,
            LIGHTGRAY,
        );
        draw_text(
            &format!("[H]eal: {:?}", policy.auto_heal_if_below_threshold),
            20.0,
            520.0,
            18.0,
            LIGHTGRAY,
        );
        draw_text(&format!("[I]ntent: {:?}", policy.position_intent), 20.0, 540.0, 18.0, LIGHTGRAY);
        draw_text(
            &format!("[E]xplore: {:?}", policy.exploration_mode),
            20.0,
            560.0,
            18.0,
            LIGHTGRAY,
        );
        draw_text(
            &format!("[G]reed: {:?}", policy.resource_aggression),
            20.0,
            580.0,
            18.0,
            LIGHTGRAY,
        );

        draw_text("Threat Trace:", 240.0, 420.0, 20.0, RED);
        for (i, trace) in game.state().threat_trace.iter().take(5).enumerate() {
            let desc = format!(
                "T{}: {} vis, dist {:?}",
                trace.tick, trace.visible_enemy_count, trace.min_enemy_distance
            );
            let color = if trace.retreat_triggered { ORANGE } else { LIGHTGRAY };
            draw_text(&desc, 240.0, 440.0 + (i as f32 * 20.0), 18.0, color);
        }

        next_frame().await
    }
}

fn draw_ascii_map(game: &Game, left: f32, top: f32, line_height: f32) {
    let state = game.state();
    let map = &state.map;
    for y in 0..map.internal_height {
        for x in 0..map.internal_width {
            let pos = Pos { x: x as i32, y: y as i32 };
            if !map.is_discovered(pos) {
                draw_text(
                    " ",
                    left + x as f32 * 10.0,
                    top + y as f32 * line_height,
                    22.0,
                    LIGHTGRAY,
                );
                continue;
            }

            let mut glyph = match map.tile_at(pos) {
                TileKind::Wall => "#",
                TileKind::Floor => ".",
                TileKind::ClosedDoor => "+",
                TileKind::DownStairs => ">",
            };

            let color = if map.is_visible(pos) { WHITE } else { GRAY };

            let mut final_color = color;

            for (_, item) in &state.items {
                if item.pos == pos && map.is_visible(pos) {
                    glyph = "!";
                    final_color = YELLOW;
                    break;
                }
            }

            for (_, actor) in &state.actors {
                if actor.pos == pos && map.is_visible(pos) {
                    glyph = match actor.kind {
                        core::ActorKind::Player => "@",
                        core::ActorKind::Goblin => "g",
                        core::ActorKind::FeralHound => "h",
                        core::ActorKind::BloodAcolyte => "a",
                        core::ActorKind::CorruptedGuard => "C",
                        core::ActorKind::LivingArmor => "A",
                        core::ActorKind::Gargoyle => "G",
                        core::ActorKind::ShadowStalker => "S",
                        core::ActorKind::AbyssalWarden => "W",
                    };
                    final_color = match actor.kind {
                        core::ActorKind::Player => GREEN,
                        core::ActorKind::Goblin => RED,
                        core::ActorKind::FeralHound => ORANGE,
                        core::ActorKind::BloodAcolyte => RED,
                        core::ActorKind::CorruptedGuard => BLUE,
                        core::ActorKind::LivingArmor => LIGHTGRAY,
                        core::ActorKind::Gargoyle => GRAY,
                        core::ActorKind::ShadowStalker => PURPLE,
                        core::ActorKind::AbyssalWarden => MAGENTA,
                    };
                    break;
                }
            }

            draw_text(
                glyph,
                left + x as f32 * 11.0,
                top + y as f32 * line_height,
                22.0,
                final_color,
            );
        }
    }
}

fn draw_event_log(game: &Game, left: f32, top: f32, line_height: f32) {
    draw_text("Event log", left, top, 24.0, YELLOW);
    let events = game.log();
    let start = events.len().saturating_sub(10);
    for (idx, event) in events[start..].iter().enumerate() {
        let line = match event {
            LogEvent::AutoReasonChanged { reason, .. } => auto_reason_text(*reason).to_string(),
            LogEvent::EnemyEncountered { enemy } => format!("enemy encountered {:?}", enemy),
            LogEvent::ItemPickedUp { kind: _ } => "picked up item".to_string(),
            LogEvent::ItemDiscarded { kind: _ } => "discarded item".to_string(),
            LogEvent::EncounterResolved { enemy, fought } => {
                format!("encounter {:?} resolved fought={}", enemy, fought)
            }
        };
        draw_text(&line, left, top + ((idx + 1) as f32 * line_height), 18.0, LIGHTGRAY);
    }
}

fn prompt_text(interrupt: &Interrupt) -> String {
    match interrupt {
        Interrupt::LootFound { .. } => "INTERRUPT: Loot found (L=keep, D=discard)".to_string(),
        Interrupt::EnemyEncounter { threat, .. } => {
            format!("INTERRUPT: Enemy sighted (F=fight, A=avoid) Tags: {:?}", threat.danger_tags)
        }
        Interrupt::DoorBlocked { .. } => "INTERRUPT: Door blocked (O=open)".to_string(),
        Interrupt::FloorTransition { next_floor, requires_branch_choice, .. } => {
            if *requires_branch_choice {
                "INTERRUPT: Stairs reached — choose branch (A=enemies, B=hazards)".to_string()
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

fn auto_reason_text(reason: core::AutoReason) -> &'static str {
    match reason {
        core::AutoReason::Frontier => "Exploring the unknown...",
        core::AutoReason::Loot => "Moving to collect loot...",
        core::AutoReason::ThreatAvoidance => "Pathing around threats...",
        core::AutoReason::Stuck => "Auto-explore is stuck.",
        core::AutoReason::Door => "Moving to open a door...",
    }
}
