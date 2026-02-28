//! Fortification scroll validation and map mutation rules.

use std::collections::BTreeSet;

use super::*;

impl Game {
    pub(super) fn apply_fortification_scroll(&mut self) {
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
}

#[cfg(test)]
mod tests {
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
}
