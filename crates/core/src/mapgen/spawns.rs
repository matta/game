//! Enemy and item spawn selection and placement rules for generated maps.

use crate::{
    content::keys,
    types::{ActorKind, ItemKind, Pos, TileKind},
};

use super::grid::{manhattan, nearest_walkable_floor_tile};
use super::model::{EnemySpawn, ItemSpawn};
use super::progression::{self, BranchProfile};
use super::seed::random_usize;

const ITEM_ROLL_WEAPON_THRESHOLD: usize = 22;
const ITEM_ROLL_CONSUMABLE_THRESHOLD: usize = 72;

pub(super) struct SpawnContext<'a> {
    pub(super) floor_index: u8,
    pub(super) branch_profile: BranchProfile,
    pub(super) floor_seed: u64,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) tiles: &'a [TileKind],
    pub(super) entry_tile: Pos,
    pub(super) down_stairs_tile: Pos,
}

pub(super) fn generate_enemy_spawns(context: &SpawnContext<'_>) -> Vec<EnemySpawn> {
    let enemy_count = progression::enemy_spawn_count(context.floor_index, context.branch_profile);
    let target_total = enemy_count + usize::from(progression::is_final_floor(context.floor_index));

    let mut enemy_spawns = Vec::with_capacity(target_total);
    if progression::is_final_floor(context.floor_index) {
        enemy_spawns
            .push(EnemySpawn { kind: ActorKind::AbyssalWarden, pos: context.down_stairs_tile });
    }

    for enemy_index in 0..enemy_count {
        let enemy_x_shift = ((enemy_index * 7 + 11) % 64) as u32;
        let enemy_y_shift = ((enemy_index * 11 + 17) % 64) as u32;
        let x =
            2 + ((context.floor_seed.rotate_right(enemy_x_shift) as usize) % (context.width - 4));
        let y =
            2 + ((context.floor_seed.rotate_right(enemy_y_shift) as usize) % (context.height - 4));
        let candidate = Pos { y: y as i32, x: x as i32 };
        let pos =
            nearest_walkable_floor_tile(context.tiles, context.width, context.height, candidate);
        if manhattan(pos, context.entry_tile) > 1
            && pos != context.down_stairs_tile
            && !enemy_spawns.iter().any(|spawn| spawn.pos == pos)
        {
            let kind = pick_enemy_kind(context.floor_index, context.floor_seed, enemy_index);
            enemy_spawns.push(EnemySpawn { kind, pos });
        }
    }

    if enemy_spawns.len() < target_total {
        for y in 1..(context.height - 1) {
            for x in 1..(context.width - 1) {
                if enemy_spawns.len() >= target_total {
                    break;
                }
                let pos = Pos { y: y as i32, x: x as i32 };
                let tile = context.tiles[y * context.width + x];
                if tile != TileKind::Floor {
                    continue;
                }
                if manhattan(pos, context.entry_tile) <= 1
                    || pos == context.down_stairs_tile
                    || enemy_spawns.iter().any(|spawn| spawn.pos == pos)
                {
                    continue;
                }
                let kind =
                    pick_enemy_kind(context.floor_index, context.floor_seed, enemy_spawns.len());
                enemy_spawns.push(EnemySpawn { kind, pos });
            }
        }
    }

    enemy_spawns.sort_by_key(|spawn| (spawn.pos.y, spawn.pos.x, spawn.kind));
    enemy_spawns
}

pub(super) fn generate_item_spawns(
    context: &SpawnContext<'_>,
    enemy_spawns: &[EnemySpawn],
) -> Vec<ItemSpawn> {
    let mut item_spawns = Vec::new();
    let spawn_attempts = progression::item_spawn_attempts(context.floor_index);

    for item_index in 0..spawn_attempts {
        let item_y_shift = ((6 + item_index * 4) % 64) as u32;
        let item_x_shift = ((2 + item_index * 6) % 64) as u32;
        let item_target = Pos {
            y: (2
                + ((context.floor_seed.rotate_right(item_y_shift) as usize) % (context.height - 4)))
                as i32,
            x: (2
                + ((context.floor_seed.rotate_right(item_x_shift) as usize) % (context.width - 4)))
                as i32,
        };
        let item_pos =
            nearest_walkable_floor_tile(context.tiles, context.width, context.height, item_target);

        if item_pos != context.entry_tile
            && item_pos != context.down_stairs_tile
            && !item_spawns.iter().any(|spawn: &ItemSpawn| spawn.pos == item_pos)
            && !enemy_spawns.iter().any(|spawn| spawn.pos == item_pos)
        {
            item_spawns.push(ItemSpawn {
                kind: pick_item_kind(context.floor_seed, item_index),
                pos: item_pos,
            });
        }
    }

    item_spawns.sort_by_key(|spawn| (spawn.pos.y, spawn.pos.x, spawn.kind));
    item_spawns
}

pub(super) fn pick_item_kind(floor_seed: u64, spawn_index: usize) -> ItemKind {
    let roll = random_usize(floor_seed, 6000 + spawn_index as u64, 0, 99);

    if roll < ITEM_ROLL_WEAPON_THRESHOLD {
        let weapon_roll = random_usize(floor_seed, 6001 + spawn_index as u64, 0, 99);
        match weapon_roll % 5 {
            0 => ItemKind::Weapon(keys::WEAPON_RUSTY_SWORD),
            1 => ItemKind::Weapon(keys::WEAPON_IRON_MACE),
            2 => ItemKind::Weapon(keys::WEAPON_STEEL_LONGSWORD),
            3 => ItemKind::Weapon(keys::WEAPON_PHASE_DAGGER),
            _ => ItemKind::Weapon(keys::WEAPON_BLOOD_AXE),
        }
    } else if roll < ITEM_ROLL_CONSUMABLE_THRESHOLD {
        let consumable_roll = random_usize(floor_seed, 6002 + spawn_index as u64, 0, 99);
        match consumable_roll % 10 {
            0 => ItemKind::Consumable(keys::CONSUMABLE_MINOR_HP_POT),
            1 => ItemKind::Consumable(keys::CONSUMABLE_MAJOR_HP_POT),
            2 => ItemKind::Consumable(keys::CONSUMABLE_TELEPORT_RUNE),
            3 => ItemKind::Consumable(keys::CONSUMABLE_FORTIFICATION_SCROLL),
            4 => ItemKind::Consumable(keys::CONSUMABLE_STASIS_HOURGLASS),
            5 => ItemKind::Consumable(keys::CONSUMABLE_MAGNETIC_LURE),
            6 => ItemKind::Consumable(keys::CONSUMABLE_SMOKE_BOMB),
            7 => ItemKind::Consumable(keys::CONSUMABLE_SHRAPNEL_BOMB),
            8 => ItemKind::Consumable(keys::CONSUMABLE_HASTE_POTION),
            _ => ItemKind::Consumable(keys::CONSUMABLE_IRON_SKIN_POTION),
        }
    } else {
        let perk_roll = random_usize(floor_seed, 6003 + spawn_index as u64, 0, 99);
        match perk_roll % 10 {
            0 => ItemKind::Perk(keys::PERK_TOUGHNESS),
            1 => ItemKind::Perk(keys::PERK_SWIFT),
            2 => ItemKind::Perk(keys::PERK_BERSERKER_RHYTHM),
            3 => ItemKind::Perk(keys::PERK_PACIFISTS_BOUNTY),
            4 => ItemKind::Perk(keys::PERK_SNIPERS_EYE),
            5 => ItemKind::Perk(keys::PERK_IRON_WILL),
            6 => ItemKind::Perk(keys::PERK_BLOODLUST),
            7 => ItemKind::Perk(keys::PERK_SCOUT),
            8 => ItemKind::Perk(keys::PERK_RECKLESS_STRIKE),
            _ => ItemKind::Perk(keys::PERK_SHADOW_STEP),
        }
    }
}

fn pick_enemy_kind(floor_index: u8, floor_seed: u64, spawn_index: usize) -> ActorKind {
    let roll = random_usize(floor_seed, 5000 + spawn_index as u64, 0, 99);
    match floor_index {
        1 => {
            if roll < 60 {
                ActorKind::Goblin
            } else if roll < 90 {
                ActorKind::FeralHound
            } else {
                ActorKind::BloodAcolyte
            }
        }
        2 => {
            if roll < 20 {
                ActorKind::FeralHound
            } else if roll < 50 {
                ActorKind::BloodAcolyte
            } else if roll < 80 {
                ActorKind::CorruptedGuard
            } else {
                ActorKind::Gargoyle
            }
        }
        3 => {
            if roll < 20 {
                ActorKind::BloodAcolyte
            } else if roll < 50 {
                ActorKind::CorruptedGuard
            } else if roll < 80 {
                ActorKind::Gargoyle
            } else {
                ActorKind::LivingArmor
            }
        }
        4 => {
            if roll < 20 {
                ActorKind::CorruptedGuard
            } else if roll < 50 {
                ActorKind::Gargoyle
            } else if roll < 80 {
                ActorKind::LivingArmor
            } else {
                ActorKind::ShadowStalker
            }
        }
        _ => {
            if roll < 20 {
                ActorKind::Gargoyle
            } else if roll < 40 {
                ActorKind::LivingArmor
            } else if roll < 70 {
                ActorKind::ShadowStalker
            } else {
                ActorKind::AbyssalWarden
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::types::TileKind;

    use super::*;

    #[test]
    fn pick_item_kind_is_deterministic_for_seed_and_index() {
        let a = pick_item_kind(123, 4);
        let b = pick_item_kind(123, 4);
        assert_eq!(a, b);
    }

    #[test]
    fn enemy_kind_has_diversity_across_seed_and_floor() {
        let mut kinds = BTreeSet::new();
        for floor in 1..=3 {
            for spawn_index in 0..12 {
                kinds.insert(pick_enemy_kind(floor, 77_777, spawn_index));
            }
        }
        assert!(kinds.len() >= 4, "expected at least four kinds, got {kinds:?}");
    }

    #[test]
    fn generated_enemy_spawns_avoid_sanctuary_and_stairs() {
        let width = 20;
        let height = 15;
        let mut tiles = vec![TileKind::Wall; width * height];
        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                tiles[y * width + x] = TileKind::Floor;
            }
        }

        let context = SpawnContext {
            floor_index: 3,
            branch_profile: BranchProfile::BranchA,
            floor_seed: 9_876,
            width,
            height,
            tiles: &tiles,
            entry_tile: Pos { y: 2, x: 2 },
            down_stairs_tile: Pos { y: 12, x: 16 },
        };

        let enemy_spawns = generate_enemy_spawns(&context);
        for spawn in enemy_spawns {
            assert!(manhattan(spawn.pos, context.entry_tile) > 1);
            assert_ne!(spawn.pos, context.down_stairs_tile);
        }
    }
}
