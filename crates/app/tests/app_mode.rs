use app::app_loop::{AppMode, AppState};
use core::{ContentPack, GameMode, Game};
use macroquad::prelude::KeyCode;

#[test]
fn test_manual_stepping_preserves_suspended_state() {
    let content = ContentPack {};
    let mut game = Game::new(12345, &content, GameMode::Ironman);
    let mut app = AppState::new();

    // Ensure we start paused
    assert_eq!(app.mode, AppMode::Paused);

    // Press right to advance. Depending on the mock/game behavior, we might hit loot immediately.
    app.tick(&mut game, &[KeyCode::Right]);

    // If we hit a prompt, auto_play_suspended MUST be false since we were paused
    if let AppMode::PendingPrompt { auto_play_suspended, .. } = app.mode {
        assert!(!auto_play_suspended);
    }
}

#[test]
fn test_autoplay_sets_suspended_state() {
    let content = ContentPack {};
    let mut game = Game::new(12345, &content, GameMode::Ironman);
    let mut app = AppState::new();

    // Start auto-play
    app.tick(&mut game, &[KeyCode::Space]);
    assert_eq!(app.mode, AppMode::AutoPlay);

    // Let it run. It should hit loot eventually.
    for _ in 0..100 {
        app.tick(&mut game, &[]);
        if let AppMode::PendingPrompt { auto_play_suspended, .. } = app.mode {
            assert!(auto_play_suspended);
            return;
        }
    }
    panic!("Did not encounter pending prompt during autoplay");
}
