//! Tests for auto-intent planning, movement side effects, and reason-change logging.

use super::support::*;

#[test]
fn advance_uses_hazard_path_for_threat_avoidance_intent() {
    let mut game = Game::new(123, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);

    let mut map = Map::new(11, 9);
    for y in 1..8 {
        for x in 1..10 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 2..=8 {
        map.set_tile(Pos { y: 4, x }, TileKind::Floor);
    }
    map.discovered.fill(true);
    map.visible.fill(true);
    map.set_hazard(Pos { y: 4, x: 4 }, true);
    map.set_hazard(Pos { y: 4, x: 6 }, true);
    map.discovered[(4 * map.internal_width) + 1] = false;
    map.discovered[(4 * map.internal_width) + 9] = false;
    game.state.map = map;

    let start = Pos { y: 4, x: 5 };
    game.state.actors[game.state.player_id].pos = start;

    let result = game.advance(1);
    assert!(matches!(result.stop_reason, AdvanceStopReason::BudgetExhausted));
    assert_eq!(game.state.actors[game.state.player_id].pos, Pos { y: 4, x: 4 });
    assert_eq!(
        game.state.auto_intent.map(|intent| intent.reason),
        Some(AutoReason::ThreatAvoidance)
    );
}

#[test]
fn movement_updates_visibility_and_expands_discovery() {
    let mut game = Game::new(123, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);

    let mut map = Map::new(30, 10);
    for y in 1..9 {
        for x in 1..29 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 1..26 {
        map.set_tile(Pos { y: 4, x }, TileKind::Floor);
    }
    map.discovered.fill(false);
    game.state.map = map;

    let start = Pos { y: 4, x: 5 };
    game.state.actors[game.state.player_id].pos = start;
    compute_fov(&mut game.state.map, start, FOV_RADIUS);
    // Create visible frontier at (4,15) by leaving (4,16) unknown.
    game.state.map.discovered[(4 * game.state.map.internal_width) + 16] = false;
    let discovered_before = game.state.map.discovered.iter().filter(|&&known| known).count();

    let result = game.advance(1);
    assert!(matches!(result.stop_reason, AdvanceStopReason::BudgetExhausted));
    let moved_to = game.state.actors[game.state.player_id].pos;
    assert_eq!(manhattan(start, moved_to), 1, "player should move exactly one tile");
    let discovered_after = game.state.map.discovered.iter().filter(|&&known| known).count();
    assert!(
        discovered_after > discovered_before,
        "moving with FOV recompute should discover at least one new tile"
    );
}

#[test]
fn unchanged_intent_does_not_duplicate_reason_change_log() {
    let mut game = Game::new(123, &ContentPack::default(), GameMode::Ironman);
    if let AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) =
        game.advance(1).stop_reason
    {
        game.apply_choice(prompt_id, Choice::KeepLoot).unwrap();
    }
    let pos = game.state.actors[game.state.player_id].pos;
    compute_fov(&mut game.state.map, pos, FOV_RADIUS);
    game.plan_auto_intent(pos);
    let count_after_first = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_first, 1);
    game.plan_auto_intent(pos);
    let count_after_second = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_second, 1);
}

#[test]
fn path_len_only_change_does_not_emit_auto_reason_changed() {
    let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);
    let mut map = Map::new(12, 7);
    for y in 1..(map.internal_height - 1) {
        for x in 1..(map.internal_width - 1) {
            map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
        }
    }
    for x in 1..=9 {
        map.set_tile(Pos { y: 3, x }, TileKind::Floor);
    }
    map.discovered.fill(true);
    map.visible.fill(false);
    for x in 1..=8 {
        map.set_visible(Pos { y: 3, x }, true);
    }
    map.discovered[(3 * map.internal_width) + 9] = false;

    game.state.map = map;
    let first_pos = Pos { y: 3, x: 3 };
    game.state.actors[game.state.player_id].pos = first_pos;
    game.plan_auto_intent(first_pos);
    let previous_intent = game.state.auto_intent.unwrap_or_else(|| {
        panic!("No first intent! Map:\n{}", draw_map_diag(&game.state.map, first_pos));
    });
    assert_eq!(previous_intent.target, Pos { y: 3, x: 8 });

    let second_pos = Pos { y: 3, x: 4 };
    game.state.actors[game.state.player_id].pos = second_pos;
    game.plan_auto_intent(second_pos);
    let next_intent = game.state.auto_intent.unwrap();
    assert_eq!(previous_intent.target, next_intent.target);
    assert_ne!(previous_intent.path_len, next_intent.path_len);
    let count = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count, 1);
}

#[test]
fn auto_reason_changed_emits_only_on_reason_or_target_changes() {
    let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);
    let mut map = Map::new(10, 7);
    for y in 1..6 {
        for x in 1..9 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 1..=7 {
        map.set_tile(Pos { y: 3, x }, TileKind::Floor);
    }
    map.discovered.fill(true);
    map.visible.fill(true);
    map.discovered[(3 * map.internal_width) + 8] = false;
    game.state.map = map;

    let pos = Pos { y: 3, x: 2 };
    game.state.actors[game.state.player_id].pos = pos;
    game.plan_auto_intent(pos);
    let count_after_first = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_first, 1);

    // Same target, different reason due to hazard fallback.
    game.state.map.set_hazard(Pos { y: 3, x: 7 }, true);
    game.plan_auto_intent(pos);
    let count_after_reason_change = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_reason_change, 2);

    // No further reason/target change means no extra log.
    game.plan_auto_intent(pos);
    let count_after_repeat = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_repeat, 2);
}
