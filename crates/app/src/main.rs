//! Macroquad binary entrypoint that wires input, simulation, persistence, and rendering.

mod frame_input;
mod game_layout;
mod ui_render;
mod ui_scale_file;
mod ui_text;
mod window_config;

use app::{
    APP_NAME,
    app_loop::AppState,
    format_snapshot_hash, get_current_unix_ms,
    run_state_file::RunStateFile,
    seed::{generate_runtime_seed, resolve_seed_from_args},
    ui_scale::clamp_ui_scale,
};
use core::{
    ContentPack, Game, GameMode, JournalWriter, LogEvent, load_journal_from_file,
    replay::replay_journal_inputs,
};
use frame_input::capture_frame_input;
use game_layout::{compute_frame_layout, setup_layout};
use macroquad::prelude::*;
use macroquad::window::Conf;
use std::{env, path::PathBuf, process::exit};
use taffy::TaffyTree;
use ui_render::draw_frame;
use ui_scale_file::UiScaleFile;
use window_config::{build_window_conf, display_scale_notice, runtime_ui_scale};

fn window_conf() -> Conf {
    build_window_conf()
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
    let journal_path = get_journal_path();
    let ui_scale_path = UiScaleFile::get_default_path();
    let (recovered_seed, recovery_hint) = load_recovery_hint(&diagnostics_path);
    let persisted_ui_scale = load_persisted_ui_scale(&ui_scale_path);

    let content = ContentPack::default();
    let mut current_run_seed = selected_seed.value();
    let mut game = Game::new(current_run_seed, &content, GameMode::Ironman);
    let (mut journal_writer, preserved_existing_journal) =
        prepare_startup_journal_writer(&journal_path, current_run_seed);

    if let Some(path) = &diagnostics_path {
        game.push_log(LogEvent::Notice(format!("Logs: {}", path.display())));
    }
    if let Some(path) = &journal_path {
        game.push_log(LogEvent::Notice(format!("Journal: {}", path.display())));
    }
    if preserved_existing_journal {
        game.push_log(LogEvent::Notice(
            "Existing journal preserved. Press Shift+K to replay, or play to start a new journal."
                .to_string(),
        ));
    }
    if let Some(hint) = recovery_hint {
        game.push_log(hint);
        game.push_log(LogEvent::Notice("Press Shift+K to replay from last journal".to_string()));
    }
    game.push_log(LogEvent::Notice(display_scale_notice(persisted_ui_scale)));
    game.push_log(LogEvent::Notice(
        "UI scale hotkeys: Ctrl+= larger, Ctrl+- smaller, Ctrl+0 reset".to_string(),
    ));

    let mut app_state =
        AppState { ui_scale: runtime_ui_scale(persisted_ui_scale), ..AppState::default() };

    let mut taffy: TaffyTree<()> = TaffyTree::new();
    let layout_nodes = setup_layout(&mut taffy);

    loop {
        clear_background(BLACK);

        let frame_input = capture_frame_input();
        if frame_input.restart_with_recovered_seed
            && let Some(seed) = recovered_seed
        {
            match try_replay_from_journal(&journal_path, &content) {
                Ok(replayed_game) => {
                    current_run_seed = seed;
                    game = replayed_game;
                    app_state = AppState { ui_scale: app_state.ui_scale, ..AppState::default() };
                    game.push_log(LogEvent::Notice(format!(
                        "REPLAYED journal for seed {seed} — tick {}",
                        game.current_tick()
                    )));
                    // Resume appending to the same journal file
                    journal_writer = resume_journal_writer(&journal_path);
                }
                Err(reason) => {
                    current_run_seed = seed;
                    game = Game::new(current_run_seed, &content, GameMode::Ironman);
                    app_state = AppState { ui_scale: app_state.ui_scale, ..AppState::default() };
                    journal_writer = create_journal_writer(&journal_path, current_run_seed);
                    game.push_log(LogEvent::Notice(format!("REPLAY INCOMPLETE: {reason}")));
                    game.push_log(LogEvent::Notice(format!("RESTARTED WITH SEED: {seed}")));
                }
            }
        }

        if let Some(action) = frame_input.ui_scale_action
            && app_state.apply_ui_scale_action(action)
        {
            persist_ui_scale(&ui_scale_path, app_state.ui_scale);
            game.push_log(LogEvent::Notice(format!("UI scale set to {:.2}", app_state.ui_scale)));
        }

        app_state.tick(&mut game, &frame_input.keys_pressed);

        // Flush accepted inputs to the journal file
        if journal_writer.is_none() && !app_state.accepted_inputs.is_empty() {
            // We deferred writer creation to avoid truncating a previous run
            // before the user has a chance to replay it.
            journal_writer = create_journal_writer(&journal_path, current_run_seed);
        }
        if let Some(writer) = &mut journal_writer {
            for input in app_state.accepted_inputs.drain(..) {
                if writer.append(input.tick_boundary, &input.payload).is_err() {
                    game.push_log(LogEvent::Notice(
                        "Warning: failed to write journal entry".to_string(),
                    ));
                }
            }
        }

        persist_run_state(&diagnostics_path, &game);

        let frame_layout =
            compute_frame_layout(&mut taffy, &layout_nodes, screen_width(), screen_height());
        draw_frame(&game, &app_state, current_run_seed, &frame_layout, app_state.ui_scale);

        next_frame().await
    }
}

fn load_persisted_ui_scale(path: &Option<PathBuf>) -> Option<f32> {
    let path = path.as_ref()?;
    UiScaleFile::load(path).ok().map(|state| clamp_ui_scale(state.ui_scale))
}

fn persist_ui_scale(path: &Option<PathBuf>, ui_scale: f32) {
    let Some(path) = path.as_ref() else {
        return;
    };
    let state = UiScaleFile { format_version: 1, ui_scale: clamp_ui_scale(ui_scale) };
    if let Err(error) = state.write_atomic(path) {
        eprintln!("Warning: failed to persist UI scale: {error}");
    }
}

// ---------------------------------------------------------------------------
// Journal helpers
// ---------------------------------------------------------------------------

/// OS-idiomatic path for the journal file, alongside the diagnostics file.
fn get_journal_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", APP_NAME).map(|proj_dirs| {
        let mut path = proj_dirs.data_dir().to_path_buf();
        path.push("journal.jsonl");
        path
    })
}

/// Create a fresh journal file for a new run.
fn create_journal_writer(path: &Option<PathBuf>, seed: u64) -> Option<JournalWriter> {
    let path = path.as_ref()?;
    match JournalWriter::create(path, seed, "dev", 0) {
        Ok(writer) => Some(writer),
        Err(e) => {
            eprintln!("Warning: could not create journal file: {e}");
            None
        }
    }
}

/// On startup, preserve an existing non-empty journal so Shift+K replay can
/// consume it without being truncated by a new run initialization.
fn prepare_startup_journal_writer(
    path: &Option<PathBuf>,
    seed: u64,
) -> (Option<JournalWriter>, bool) {
    let Some(path_ref) = path.as_ref() else {
        return (None, false);
    };

    if let Ok(loaded) = load_journal_from_file(path_ref)
        && !loaded.journal.inputs.is_empty()
    {
        return (None, true);
    }

    (create_journal_writer(path, seed), false)
}

/// Resume appending to an existing journal after replay.
fn resume_journal_writer(path: &Option<PathBuf>) -> Option<JournalWriter> {
    let path = path.as_ref()?;
    let loaded = load_journal_from_file(path).ok()?;
    match JournalWriter::resume(path, loaded.last_sha256_hex, loaded.next_seq) {
        Ok(writer) => Some(writer),
        Err(e) => {
            eprintln!("Warning: could not resume journal file: {e}");
            None
        }
    }
}

/// Try to load and replay a journal file. Returns the reconstructed Game
/// on success, or an explanatory error if replay is incomplete.
fn try_replay_from_journal(
    journal_path: &Option<PathBuf>,
    content: &ContentPack,
) -> Result<Game, String> {
    let path = journal_path.as_ref().ok_or_else(|| "journal path is unavailable".to_string())?;
    let loaded = match load_journal_from_file(path) {
        Ok(loaded) => loaded,
        Err(e) => {
            return Err(format!("{e}"));
        }
    };
    if loaded.journal.inputs.is_empty() {
        return Err("journal has no recorded inputs".to_string());
    }
    match replay_journal_inputs(content, &loaded.journal) {
        Ok(game) => Ok(game),
        Err(e) => Err(format!("{e}")),
    }
}

// ---------------------------------------------------------------------------
// Diagnostics persistence
// ---------------------------------------------------------------------------

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
