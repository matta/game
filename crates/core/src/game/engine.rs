//! Simulation engine composition for tick advancement and planning helpers.
//! This file wires focused engine submodules together.

use super::*;

mod advance;
mod encounters;
mod intent;

#[cfg(test)]
mod tests;
