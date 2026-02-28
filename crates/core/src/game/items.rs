//! Item effect application and inventory-side gameplay effects.
//! This module exists to keep item behavior separate from core tick orchestration.
//! It does not own prompt generation or floor-transition policy.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::*;
use crate::content::keys;

impl Game {
    pub(super) fn active_player_weapon(&self) -> Option<&'static str> {
        let player = &self.state.actors[self.state.player_id];
        match player.active_weapon_slot {
            WeaponSlot::Primary => player.equipped_weapon,
            WeaponSlot::Reserve => player.reserve_weapon,
        }
    }

    pub(super) fn visible_enemy_ids_sorted(&self, distance_from: Option<Pos>) -> Vec<EntityId> {
        let mut ids: Vec<EntityId> = self
            .state
            .actors
            .iter()
            .filter(|(id, actor)| {
                *id != self.state.player_id && self.state.map.is_visible(actor.pos)
            })
            .map(|(id, _)| id)
            .collect();
        ids.sort_by(|a_id, b_id| {
            let a = &self.state.actors[*a_id];
            let b = &self.state.actors[*b_id];

            let distance_cmp = distance_from.map_or(Ordering::Equal, |origin| {
                manhattan(origin, a.pos).cmp(&manhattan(origin, b.pos))
            });
            if distance_cmp != Ordering::Equal {
                return distance_cmp;
            }
            let y_cmp = a.pos.y.cmp(&b.pos.y);
            if y_cmp != Ordering::Equal {
                return y_cmp;
            }
            let x_cmp = a.pos.x.cmp(&b.pos.x);
            if x_cmp != Ordering::Equal {
                return x_cmp;
            }
            a.kind
                .cmp(&b.kind)
                .then(a.hp.cmp(&b.hp))
                .then(a.next_action_tick.cmp(&b.next_action_tick))
        });
        ids
    }

    pub(super) fn apply_item_effect(&mut self, kind: ItemKind) {
        match kind {
            ItemKind::Weapon(id) => {
                let player =
                    self.state.actors.get_mut(self.state.player_id).expect("player should exist");
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
            ItemKind::Perk(id) => {
                if !self.state.active_perks.contains(&id) {
                    self.state.active_perks.push(id);
                }
            }
            ItemKind::Consumable(id) => self.apply_consumable_effect(id),
        }
    }

    fn apply_consumable_effect(&mut self, id: &'static str) {
        match id {
            keys::CONSUMABLE_MINOR_HP_POT => {
                let player =
                    self.state.actors.get_mut(self.state.player_id).expect("player should exist");
                player.hp = (player.hp + 10).min(player.max_hp);
            }
            keys::CONSUMABLE_MAJOR_HP_POT => {
                let player =
                    self.state.actors.get_mut(self.state.player_id).expect("player should exist");
                player.hp = (player.hp + 25).min(player.max_hp);
            }
            keys::CONSUMABLE_TELEPORT_RUNE => self.apply_teleport_rune(),
            keys::CONSUMABLE_FORTIFICATION_SCROLL => self.apply_fortification_scroll(),
            keys::CONSUMABLE_STASIS_HOURGLASS => {
                for enemy_id in self.visible_enemy_ids_sorted(None) {
                    self.state
                        .actors
                        .get_mut(enemy_id)
                        .expect("enemy should exist")
                        .next_action_tick += 50;
                }
            }
            keys::CONSUMABLE_MAGNETIC_LURE => self.apply_magnetic_lure(),
            keys::CONSUMABLE_SMOKE_BOMB => {
                self.state.threat_trace.clear();
                self.suppressed_enemy = None;
                for enemy_id in self.visible_enemy_ids_sorted(None) {
                    self.state
                        .actors
                        .get_mut(enemy_id)
                        .expect("enemy should exist")
                        .next_action_tick += 20;
                }
            }
            keys::CONSUMABLE_SHRAPNEL_BOMB => {
                let mut to_remove = Vec::new();
                for enemy_id in self.visible_enemy_ids_sorted(None) {
                    let actor = self.state.actors.get_mut(enemy_id).expect("enemy should exist");
                    actor.hp -= 5;
                    if actor.hp <= 0 {
                        to_remove.push(enemy_id);
                    }
                }
                for enemy_id in to_remove {
                    self.state.actors.remove(enemy_id);
                }
            }
            keys::CONSUMABLE_HASTE_POTION => {
                let tick = self.tick;
                let player =
                    self.state.actors.get_mut(self.state.player_id).expect("player should exist");
                let target = player.next_action_tick.saturating_sub(50);
                player.next_action_tick = target.max(tick + 1);
            }
            keys::CONSUMABLE_IRON_SKIN_POTION => {
                let player =
                    self.state.actors.get_mut(self.state.player_id).expect("player should exist");
                player.max_hp += 5;
                player.hp += 5;
            }
            _ => {}
        }
    }

    fn apply_teleport_rune(&mut self) {
        let player_pos = self.state.actors[self.state.player_id].pos;
        let nearest = self.visible_enemy_ids_sorted(Some(player_pos)).into_iter().next();
        if let Some(enemy_id) = nearest {
            let enemy_pos = self.state.actors[enemy_id].pos;
            self.state.actors.get_mut(self.state.player_id).expect("player should exist").pos =
                enemy_pos;
            self.state.actors.get_mut(enemy_id).expect("enemy should exist").pos = player_pos;
        }
    }

    fn apply_fortification_scroll(&mut self) {
        let player_pos = self.state.actors[self.state.player_id].pos;
        let occupied_positions: BTreeSet<Pos> =
            self.state.actors.values().map(|actor| actor.pos).collect();
        let mut fortified_map = self.state.map.clone();
        let mut reachable_before = reachable_discovered_walkable_tiles(&fortified_map, player_pos);
        let had_intent_before = choose_frontier_intent(&fortified_map, player_pos).is_some();

        for neighbor in neighbors(player_pos) {
            if !fortified_map.is_discovered_walkable(neighbor)
                || fortified_map.tile_at(neighbor) == TileKind::DownStairs
                || occupied_positions.contains(&neighbor)
                || is_frontier_candidate(&fortified_map, neighbor)
            {
                continue;
            }

            let original_tile = fortified_map.tile_at(neighbor);
            fortified_map.set_tile(neighbor, TileKind::Wall);

            let reachable_after = reachable_discovered_walkable_tiles(&fortified_map, player_pos);
            let preserves_reachable_component = reachable_before
                .iter()
                .all(|pos| *pos == neighbor || reachable_after.contains(pos));
            let preserves_progress_intent =
                !had_intent_before || choose_frontier_intent(&fortified_map, player_pos).is_some();

            if preserves_reachable_component && preserves_progress_intent {
                reachable_before = reachable_after;
            } else {
                fortified_map.set_tile(neighbor, original_tile);
            }
        }
        self.state.map = fortified_map;
        let radius = self.get_fov_radius();
        compute_fov(&mut self.state.map, self.state.actors[self.state.player_id].pos, radius);
    }

    fn apply_magnetic_lure(&mut self) {
        let player_pos = self.state.actors[self.state.player_id].pos;
        let mut moves = Vec::new();
        for enemy_id in self.visible_enemy_ids_sorted(Some(player_pos)) {
            let actor_pos = self.state.actors[enemy_id].pos;
            if let Some(path) = astar_path(&self.state.map, actor_pos, player_pos)
                && let Some(next_step) = path.first().copied()
            {
                moves.push((enemy_id, actor_pos, next_step));
            }
        }
        let mut occupied: BTreeSet<Pos> =
            self.state.actors.values().map(|actor| actor.pos).collect();
        for (enemy_id, from_pos, target_pos) in moves {
            if !occupied.contains(&target_pos) {
                occupied.remove(&from_pos);
                occupied.insert(target_pos);
                self.state.actors.get_mut(enemy_id).expect("enemy should exist").pos = target_pos;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::content::{ContentPack, keys};
    use crate::game::test_support::*;
    use crate::*;

    #[test]
    fn fortification_scroll_never_walls_tile_occupied_by_actor() {
        let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player_pos = game.state.actors[game.state.player_id].pos;
        let enemy_pos = Pos { y: player_pos.y, x: player_pos.x + 1 };
        let enemy_id = add_goblin(&mut game, enemy_pos);
        assert_eq!(game.state.map.tile_at(enemy_pos), TileKind::Floor);

        game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_FORTIFICATION_SCROLL));

        assert_eq!(
            game.state.map.tile_at(enemy_pos),
            TileKind::Floor,
            "fortification should not convert actor-occupied tiles into walls"
        );
        assert_eq!(game.state.actors[enemy_id].pos, enemy_pos);
    }

    #[test]
    fn fortification_scroll_preserves_an_adjacent_escape_tile() {
        let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let mut map = Map::new(9, 9);
        for y in 1..8 {
            for x in 1..8 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        game.state.map = map;

        let player_pos = Pos { y: 4, x: 4 };
        game.state.actors[game.state.player_id].pos = player_pos;

        game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_FORTIFICATION_SCROLL));

        let adjacent_walkable_count = neighbors(player_pos)
            .into_iter()
            .filter(|neighbor| game.state.map.is_discovered_walkable(*neighbor))
            .count();
        assert!(
            adjacent_walkable_count >= 1,
            "fortification must keep at least one adjacent walkable escape tile"
        );
    }

    #[test]
    fn teleport_rune_tie_break_uses_position_not_insertion_order() {
        let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let mut map = Map::new(12, 8);
        for y in 1..7 {
            for x in 1..11 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        game.state.map = map;

        let player_pos = Pos { y: 4, x: 5 };
        game.state.actors[game.state.player_id].pos = player_pos;

        let farther_in_sort_order =
            add_goblin(&mut game, Pos { y: player_pos.y + 1, x: player_pos.x + 1 });
        let nearer_in_sort_order =
            add_goblin(&mut game, Pos { y: player_pos.y - 1, x: player_pos.x + 1 });

        game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_TELEPORT_RUNE));

        assert_eq!(
            game.state.actors[game.state.player_id].pos,
            Pos { y: player_pos.y - 1, x: player_pos.x + 1 }
        );
        assert_eq!(game.state.actors[nearer_in_sort_order].pos, player_pos);
        assert_eq!(
            game.state.actors[farther_in_sort_order].pos,
            Pos { y: player_pos.y + 1, x: player_pos.x + 1 }
        );
    }

    #[test]
    fn test_magnetic_lure_synergy() {
        let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        let mut map = Map::new(12, 8);
        for y in 1..7 {
            for x in 1..11 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        game.state.map = map;

        let player_pos = Pos { y: 4, x: 3 };
        game.state.actors[game.state.player_id].pos = player_pos;
        let enemy_pos = Pos { y: 4, x: 8 };
        let enemy_id = add_goblin(&mut game, enemy_pos);

        // Apply magnetic lure
        game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_MAGNETIC_LURE));

        let new_enemy_pos = game.state.actors[enemy_id].pos;
        assert!(manhattan(player_pos, new_enemy_pos) < manhattan(player_pos, enemy_pos));
    }

    #[test]
    fn magnetic_lure_is_stable_across_enemy_insertion_order() {
        fn run_order(first: Pos, second: Pos) -> Vec<Pos> {
            let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
            game.state.items.clear();
            game.state.actors.retain(|id, _| id == game.state.player_id);

            let mut map = Map::new(12, 8);
            for y in 1..7 {
                for x in 1..11 {
                    map.set_tile(Pos { y, x }, TileKind::Floor);
                }
            }
            map.discovered.fill(true);
            map.visible.fill(true);
            game.state.map = map;

            let player_pos = Pos { y: 4, x: 4 };
            game.state.actors[game.state.player_id].pos = player_pos;

            let enemy_a = add_goblin(&mut game, first);
            let enemy_b = add_goblin(&mut game, second);

            game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_MAGNETIC_LURE));

            let mut positions =
                vec![game.state.actors[enemy_a].pos, game.state.actors[enemy_b].pos];
            positions.sort_by_key(|p| (p.y, p.x));
            positions
        }

        let left = run_order(Pos { y: 4, x: 6 }, Pos { y: 4, x: 7 });
        let right = run_order(Pos { y: 4, x: 7 }, Pos { y: 4, x: 6 });

        assert_eq!(left, right, "magnetic lure results should not depend on insertion order");
        assert_eq!(left, vec![Pos { y: 4, x: 5 }, Pos { y: 4, x: 6 }]);
    }
}
