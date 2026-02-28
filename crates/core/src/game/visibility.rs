//! Field-of-view and line-of-sight calculations for map visibility.
//! This module exists to keep rendering-relevant sight rules deterministic and isolated.
//! It does not own movement planning or encounter policy decisions.

use super::*;
use crate::state::Map;

fn transform_octant(orig: Pos, x: i32, y: i32, oct: u8) -> Pos {
    match oct {
        0 => Pos { y: orig.y - y, x: orig.x + x },
        1 => Pos { y: orig.y - x, x: orig.x + y },
        2 => Pos { y: orig.y - x, x: orig.x - y },
        3 => Pos { y: orig.y - y, x: orig.x - x },
        4 => Pos { y: orig.y + y, x: orig.x - x },
        5 => Pos { y: orig.y + x, x: orig.x - y },
        6 => Pos { y: orig.y + x, x: orig.x + y },
        7 => Pos { y: orig.y + y, x: orig.x + x },
        _ => orig,
    }
}

pub(super) fn compute_fov(map: &mut Map, origin: Pos, range: i32) {
    let prev_discovered = map.discovered.clone();
    map.clear_visible();
    map.set_visible(origin, true);
    for octant in 0..8 {
        scan_octant(map, origin, range, 1, Slope::new(1, 1), Slope::new(0, 1), octant);
    }

    let min_y = (origin.y - range).max(0);
    let max_y = (origin.y + range + 1).min(map.internal_height as i32);
    let min_x = (origin.x - range).max(0);
    let max_x = (origin.x + range + 1).min(map.internal_width as i32);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = Pos { y, x };
            if p == origin || !map.is_visible(p) {
                continue;
            }
            if !has_direct_line_of_sight(map, origin, p) {
                let idx = (y as usize) * map.internal_width + (x as usize);
                map.visible[idx] = false;
                map.discovered[idx] = prev_discovered[idx];
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Slope {
    y: i32,
    x: i32,
}

impl Slope {
    fn new(y: i32, x: i32) -> Self {
        Self { y, x }
    }

    fn greater_or_equal(&self, other: &Slope) -> bool {
        self.y * other.x >= other.y * self.x
    }

    fn greater_than(&self, other: &Slope) -> bool {
        self.y * other.x > other.y * self.x
    }
}

fn scan_octant(map: &mut Map, orig: Pos, range: i32, dist: i32, start: Slope, end: Slope, oct: u8) {
    if dist > range {
        return;
    }
    let range_u = u32::try_from(range).expect("FOV range must be non-negative");
    let mut blocked = false;
    let mut cur_start = start;
    for y in (0..=dist).rev() {
        let top = Slope::new(2 * y + 1, 2 * dist - 1);
        let bot = Slope::new(2 * y - 1, 2 * dist + 1);
        if cur_start.greater_or_equal(&bot) && top.greater_than(&end) {
            let p = transform_octant(orig, dist, y, oct);
            if manhattan(orig, p) <= range_u {
                map.set_visible(p, true);
            }
            let opaque = map.tile_at(p) == TileKind::Wall || map.tile_at(p) == TileKind::ClosedDoor;
            if opaque {
                if !blocked {
                    scan_octant(map, orig, range, dist + 1, cur_start, top, oct);
                    blocked = true;
                }
                cur_start = bot;
            } else if blocked {
                blocked = false;
            }
        }
    }
    if !blocked {
        scan_octant(map, orig, range, dist + 1, cur_start, end, oct);
    }
}

fn has_direct_line_of_sight(map: &Map, origin: Pos, target: Pos) -> bool {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let sx = dx.signum();
    let sy = dy.signum();
    let total_dist_x = dx.abs();
    let total_dist_y = dy.abs();

    let mut x = origin.x;
    let mut y = origin.y;
    let mut current_step_x = 0;
    let mut current_step_y = 0;

    while current_step_x < total_dist_x || current_step_y < total_dist_y {
        let lhs = (1 + 2 * current_step_x) * total_dist_y;
        let rhs = (1 + 2 * current_step_y) * total_dist_x;

        if lhs == rhs {
            x += sx;
            y += sy;
            current_step_x += 1;
            current_step_y += 1;
        } else if lhs < rhs {
            x += sx;
            current_step_x += 1;
        } else {
            y += sy;
            current_step_y += 1;
        }

        if x == target.x && y == target.y {
            break;
        }
        let tile = map.tile_at(Pos { y, x });
        if tile == TileKind::Wall || tile == TileKind::ClosedDoor {
            return false;
        }
    }
    true
}

#[allow(dead_code)]
pub(super) fn draw_map_diag(map: &Map, player: Pos) -> String {
    let mut text = String::new();
    for y in 0..map.internal_height {
        for x in 0..map.internal_width {
            let p = Pos { y: y as i32, x: x as i32 };
            let c = if p == player {
                '@'
            } else if map.tile_at(p) == TileKind::Wall {
                '#'
            } else if map.tile_at(p) == TileKind::ClosedDoor {
                '+'
            } else if map.tile_at(p) == TileKind::DownStairs {
                '>'
            } else if super::auto_explore::is_safe_frontier_candidate(map, p) {
                'F'
            } else {
                '.'
            };
            let v = if map.is_visible(p) { 'v' } else { 'h' };
            let d = if map.is_discovered(p) { 'd' } else { 'u' };
            text.push_str(&format!("{c}{v}{d} "));
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::content::ContentPack;
    use crate::game::test_support::*;
    use crate::*;

    #[test]
    fn fov_open_room_visibility() {
        let (mut map, origin) = open_room_fixture();
        compute_fov(&mut map, origin, 3);
        assert!(map.is_visible(origin));
        assert!(map.is_visible(Pos { y: 5, x: 8 }));
        assert!(!map.is_visible(Pos { y: 1, x: 1 }));
    }

    #[test]
    fn fov_repeat_is_deterministic_for_same_state() {
        let (mut map, origin) = open_room_fixture();
        map.set_tile(Pos { y: 5, x: 7 }, TileKind::Wall);
        map.set_tile(Pos { y: 6, x: 7 }, TileKind::Wall);

        compute_fov(&mut map, origin, FOV_RADIUS);
        let first = map.visible.clone();
        compute_fov(&mut map, origin, FOV_RADIUS);
        let second = map.visible.clone();

        assert_eq!(first, second, "FOV result must be identical for same map/origin");
    }

    #[test]
    fn fov_wall_occlusion_blocks_tiles_behind_wall_in_corridor() {
        let (mut map, origin) = wall_occlusion_fixture();
        compute_fov(&mut map, origin, 10);

        assert!(map.is_visible(Pos { y: 5, x: 5 }));
        assert!(map.is_visible(Pos { y: 5, x: 6 }));
        assert!(
            !map.is_visible(Pos { y: 5, x: 7 }),
            "tile directly behind corridor wall should be occluded"
        );
    }

    #[test]
    fn fov_does_not_leak_through_corners() {
        let mut map = Map::new(20, 20);
        // Create a 5x5 interior room from (5,5) to (9,9)
        // Walls at (4,4) to (10,10)
        let r_start = 4;
        let r_end = 10;
        for y in r_start..=r_end {
            for x in r_start..=r_end {
                if y == r_start || y == r_end || x == r_start || x == r_end {
                    map.set_tile(Pos { y, x }, TileKind::Wall);
                } else {
                    map.set_tile(Pos { y, x }, TileKind::Floor);
                }
            }
        }

        // Test with player at various positions inside the room
        for py in (r_start + 1)..r_end {
            for px in (r_start + 1)..r_end {
                let origin = Pos { y: py, x: px };
                compute_fov(&mut map, origin, 15);
                for y in 0..map.internal_height {
                    for x in 0..map.internal_width {
                        let p = Pos { y: y as i32, x: x as i32 };
                        if (p.y < r_start || p.y > r_end || p.x < r_start || p.x > r_end)
                            && map.is_visible(p)
                        {
                            panic!(
                                "Light leaked to {p:?} from origin {origin:?}\n{}",
                                draw_map_diag(&map, origin)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fov_fixture_does_not_show_corner_handles() {
        let (mut map, origin) = corner_handle_fixture();
        compute_fov(&mut map, origin, FOV_RADIUS);

        // Cells that can be incorrectly discovered as "corner handles" should stay unknown.
        for p in [Pos { y: 1, x: 0 }, Pos { y: 9, x: 0 }, Pos { y: 1, x: 1 }, Pos { y: 9, x: 1 }] {
            assert!(
                !map.is_visible(p),
                "unexpected corner handle at {p:?}\n{}",
                draw_map_diag(&map, origin)
            );
            assert!(
                !map.is_discovered(p),
                "unexpected discovered corner handle at {p:?}\n{}",
                draw_map_diag(&map, origin)
            );
        }
    }
}
