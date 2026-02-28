//! Tests for downstairs target selection and hazard fallback behavior.

use super::*;

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
