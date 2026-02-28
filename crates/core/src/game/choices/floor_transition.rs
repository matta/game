//! Floor-transition choice validation and effects.
//! This module applies branch/god selection and floor-descend outcomes from prompts.

use super::*;
use crate::content::keys;
use crate::mapgen::BranchProfile;

impl Game {
    pub(super) fn is_floor_transition_choice(choice: &Choice) -> bool {
        matches!(
            choice,
            Choice::Descend
                | Choice::DescendBranchAVeil
                | Choice::DescendBranchAForge
                | Choice::DescendBranchBVeil
                | Choice::DescendBranchBForge
        )
    }

    pub(super) fn resolve_floor_transition_choice(
        &mut self,
        current_floor: u8,
        next_floor: Option<u8>,
        requires_branch_god_choice: bool,
        choice: Choice,
    ) -> Result<(), GameError> {
        if self.state.floor_index != current_floor {
            return Err(GameError::InvalidChoice);
        }

        if requires_branch_god_choice && !Self::is_branch_choice(&choice) {
            return Err(GameError::InvalidChoice);
        }
        if !requires_branch_god_choice && !matches!(choice, Choice::Descend) {
            return Err(GameError::InvalidChoice);
        }

        match &choice {
            Choice::DescendBranchAVeil => {
                self.state.branch_profile = BranchProfile::BranchA;
                self.state.active_god = Some(GodId::Veil);
            }
            Choice::DescendBranchAForge => {
                self.state.branch_profile = BranchProfile::BranchA;
                self.state.active_god = Some(GodId::Forge);
            }
            Choice::DescendBranchBVeil => {
                self.state.branch_profile = BranchProfile::BranchB;
                self.state.active_god = Some(GodId::Veil);
            }
            Choice::DescendBranchBForge => {
                self.state.branch_profile = BranchProfile::BranchB;
                self.state.active_god = Some(GodId::Forge);
            }
            Choice::Descend => {
                if self.state.branch_profile == BranchProfile::Uncommitted
                    || self.state.active_god.is_none()
                {
                    return Err(GameError::InvalidChoice);
                }
            }
            _ => {}
        }

        if requires_branch_god_choice && self.state.active_god == Some(GodId::Forge) {
            let player =
                self.state.actors.get_mut(self.state.player_id).expect("player should exist");
            player.max_hp += 2;
            player.hp = (player.hp + 2).min(player.max_hp);
        }

        if self.state.active_perks.contains(&keys::PERK_PACIFISTS_BOUNTY)
            && self.state.kills_this_floor == 0
        {
            let player =
                self.state.actors.get_mut(self.state.player_id).expect("player should exist");
            player.max_hp += 5;
            player.hp = player.max_hp;
        }

        self.state.kills_this_floor = 0;
        match next_floor {
            Some(next_index) => self.descend_to_floor(next_index),
            None => {
                self.finished_outcome = Some(RunOutcome::Victory);
            }
        }

        Ok(())
    }

    fn is_branch_choice(choice: &Choice) -> bool {
        matches!(
            choice,
            Choice::DescendBranchAVeil
                | Choice::DescendBranchAForge
                | Choice::DescendBranchBVeil
                | Choice::DescendBranchBForge
        )
    }
}
