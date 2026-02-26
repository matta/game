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
        for key in [KeyCode::L, KeyCode::D, KeyCode::F, KeyCode::A] {
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

        next_frame().await
    }
}

fn draw_ascii_map(game: &Game, left: f32, top: f32, line_height: f32) {
    let state = game.state();
    let map = &state.map;
    for y in 0..map.internal_height {
        let mut row = String::with_capacity(map.internal_width);
        for x in 0..map.internal_width {
            let pos = Pos { x: x as i32, y: y as i32 };
            let mut ch = if !map.is_discovered(pos) {
                ' '
            } else {
                match map.tile_at(pos) {
                    TileKind::Wall => '#',
                    TileKind::Floor => '.',
                }
            };

            for (_, item) in &state.items {
                if item.pos == pos {
                    ch = '!';
                    break;
                }
            }

            for (_, actor) in &state.actors {
                if actor.pos == pos {
                    ch = match actor.kind {
                        core::ActorKind::Player => '@',
                        core::ActorKind::Goblin => 'g',
                    };
                    break;
                }
            }
            row.push(ch);
        }
        draw_text(&row, left, top + y as f32 * line_height, 22.0, LIGHTGRAY);
    }
}

fn draw_event_log(game: &Game, left: f32, top: f32, line_height: f32) {
    draw_text("Event log", left, top, 24.0, YELLOW);
    let events = game.log();
    let start = events.len().saturating_sub(10);
    for (idx, event) in events[start..].iter().enumerate() {
        let line = match event {
            LogEvent::AutoReasonChanged { reason, target, path_len } => {
                format!("auto {:?} -> ({}, {}) len={}", reason, target.x, target.y, path_len)
            }
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
    }
}
