//! High-level target selection policy for auto-explore.

use super::super::{AutoExploreIntent, AutoReason, Pos, TileKind};
use super::frontier::is_frontier_candidate;
use super::search::find_nearest_auto_target;
use crate::state::Map;

pub(in crate::game) fn choose_frontier_intent(map: &Map, start: Pos) -> Option<AutoExploreIntent> {
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

fn choose_downstairs_intent(map: &Map, start: Pos) -> Option<AutoExploreIntent> {
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

#[cfg(test)]
mod tests;
