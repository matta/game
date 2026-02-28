//! Breadth-first target search primitives for auto-explore planning.

use std::collections::{BTreeMap, VecDeque, btree_map::Entry};

use super::super::{AutoExploreIntent, AutoReason, Pos, TileKind, neighbors};
use crate::state::Map;

pub(super) fn find_nearest_auto_target<IsTarget, ReasonForTarget>(
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

    best_target.map(|(dist, target)| AutoExploreIntent {
        target,
        reason: reason_for_target(target),
        path_len: dist,
    })
}
