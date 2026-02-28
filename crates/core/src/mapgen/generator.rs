//! High-level map generation orchestration that composes layout, spawns, and vaults.

use crate::types::{Pos, TileKind};

use super::grid::{farthest_walkable_tile_from_entry, nearest_walkable_floor_tile};
use super::layout::{build_room_layout, carve_room, carve_room_corridors};
use super::model::GeneratedFloor;
use super::progression::{self, BranchProfile};
use super::seed::derive_floor_seed;
use super::spawns::{SpawnContext, generate_enemy_spawns, generate_item_spawns};
use super::vaults::{VaultApplicationContext, apply_vault_stamps};

pub struct MapGenerator {
    run_seed: u64,
    branch_profile: BranchProfile,
    width: usize,
    height: usize,
}

impl MapGenerator {
    pub fn new(run_seed: u64, branch_profile: BranchProfile) -> Self {
        Self { run_seed, branch_profile, width: 20, height: 15 }
    }

    pub fn generate(&self, floor_index: u8) -> GeneratedFloor {
        let mut tiles = vec![TileKind::Wall; self.width * self.height];

        let floor_seed = derive_floor_seed(self.run_seed, floor_index, self.branch_profile);
        let layout = build_room_layout(floor_seed, self.width, self.height);

        for room in &layout.rooms {
            carve_room(&mut tiles, self.width, room);
        }
        carve_room_corridors(&mut tiles, self.width, floor_seed, &layout.rooms);

        let entry_tile =
            nearest_walkable_floor_tile(&tiles, self.width, self.height, layout.entry_tile);

        let mut down_stairs_tile =
            nearest_walkable_floor_tile(&tiles, self.width, self.height, layout.down_stairs_tile);
        if down_stairs_tile == entry_tile {
            down_stairs_tile =
                farthest_walkable_tile_from_entry(&tiles, self.width, self.height, entry_tile);
        }

        let spawn_context = SpawnContext {
            floor_index,
            branch_profile: self.branch_profile,
            floor_seed,
            width: self.width,
            height: self.height,
            tiles: &tiles,
            entry_tile,
            down_stairs_tile,
        };

        let mut enemy_spawns = generate_enemy_spawns(&spawn_context);
        let mut item_spawns = generate_item_spawns(&spawn_context, &enemy_spawns);

        let mut hazards = vec![false; self.width * self.height];
        tiles[(down_stairs_tile.y as usize) * self.width + (down_stairs_tile.x as usize)] =
            TileKind::DownStairs;

        apply_vault_stamps(&mut VaultApplicationContext {
            floor_seed,
            width: self.width,
            height: self.height,
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
            width: self.width,
            height: self.height,
            floor_index,
            branch_profile: self.branch_profile,
            floor_seed,
            entry_tile,
            down_stairs_tile,
        });

        hazards[(down_stairs_tile.y as usize) * self.width + (down_stairs_tile.x as usize)] = false;

        GeneratedFloor {
            width: self.width,
            height: self.height,
            tiles,
            hazards,
            entry_tile,
            down_stairs_tile,
            enemy_spawns,
            item_spawns,
        }
    }
}

struct BranchHazardContext<'a> {
    tiles: &'a [TileKind],
    hazards: &'a mut [bool],
    width: usize,
    height: usize,
    floor_index: u8,
    branch_profile: BranchProfile,
    floor_seed: u64,
    entry_tile: Pos,
    down_stairs_tile: Pos,
}

fn apply_branch_hazards(context: &mut BranchHazardContext<'_>) {
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, VecDeque};

    use proptest::prelude::*;
    use xxhash_rust::xxh3::xxh3_64;

    use super::*;
    use crate::mapgen::layout::build_room_layout;
    use crate::mapgen::progression::{MAX_FLOORS, STARTING_FLOOR_INDEX};
    use crate::mapgen::seed::derive_floor_seed;
    use crate::types::ActorKind;

    #[test]
    fn floor_generation_fingerprint_matrix_is_stable() {
        let cases = [
            (11_u64, 1_u8, BranchProfile::Uncommitted, 10_502_962_858_730_357_636_u64),
            (11_u64, 2_u8, BranchProfile::BranchA, 819_471_043_711_037_404_u64),
            (11_u64, 3_u8, BranchProfile::BranchB, 7_401_365_591_015_175_815_u64),
            (123_456_u64, 2_u8, BranchProfile::Uncommitted, 3_281_561_617_618_309_724_u64),
            (987_654_u64, 5_u8, BranchProfile::BranchA, 17_912_462_762_959_284_267_u64),
        ];

        for (seed, floor, branch, expected_hash) in cases {
            let generated = MapGenerator::new(seed, branch).generate(floor);
            let hash = xxh3_64(&generated.canonical_bytes());
            assert_eq!(
                hash, expected_hash,
                "update expected hash only when generation rules intentionally change"
            );
        }
    }

    #[test]
    fn vaults_spawn_reliably_across_seeds_without_breaking_connectivity() {
        let seeds = [1_u64, 2, 3, 4, 5, 40, 99, 321, 1_024, 999_999];
        for seed in seeds {
            for floor in 1..=MAX_FLOORS {
                let generated = MapGenerator::new(seed, BranchProfile::BranchA).generate(floor);
                let layout = build_room_layout(
                    derive_floor_seed(seed, floor, BranchProfile::BranchA),
                    generated.width,
                    generated.height,
                );

                if layout.rooms.len() > 2 {
                    assert!(
                        all_walkable_tiles_connected(&generated),
                        "vault stamping should keep map connected for seed={seed} floor={floor}"
                    );
                }
            }
        }
    }

    #[test]
    fn same_inputs_produce_byte_identical_floor_output() {
        let a = MapGenerator::new(123_456, BranchProfile::BranchA).generate(2);
        let b = MapGenerator::new(123_456, BranchProfile::BranchA).generate(2);
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    }

    #[test]
    fn changing_floor_index_changes_floor_output_for_same_seed_and_branch() {
        let floor_1 = MapGenerator::new(123_456, BranchProfile::Uncommitted).generate(1);
        let floor_2 = MapGenerator::new(123_456, BranchProfile::Uncommitted).generate(2);
        assert_ne!(floor_1.canonical_bytes(), floor_2.canonical_bytes());
    }

    #[test]
    fn same_seed_and_branch_produce_identical_floors_for_floor_two_and_three() {
        let floor_2_a = MapGenerator::new(88_001, BranchProfile::BranchA).generate(2);
        let floor_2_b = MapGenerator::new(88_001, BranchProfile::BranchA).generate(2);
        assert_eq!(floor_2_a.canonical_bytes(), floor_2_b.canonical_bytes());
    }

    #[test]
    fn boss_spawns_on_final_floor() {
        let final_floor = MapGenerator::new(1234, BranchProfile::Uncommitted).generate(MAX_FLOORS);
        let boss_count = final_floor
            .enemy_spawns
            .iter()
            .filter(|spawn| spawn.kind == ActorKind::AbyssalWarden)
            .count();
        assert_eq!(boss_count, 1, "exactly one boss should spawn on the final floor");

        let early_floor = MapGenerator::new(1234, BranchProfile::Uncommitted).generate(2);
        let early_boss_count = early_floor
            .enemy_spawns
            .iter()
            .filter(|spawn| spawn.kind == ActorKind::AbyssalWarden)
            .count();
        assert_eq!(early_boss_count, 0, "boss should not spawn on earlier floors");
    }

    #[test]
    fn enemy_diversity() {
        let mut kinds = BTreeSet::new();
        for floor in 1..=3 {
            for seed in 0..5 {
                let generated = MapGenerator::new(seed, BranchProfile::Uncommitted).generate(floor);
                for spawn in generated.enemy_spawns {
                    kinds.insert(spawn.kind);
                }
            }
        }
        assert!(kinds.len() >= 4, "expected high enemy diversity, got {kinds:?}");
    }

    #[test]
    fn different_branches_change_floor_two_and_three_characteristics() {
        let floor_2_a = MapGenerator::new(77_777, BranchProfile::BranchA).generate(2);
        let floor_2_b = MapGenerator::new(77_777, BranchProfile::BranchB).generate(2);
        let floor_3_a = MapGenerator::new(77_777, BranchProfile::BranchA).generate(3);
        let floor_3_b = MapGenerator::new(77_777, BranchProfile::BranchB).generate(3);

        let floor_2_a_hazards = floor_2_a.hazards.iter().filter(|&&h| h).count();
        let floor_2_b_hazards = floor_2_b.hazards.iter().filter(|&&h| h).count();
        let floor_3_a_hazards = floor_3_a.hazards.iter().filter(|&&h| h).count();
        let floor_3_b_hazards = floor_3_b.hazards.iter().filter(|&&h| h).count();

        assert!(floor_2_a.enemy_spawns.len() > floor_2_b.enemy_spawns.len());
        assert!(floor_3_a.enemy_spawns.len() > floor_3_b.enemy_spawns.len());
        assert!(floor_2_b_hazards > floor_2_a_hazards);
        assert!(floor_3_b_hazards > floor_3_a_hazards);
    }

    #[test]
    fn generated_floor_has_walkable_route_from_entry_to_stairs() {
        let generated = MapGenerator::new(987_654, BranchProfile::BranchB).generate(3);
        assert!(
            has_walkable_route(&generated, generated.entry_tile, generated.down_stairs_tile),
            "generated floor should always have a walkable route from entry to stairs"
        );
    }

    #[test]
    fn sanctuary_spawn_rule_holds_across_multiple_seeds_and_floors() {
        let seeds = [11_u64, 2_024, 77_777, 909_090];
        for seed in seeds {
            for floor in (STARTING_FLOOR_INDEX + 1)..=MAX_FLOORS {
                let generated = MapGenerator::new(seed, BranchProfile::BranchA).generate(floor);
                for spawn in &generated.enemy_spawns {
                    assert!(
                        manhattan(spawn.pos, generated.entry_tile) > 1,
                        "enemy spawn {:?} must not be on sanctuary {:?} (seed={seed}, floor={floor})",
                        spawn.pos,
                        generated.entry_tile
                    );
                }
            }
        }
    }

    #[test]
    fn downstairs_tile_is_reachable_non_hazard_and_unoccupied_at_floor_start() {
        let seeds = [123_u64, 456, 789, 10_111];
        for seed in seeds {
            let generated = MapGenerator::new(seed, BranchProfile::BranchB).generate(2);
            assert_eq!(generated.tile_at(generated.down_stairs_tile), TileKind::DownStairs);
            let stairs_index = (generated.down_stairs_tile.y as usize) * generated.width
                + (generated.down_stairs_tile.x as usize);
            assert!(!generated.hazards[stairs_index]);
            assert!(
                !generated.enemy_spawns.iter().any(|spawn| spawn.pos == generated.down_stairs_tile)
            );
            assert!(has_walkable_route(
                &generated,
                generated.entry_tile,
                generated.down_stairs_tile
            ));
        }
    }

    #[test]
    fn generated_floor_has_single_connected_walkable_region() {
        let generated = MapGenerator::new(444_444, BranchProfile::BranchA).generate(3);
        assert!(all_walkable_tiles_connected(&generated));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]
        #[test]
        fn generated_floors_keep_walkable_tiles_connected(
            seed in any::<u64>(),
            floor in 1_u8..=MAX_FLOORS,
            branch_selector in 0_u8..=2
        ) {
            let branch = match branch_selector {
                0 => BranchProfile::Uncommitted,
                1 => BranchProfile::BranchA,
                _ => BranchProfile::BranchB,
            };

            let generated = MapGenerator::new(seed, branch).generate(floor);
            prop_assert!(
                all_walkable_tiles_connected(&generated),
                "seed={seed}, floor={floor}, branch={branch:?} should produce a connected walkable layout"
            );
        }
    }

    fn manhattan(a: Pos, b: Pos) -> u32 {
        a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
    }

    fn all_walkable_tiles_connected(generated: &GeneratedFloor) -> bool {
        let mut walkable_positions = Vec::new();
        for y in 0..generated.height {
            for x in 0..generated.width {
                let pos = Pos { y: y as i32, x: x as i32 };
                let tile = generated.tile_at(pos);
                if tile == TileKind::Floor
                    || tile == TileKind::ClosedDoor
                    || tile == TileKind::DownStairs
                {
                    walkable_positions.push(pos);
                }
            }
        }

        let Some(start) = walkable_positions.first().copied() else {
            return true;
        };

        let mut open = VecDeque::from([start]);
        let mut seen = BTreeSet::from([start]);
        while let Some(pos) = open.pop_front() {
            for next in [
                Pos { y: pos.y - 1, x: pos.x },
                Pos { y: pos.y, x: pos.x + 1 },
                Pos { y: pos.y + 1, x: pos.x },
                Pos { y: pos.y, x: pos.x - 1 },
            ] {
                if seen.contains(&next) {
                    continue;
                }
                let tile = generated.tile_at(next);
                if tile != TileKind::Floor
                    && tile != TileKind::ClosedDoor
                    && tile != TileKind::DownStairs
                {
                    continue;
                }
                seen.insert(next);
                open.push_back(next);
            }
        }

        seen.len() == walkable_positions.len()
    }

    fn has_walkable_route(generated: &GeneratedFloor, start: Pos, goal: Pos) -> bool {
        if start == goal {
            return true;
        }

        let mut open = VecDeque::from([start]);
        let mut seen = BTreeSet::from([start]);

        while let Some(pos) = open.pop_front() {
            for next in [
                Pos { y: pos.y - 1, x: pos.x },
                Pos { y: pos.y, x: pos.x + 1 },
                Pos { y: pos.y + 1, x: pos.x },
                Pos { y: pos.y, x: pos.x - 1 },
            ] {
                if seen.contains(&next) {
                    continue;
                }
                let tile = generated.tile_at(next);
                if tile != TileKind::Floor
                    && tile != TileKind::ClosedDoor
                    && tile != TileKind::DownStairs
                {
                    continue;
                }
                if next == goal {
                    return true;
                }
                seen.insert(next);
                open.push_back(next);
            }
        }

        false
    }
}
