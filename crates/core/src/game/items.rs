//! Item effect application and inventory-side gameplay effects.
//! This module exists to keep item behavior separate from core tick orchestration.
//! It does not own prompt generation or floor-transition policy.

use super::*;

mod consumables;
mod fortification;
mod inventory;
mod search;
mod spatial_effects;

impl Game {
    pub(super) fn apply_item_effect(&mut self, kind: ItemKind) {
        match kind {
            ItemKind::Weapon(id) => self.apply_weapon_pickup(id),
            ItemKind::Perk(id) => self.apply_perk_pickup(id),
            ItemKind::Consumable(id) => self.apply_consumable_effect(id),
        }
    }
}
