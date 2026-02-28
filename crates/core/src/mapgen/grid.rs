//! Grid and tile-space primitives used by layout, spawning, and hazard placement.

use crate::types::{Pos, TileKind};

pub(super) fn in_bounds(width: usize, height: usize, pos: Pos) -> bool {
    pos.x >= 0 && pos.y >= 0 && (pos.x as usize) < width && (pos.y as usize) < height
}

pub(super) fn manhattan(a: Pos, b: Pos) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

pub(super) fn tile_at(tiles: &[TileKind], width: usize, pos: Pos) -> TileKind {
    tiles[(pos.y as usize) * width + (pos.x as usize)]
}

pub(super) fn farthest_walkable_tile_from_entry(
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

pub(super) fn nearest_walkable_floor_tile(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_walkable_prefers_lowest_y_then_x_for_tie_breaks() {
        let width = 7;
        let height = 7;
        let mut tiles = vec![TileKind::Wall; width * height];
        tiles[2 * width + 3] = TileKind::Floor;
        tiles[3 * width + 2] = TileKind::Floor;

        let desired = Pos { y: 1, x: 1 };
        let chosen = nearest_walkable_floor_tile(&tiles, width, height, desired);

        assert_eq!(chosen, Pos { y: 2, x: 3 });
    }
}
