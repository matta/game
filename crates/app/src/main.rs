use core::{ContentPack, Game, GameMode, Interrupt, LogEvent, Pos, TileKind};
use macroquad::prelude::*;

#[macroquad::main("Roguelike")]
async fn main() {
    let content = ContentPack {};
    let mut game = Game::new(12345, &content, GameMode::Ironman);

    let mut app_state = app::app_loop::AppState::default();

    loop {
        clear_background(BLACK);

        let mut keys_pressed = vec![];
        if is_key_pressed(KeyCode::Space) {
            keys_pressed.push(KeyCode::Space);
        }
        if is_key_pressed(KeyCode::Right) {
            keys_pressed.push(KeyCode::Right);
        }
        for key in
            [KeyCode::L, KeyCode::D, KeyCode::F, KeyCode::A, KeyCode::O, KeyCode::M, KeyCode::T]
        {
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
            app::app_loop::AppMode::PendingPrompt { ref interrupt, .. } => prompt_text(interrupt),
            app::app_loop::AppMode::Finished => "Finished (Victory!)",
            app::app_loop::AppMode::AutoPlay => "Auto-Explore ON (Space to pause)",
            app::app_loop::AppMode::Paused => "Paused (Space to Auto-Explore, Right to step)",
        };
        draw_text(status, 20.0, 30.0, 20.0, WHITE);
        draw_text(&format!("Tick: {}", game.current_tick()), 20.0, 350.0, 20.0, WHITE);

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
                    };
                    final_color = if actor.kind == core::ActorKind::Player { GREEN } else { RED };
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
            LogEvent::ItemPickedUp => "picked up item".to_string(),
            LogEvent::ItemDiscarded => "discarded item".to_string(),
            LogEvent::EncounterResolved { enemy, fought } => {
                format!("encounter {:?} resolved fought={}", enemy, fought)
            }
        };
        draw_text(&line, left, top + ((idx + 1) as f32 * line_height), 18.0, LIGHTGRAY);
    }
}

fn prompt_text(interrupt: &Interrupt) -> &'static str {
    match interrupt {
        Interrupt::LootFound { .. } => "INTERRUPT: Loot found (L=keep, D=discard)",
        Interrupt::EnemyEncounter { .. } => "INTERRUPT: Enemy sighted (F=fight, A=avoid)",
        Interrupt::DoorBlocked { .. } => "INTERRUPT: Door blocked (O=open)",
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
