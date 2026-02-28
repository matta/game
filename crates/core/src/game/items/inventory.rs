//! Inventory policy for equipping weapons and activating perks.

use super::*;

impl Game {
    pub(in crate::game) fn active_player_weapon(&self) -> Option<&'static str> {
        let player = &self.state.actors[self.state.player_id];
        match player.active_weapon_slot {
            WeaponSlot::Primary => player.equipped_weapon,
            WeaponSlot::Reserve => player.reserve_weapon,
        }
    }

    pub(super) fn apply_weapon_pickup(&mut self, id: &'static str) {
        let player = self.state.actors.get_mut(self.state.player_id).expect("player should exist");
        if player.equipped_weapon.is_none() {
            player.equipped_weapon = Some(id);
        } else if player.reserve_weapon.is_none() {
            player.reserve_weapon = Some(id);
        } else {
            match player.active_weapon_slot {
                WeaponSlot::Primary => player.equipped_weapon = Some(id),
                WeaponSlot::Reserve => player.reserve_weapon = Some(id),
            }
        }
    }

    pub(super) fn apply_perk_pickup(&mut self, id: &'static str) {
        if !self.state.active_perks.contains(&id) {
            self.state.active_perks.push(id);
        }
    }
}
