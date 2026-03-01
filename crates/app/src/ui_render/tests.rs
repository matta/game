use super::{fit_lines_to_panel, map_cell_index, resolve_cell_render};
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

#[test]
fn fit_lines_to_panel_keeps_all_lines_when_space_is_sufficient() {
    let lines = vec!["Tick".to_string(), "Floor".to_string(), "HP".to_string()];
    let fitted = fit_lines_to_panel(&lines, 200.0, 15.0, 25.0);
    assert_eq!(fitted, lines);
}

#[test]
fn fit_lines_to_panel_truncates_and_shows_hidden_count() {
    let lines =
        vec!["Tick".to_string(), "Seed".to_string(), "Floor".to_string(), "Branch".to_string()];

    let fitted = fit_lines_to_panel(&lines, 70.0, 20.0, 25.0);
    assert_eq!(fitted.len(), 2);
    assert_eq!(fitted[0], "Tick");
    assert_eq!(fitted[1], "... and 3 more");
}

#[test]
fn fit_lines_to_panel_returns_empty_when_no_vertical_space() {
    let lines = vec!["Tick".to_string(), "Seed".to_string()];
    let fitted = fit_lines_to_panel(&lines, 20.0, 15.0, 25.0);
    assert!(fitted.is_empty());
}
