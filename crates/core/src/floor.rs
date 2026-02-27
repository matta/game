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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RoomRect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl RoomRect {
    fn right(self) -> usize {
        self.x + self.width - 1
    }

    fn bottom(self) -> usize {
        self.y + self.height - 1
    }

    fn center(self) -> Pos {
        Pos { y: (self.y + (self.height / 2)) as i32, x: (self.x + (self.width / 2)) as i32 }
    }

    fn expanded(self, margin: usize) -> Self {
        let expanded_x = self.x.saturating_sub(margin);
        let expanded_y = self.y.saturating_sub(margin);
        let expanded_right = self.right().saturating_add(margin);
        let expanded_bottom = self.bottom().saturating_add(margin);
        Self {
            x: expanded_x,
            y: expanded_y,
            width: expanded_right - expanded_x + 1,
            height: expanded_bottom - expanded_y + 1,
        }
    }

    fn intersects(self, other: &Self) -> bool {
        self.x <= other.right()
            && self.right() >= other.x
            && self.y <= other.bottom()
            && self.bottom() >= other.y
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RoomLayout {
    rooms: Vec<RoomRect>,
    entry_tile: Pos,
    down_stairs_tile: Pos,
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
    let layout = build_room_layout(floor_seed, width, height);
    for room in &layout.rooms {
        carve_room(&mut tiles, width, room);
    }
    carve_room_corridors(&mut tiles, width, floor_seed, &layout.rooms);
    let entry_tile = nearest_walkable_floor_tile(&tiles, width, height, layout.entry_tile);
    let mut down_stairs_tile =
        nearest_walkable_floor_tile(&tiles, width, height, layout.down_stairs_tile);
    if down_stairs_tile == entry_tile {
        down_stairs_tile = farthest_walkable_tile_from_entry(&tiles, width, height, entry_tile);
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
        if manhattan(pos, entry_tile) > 1
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
                if manhattan(pos, entry_tile) <= 1
                    || pos == down_stairs_tile
                    || enemy_spawns.iter().any(|spawn| spawn.pos == pos)
                {
                    continue;
                }
                enemy_spawns.push(EnemySpawn { kind: ActorKind::Goblin, pos });
            }
        }
    }
    enemy_spawns.sort_by_key(|spawn| (spawn.pos.y, spawn.pos.x, spawn.kind));

    let mut item_spawns = Vec::new();
    let item_target = Pos {
        y: (2 + ((floor_seed as usize >> 6) % (height - 4))) as i32,
        x: (2 + ((floor_seed as usize >> 2) % (width - 4))) as i32,
    };
    let item_pos = nearest_walkable_floor_tile(&tiles, width, height, item_target);
    if item_pos != entry_tile && item_pos != down_stairs_tile {
        item_spawns.push(ItemSpawn { pos: item_pos });
    }
    item_spawns.sort_by_key(|spawn| (spawn.pos.y, spawn.pos.x));

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
    tiles[(down_stairs_tile.y as usize) * width + (down_stairs_tile.x as usize)] =
        TileKind::DownStairs;
    hazards[(down_stairs_tile.y as usize) * width + (down_stairs_tile.x as usize)] = false;

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

fn build_room_layout(floor_seed: u64, width: usize, height: usize) -> RoomLayout {
    let minimum_room_width = 4usize;
    let maximum_room_width = 7usize;
    let minimum_room_height = 3usize;
    let maximum_room_height = 5usize;
    let target_room_count = 5 + random_usize(floor_seed, 1, 0, 2);

    let mut rooms = Vec::new();
    for attempt in 0_u64..120 {
        if rooms.len() >= target_room_count {
            break;
        }
        let room_width =
            random_usize(floor_seed, attempt * 8 + 2, minimum_room_width, maximum_room_width);
        let room_height =
            random_usize(floor_seed, attempt * 8 + 3, minimum_room_height, maximum_room_height);
        if room_width + 2 >= width || room_height + 2 >= height {
            continue;
        }

        let max_x = width - room_width - 1;
        let max_y = height - room_height - 1;
        if max_x <= 1 || max_y <= 1 {
            continue;
        }

        let x = random_usize(floor_seed, attempt * 8 + 4, 1, max_x);
        let y = random_usize(floor_seed, attempt * 8 + 5, 1, max_y);
        let candidate = RoomRect { x, y, width: room_width, height: room_height };
        let candidate_with_margin = candidate.expanded(1);
        if rooms.iter().any(|existing_room: &RoomRect| {
            existing_room.expanded(1).intersects(&candidate_with_margin)
        }) {
            continue;
        }
        rooms.push(candidate);
    }

    add_fallback_rooms(width, height, &mut rooms);
    rooms.sort_by_key(|room| {
        let center = room.center();
        (center.y, center.x, room.height, room.width)
    });

    let entry_tile = rooms.first().map(|room| room.center()).unwrap_or(Pos { y: 1, x: 1 });

    let mut down_stairs_tile = entry_tile;
    let mut best_distance = 0_u32;
    for room in &rooms {
        let center = room.center();
        let distance = manhattan(entry_tile, center);
        if distance > best_distance
            || (distance == best_distance
                && (center.y, center.x) > (down_stairs_tile.y, down_stairs_tile.x))
        {
            down_stairs_tile = center;
            best_distance = distance;
        }
    }

    RoomLayout { rooms, entry_tile, down_stairs_tile }
}

fn add_fallback_rooms(width: usize, height: usize, rooms: &mut Vec<RoomRect>) {
    let fallback_room_width = 4usize;
    let fallback_room_height = 4usize;
    if fallback_room_width + 2 >= width || fallback_room_height + 2 >= height {
        return;
    }

    let fallback_positions = [
        (1usize, 1usize),
        (width - fallback_room_width - 1, 1usize),
        (1usize, height - fallback_room_height - 1),
        (width - fallback_room_width - 1, height - fallback_room_height - 1),
    ];

    for (x, y) in fallback_positions {
        if rooms.len() >= 4 {
            break;
        }
        let candidate = RoomRect { x, y, width: fallback_room_width, height: fallback_room_height };
        let candidate_with_margin = candidate.expanded(1);
        if rooms
            .iter()
            .any(|existing_room| existing_room.expanded(1).intersects(&candidate_with_margin))
        {
            continue;
        }
        rooms.push(candidate);
    }

    if rooms.is_empty() {
        rooms.push(RoomRect {
            x: width / 3,
            y: height / 3,
            width: fallback_room_width.min(width.saturating_sub(2)),
            height: fallback_room_height.min(height.saturating_sub(2)),
        });
    }
}

fn carve_room(tiles: &mut [TileKind], width: usize, room: &RoomRect) {
    for y in room.y..=room.bottom() {
        for x in room.x..=room.right() {
            tiles[y * width + x] = TileKind::Floor;
        }
    }
}

fn carve_room_corridors(tiles: &mut [TileKind], width: usize, floor_seed: u64, rooms: &[RoomRect]) {
    if rooms.len() < 2 {
        return;
    }

    let mut connected_room_indices = vec![0_usize];
    let mut pending_room_indices: Vec<usize> = (1..rooms.len()).collect();

    while !pending_room_indices.is_empty() {
        let mut best_choice: Option<(u32, usize, usize)> = None;
        for &connected_index in &connected_room_indices {
            let connected_center = rooms[connected_index].center();
            for &pending_index in &pending_room_indices {
                let pending_center = rooms[pending_index].center();
                let distance = manhattan(connected_center, pending_center);
                let should_replace = match best_choice {
                    None => true,
                    Some((best_distance, best_connected_index, best_pending_index)) => {
                        (distance, connected_index, pending_index)
                            < (best_distance, best_connected_index, best_pending_index)
                    }
                };
                if should_replace {
                    best_choice = Some((distance, connected_index, pending_index));
                }
            }
        }

        let (_, connected_index, pending_index) = best_choice.expect("pending list is non-empty");
        let connected_center = rooms[connected_index].center();
        let pending_center = rooms[pending_index].center();
        let horizontal_first =
            mix_seed_stream(floor_seed, ((connected_index as u64) << 32) | (pending_index as u64))
                & 1
                == 0;
        carve_l_shaped_corridor(tiles, width, connected_center, pending_center, horizontal_first);

        connected_room_indices.push(pending_index);
        if let Some(position) =
            pending_room_indices.iter().position(|&index| index == pending_index)
        {
            pending_room_indices.remove(position);
        }
    }
}

fn carve_l_shaped_corridor(
    tiles: &mut [TileKind],
    width: usize,
    start: Pos,
    end: Pos,
    horizontal_first: bool,
) {
    if horizontal_first {
        carve_horizontal_line(tiles, width, start.y, start.x, end.x);
        carve_vertical_line(tiles, width, end.x, start.y, end.y);
    } else {
        carve_vertical_line(tiles, width, start.x, start.y, end.y);
        carve_horizontal_line(tiles, width, end.y, start.x, end.x);
    }
}

fn carve_horizontal_line(tiles: &mut [TileKind], width: usize, y: i32, left_x: i32, right_x: i32) {
    let from_x = left_x.min(right_x);
    let to_x = left_x.max(right_x);
    for x in from_x..=to_x {
        let pos = Pos { y, x };
        if pos.x <= 0 || pos.y <= 0 {
            continue;
        }
        let row = pos.y as usize;
        let column = pos.x as usize;
        if column >= width - 1 {
            continue;
        }
        tiles[row * width + column] = TileKind::Floor;
    }
}

fn carve_vertical_line(tiles: &mut [TileKind], width: usize, x: i32, top_y: i32, bottom_y: i32) {
    let from_y = top_y.min(bottom_y);
    let to_y = top_y.max(bottom_y);
    for y in from_y..=to_y {
        let pos = Pos { y, x };
        if pos.x <= 0 || pos.y <= 0 {
            continue;
        }
        let row = pos.y as usize;
        let column = pos.x as usize;
        if column >= width - 1 {
            continue;
        }
        tiles[row * width + column] = TileKind::Floor;
    }
}

fn farthest_walkable_tile_from_entry(
    tiles: &[TileKind],
    width: usize,
    height: usize,
    entry_tile: Pos,
) -> Pos {
    let mut best = entry_tile;
    let mut best_distance = 0_u32;
    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let pos = Pos { y: y as i32, x: x as i32 };
            let tile = tile_at(tiles, width, pos);
            if tile != TileKind::Floor && tile != TileKind::DownStairs {
                continue;
            }
            let distance = manhattan(entry_tile, pos);
            if distance > best_distance
                || (distance == best_distance && (pos.y, pos.x) > (best.y, best.x))
            {
                best = pos;
                best_distance = distance;
            }
        }
    }
    best
}

fn random_usize(seed: u64, stream: u64, min_value: usize, max_value: usize) -> usize {
    debug_assert!(min_value <= max_value);
    let range_size = max_value - min_value + 1;
    min_value + (mix_seed_stream(seed, stream) as usize % range_size)
}

fn mix_seed_stream(seed: u64, stream: u64) -> u64 {
    let mut mixed = seed ^ stream.wrapping_mul(0xD6E8_FD9A_5B89_7A4D);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    mixed ^ (mixed >> 33)
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

fn manhattan(a: Pos, b: Pos) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

fn tile_at(tiles: &[TileKind], width: usize, pos: Pos) -> TileKind {
    tiles[(pos.y as usize) * width + (pos.x as usize)]
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, VecDeque};

    use proptest::prelude::*;

    use super::*;

    #[test]
    fn room_layout_places_multiple_non_overlapping_rooms() {
        let layout = build_room_layout(42, 20, 15);
        assert!(
            layout.rooms.len() >= 4,
            "expected at least four rooms, got {}",
            layout.rooms.len()
        );

        for left_index in 0..layout.rooms.len() {
            for right_index in (left_index + 1)..layout.rooms.len() {
                let left_with_margin = layout.rooms[left_index].expanded(1);
                let right_with_margin = layout.rooms[right_index].expanded(1);
                assert!(
                    !left_with_margin.intersects(&right_with_margin),
                    "rooms must not overlap or touch: {:?} vs {:?}",
                    layout.rooms[left_index],
                    layout.rooms[right_index]
                );
            }
        }
    }

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

    #[test]
    fn sanctuary_spawn_rule_holds_across_multiple_seeds_and_floors() {
        let seeds = [11_u64, 2_024, 77_777, 909_090];
        for seed in seeds {
            for floor in 2..=MAX_FLOORS {
                let generated = generate_floor(seed, floor, BranchProfile::BranchA);
                for spawn in &generated.enemy_spawns {
                    assert!(
                        manhattan(spawn.pos, generated.entry_tile) > 1,
                        "enemy spawn {:?} must not be on or adjacent to sanctuary {:?} (seed={seed}, floor={floor})",
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
            let generated = generate_floor(seed, 2, BranchProfile::BranchB);
            assert_eq!(
                generated.tile_at(generated.down_stairs_tile),
                TileKind::DownStairs,
                "stairs tile type should remain DownStairs"
            );
            let stairs_index = (generated.down_stairs_tile.y as usize) * generated.width
                + (generated.down_stairs_tile.x as usize);
            assert!(!generated.hazards[stairs_index], "stairs tile must not start hazardous");
            assert!(
                !generated.enemy_spawns.iter().any(|spawn| spawn.pos == generated.down_stairs_tile),
                "stairs tile must not start occupied by an enemy"
            );
            assert!(
                has_walkable_route(&generated, generated.entry_tile, generated.down_stairs_tile),
                "stairs must be reachable from entry"
            );
        }
    }

    #[test]
    fn generated_floor_has_single_connected_walkable_region() {
        let generated = generate_floor(444_444, 3, BranchProfile::BranchA);
        assert!(
            all_walkable_tiles_connected(&generated),
            "all walkable tiles should be part of one connected region"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn generated_floors_keep_walkable_tiles_connected(seed in any::<u64>(), floor in 1_u8..=MAX_FLOORS, branch_selector in 0_u8..=2) {
            let branch = match branch_selector {
                0 => BranchProfile::Uncommitted,
                1 => BranchProfile::BranchA,
                _ => BranchProfile::BranchB,
            };

            let generated = generate_floor(seed, floor, branch);
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
