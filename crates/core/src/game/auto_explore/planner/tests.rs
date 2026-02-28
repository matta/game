//! Tests for auto-explore planner behavior and hazard fallback rules.

use super::*;
use crate::content::ContentPack;
use crate::game::test_support::*;
use crate::state::Map;
use crate::*;

#[test]
fn planner_targets_known_downstairs_when_no_frontier_remains() {
    let mut map = Map::new(12, 8);
    for y in 1..(map.internal_height - 1) {
        for x in 1..(map.internal_width - 1) {
            map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
        }
    }
    for x in 2..=9 {
        map.set_tile(Pos { y: 4, x }, TileKind::Floor);
    }
    let stairs = Pos { y: 4, x: 9 };
    map.set_tile(stairs, TileKind::DownStairs);
    map.discovered.fill(true);
    map.visible.fill(true);

    let start = Pos { y: 4, x: 3 };
    let intent = choose_frontier_intent(&map, start).expect("stairs should be selected");
    assert_eq!(intent.target, stairs);
}

#[test]
fn downstairs_prefers_nearest_then_y_x_tie_break() {
    let mut map = Map::new(12, 8);
    for y in 1..(map.internal_height - 1) {
        for x in 1..(map.internal_width - 1) {
            map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
        }
    }
    for x in 2..=9 {
        map.set_tile(Pos { y: 4, x }, TileKind::Floor);
    }
    let near_stairs = Pos { y: 4, x: 4 };
    let far_stairs = Pos { y: 4, x: 8 };
    map.set_tile(near_stairs, TileKind::DownStairs);
    map.set_tile(far_stairs, TileKind::DownStairs);
    map.discovered.fill(true);
    map.visible.fill(true);

    let start = Pos { y: 4, x: 2 };
    let nearest_intent = choose_downstairs_intent(&map, start).expect("stairs should be found");
    assert_eq!(nearest_intent.target, near_stairs);
    assert_eq!(nearest_intent.path_len, 2);
    assert_eq!(nearest_intent.reason, AutoReason::Frontier);

    let mut tie_map = Map::new(11, 11);
    for y in 1..10 {
        for x in 1..10 {
            tie_map.set_tile(Pos { y, x }, TileKind::Floor);
        }
    }
    let top_stairs = Pos { y: 3, x: 5 };
    let bottom_stairs = Pos { y: 5, x: 3 };
    tie_map.set_tile(top_stairs, TileKind::DownStairs);
    tie_map.set_tile(bottom_stairs, TileKind::DownStairs);
    tie_map.discovered.fill(true);
    tie_map.visible.fill(true);

    let tie_start = Pos { y: 4, x: 4 };
    let tie_intent =
        choose_downstairs_intent(&tie_map, tie_start).expect("tied stairs should be found");
    assert_eq!(tie_intent.target, top_stairs);
    assert_eq!(tie_intent.path_len, 2);
    assert_eq!(tie_intent.reason, AutoReason::Frontier);
}

#[test]
fn downstairs_hazard_fallback_reports_threat_avoidance() {
    let (mut map, start) = hazard_lane_fixture();
    let stairs = Pos { y: 4, x: 5 };
    map.set_tile(stairs, TileKind::DownStairs);
    map.set_hazard(Pos { y: 4, x: 4 }, true);

    let intent = choose_downstairs_intent(&map, start).expect("hazard fallback intent");
    assert_eq!(intent.target, stairs);
    assert_eq!(intent.path_len, 3);
    assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
}

#[test]
fn planner_avoids_hazard_route_when_safe_frontier_exists() {
    let (mut map, start) = hazard_lane_fixture();
    map.set_hazard(Pos { y: 4, x: 3 }, true);

    for y in 2..=4 {
        map.set_tile(Pos { y, x: 2 }, TileKind::Floor);
    }
    for x in 2..=4 {
        map.set_tile(Pos { y: 2, x }, TileKind::Floor);
    }

    map.discovered.fill(true);
    map.visible.fill(true);
    map.discovered[(4 * map.internal_width) + 6] = false;
    map.discovered[(2 * map.internal_width) + 5] = false;

    let intent = choose_frontier_intent(&map, start).expect("expected frontier intent");
    assert_eq!(intent.target, Pos { y: 2, x: 4 });
}

#[test]
fn planner_reports_threat_avoidance_when_only_hazard_frontier_exists() {
    let (mut map, start) = hazard_lane_fixture();
    map.set_hazard(Pos { y: 4, x: 5 }, true);
    map.discovered[(4 * map.internal_width) + 6] = false;

    let intent = choose_frontier_intent(&map, start).expect("hazard fallback intent");
    assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
}

#[test]
fn safe_frontier_reachable_only_through_hazards_uses_threat_avoidance() {
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

    let start = Pos { y: 4, x: 5 };
    let intent = choose_frontier_intent(&map, start).expect("fallback on safe frontier");
    assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
    assert_eq!(intent.target, Pos { y: 4, x: 2 });
}

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
fn frontier_selection_ignores_non_visible_frontiers() {
    let mut map = Map::new(10, 10);
    for y in 1..9 {
        for x in 1..9 {
            map.set_tile(Pos { y, x }, TileKind::Floor);
        }
    }
    map.discovered.fill(true);
    map.visible.fill(false);
    for x in 2..=5 {
        map.set_visible(Pos { y: 4, x }, true);
    }
    map.discovered[(4 * map.internal_width) + 6] = false;
    map.discovered[(6 * map.internal_width) + 8] = false;

    let start = Pos { y: 4, x: 3 };
    let intent = choose_frontier_intent(&map, start).expect("visible frontier");
    assert_eq!(intent.target, Pos { y: 4, x: 5 });
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

#[test]
fn choose_frontier_intent_optimized_behavior() {
    let (mut map, start) = hazard_lane_fixture();
    map.discovered[3 * map.internal_width + 3] = false;
    map.discovered[4 * map.internal_width + 6] = false;
    map.set_hazard(Pos { y: 4, x: 4 }, true);

    let intent = choose_frontier_intent(&map, start).expect("frontier should be found");
    assert_eq!(intent.target, Pos { y: 4, x: 3 }, "should prefer safe frontier");
    assert_eq!(intent.reason, AutoReason::Frontier);
}
