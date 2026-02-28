//! Pause-bound policy and loadout updates.
//! This module mutates policy settings and active weapon slot at pause boundaries.

use super::*;

impl Game {
    pub fn apply_policy_update(&mut self, update: PolicyUpdate) -> Result<(), GameError> {
        if !self.at_pause_boundary && self.pending_prompt.is_none() {
            return Err(GameError::NotAtPauseBoundary);
        }
        match update {
            PolicyUpdate::FightMode(mode) => self.state.policy.fight_or_avoid = mode,
            PolicyUpdate::Stance(stance) => self.state.policy.stance = stance,
            PolicyUpdate::TargetPriority(target_priority) => {
                self.state.policy.target_priority = target_priority
            }
            PolicyUpdate::RetreatHpThreshold(threshold) => {
                self.state.policy.retreat_hp_threshold = threshold
            }
            PolicyUpdate::AutoHealIfBelowThreshold(threshold) => {
                self.state.policy.auto_heal_if_below_threshold = threshold
            }
            PolicyUpdate::PositionIntent(intent) => self.state.policy.position_intent = intent,
            PolicyUpdate::ResourceAggression(aggression) => {
                self.state.policy.resource_aggression = aggression
            }
            PolicyUpdate::ExplorationMode(mode) => self.state.policy.exploration_mode = mode,
        }
        self.no_progress_ticks = 0;
        Ok(())
    }

    pub fn apply_swap_weapon(&mut self) -> Result<(), GameError> {
        if !self.at_pause_boundary && self.pending_prompt.is_none() {
            return Err(GameError::NotAtPauseBoundary);
        }
        let player = self.state.actors.get_mut(self.state.player_id).expect("player should exist");
        player.active_weapon_slot = match player.active_weapon_slot {
            WeaponSlot::Primary => WeaponSlot::Reserve,
            WeaponSlot::Reserve => WeaponSlot::Primary,
        };
        player.next_action_tick += 10;
        self.no_progress_ticks = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentPack;

    #[test]
    fn swap_active_weapon_toggles_slot_and_consumes_ticks() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player_id = game.state.player_id;

        assert_eq!(game.state.actors[player_id].active_weapon_slot, WeaponSlot::Primary);
        let start_ticks = game.state.actors[player_id].next_action_tick;

        game.apply_swap_weapon().expect("swap should succeed at pause boundary");

        assert_eq!(game.state.actors[player_id].active_weapon_slot, WeaponSlot::Reserve);
        assert_eq!(game.state.actors[player_id].next_action_tick, start_ticks + 10);

        game.apply_swap_weapon().unwrap();
        assert_eq!(game.state.actors[player_id].active_weapon_slot, WeaponSlot::Primary);
        assert_eq!(game.state.actors[player_id].next_action_tick, start_ticks + 20);
    }
}
