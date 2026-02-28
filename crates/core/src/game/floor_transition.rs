//! Floor-change mechanics and generated floor state installation.
//! This module delegates generation, spawn installation, and transition state reset.

use super::*;

mod actors;
mod install;

#[cfg(test)]
mod tests;

impl Game {
    pub(super) fn descend_to_floor(&mut self, floor_index: u8) {
        install::install_generated_floor(self, floor_index);
    }
}
