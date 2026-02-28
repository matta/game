//! Shared test fixtures for the `game` submodule test suites.
//! This module exists to avoid repeating map and actor setup across many tests.
//! It does not own production gameplay logic.

use super::*;
use crate::state::{Actor, Map};

pub(super) fn open_room_fixture() -> (Map, Pos) {
    let map = Map::new(10, 10);
    let origin = Pos { y: 5, x: 5 };
    (map, origin)
}

pub(super) fn wall_occlusion_fixture() -> (Map, Pos) {
    let mut map = Map::new(11, 11);
    for y in 1..10 {
        for x in 1..10 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 1..10 {
        map.set_tile(Pos { y: 5, x }, TileKind::Floor);
    }
    map.set_tile(Pos { y: 5, x: 6 }, TileKind::Wall);
    (map, Pos { y: 5, x: 3 })
}

pub(super) fn hazard_lane_fixture() -> (Map, Pos) {
    let mut map = Map::new(9, 9);
    for y in 1..8 {
        for x in 1..8 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 2..=5 {
        map.set_tile(Pos { y: 4, x }, TileKind::Floor);
    }
    map.discovered.fill(true);
    map.visible.fill(true);
    (map, Pos { y: 4, x: 2 })
}

pub(super) fn closed_door_choke_fixture() -> (Map, Pos, Pos) {
    let mut map = Map::new(10, 10);
    for x in 0..10 {
        for y in 0..10 {
            map.set_tile(Pos { y, x }, if y == 5 { TileKind::Floor } else { TileKind::Wall });
        }
    }
    map.discovered.fill(true);
    let door = Pos { y: 5, x: 6 };
    map.set_tile(door, TileKind::ClosedDoor);
    map.discovered[5 * 10 + 7] = false;
    (map, Pos { y: 5, x: 5 }, door)
}

pub(super) fn corner_handle_fixture() -> (Map, Pos) {
    let mut map = Map::new(20, 15);
    for y in 1..(map.internal_height - 1) {
        for x in 1..(map.internal_width - 1) {
            map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
        }
    }
    for y in 3..=7 {
        for x in 2..=6 {
            map.set_tile(Pos { y, x }, TileKind::Floor);
        }
    }
    map.set_tile(Pos { y: 5, x: 7 }, TileKind::Floor);
    map.set_tile(Pos { y: 5, x: 8 }, TileKind::ClosedDoor);
    (map, Pos { y: 5, x: 6 })
}

pub(super) fn add_goblin(game: &mut Game, pos: Pos) -> EntityId {
    let enemy = Actor {
        id: EntityId::default(),
        kind: ActorKind::Goblin,
        pos,
        hp: 10,
        max_hp: 10,
        attack: 2,
        defense: 0,
        active_weapon_slot: WeaponSlot::Primary,
        equipped_weapon: None,
        reserve_weapon: None,
        next_action_tick: 12,
        speed: 12,
    };
    let id = game.state.actors.insert(enemy);
    game.state.actors[id].id = id;
    id
}
