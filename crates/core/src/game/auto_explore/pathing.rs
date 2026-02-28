//! Route selection for executing a chosen auto-explore intent.

use super::super::{AutoExploreIntent, AutoReason, Pos, astar_path, astar_path_allow_hazards};
use crate::state::Map;

pub(in crate::game) fn path_for_intent(
    map: &Map,
    start: Pos,
    intent: AutoExploreIntent,
) -> Option<Vec<Pos>> {
    match intent.reason {
        AutoReason::ThreatAvoidance => astar_path_allow_hazards(map, start, intent.target),
        _ => astar_path(map, start, intent.target),
    }
}
