//! Deterministic movement primitives and shortest-path helpers.
//! This module exists so navigation rules are reusable across simulation systems.
//! It does not own high-level exploration policy or player decision flow.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::*;
use crate::state::Map;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct OpenNode {
    f: u32,
    h: u32,
    y: i32,
    x: i32,
}

pub(super) fn reachable_discovered_walkable_tiles(map: &Map, start: Pos) -> BTreeSet<Pos> {
    let mut visited = BTreeSet::new();
    if !map.is_discovered_walkable(start) {
        return visited;
    }

    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        for neighbor in neighbors(current) {
            if map.is_discovered_walkable(neighbor) && visited.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }

    visited
}

pub(super) fn astar_path(map: &Map, start: Pos, goal: Pos) -> Option<Vec<Pos>> {
    astar_path_internal(map, start, goal, true, None, true)
}

pub(super) fn astar_path_allow_hazards(map: &Map, start: Pos, goal: Pos) -> Option<Vec<Pos>> {
    astar_path_internal(map, start, goal, false, None, true)
}

pub(super) fn enemy_path_to_player(
    map: &Map,
    enemy_pos: Pos,
    player_pos: Pos,
    sanctuary_tile: Option<Pos>,
) -> Option<Vec<Pos>> {
    astar_path_internal(map, enemy_pos, player_pos, false, sanctuary_tile, false)
}

fn astar_path_internal(
    map: &Map,
    start: Pos,
    goal: Pos,
    avoid_hazards: bool,
    blocked_tile: Option<Pos>,
    allow_goal_on_blocked_tile: bool,
) -> Option<Vec<Pos>> {
    if !map.is_discovered_walkable(start) || !map.is_discovered_walkable(goal) {
        return None;
    }
    if blocked_tile.is_some_and(|blocked| blocked == goal && !allow_goal_on_blocked_tile) {
        return None;
    }
    if start == goal {
        return Some(vec![]);
    }
    let mut open_set = BTreeSet::new();
    let mut g_score = BTreeMap::new();
    let mut came_from = BTreeMap::new();
    let h = manhattan(start, goal);
    open_set.insert(OpenNode { f: h, h, y: start.y, x: start.x });
    g_score.insert(start, 0);
    while let Some(curr) = open_set.pop_first() {
        let p = Pos { y: curr.y, x: curr.x };
        if p == goal {
            return Some(reconstruct_path(&came_from, start, goal));
        }
        let cur_g = *g_score.get(&p).expect("current node must have g-score");
        for n in neighbors_for_astar(p, blocked_tile) {
            if !is_astar_step_walkable(
                map,
                n,
                goal,
                avoid_hazards,
                blocked_tile,
                allow_goal_on_blocked_tile,
            ) {
                continue;
            }
            let tg = cur_g + 1;
            if tg < *g_score.get(&n).unwrap_or(&u32::MAX) {
                came_from.insert(n, p);
                g_score.insert(n, tg);
                let h = manhattan(n, goal);
                open_set.insert(OpenNode { f: tg + h, h, y: n.y, x: n.x });
            }
        }
    }
    None
}

fn is_astar_step_walkable(
    map: &Map,
    pos: Pos,
    goal: Pos,
    avoid_hazards: bool,
    blocked_tile: Option<Pos>,
    allow_goal_on_blocked_tile: bool,
) -> bool {
    if blocked_tile
        .is_some_and(|blocked| blocked == pos && (pos != goal || !allow_goal_on_blocked_tile))
    {
        return false;
    }
    if !map.is_discovered_walkable(pos) {
        return false;
    }
    if avoid_hazards && map.is_hazard(pos) {
        return false;
    }
    if map.tile_at(pos) == TileKind::ClosedDoor && pos != goal {
        return false;
    }
    true
}

fn neighbors_for_astar(p: Pos, blocked_tile: Option<Pos>) -> Vec<Pos> {
    neighbors(p).into_iter().filter(|next| Some(*next) != blocked_tile).collect()
}

fn reconstruct_path(came: &BTreeMap<Pos, Pos>, start: Pos, goal: Pos) -> Vec<Pos> {
    let mut p = goal;
    let mut result = vec![p];
    while p != start {
        p = *came.get(&p).expect("path must be reconstructible");
        result.push(p);
    }
    result.reverse();
    result.remove(0);
    result
}

pub(super) fn neighbors(p: Pos) -> [Pos; 4] {
    [
        Pos { y: p.y - 1, x: p.x },
        Pos { y: p.y, x: p.x + 1 },
        Pos { y: p.y + 1, x: p.x },
        Pos { y: p.y, x: p.x - 1 },
    ]
}

pub(super) fn manhattan(a: Pos, b: Pos) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::game::test_support::*;
    use crate::*;

    #[test]
    fn closed_door_is_not_used_as_path_transit_tile() {
        let mut map = Map::new(12, 6);
        for y in 1..5 {
            for x in 1..11 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 1..=10 {
            map.set_tile(Pos { y: 3, x }, TileKind::Floor);
        }
        map.set_tile(Pos { y: 3, x: 5 }, TileKind::ClosedDoor);
        map.discovered.fill(true);

        let start = Pos { y: 3, x: 2 };
        let beyond_door = Pos { y: 3, x: 8 };
        assert!(
            astar_path(&map, start, beyond_door).is_none(),
            "closed door should block traversal to tiles behind it"
        );
        assert!(
            astar_path(&map, start, Pos { y: 3, x: 5 }).is_some(),
            "closed door may still be targeted directly for interrupt handling"
        );
    }

    #[test]
    fn enemy_pathfinding_cannot_step_onto_sanctuary_tile() {
        let mut map = Map::new(10, 7);
        for y in 1..6 {
            for x in 1..9 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);

        let enemy_pos = Pos { y: 3, x: 6 };
        let sanctuary = Pos { y: 3, x: 4 };
        let player_on_sanctuary = sanctuary;

        let path = enemy_path_to_player(&map, enemy_pos, player_on_sanctuary, Some(sanctuary));
        assert!(
            path.is_none(),
            "enemy A* should treat sanctuary as non-walkable even when the player stands on it"
        );
    }
}
