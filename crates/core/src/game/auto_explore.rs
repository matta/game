//! Auto-explore target selection and deterministic intent planning.
//! This module composes focused submodules for target selection, frontier rules, and route choice.

mod frontier;
mod pathing;
mod planner;
mod search;

pub(super) use frontier::{
    is_frontier_candidate, is_intent_target_still_valid, is_safe_frontier_candidate,
};
pub(super) use pathing::path_for_intent;
pub(super) use planner::choose_frontier_intent;
