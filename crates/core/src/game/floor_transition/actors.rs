//! Actor-installation rules for floor transitions.

use super::*;
use crate::content::get_enemy_stats;
use crate::mapgen::GeneratedFloor;
use crate::state::Actor;

pub(in crate::game) fn install_floor_actors(game: &mut Game, generated: &GeneratedFloor) {
    let player_id = game.state.player_id;
    game.state.actors.retain(|id, _| id == player_id);
    game.state.actors[player_id].pos = generated.entry_tile;

    for spawn in &generated.enemy_spawns {
        let stats = get_enemy_stats(spawn.kind);
        let enemy = Actor {
            id: EntityId::default(),
            kind: spawn.kind,
            pos: spawn.pos,
            hp: stats.hp,
            max_hp: stats.hp,
            attack: stats.attack,
            defense: stats.defense,
            active_weapon_slot: WeaponSlot::Primary,
            equipped_weapon: None,
            reserve_weapon: None,
            next_action_tick: stats.speed as u64,
            speed: stats.speed,
        };
        let enemy_id = game.state.actors.insert(enemy);
        game.state.actors[enemy_id].id = enemy_id;
    }
}
