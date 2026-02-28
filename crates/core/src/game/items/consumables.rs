//! Consumable routing and direct actor-state effects.

use super::*;
use crate::content::keys;

impl Game {
    pub(super) fn apply_consumable_effect(&mut self, id: &'static str) {
        match id {
            keys::CONSUMABLE_MINOR_HP_POT => self.apply_heal(10),
            keys::CONSUMABLE_MAJOR_HP_POT => self.apply_heal(25),
            keys::CONSUMABLE_TELEPORT_RUNE => self.apply_teleport_rune(),
            keys::CONSUMABLE_FORTIFICATION_SCROLL => self.apply_fortification_scroll(),
            keys::CONSUMABLE_STASIS_HOURGLASS => self.delay_visible_enemies(50),
            keys::CONSUMABLE_MAGNETIC_LURE => self.apply_magnetic_lure(),
            keys::CONSUMABLE_SMOKE_BOMB => self.apply_smoke_bomb(),
            keys::CONSUMABLE_SHRAPNEL_BOMB => self.apply_shrapnel_bomb(),
            keys::CONSUMABLE_HASTE_POTION => self.apply_haste_potion(),
            keys::CONSUMABLE_IRON_SKIN_POTION => self.apply_iron_skin_potion(),
            _ => {}
        }
    }

    fn apply_heal(&mut self, amount: i32) {
        let player = self.state.actors.get_mut(self.state.player_id).expect("player should exist");
        player.hp = (player.hp + amount).min(player.max_hp);
    }

    fn delay_visible_enemies(&mut self, delay: u64) {
        for enemy_id in self.visible_enemy_ids_sorted(None) {
            self.state.actors.get_mut(enemy_id).expect("enemy should exist").next_action_tick +=
                delay;
        }
    }

    fn apply_smoke_bomb(&mut self) {
        self.state.threat_trace.clear();
        self.suppressed_enemy = None;
        self.delay_visible_enemies(20);
    }

    fn apply_shrapnel_bomb(&mut self) {
        let mut defeated = Vec::new();
        for enemy_id in self.visible_enemy_ids_sorted(None) {
            let actor = self.state.actors.get_mut(enemy_id).expect("enemy should exist");
            actor.hp -= 5;
            if actor.hp <= 0 {
                defeated.push(enemy_id);
            }
        }
        for enemy_id in defeated {
            self.state.actors.remove(enemy_id);
        }
    }

    fn apply_haste_potion(&mut self) {
        let tick = self.tick;
        let player = self.state.actors.get_mut(self.state.player_id).expect("player should exist");
        let target = player.next_action_tick.saturating_sub(50);
        player.next_action_tick = target.max(tick + 1);
    }

    fn apply_iron_skin_potion(&mut self) {
        let player = self.state.actors.get_mut(self.state.player_id).expect("player should exist");
        player.max_hp += 5;
        player.hp += 5;
    }
}
