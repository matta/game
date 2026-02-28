//! Integration and regression tests for auto-explore planner stateful behavior.

use super::*;

#[test]
fn threat_avoidance_intent_is_reused_without_retarget_replan() {
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

    let p1 = Pos { y: 4, x: 5 };
    game.state.actors[game.state.player_id].pos = p1;
    game.plan_auto_intent(p1);
    let first_intent = game.state.auto_intent.expect("first intent");
    assert_eq!(first_intent.reason, AutoReason::ThreatAvoidance);
    assert_eq!(first_intent.target, Pos { y: 4, x: 2 });
    let first_log_count = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(first_log_count, 1);

    let p2 = Pos { y: 4, x: 6 };
    game.state.actors[game.state.player_id].pos = p2;
    game.plan_auto_intent(p2);
    let second_intent = game.state.auto_intent.expect("second intent");
    assert_eq!(second_intent.reason, AutoReason::ThreatAvoidance);
    assert_eq!(second_intent.target, Pos { y: 4, x: 2 });
    let second_log_count = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(second_log_count, 1);
}

#[test]
fn auto_explore_frontier_regression() {
    let (map, player_pos) = open_room_fixture();
    let mut map = map;
    map.discovered.fill(true);
    map.discovered[3 * map.internal_width + 5] = false;
    let intent = choose_frontier_intent(&map, player_pos).expect("frontier should be found");
    assert_eq!(intent.target, Pos { y: 4, x: 5 });
    assert_eq!(intent.path_len, 1);

    let mut map = Map::new(10, 10);
    for y in 0..10 {
        for x in 0..10 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 1..=8 {
        map.set_tile(Pos { y: 1, x }, TileKind::Floor);
    }
    for y in 2..=8 {
        map.set_tile(Pos { y, x: 8 }, TileKind::Floor);
    }
    for x in 1..=7 {
        map.set_tile(Pos { y: 8, x }, TileKind::Floor);
    }
    map.discovered.fill(true);
    map.discovered[9 * 10 + 1] = false;
    let start = Pos { y: 1, x: 1 };
    let intent = choose_frontier_intent(&map, start).expect("frontier should be found in maze");
    assert_eq!(intent.target, Pos { y: 8, x: 1 });
    assert_eq!(intent.path_len, 21);

    let (mut map, start) = hazard_lane_fixture();
    map.discovered[4 * map.internal_width + 6] = false;
    map.set_hazard(Pos { y: 4, x: 4 }, true);
    let intent = choose_frontier_intent(&map, start).expect("hazard fallback should work");
    assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
    assert_eq!(intent.target, Pos { y: 4, x: 5 });

    let (map, start, door) = closed_door_choke_fixture();
    let intent = choose_frontier_intent(&map, start).expect("door frontier should be found");
    assert_eq!(intent.target, door);
    assert_eq!(intent.reason, AutoReason::Door);
}
