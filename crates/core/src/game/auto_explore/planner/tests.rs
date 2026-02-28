//! Tests for auto-explore planner behavior and hazard fallback rules.

use super::*;
use crate::content::ContentPack;
use crate::game::test_support::*;
use crate::state::Map;
use crate::*;

mod downstairs_policy;
mod frontier_policy;
mod integration_regressions;
