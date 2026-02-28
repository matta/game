//! Frontier validity rules for auto-explore targets.

use super::super::{AutoExploreIntent, AutoReason, Pos, TileKind, neighbors};
use crate::state::Map;

pub(in crate::game) fn is_safe_frontier_candidate(map: &Map, pos: Pos) -> bool {
    is_frontier_candidate(map, pos) && !map.is_hazard(pos)
}

pub(in crate::game) fn is_frontier_candidate(map: &Map, pos: Pos) -> bool {
    map.is_discovered(pos)
        && map.tile_at(pos) != TileKind::Wall
        && neighbors(pos)
            .iter()
            .any(|neighbor| map.in_bounds(*neighbor) && !map.is_discovered(*neighbor))
}

pub(in crate::game) fn is_intent_target_still_valid(map: &Map, intent: AutoExploreIntent) -> bool {
    match intent.reason {
        AutoReason::ThreatAvoidance => is_frontier_candidate(map, intent.target),
        _ => is_safe_frontier_candidate(map, intent.target),
    }
}
