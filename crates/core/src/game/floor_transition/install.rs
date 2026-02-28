//! End-to-end installation workflow for generated floors.

use super::actors::install_floor_actors;
use super::*;
use crate::mapgen::{GeneratedFloor, MapGenerator};
use crate::state::{Item, Map};

pub(in crate::game) fn install_generated_floor(game: &mut Game, floor_index: u8) {
    let generated = MapGenerator::new(game.seed, game.state.branch_profile).generate(floor_index);

    install_floor_actors(game, &generated);
    install_floor_items(game, &generated);

    let mut map = Map::new(generated.width, generated.height);
    map.tiles = generated.tiles;
    map.hazards = generated.hazards;

    compute_fov(&mut map, generated.entry_tile, FOV_RADIUS);
    game.state.map = map;

    apply_floor_transition_state(game, floor_index, generated.entry_tile);
}

fn install_floor_items(game: &mut Game, generated: &GeneratedFloor) {
    game.state.items.clear();

    for spawn in &generated.item_spawns {
        let item = Item { id: ItemId::default(), kind: spawn.kind, pos: spawn.pos };
        let item_id = game.state.items.insert(item);
        game.state.items[item_id].id = item_id;
    }
}

fn apply_floor_transition_state(game: &mut Game, floor_index: u8, entry: Pos) {
    game.state.sanctuary_tile = entry;
    game.state.sanctuary_active = true;
    game.state.floor_index = floor_index;
    game.state.auto_intent = None;
    game.suppressed_enemy = None;
    game.no_progress_ticks = 0;
}
