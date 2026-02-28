//! Tests for frontier selection policy, visibility filtering, and hazard tradeoffs.

use super::*;

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
fn choose_frontier_intent_optimized_behavior() {
    let (mut map, start) = hazard_lane_fixture();
    map.discovered[3 * map.internal_width + 3] = false;
    map.discovered[4 * map.internal_width + 6] = false;
    map.set_hazard(Pos { y: 4, x: 4 }, true);

    let intent = choose_frontier_intent(&map, start).expect("frontier should be found");
    assert_eq!(intent.target, Pos { y: 4, x: 3 }, "should prefer safe frontier");
    assert_eq!(intent.reason, AutoReason::Frontier);
}
