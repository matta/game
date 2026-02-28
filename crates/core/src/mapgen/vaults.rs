//! Vault template selection and post-layout map mutation logic.

use crate::types::{Pos, TileKind};

use super::grid::in_bounds;
use super::layout::{RoomLayout, RoomRect};
use super::model::{EnemySpawn, ItemSpawn};
use super::seed::random_usize;
use super::spawns::pick_item_kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VaultTemplate {
    ShrineRoom,
    GoblinCamp,
    PillarRoom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VaultStamp {
    template: VaultTemplate,
    room_index: usize,
}

pub(super) struct VaultApplicationContext<'a> {
    pub(super) floor_seed: u64,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) layout: &'a RoomLayout,
    pub(super) entry_tile: Pos,
    pub(super) down_stairs_tile: Pos,
    pub(super) tiles: &'a mut [TileKind],
    pub(super) hazards: &'a mut [bool],
    pub(super) enemy_spawns: &'a mut Vec<EnemySpawn>,
    pub(super) item_spawns: &'a mut Vec<ItemSpawn>,
}

pub(super) fn apply_vault_stamps(context: &mut VaultApplicationContext<'_>) {
    let stamps = build_vault_stamps(
        context.floor_seed,
        &context.layout.rooms,
        context.entry_tile,
        context.down_stairs_tile,
    );

    for stamp in stamps {
        let room = context.layout.rooms[stamp.room_index];
        let center = room.center();
        let center_y = center.y as usize;
        let center_x = center.x as usize;

        match stamp.template {
            VaultTemplate::GoblinCamp => {
                context.hazards[center_y * context.width + center_x] = true;
                let orthogonal = [
                    Pos { y: center.y - 1, x: center.x },
                    Pos { y: center.y + 1, x: center.x },
                    Pos { y: center.y, x: center.x - 1 },
                    Pos { y: center.y, x: center.x + 1 },
                ];
                let mut moved_enemy_count = 0;
                for spawn in &mut *context.enemy_spawns {
                    if moved_enemy_count >= 4 {
                        break;
                    }
                    if spawn.pos != context.entry_tile && spawn.pos != context.down_stairs_tile {
                        let target = orthogonal[moved_enemy_count];
                        if in_bounds(context.width, context.height, target)
                            && context.tiles
                                [(target.y as usize) * context.width + (target.x as usize)]
                                == TileKind::Floor
                        {
                            spawn.pos = target;
                            moved_enemy_count += 1;
                        }
                    }
                }
            }
            VaultTemplate::PillarRoom => {
                if center != context.entry_tile
                    && center != context.down_stairs_tile
                    && in_bounds(context.width, context.height, center)
                {
                    context.tiles[(center.y as usize) * context.width + (center.x as usize)] =
                        TileKind::Wall;
                    context.hazards[(center.y as usize) * context.width + (center.x as usize)] =
                        false;
                    context.item_spawns.retain(|spawn| spawn.pos != center);
                    context.enemy_spawns.retain(|spawn| spawn.pos != center);
                }
            }
            VaultTemplate::ShrineRoom => {
                if center != context.entry_tile
                    && center != context.down_stairs_tile
                    && !context.item_spawns.iter().any(|spawn| spawn.pos == center)
                {
                    context.item_spawns.push(ItemSpawn {
                        kind: pick_item_kind(context.floor_seed, context.item_spawns.len()),
                        pos: center,
                    });
                }
            }
        }
    }

    context.enemy_spawns.sort_by_key(|spawn| (spawn.pos.y, spawn.pos.x, spawn.kind));
    context.enemy_spawns.dedup_by_key(|spawn| spawn.pos);
    context.item_spawns.sort_by_key(|spawn| (spawn.pos.y, spawn.pos.x, spawn.kind));
}

fn build_vault_stamps(
    floor_seed: u64,
    rooms: &[RoomRect],
    entry_tile: Pos,
    down_stairs_tile: Pos,
) -> Vec<VaultStamp> {
    let mut valid_indices = Vec::new();
    for (index, room) in rooms.iter().enumerate() {
        if !room.contains(entry_tile) && !room.contains(down_stairs_tile) {
            valid_indices.push(index);
        }
    }

    if valid_indices.is_empty() {
        return Vec::new();
    }

    valid_indices.sort();
    let num_vaults = valid_indices.len().min(1 + random_usize(floor_seed, 2026, 0, 2));
    let mut stamps = Vec::with_capacity(num_vaults);

    for stamp_index in 0..num_vaults {
        let pick_index =
            random_usize(floor_seed, 3000 + stamp_index as u64, 0, valid_indices.len() - 1);
        let room_index = valid_indices.remove(pick_index);

        let template = match random_usize(floor_seed, 4000 + stamp_index as u64, 0, 2) {
            0 => VaultTemplate::ShrineRoom,
            1 => VaultTemplate::GoblinCamp,
            _ => VaultTemplate::PillarRoom,
        };

        stamps.push(VaultStamp { template, room_index });
    }

    stamps.sort_by_key(|stamp| stamp.room_index);
    stamps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapgen::layout::build_room_layout;

    #[test]
    fn vault_stamps_are_deterministic_for_same_floor_seed() {
        let layout = build_room_layout(2026, 20, 15);
        let left =
            build_vault_stamps(2026, &layout.rooms, layout.entry_tile, layout.down_stairs_tile);
        let right =
            build_vault_stamps(2026, &layout.rooms, layout.entry_tile, layout.down_stairs_tile);
        assert_eq!(left, right);
    }

    #[test]
    fn vault_stamps_never_use_entry_or_stairs_room() {
        let seeds = [14_u64, 777, 9_001, 123_456];
        for seed in seeds {
            let layout = build_room_layout(seed, 20, 15);
            let stamps =
                build_vault_stamps(seed, &layout.rooms, layout.entry_tile, layout.down_stairs_tile);
            assert!(!stamps.is_empty(), "expected vault stamps for seed {seed}");
            for stamp in stamps {
                let room = layout.rooms[stamp.room_index];
                assert!(!room.contains(layout.entry_tile));
                assert!(!room.contains(layout.down_stairs_tile));
            }
        }
    }
}
