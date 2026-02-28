//! Room placement and corridor carving logic for base map topology.

use crate::types::{Pos, TileKind};

use super::grid::manhattan;
use super::seed::{mix_seed_stream, random_usize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RoomRect {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl RoomRect {
    fn right(self) -> usize {
        self.x + self.width - 1
    }

    fn bottom(self) -> usize {
        self.y + self.height - 1
    }

    pub(super) fn center(self) -> Pos {
        Pos { y: (self.y + (self.height / 2)) as i32, x: (self.x + (self.width / 2)) as i32 }
    }

    pub(super) fn expanded(self, margin: usize) -> Self {
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

    pub(super) fn intersects(self, other: &Self) -> bool {
        self.x <= other.right()
            && self.right() >= other.x
            && self.y <= other.bottom()
            && self.bottom() >= other.y
    }

    pub(super) fn contains(self, pos: Pos) -> bool {
        let px = pos.x as usize;
        let py = pos.y as usize;
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RoomLayout {
    pub(super) rooms: Vec<RoomRect>,
    pub(super) entry_tile: Pos,
    pub(super) down_stairs_tile: Pos,
}

pub(super) fn build_room_layout(floor_seed: u64, width: usize, height: usize) -> RoomLayout {
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

pub(super) fn carve_room(tiles: &mut [TileKind], width: usize, room: &RoomRect) {
    for y in room.y..=room.bottom() {
        for x in room.x..=room.right() {
            tiles[y * width + x] = TileKind::Floor;
        }
    }
}

pub(super) fn carve_room_corridors(
    tiles: &mut [TileKind],
    width: usize,
    floor_seed: u64,
    rooms: &[RoomRect],
) {
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

#[cfg(test)]
mod tests {
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
}
