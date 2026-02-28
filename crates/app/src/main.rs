//! Macroquad binary entrypoint that wires input, simulation, persistence, and rendering.

mod frame_input;
mod game_layout;
mod ui_render;
mod ui_text;

use app::{
    app_loop::AppState,
    format_snapshot_hash, get_current_unix_ms,
    run_state_file::RunStateFile,
    seed::{generate_runtime_seed, resolve_seed_from_args},
};
use core::{ContentPack, Game, GameMode, LogEvent};
use frame_input::capture_frame_input;
use game_layout::{compute_frame_layout, setup_layout};
use macroquad::prelude::*;
use macroquad::window::Conf;
use std::{env, path::PathBuf, process::exit};
use taffy::TaffyTree;
use ui_render::draw_frame;

fn window_conf() -> Conf {
    Conf {
        window_title: "Roguelike".to_owned(),
        window_width: 1000,
        window_height: 750,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
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

    let diagnostics_path = RunStateFile::get_default_path();
    let (recovered_seed, recovery_hint) = load_recovery_hint(&diagnostics_path);

    let content = ContentPack::default();
    let mut current_run_seed = selected_seed.value();
    let mut game = Game::new(current_run_seed, &content, GameMode::Ironman);

    if let Some(path) = &diagnostics_path {
        game.push_log(LogEvent::Notice(format!("Logs: {}", path.display())));
    }
    if let Some(hint) = recovery_hint {
        game.push_log(hint);
        game.push_log(LogEvent::Notice("Press Shift+K to restart with this seed".to_string()));
    }

    let mut app_state = AppState::default();

    let mut taffy: TaffyTree<()> = TaffyTree::new();
    let layout_nodes = setup_layout(&mut taffy);

    loop {
        clear_background(BLACK);

        let frame_input = capture_frame_input();
        if frame_input.restart_with_recovered_seed
            && let Some(seed) = recovered_seed
        {
            current_run_seed = seed;
            game = Game::new(current_run_seed, &content, GameMode::Ironman);
            app_state = AppState::default();
            game.push_log(LogEvent::Notice(format!("RESTARTED WITH SEED: {seed}")));
        }

        app_state.tick(&mut game, &frame_input.keys_pressed);
        persist_run_state(&diagnostics_path, &game);

        let frame_layout =
            compute_frame_layout(&mut taffy, &layout_nodes, screen_width(), screen_height());
        draw_frame(&game, &app_state, current_run_seed, &frame_layout);

        next_frame().await
    }
}

fn load_recovery_hint(diagnostics_path: &Option<PathBuf>) -> (Option<u64>, Option<LogEvent>) {
    if let Some(state) = diagnostics_path.as_ref().and_then(|path| RunStateFile::load(path).ok()) {
        return (
            Some(state.run_seed),
            Some(LogEvent::RecoveryHint {
                seed: state.run_seed,
                hash_hex: state.snapshot_hash_hex,
            }),
        );
    }

    (None, None)
}

fn persist_run_state(diagnostics_path: &Option<PathBuf>, game: &Game) {
    let Some(path) = diagnostics_path else {
        return;
    };

    let state = RunStateFile {
        format_version: 1,
        run_seed: game.seed(),
        snapshot_hash_hex: format_snapshot_hash(game.snapshot_hash()),
        tick: game.current_tick(),
        floor_index: game.state().floor_index,
        branch_profile: format!("{:?}", game.state().branch_profile),
        active_god: format!("{:?}", game.state().active_god),
        updated_at_unix_ms: get_current_unix_ms(),
    };

    let _ = state.write_atomic(path);
}
