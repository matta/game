use core::{AdvanceStopReason, ContentPack, Game, GameMode, TileKind};
use macroquad::prelude::*;

enum AppMode {
    Paused,
    AutoPlay,
    PendingPrompt {
        prompt_id: core::ChoicePromptId,
        auto_play_suspended: bool,
    },
    Finished,
}

#[macroquad::main("Roguelike")]
async fn main() {
    let content = ContentPack {};
    let mut game = Game::new(12345, &content, GameMode::Ironman);

    let mut mode = AppMode::Paused;

    loop {
        clear_background(BLACK);

        let mut advance_result = None;

        // Input handling
        match &mode {
            AppMode::Paused | AppMode::AutoPlay => {
                if is_key_pressed(KeyCode::Space) {
                    mode = match mode {
                        AppMode::Paused => AppMode::AutoPlay,
                        AppMode::AutoPlay => {
                            game.request_pause();
                            AppMode::Paused
                        }
                        _ => mode,
                    };
                }

                if is_key_pressed(KeyCode::Right) && matches!(mode, AppMode::Paused) {
                    advance_result = Some(game.advance(1));
                }
            }
            AppMode::PendingPrompt { prompt_id, auto_play_suspended } => {
                let id = *prompt_id;
                let resume = *auto_play_suspended;
                
                if is_key_pressed(KeyCode::L) {
                    game.apply_choice(id, core::Choice::KeepLoot)
                        .expect("Failed to apply pending choice");
                        
                    mode = if resume {
                        AppMode::AutoPlay
                    } else {
                        AppMode::Paused
                    };
                }
            }
            AppMode::Finished => {
                // No inputs valid after completion
            }
        }

        // Logic stepping
        if matches!(mode, AppMode::AutoPlay) {
            advance_result = Some(game.advance(10)); // Batch step for synchronous iteration
        }

        if let Some(result) = advance_result {
            match result.stop_reason {
                AdvanceStopReason::PausedAtBoundary { .. } => {
                    mode = AppMode::Paused;
                }
                AdvanceStopReason::Interrupted(core::Interrupt::LootFound(prompt_id)) => {
                    mode = AppMode::PendingPrompt {
                        prompt_id,
                        auto_play_suspended: matches!(mode, AppMode::AutoPlay),
                    };
                }
                AdvanceStopReason::Finished(_) => {
                    mode = AppMode::Finished;
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

        let status = match mode {
            AppMode::PendingPrompt { .. } => "INTERRUPT: Loot Found! (Press 'L' to Keep & Resume)",
            AppMode::Finished => "Finished (Victory!)",
            AppMode::AutoPlay => "Auto-Explore ON (Space to pause)",
            AppMode::Paused => "Paused (Space to Auto-Explore, Right to step)",
        };
        draw_text(status, 20.0, 30.0, 20.0, WHITE);
        draw_text(&format!("Tick: {}", game.current_tick()), 20.0, 400.0, 20.0, WHITE);

        next_frame().await
    }
}
