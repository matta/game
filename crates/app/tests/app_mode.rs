use app::app_loop::{AppMode, AppState};
use core::{ContentPack, Game, GameMode, Interrupt};
use macroquad::prelude::KeyCode;

#[test]
fn test_manual_stepping_preserves_suspended_state() {
    let content = ContentPack::default();
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
    let content = ContentPack::default();
    let mut game = Game::new(12345, &content, GameMode::Ironman);
    let mut app = AppState::new();

    // Start auto-play
    app.tick(&mut game, &[KeyCode::Space]);
    match app.mode {
        AppMode::AutoPlay => {}
        AppMode::PendingPrompt { auto_play_suspended, .. } => assert!(auto_play_suspended),
        _ => panic!("Expected autoplay or pending prompt immediately after enabling autoplay"),
    }

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

#[test]
fn test_auto_explore_interrupt_choice_and_resume_loop() {
    let content = ContentPack::default();
    let mut game = Game::new(12345, &content, GameMode::Ironman);
    let mut app = AppState::new();

    app.tick(&mut game, &[KeyCode::Space]);

    if let AppMode::PendingPrompt { interrupt, .. } = &app.mode {
        let key = match interrupt {
            Interrupt::LootFound { .. } => KeyCode::L,
            Interrupt::EnemyEncounter { .. } => KeyCode::F,
            Interrupt::DoorBlocked { .. } => KeyCode::O,
            Interrupt::FloorTransition { .. } => KeyCode::C,
        };
        app.tick(&mut game, &[key]);
    }

    assert!(
        matches!(app.mode, AppMode::AutoPlay | AppMode::PendingPrompt { .. }),
        "app should continue the loop after resolving a prompt"
    );
}

#[test]
fn test_app_branch_choice_navigation() {
    let content = ContentPack::default();
    let mut game = Game::new(12345, &content, GameMode::Ironman);
    let mut app = AppState::new();

    // Run until floor transition
    let mut reached_transition = false;
    for _ in 0..1000 {
        app.tick(&mut game, &[KeyCode::Right]); // manual step for precision
        if let AppMode::PendingPrompt { interrupt, .. } = &app.mode {
            if matches!(interrupt, Interrupt::FloorTransition { .. }) {
                reached_transition = true;
                break;
            }
            // Resolve other interrupts to keep going
            let key = match interrupt {
                Interrupt::LootFound { .. } => KeyCode::L,
                Interrupt::EnemyEncounter { .. } => KeyCode::F,
                Interrupt::DoorBlocked { .. } => KeyCode::O,
                _ => break,
            };
            app.tick(&mut game, &[key]);
        }
    }
    assert!(reached_transition, "Did not reach floor transition");

    // Select Branch B
    app.tick(&mut game, &[KeyCode::B]);

    // Verify branch is committed in game state
    assert_eq!(game.state().branch_profile, core::BranchProfile::BranchB);
    assert_eq!(game.state().floor_index, 2);
}

#[test]
fn test_regression_no_ascend_bindings() {
    let content = ContentPack::default();
    let mut game = Game::new(12345, &content, GameMode::Ironman);
    let mut app = AppState::new();

    // Floor 1
    assert_eq!(game.state().floor_index, 1);

    // Try various keys that might be "ascend" in other games (U for Up, etc.)
    for key in [KeyCode::U, KeyCode::PageUp, KeyCode::W] {
        app.tick(&mut game, &[key]);
        assert_eq!(game.state().floor_index, 1, "Floor index should not change on key {:?}", key);
    }
}
