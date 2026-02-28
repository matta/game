//! Floor-construction pipeline that composes mapgen subsystems.

use crate::types::{Pos, TileKind};

use super::super::grid::{farthest_walkable_tile_from_entry, nearest_walkable_floor_tile};
use super::super::layout::{build_room_layout, carve_room, carve_room_corridors};
use super::super::model::GeneratedFloor;
use super::super::progression::BranchProfile;
use super::super::seed::derive_floor_seed;
use super::super::spawns::{SpawnContext, generate_enemy_spawns, generate_item_spawns};
use super::super::vaults::{VaultApplicationContext, apply_vault_stamps};
use super::hazards::{BranchHazardContext, apply_branch_hazards};

pub(super) fn generate_floor(
    run_seed: u64,
    branch_profile: BranchProfile,
    width: usize,
    height: usize,
    floor_index: u8,
) -> GeneratedFloor {
    let mut tiles = vec![TileKind::Wall; width * height];
    let floor_seed = derive_floor_seed(run_seed, floor_index, branch_profile);
    let layout = build_room_layout(floor_seed, width, height);

    for room in &layout.rooms {
        carve_room(&mut tiles, width, room);
    }
    carve_room_corridors(&mut tiles, width, floor_seed, &layout.rooms);

    let entry_tile = nearest_walkable_floor_tile(&tiles, width, height, layout.entry_tile);
    let down_stairs_tile =
        resolve_down_stairs_tile(&tiles, width, height, entry_tile, layout.down_stairs_tile);

    let spawn_context = SpawnContext {
        floor_index,
        branch_profile,
        floor_seed,
        width,
        height,
        tiles: &tiles,
        entry_tile,
        down_stairs_tile,
    };
    let mut enemy_spawns = generate_enemy_spawns(&spawn_context);
    let mut item_spawns = generate_item_spawns(&spawn_context, &enemy_spawns);
    let mut hazards = vec![false; width * height];

    tiles[tile_index(down_stairs_tile, width)] = TileKind::DownStairs;
    apply_vault_stamps(&mut VaultApplicationContext {
        floor_seed,
        width,
        height,
        layout: &layout,
        entry_tile,
        down_stairs_tile,
        tiles: &mut tiles,
        hazards: &mut hazards,
        enemy_spawns: &mut enemy_spawns,
        item_spawns: &mut item_spawns,
    });

    apply_branch_hazards(&mut BranchHazardContext {
        tiles: &tiles,
        hazards: &mut hazards,
        width,
        height,
        floor_index,
        branch_profile,
        floor_seed,
        entry_tile,
        down_stairs_tile,
    });
    hazards[tile_index(down_stairs_tile, width)] = false;

    GeneratedFloor {
        width,
        height,
        tiles,
        hazards,
        entry_tile,
        down_stairs_tile,
        enemy_spawns,
        item_spawns,
    }
}

fn resolve_down_stairs_tile(
    tiles: &[TileKind],
    width: usize,
    height: usize,
    entry_tile: Pos,
    suggested_tile: Pos,
) -> Pos {
    let mut down_stairs_tile = nearest_walkable_floor_tile(tiles, width, height, suggested_tile);
    if down_stairs_tile == entry_tile {
        down_stairs_tile = farthest_walkable_tile_from_entry(tiles, width, height, entry_tile);
    }
    down_stairs_tile
}

fn tile_index(pos: Pos, width: usize) -> usize {
    (pos.y as usize) * width + (pos.x as usize)
}
