use core::{ContentPack, Game, GameMode, TileKind};
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
        if is_key_pressed(KeyCode::L) {
            keys_pressed.push(KeyCode::L);
        }

        app_state.tick(&mut game, &keys_pressed);

        let state = game.state();
        let map = &state.map;
        let tile_size = 20.0;
        let pad_x = 20.0;
        let pad_y = 50.0;

        for y in 0..map.internal_height {
            for x in 0..map.internal_width {
                let kind = map.tile_at(core::Pos { x: x as i32, y: y as i32 });
                let color = match kind {
                    TileKind::Wall => DARKGRAY,
                    TileKind::Floor => LIGHTGRAY,
                };
                draw_rectangle(
                    pad_x + x as f32 * tile_size,
                    pad_y + y as f32 * tile_size,
                    tile_size - 1.0,
                    tile_size - 1.0,
                    color,
                );
            }
        }

        for (_id, actor) in &state.actors {
            let color = match actor.kind {
                core::ActorKind::Player => GREEN,
                core::ActorKind::Goblin => RED,
            };
            draw_rectangle(
                pad_x + actor.pos.x as f32 * tile_size,
                pad_y + actor.pos.y as f32 * tile_size,
                tile_size - 1.0,
                tile_size - 1.0,
                color,
            );
        }

        let status = match app_state.mode {
            app::app_loop::AppMode::PendingPrompt { .. } => "INTERRUPT: Loot Found! (Press 'L' to Keep & Resume)",
            app::app_loop::AppMode::Finished => "Finished (Victory!)",
            app::app_loop::AppMode::AutoPlay => "Auto-Explore ON (Space to pause)",
            app::app_loop::AppMode::Paused => "Paused (Space to Auto-Explore, Right to step)",
        };
        draw_text(status, 20.0, 30.0, 20.0, WHITE);
        draw_text(&format!("Tick: {}", game.current_tick()), 20.0, 400.0, 20.0, WHITE);

        next_frame().await
    }
}
