use super::{map_cell_index, resolve_cell_render};
use core::{Map, Pos};
use macroquad::prelude::{GRAY, LIGHTGRAY, RED, YELLOW};

#[test]
fn actor_overrides_item_and_tile() {
    let mut map = Map::new(3, 3);
    let position = Pos { y: 1, x: 1 };
    map.reveal(position);
    map.set_visible(position, true);
    let index = map_cell_index(&map, position).expect("position should be in bounds");
    let mut item_overlay = vec![None; map.internal_width * map.internal_height];
    let mut actor_overlay = vec![None; map.internal_width * map.internal_height];
    item_overlay[index] = Some(("!", YELLOW));
    actor_overlay[index] = Some(("@", RED));

    let rendered = resolve_cell_render(&map, position, &item_overlay, &actor_overlay);
    assert_eq!(rendered, ("@", RED));
}

#[test]
fn item_overrides_tile_when_visible() {
    let mut map = Map::new(3, 3);
    let position = Pos { y: 1, x: 1 };
    map.reveal(position);
    map.set_visible(position, true);
    let index = map_cell_index(&map, position).expect("position should be in bounds");
    let mut item_overlay = vec![None; map.internal_width * map.internal_height];
    let actor_overlay = vec![None; map.internal_width * map.internal_height];
    item_overlay[index] = Some(("!", YELLOW));

    let rendered = resolve_cell_render(&map, position, &item_overlay, &actor_overlay);
    assert_eq!(rendered, ("!", YELLOW));
}

#[test]
fn undiscovered_cell_remains_hidden_even_if_overlay_exists() {
    let map = Map::new(3, 3);
    let position = Pos { y: 1, x: 1 };
    let index = map_cell_index(&map, position).expect("position should be in bounds");
    let mut item_overlay = vec![None; map.internal_width * map.internal_height];
    let mut actor_overlay = vec![None; map.internal_width * map.internal_height];
    item_overlay[index] = Some(("!", YELLOW));
    actor_overlay[index] = Some(("g", RED));

    let rendered = resolve_cell_render(&map, position, &item_overlay, &actor_overlay);
    assert_eq!(rendered, (" ", LIGHTGRAY));
}

#[test]
fn hidden_discovered_cell_uses_dim_tile_without_entity_overlay() {
    let mut map = Map::new(3, 3);
    let position = Pos { y: 1, x: 1 };
    map.reveal(position);
    let item_overlay = vec![None; map.internal_width * map.internal_height];
    let actor_overlay = vec![None; map.internal_width * map.internal_height];

    let rendered = resolve_cell_render(&map, position, &item_overlay, &actor_overlay);
    assert_eq!(rendered, (".", GRAY));
}
