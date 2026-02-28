//! Position-changing consumable effects and enemy movement pathing.

use std::collections::BTreeSet;

use super::*;

impl Game {
    pub(super) fn apply_teleport_rune(&mut self) {
        let player_pos = self.state.actors[self.state.player_id].pos;
        let nearest = self.visible_enemy_ids_sorted(Some(player_pos)).into_iter().next();
        if let Some(enemy_id) = nearest {
            let enemy_pos = self.state.actors[enemy_id].pos;
            self.state.actors.get_mut(self.state.player_id).expect("player should exist").pos =
                enemy_pos;
            self.state.actors.get_mut(enemy_id).expect("enemy should exist").pos = player_pos;
        }
    }

    pub(super) fn apply_magnetic_lure(&mut self) {
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
    use super::*;
    use crate::content::{ContentPack, keys};
    use crate::game::test_support::*;
    use crate::*;

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
