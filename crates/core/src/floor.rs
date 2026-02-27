use crate::types::{ActorKind, Pos, TileKind};

pub const MAX_FLOORS: u8 = 3;
pub const STARTING_FLOOR_INDEX: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BranchProfile {
    Uncommitted,
    BranchA,
    BranchB,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnemySpawn {
    pub kind: ActorKind,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSpawn {
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedFloor {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<TileKind>,
    pub hazards: Vec<bool>,
    pub entry_tile: Pos,
    pub down_stairs_tile: Pos,
    pub enemy_spawns: Vec<EnemySpawn>,
    pub item_spawns: Vec<ItemSpawn>,
}

impl GeneratedFloor {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend((self.width as u32).to_le_bytes());
        bytes.extend((self.height as u32).to_le_bytes());
        for tile in &self.tiles {
            bytes.push(match tile {
                TileKind::Wall => 0,
                TileKind::Floor => 1,
                TileKind::ClosedDoor => 2,
                TileKind::DownStairs => 3,
            });
        }
        for &h in &self.hazards {
            bytes.push(u8::from(h));
        }
        bytes.extend(self.entry_tile.y.to_le_bytes());
        bytes.extend(self.entry_tile.x.to_le_bytes());
        bytes.extend(self.down_stairs_tile.y.to_le_bytes());
        bytes.extend(self.down_stairs_tile.x.to_le_bytes());

        bytes.extend((self.enemy_spawns.len() as u32).to_le_bytes());
        for spawn in &self.enemy_spawns {
            bytes.push(match spawn.kind {
                ActorKind::Player => 0,
                ActorKind::Goblin => 1,
            });
            bytes.extend(spawn.pos.y.to_le_bytes());
            bytes.extend(spawn.pos.x.to_le_bytes());
        }

        bytes.extend((self.item_spawns.len() as u32).to_le_bytes());
        for spawn in &self.item_spawns {
            bytes.extend(spawn.pos.y.to_le_bytes());
            bytes.extend(spawn.pos.x.to_le_bytes());
        }

        bytes
    }

    pub fn tile_at(&self, pos: Pos) -> TileKind {
        if pos.x < 0 || pos.y < 0 {
            return TileKind::Wall;
        }
        let x = pos.x as usize;
        let y = pos.y as usize;
        if x >= self.width || y >= self.height {
            return TileKind::Wall;
        }
        self.tiles[y * self.width + x]
    }
}

pub fn generate_floor(
    run_seed: u64,
    floor_index: u8,
    branch_profile: BranchProfile,
) -> GeneratedFloor {
    let width = 20usize;
    let height = 15usize;
    let mut tiles = vec![TileKind::Wall; width * height];

    let floor_seed = derive_floor_seed(run_seed, floor_index, branch_profile);

    // Keep deterministic boundaries as walls and carve a guaranteed tunnel from entry to stairs.
    let tunnel_y = 2 + ((floor_seed as usize) % (height.saturating_sub(4)));
    let entry_tile = Pos { y: tunnel_y as i32, x: 1 };
    let down_stairs_tile = Pos { y: tunnel_y as i32, x: (width - 2) as i32 };

    for x in 1..(width - 1) {
        tiles[tunnel_y * width + x] = TileKind::Floor;
    }
    tiles[(down_stairs_tile.y as usize) * width + (down_stairs_tile.x as usize)] =
        TileKind::DownStairs;

    // Add deterministic side pockets to avoid every floor feeling identical.
    let pocket_count = 3 + (floor_index as usize % 2);
    for pocket_index in 0..pocket_count {
        let x = 3 + (((floor_seed >> (pocket_index * 9)) as usize) % (width - 6));
        let y = 2 + (((floor_seed >> (pocket_index * 5 + 3)) as usize) % (height - 4));
        for dy in 0..=1 {
            for dx in 0..=1 {
                let px = (x + dx).min(width - 2);
                let py = (y + dy).min(height - 2);
                tiles[py * width + px] = TileKind::Floor;
            }
        }
        // Ensure each pocket connects to the tunnel.
        let min_y = tunnel_y.min(y);
        let max_y = tunnel_y.max(y);
        for py in min_y..=max_y {
            tiles[py * width + x] = TileKind::Floor;
        }
    }

    // Branch A bonus: +1 enemy spawn attempt on floors after the starting floor.
    let branch_enemy_bonus = match branch_profile {
        BranchProfile::BranchA if floor_index > STARTING_FLOOR_INDEX => 1,
        _ => 0,
    };
    let enemy_count = 2 + ((floor_index as usize).min(2)) + branch_enemy_bonus;
    let mut enemy_spawns = Vec::with_capacity(enemy_count);
    for enemy_index in 0..enemy_count {
        let x = 2 + (((floor_seed >> (enemy_index * 7 + 11)) as usize) % (width - 4));
        let y = 2 + (((floor_seed >> (enemy_index * 11 + 17)) as usize) % (height - 4));
        let pos =
            nearest_walkable_floor_tile(&tiles, width, height, Pos { y: y as i32, x: x as i32 });
        if pos != entry_tile
            && pos != down_stairs_tile
            && !enemy_spawns.iter().any(|spawn: &EnemySpawn| spawn.pos == pos)
        {
            enemy_spawns.push(EnemySpawn { kind: ActorKind::Goblin, pos });
        }
    }
    if enemy_spawns.len() < enemy_count {
        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                if enemy_spawns.len() >= enemy_count {
                    break;
                }
                let pos = Pos { y: y as i32, x: x as i32 };
                let tile = tiles[y * width + x];
                if tile != TileKind::Floor {
                    continue;
                }
                if pos == entry_tile
                    || pos == down_stairs_tile
                    || enemy_spawns.iter().any(|spawn| spawn.pos == pos)
                {
                    continue;
                }
                enemy_spawns.push(EnemySpawn { kind: ActorKind::Goblin, pos });
            }
        }
    }

    let mut item_spawns = Vec::new();
    let item_target = Pos {
        y: (2 + ((floor_seed as usize >> 6) % (height - 4))) as i32,
        x: (2 + ((floor_seed as usize >> 2) % (width - 4))) as i32,
    };
    let item_pos = nearest_walkable_floor_tile(&tiles, width, height, item_target);
    if item_pos != entry_tile && item_pos != down_stairs_tile {
        item_spawns.push(ItemSpawn { pos: item_pos });
    }

    // Branch B bonus: +3 hazard tiles on floors after the starting floor.
    let mut hazards = vec![false; width * height];
    let branch_hazard_count = match branch_profile {
        BranchProfile::BranchB if floor_index > STARTING_FLOOR_INDEX => 3,
        _ => 0,
    };
    for hazard_index in 0..branch_hazard_count {
        let hx = 2 + (((floor_seed >> (hazard_index * 13 + 23)) as usize) % (width - 4));
        let hy = 2 + (((floor_seed >> (hazard_index * 9 + 29)) as usize) % (height - 4));
        let pos =
            nearest_walkable_floor_tile(&tiles, width, height, Pos { y: hy as i32, x: hx as i32 });
        if pos != entry_tile && pos != down_stairs_tile {
            hazards[(pos.y as usize) * width + (pos.x as usize)] = true;
        }
    }

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

fn derive_floor_seed(run_seed: u64, floor_index: u8, branch_profile: BranchProfile) -> u64 {
    let branch_code = match branch_profile {
        BranchProfile::Uncommitted => 0_u64,
        BranchProfile::BranchA => 1_u64,
        BranchProfile::BranchB => 2_u64,
    };

    let mut mixed = run_seed ^ 0x9E37_79B9_7F4A_7C15;
    mixed ^= (floor_index as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed ^= branch_code.wrapping_mul(0x94D0_49BB_1331_11EB);
    // Final integer-only avalanche.
    mixed ^= mixed >> 30;
    mixed = mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

fn nearest_walkable_floor_tile(
    tiles: &[TileKind],
    width: usize,
    height: usize,
    desired: Pos,
) -> Pos {
    if in_bounds(width, height, desired) && tile_at(tiles, width, desired) == TileKind::Floor {
        return desired;
    }

    let mut best = Pos { y: 1, x: 1 };
    let mut best_distance = u32::MAX;
    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let pos = Pos { y: y as i32, x: x as i32 };
            if tile_at(tiles, width, pos) != TileKind::Floor
                && tile_at(tiles, width, pos) != TileKind::DownStairs
            {
                continue;
            }
            let distance = pos.x.abs_diff(desired.x) + pos.y.abs_diff(desired.y);
            if distance < best_distance
                || (distance == best_distance && (pos.y, pos.x) < (best.y, best.x))
            {
                best = pos;
                best_distance = distance;
            }
        }
    }
    best
}

fn in_bounds(width: usize, height: usize, pos: Pos) -> bool {
    pos.x >= 0 && pos.y >= 0 && (pos.x as usize) < width && (pos.y as usize) < height
}

fn tile_at(tiles: &[TileKind], width: usize, pos: Pos) -> TileKind {
    tiles[(pos.y as usize) * width + (pos.x as usize)]
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, VecDeque};

    use super::*;

    #[test]
    fn same_inputs_produce_byte_identical_floor_output() {
        let a = generate_floor(123_456, 2, BranchProfile::BranchA);
        let b = generate_floor(123_456, 2, BranchProfile::BranchA);
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    }

    #[test]
    fn changing_floor_index_changes_floor_output_for_same_seed_and_branch() {
        let floor_1 = generate_floor(123_456, 1, BranchProfile::Uncommitted);
        let floor_2 = generate_floor(123_456, 2, BranchProfile::Uncommitted);
        assert_ne!(floor_1.canonical_bytes(), floor_2.canonical_bytes());
    }

    #[test]
    fn same_seed_and_branch_produce_identical_floors_for_floor_two_and_three() {
        let floor_2_a = generate_floor(88_001, 2, BranchProfile::BranchA);
        let floor_2_b = generate_floor(88_001, 2, BranchProfile::BranchA);
        let floor_3_a = generate_floor(88_001, 3, BranchProfile::BranchA);
        let floor_3_b = generate_floor(88_001, 3, BranchProfile::BranchA);

        assert_eq!(floor_2_a.canonical_bytes(), floor_2_b.canonical_bytes());
        assert_eq!(floor_3_a.canonical_bytes(), floor_3_b.canonical_bytes());
    }

    #[test]
    fn different_branches_change_floor_two_and_three_characteristics() {
        let floor_2_a = generate_floor(77_777, 2, BranchProfile::BranchA);
        let floor_2_b = generate_floor(77_777, 2, BranchProfile::BranchB);
        let floor_3_a = generate_floor(77_777, 3, BranchProfile::BranchA);
        let floor_3_b = generate_floor(77_777, 3, BranchProfile::BranchB);

        let floor_2_a_hazards = floor_2_a.hazards.iter().filter(|&&h| h).count();
        let floor_2_b_hazards = floor_2_b.hazards.iter().filter(|&&h| h).count();
        let floor_3_a_hazards = floor_3_a.hazards.iter().filter(|&&h| h).count();
        let floor_3_b_hazards = floor_3_b.hazards.iter().filter(|&&h| h).count();

        assert!(
            floor_2_a.enemy_spawns.len() > floor_2_b.enemy_spawns.len(),
            "Branch A should increase floor 2 enemy density"
        );
        assert!(
            floor_3_a.enemy_spawns.len() > floor_3_b.enemy_spawns.len(),
            "Branch A should increase floor 3 enemy density"
        );
        assert!(
            floor_2_b_hazards > floor_2_a_hazards,
            "Branch B should increase floor 2 hazard density"
        );
        assert!(
            floor_3_b_hazards > floor_3_a_hazards,
            "Branch B should increase floor 3 hazard density"
        );
    }

    #[test]
    fn generated_floor_has_walkable_route_from_entry_to_stairs() {
        let generated = generate_floor(987_654, 3, BranchProfile::BranchB);
        assert!(
            has_walkable_route(&generated, generated.entry_tile, generated.down_stairs_tile),
            "generated floor should always have a walkable route from entry to stairs"
        );
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
