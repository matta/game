//! Tests for engine safeguards that prevent incorrect run termination behavior.

use super::support::*;

#[test]
fn run_does_not_end_only_because_tick_counter_grew() {
    let mut game = Game::new(55555, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);
    game.tick = 401;

    let result = game.advance(1);
    assert!(
        !matches!(result.stop_reason, AdvanceStopReason::Finished(_)),
        "run should not auto-finish from tick count"
    );
}

#[test]
fn no_progress_simulation_finishes_instead_of_spinning_budget_forever() {
    let mut game = Game::new(66666, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);

    let mut map = Map::new(7, 7);
    for y in 1..6 {
        for x in 1..6 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    let isolated = Pos { y: 3, x: 3 };
    map.set_tile(isolated, TileKind::Floor);
    map.discovered.fill(true);
    map.visible.fill(true);
    game.state.map = map;
    game.state.actors[game.state.player_id].pos = isolated;
    game.state.auto_intent = None;

    let result = game.advance(200);
    assert_eq!(
        result.simulated_ticks, MAX_NO_PROGRESS_TICKS,
        "stall watchdog should terminate within a fixed tick budget"
    );
    assert!(matches!(
        result.stop_reason,
        AdvanceStopReason::EngineFailure(crate::EngineFailureReason::StalledNoProgress)
    ));
}
