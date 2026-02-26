use core::{AdvanceStopReason, ContentPack, Game, GameMode, TileKind};
use macroquad::prelude::*;

#[macroquad::main("Roguelike")]
async fn main() {
    let content = ContentPack {};
    let mut game = Game::new(12345, &content, GameMode::Ironman);

    let mut auto_play = false;

    loop {
        clear_background(BLACK);

        // Input handling
        if is_key_pressed(KeyCode::Space) {
            auto_play = !auto_play;
            if !auto_play {
                game.request_pause();
            }
        }

        if is_key_pressed(KeyCode::Right) && !auto_play {
            game.advance(1);
        }

        // Logic stepping
        if auto_play {
            let result = game.advance(10); // Batch step for synchronous iteration
            match result.stop_reason {
                AdvanceStopReason::PausedAtBoundary { .. } => {
                    auto_play = false;
                }
                AdvanceStopReason::Interrupted(_) => {
                    auto_play = false;
                }
                AdvanceStopReason::Finished(_) => {
                    auto_play = false;
                }
                AdvanceStopReason::BudgetExhausted => {
                    // Continuing auto play on next frame
                }
            }
        }

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

        let status = if auto_play {
            "Auto-Explore ON (Space to pause)"
        } else {
            "Paused (Space to Auto-Explore, Right to step)"
        };
        draw_text(status, 20.0, 30.0, 20.0, WHITE);
        draw_text(&format!("Tick: {}", game.current_tick()), 20.0, 400.0, 20.0, WHITE);

        next_frame().await
    }
}
