//! Branch-specific hazard placement rules for generated floors.

use crate::types::{Pos, TileKind};

use super::super::grid::nearest_walkable_floor_tile;
use super::super::progression::{self, BranchProfile};

pub(super) struct BranchHazardContext<'a> {
    pub(super) tiles: &'a [TileKind],
    pub(super) hazards: &'a mut [bool],
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) floor_index: u8,
    pub(super) branch_profile: BranchProfile,
    pub(super) floor_seed: u64,
    pub(super) entry_tile: Pos,
    pub(super) down_stairs_tile: Pos,
}

pub(super) fn apply_branch_hazards(context: &mut BranchHazardContext<'_>) {
    let hazard_count =
        progression::branch_hazard_count(context.floor_index, context.branch_profile);
    for hazard_index in 0..hazard_count {
        let hx =
            2 + (((context.floor_seed >> (hazard_index * 13 + 23)) as usize) % (context.width - 4));
        let hy =
            2 + (((context.floor_seed >> (hazard_index * 9 + 29)) as usize) % (context.height - 4));
        let target = Pos { y: hy as i32, x: hx as i32 };
        let pos = nearest_walkable_floor_tile(context.tiles, context.width, context.height, target);
        if pos != context.entry_tile && pos != context.down_stairs_tile {
            context.hazards[(pos.y as usize) * context.width + (pos.x as usize)] = true;
        }
    }
}
