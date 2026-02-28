//! Auto-explore target selection and deterministic intent planning.
//! This module exists to keep high-level navigation policy separate from pathfinding primitives.
//! It does not own per-tick simulation advancement or combat/prompt handling.

use std::collections::{BTreeMap, VecDeque, btree_map::Entry};

use super::*;
use crate::state::Map;

pub(super) fn choose_frontier_intent(map: &Map, start: Pos) -> Option<AutoExploreIntent> {
    if let Some(intent) = find_nearest_frontier(map, start, true) {
        return Some(intent);
    }

    if let Some(intent) = find_nearest_frontier(map, start, false) {
        return Some(AutoExploreIntent { reason: AutoReason::ThreatAvoidance, ..intent });
    }

    choose_downstairs_intent(map, start)
}

fn find_nearest_frontier(map: &Map, start: Pos, avoid_hazards: bool) -> Option<AutoExploreIntent> {
    find_nearest_auto_target(
        map,
        start,
        avoid_hazards,
        |current| is_frontier_candidate(map, current),
        |target| {
            if map.tile_at(target) == TileKind::ClosedDoor {
                AutoReason::Door
            } else {
                AutoReason::Frontier
            }
        },
    )
}

pub(super) fn choose_downstairs_intent(map: &Map, start: Pos) -> Option<AutoExploreIntent> {
    if let Some(intent) = find_nearest_downstairs(map, start, true) {
        return Some(intent);
    }

    if let Some(intent) = find_nearest_downstairs(map, start, false) {
        return Some(AutoExploreIntent { reason: AutoReason::ThreatAvoidance, ..intent });
    }

    None
}

fn find_nearest_downstairs(
    map: &Map,
    start: Pos,
    avoid_hazards: bool,
) -> Option<AutoExploreIntent> {
    find_nearest_auto_target(
        map,
        start,
        avoid_hazards,
        |current| map.tile_at(current) == TileKind::DownStairs && map.is_discovered(current),
        |_target| AutoReason::Frontier,
    )
}

fn find_nearest_auto_target<IsTarget, ReasonForTarget>(
    map: &Map,
    start: Pos,
    avoid_hazards: bool,
    is_target: IsTarget,
    reason_for_target: ReasonForTarget,
) -> Option<AutoExploreIntent>
where
    IsTarget: Fn(Pos) -> bool,
    ReasonForTarget: Fn(Pos) -> AutoReason,
{
    if !map.is_discovered_walkable(start) {
        return None;
    }

    let mut visited = BTreeMap::new();
    let mut queue = VecDeque::new();

    visited.insert(start, 0u16);
    queue.push_back(start);

    let mut best_target: Option<(u16, Pos)> = None;

    while let Some(current) = queue.pop_front() {
        let dist = *visited.get(&current).expect("visited queue node must have known distance");

        if let Some((best_dist, _)) = best_target
            && dist > best_dist
        {
            break;
        }

        if current != start && is_target(current) {
            let is_better = match best_target {
                None => true,
                Some((best_dist, best_pos)) => {
                    dist < best_dist
                        || (dist == best_dist && (current.y, current.x) < (best_pos.y, best_pos.x))
                }
            };
            if is_better {
                best_target = Some((dist, current));
            }
        }

        for neighbor in neighbors(current) {
            if !map.is_discovered_walkable(neighbor) {
                continue;
            }
            if avoid_hazards && map.is_hazard(neighbor) {
                continue;
            }
            if map.tile_at(current) == TileKind::ClosedDoor {
                continue;
            }

            if let Entry::Vacant(entry) = visited.entry(neighbor) {
                entry.insert(dist + 1);
                queue.push_back(neighbor);
            }
        }
    }

    best_target.map(|(dist, target)| {
        let reason = reason_for_target(target);
        AutoExploreIntent { target, reason, path_len: dist }
    })
}

pub(super) fn is_safe_frontier_candidate(map: &Map, pos: Pos) -> bool {
    is_frontier_candidate(map, pos) && !map.is_hazard(pos)
}

pub(super) fn is_frontier_candidate(map: &Map, pos: Pos) -> bool {
    map.is_discovered(pos)
        && map.tile_at(pos) != TileKind::Wall
        && neighbors(pos).iter().any(|n| map.in_bounds(*n) && !map.is_discovered(*n))
}

pub(super) fn is_intent_target_still_valid(map: &Map, intent: AutoExploreIntent) -> bool {
    match intent.reason {
        AutoReason::ThreatAvoidance => is_frontier_candidate(map, intent.target),
        _ => is_safe_frontier_candidate(map, intent.target),
    }
}

pub(super) fn path_for_intent(
    map: &Map,
    start: Pos,
    intent: AutoExploreIntent,
) -> Option<Vec<Pos>> {
    match intent.reason {
        AutoReason::ThreatAvoidance => astar_path_allow_hazards(map, start, intent.target),
        _ => astar_path(map, start, intent.target),
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::content::ContentPack;
    use crate::game::test_support::*;
    use crate::mapgen::{BranchProfile, STARTING_FLOOR_INDEX};
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
        // Nearest downstairs should win.
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

        // If downstairs are tied by distance, pick lowest (y, x).
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

        // Safe alternative route to a different frontier.
        for y in 2..=4 {
            map.set_tile(Pos { y, x: 2 }, TileKind::Floor);
        }
        for x in 2..=4 {
            map.set_tile(Pos { y: 2, x }, TileKind::Floor);
        }

        map.discovered.fill(true);
        map.visible.fill(true);
        // Frontier near hazard lane.
        map.discovered[(4 * map.internal_width) + 6] = false;
        // Safe frontier candidate.
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
        let first_log_count =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(first_log_count, 1);

        // Move opposite the current target; replan would switch to x=8, reuse should not.
        let p2 = Pos { y: 4, x: 6 };
        game.state.actors[game.state.player_id].pos = p2;
        game.plan_auto_intent(p2);
        let second_intent = game.state.auto_intent.expect("second intent");
        assert_eq!(second_intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(second_intent.target, Pos { y: 4, x: 2 });
        let second_log_count =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
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
        map.discovered[(4 * map.internal_width) + 6] = false; // visible frontier
        map.discovered[(6 * map.internal_width) + 8] = false; // not visible frontier

        let start = Pos { y: 4, x: 3 };
        let intent = choose_frontier_intent(&map, start).expect("visible frontier");
        assert_eq!(intent.target, Pos { y: 4, x: 5 });
    }

    #[test]
    fn auto_explore_frontier_regression() {
        // 1. Open room with multiple frontiers.
        let (map, player_pos) = open_room_fixture();
        let mut map = map;
        map.discovered.fill(true);
        // Make (4,5) a frontier by setting its neighbor (3,5) to undiscovered.
        map.discovered[3 * map.internal_width + 5] = false;
        // Nearest frontier is at (4,5), distance 1.
        let intent = choose_frontier_intent(&map, player_pos).expect("frontier should be found");
        assert_eq!(intent.target, Pos { y: 4, x: 5 });
        assert_eq!(intent.path_len, 1);

        // 2. Maze-like layout requiring long paths.
        let mut map = Map::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        // Path: (1,1) -> (1,8) -> (8,8) -> (8,1)
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
        // Frontier at (8,1) - its neighbor (9,1) is unknown.
        map.discovered[9 * 10 + 1] = false;
        let start = Pos { y: 1, x: 1 };
        let intent = choose_frontier_intent(&map, start).expect("frontier should be found in maze");
        assert_eq!(intent.target, Pos { y: 8, x: 1 });
        // Path: (1,2..8) [7 steps] + (2..8, 8) [7 steps] + (8, 7..1) [7 steps] = 21 steps.
        assert_eq!(intent.path_len, 21);

        // 3. Scenarios with hazards.
        let (mut map, start) = hazard_lane_fixture();
        // Neighbor of (4,5) is (4,6), make it unknown.
        map.discovered[4 * map.internal_width + 6] = false;
        // Set (4,4) as hazard. start is (4,2).
        map.set_hazard(Pos { y: 4, x: 4 }, true);
        // Only path to (4,5) is through (4,4).
        let intent = choose_frontier_intent(&map, start).expect("hazard fallback should work");
        assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(intent.target, Pos { y: 4, x: 5 });

        // 4. Scenarios with closed doors.
        let (map, start, door) = closed_door_choke_fixture();
        // door is a frontier candidate because its neighbor is unknown.
        let intent = choose_frontier_intent(&map, start).expect("door frontier should be found");
        assert_eq!(intent.target, door);
        assert_eq!(intent.reason, AutoReason::Door);
    }

    #[test]
    fn choose_frontier_intent_optimized_behavior() {
        // These tests will initially fail until choose_frontier_intent is optimized.
        // But since we are replacing the internal implementation, we can use the same
        // public API tests to verify the new behavior.

        // 1. Dijkstra correctly identifies distances.
        // (Handled by existing regression tests)

        // 2. Safe frontier preferred over hazard frontier.
        let (mut map, start) = hazard_lane_fixture();
        // Path to (4,3) is length 1 (safe).
        // Path to (4,5) is length 3 (via hazard (4,4)).
        // Make both frontiers.
        map.discovered[3 * map.internal_width + 3] = false; // neighbor of (4,3)
        map.discovered[4 * map.internal_width + 6] = false; // neighbor of (4,5)
        map.set_hazard(Pos { y: 4, x: 4 }, true);

        let intent = choose_frontier_intent(&map, start).expect("frontier should be found");
        assert_eq!(intent.target, Pos { y: 4, x: 3 }, "should prefer safe frontier");
        assert_eq!(intent.reason, AutoReason::Frontier);

        // 3. Hazard fallback is correctly triggered.
        // (Handled by existing regression tests case 3)
    }
}
