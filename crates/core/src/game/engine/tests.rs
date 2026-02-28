//! Regression test module wiring for engine behaviors.

mod bootstrap_layout;
mod intent_planning;
mod interruption_flow;
mod termination_guards;

/// Shared imports for engine regression tests.
mod support {
    pub(super) use super::super::*;
    pub(super) use crate::content::ContentPack;
    pub(super) use crate::game::test_support::*;
    pub(super) use crate::game::visibility::draw_map_diag;
    pub(super) use crate::mapgen::{BranchProfile, STARTING_FLOOR_INDEX};
    pub(super) use crate::*;
}
