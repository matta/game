//! Nearby item and enemy lookup helpers used by the simulation loop.

use super::*;

impl Game {
    pub(super) fn find_item_at(&self, pos: Pos) -> Option<ItemId> {
        self.state.items.iter().find(|(_, item)| item.pos == pos).map(|(id, _)| id)
    }

    pub(super) fn find_adjacent_enemy_ids(&self, pos: Pos) -> Vec<EntityId> {
        let enemies: Vec<EntityId> = self
            .state
            .actors
            .iter()
            .filter(|(id, actor)| {
                if *id == self.state.player_id {
                    return false;
                }
                let sanctuary = self.state.sanctuary_active.then_some(self.state.sanctuary_tile);
                enemy_path_to_player(&self.state.map, actor.pos, pos, sanctuary).is_some()
            })
            .filter(|(id, actor)| {
                Some(*id) != self.suppressed_enemy
                    && *id != self.state.player_id
                    && manhattan(pos, actor.pos) == 1
            })
            .map(|(id, _)| id)
            .collect();
        self.sort_adjacent_enemies_by_policy(pos, enemies)
    }

    pub(super) fn clear_stale_suppressed_enemy(&mut self, player_pos: Pos) {
        let Some(enemy_id) = self.suppressed_enemy else {
            return;
        };
        let should_clear = match self.state.actors.get(enemy_id) {
            Some(actor) => manhattan(player_pos, actor.pos) != 1,
            None => true,
        };
        if should_clear {
            self.suppressed_enemy = None;
        }
    }
}
