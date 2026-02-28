//! Rendering for the main game frame and UI panels.

use crate::game_layout::{FrameLayout, PanelRect};
use crate::ui_text::{event_log_line, finished_recap_lines, status_text};
use app::app_loop::{AppMode, AppState};
use app::format_snapshot_hash;
use core::{ActorKind, Game, GameState, Map, Pos, TileKind};
use macroquad::prelude::*;

const BORDER_COLOR: Color = Color { r: 0.2, g: 0.2, b: 0.2, a: 1.0 };
const BORDER_THICKNESS: f32 = 1.0;
const PANEL_PAD_X: f32 = 15.0;
const PANEL_PAD_Y: f32 = 25.0;
const LINE_HEIGHT: f32 = 18.0;
type OverlayCell = (&'static str, Color);

pub fn draw_frame(game: &Game, app_state: &AppState, run_seed: u64, layout: &FrameLayout) {
    draw_panel_borders(layout);
    draw_ascii_map(game, layout.map);
    draw_event_log(game, layout.event_log);
    draw_status_panel(app_state, layout.status);
    draw_stats_panel(game, app_state, run_seed, layout.stats);
    draw_policy_panel(game, app_state, layout.policy);
    draw_threat_panel(game, layout.threat);
}

fn draw_panel_borders(layout: &FrameLayout) {
    for panel in
        [layout.status, layout.map, layout.stats, layout.policy, layout.threat, layout.event_log]
    {
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.width,
            panel.height,
            BORDER_THICKNESS,
            BORDER_COLOR,
        );
    }
}

fn draw_status_panel(app_state: &AppState, panel: PanelRect) {
    let status = status_text(&app_state.mode);
    draw_text(&status, panel.x + PANEL_PAD_X, panel.y + PANEL_PAD_Y, 20.0, WHITE);
}

fn draw_stats_panel(game: &Game, app_state: &AppState, run_seed: u64, panel: PanelRect) {
    let text_x = panel.x + PANEL_PAD_X;
    let mut text_y = panel.y + PANEL_PAD_Y;

    if let AppMode::Finished(completion) = &app_state.mode {
        for line in finished_recap_lines(game, run_seed, completion) {
            draw_text(&line, text_x, text_y, 20.0, WHITE);
            text_y += 20.0;
        }
        return;
    }

    draw_text(&format!("Tick: {}", game.current_tick()), text_x, text_y, 20.0, WHITE);
    text_y += 20.0;
    draw_text(&format!("Seed: {run_seed}"), text_x, text_y, 20.0, WHITE);
    text_y += 20.0;
    draw_text(&format!("Floor: {} / 5", game.state().floor_index), text_x, text_y, 20.0, WHITE);
    text_y += 20.0;
    draw_text(&format!("Branch: {:?}", game.state().branch_profile), text_x, text_y, 20.0, WHITE);
    text_y += 20.0;
    draw_text(&format!("God: {:?}", game.state().active_god), text_x, text_y, 20.0, WHITE);
    text_y += 20.0;
    draw_text(
        &format!("Hash: {}", format_snapshot_hash(game.snapshot_hash())),
        text_x,
        text_y,
        20.0,
        WHITE,
    );
    text_y += 20.0;

    let intent_text = if let Some(intent) = game.state().auto_intent {
        format!(
            "Intent: {:?} target=({}, {}) path_len={}",
            intent.reason, intent.target.x, intent.target.y, intent.path_len
        )
    } else {
        "Intent: none".to_string()
    };
    draw_text(&intent_text, text_x, text_y, 20.0, WHITE);
}

fn draw_policy_panel(game: &Game, app_state: &AppState, panel: PanelRect) {
    let text_x = panel.x + PANEL_PAD_X;
    let mut text_y = panel.y + PANEL_PAD_Y;
    let policy = &game.state().policy;

    if matches!(app_state.mode, AppMode::Finished(_)) {
        draw_text("Policy: run ended", text_x, text_y, 20.0, YELLOW);
        return;
    }

    draw_text("Policy:", text_x, text_y, 20.0, YELLOW);
    text_y += 20.0;
    draw_text(&format!("[M]ode: {:?}", policy.fight_or_avoid), text_x, text_y, 18.0, LIGHTGRAY);
    text_y += 20.0;
    draw_text(&format!("s[T]ance: {:?}", policy.stance), text_x, text_y, 18.0, LIGHTGRAY);
    text_y += 20.0;
    draw_text(
        &format!("[P]riority: {:?}", policy.target_priority),
        text_x,
        text_y,
        18.0,
        LIGHTGRAY,
    );
    text_y += 20.0;
    draw_text(
        &format!("[R]etreat HP: {}%", policy.retreat_hp_threshold),
        text_x,
        text_y,
        18.0,
        LIGHTGRAY,
    );
    text_y += 20.0;
    draw_text(
        &format!("[H]eal: {:?}", policy.auto_heal_if_below_threshold),
        text_x,
        text_y,
        18.0,
        LIGHTGRAY,
    );
    text_y += 20.0;
    draw_text(&format!("[I]ntent: {:?}", policy.position_intent), text_x, text_y, 18.0, LIGHTGRAY);
    text_y += 20.0;
    draw_text(
        &format!("[E]xplore: {:?}", policy.exploration_mode),
        text_x,
        text_y,
        18.0,
        LIGHTGRAY,
    );
    text_y += 20.0;
    draw_text(
        &format!("[G]reed: {:?}", policy.resource_aggression),
        text_x,
        text_y,
        18.0,
        LIGHTGRAY,
    );
}

fn draw_threat_panel(game: &Game, panel: PanelRect) {
    let text_x = panel.x + PANEL_PAD_X;
    let mut text_y = panel.y + PANEL_PAD_Y;

    draw_text("Threat Trace:", text_x, text_y, 20.0, RED);
    text_y += 20.0;

    for (index, trace) in game.state().threat_trace.iter().take(5).enumerate() {
        let description = format!(
            "T{}: {} vis, dist {:?}",
            trace.tick, trace.visible_enemy_count, trace.min_enemy_distance
        );
        let color = if trace.retreat_triggered { ORANGE } else { LIGHTGRAY };
        draw_text(&description, text_x, text_y + index as f32 * 20.0, 18.0, color);
    }
}

fn draw_ascii_map(game: &Game, panel: PanelRect) {
    let state = game.state();
    let map = &state.map;
    let item_overlay = build_item_overlay(state);
    let actor_overlay = build_actor_overlay(state);

    for y in 0..map.internal_height {
        for x in 0..map.internal_width {
            let position = Pos { x: x as i32, y: y as i32 };
            let discovered = map.is_discovered(position);
            let (glyph, final_color) =
                resolve_cell_render(map, position, &item_overlay, &actor_overlay);
            let text_x = if discovered {
                panel.x + PANEL_PAD_X + x as f32 * 11.0
            } else {
                panel.x + PANEL_PAD_X + x as f32 * 10.0
            };

            draw_text(glyph, text_x, panel.y + 20.0 + y as f32 * LINE_HEIGHT, 22.0, final_color);
        }
    }
}

fn build_item_overlay(state: &GameState) -> Vec<Option<OverlayCell>> {
    let map = &state.map;
    let mut overlay = vec![None; map.internal_width * map.internal_height];
    let mut visible_items: Vec<_> =
        state.items.values().filter(|item| map.is_visible(item.pos)).collect();
    visible_items.sort_by_key(|item| (item.pos.y, item.pos.x, item.kind));

    for item in visible_items {
        if let Some(index) = map_cell_index(map, item.pos) {
            overlay[index] = Some(("!", YELLOW));
        }
    }

    overlay
}

fn build_actor_overlay(state: &GameState) -> Vec<Option<OverlayCell>> {
    let map = &state.map;
    let mut overlay = vec![None; map.internal_width * map.internal_height];
    let mut visible_actors: Vec<_> =
        state.actors.values().filter(|actor| map.is_visible(actor.pos)).collect();
    visible_actors.sort_by_key(|actor| (actor.pos.y, actor.pos.x, actor.kind));

    for actor in visible_actors {
        if let Some(index) = map_cell_index(map, actor.pos) {
            overlay[index] = Some(actor_glyph_and_color(actor.kind));
        }
    }

    overlay
}

fn map_cell_index(map: &Map, position: Pos) -> Option<usize> {
    if map.in_bounds(position) {
        Some((position.y as usize) * map.internal_width + (position.x as usize))
    } else {
        None
    }
}

fn resolve_cell_render(
    map: &Map,
    position: Pos,
    item_overlay: &[Option<OverlayCell>],
    actor_overlay: &[Option<OverlayCell>],
) -> OverlayCell {
    if !map.is_discovered(position) {
        return (" ", LIGHTGRAY);
    }

    let mut glyph = tile_glyph(map.tile_at(position));
    let mut final_color = if map.is_visible(position) { WHITE } else { GRAY };

    if map.is_visible(position)
        && let Some(index) = map_cell_index(map, position)
    {
        if let Some((item_glyph, item_color)) = item_overlay.get(index).and_then(|entry| *entry) {
            glyph = item_glyph;
            final_color = item_color;
        }
        if let Some((actor_glyph, actor_color)) = actor_overlay.get(index).and_then(|entry| *entry)
        {
            glyph = actor_glyph;
            final_color = actor_color;
        }
    }

    (glyph, final_color)
}

fn draw_event_log(game: &Game, panel: PanelRect) {
    draw_text("Event log", panel.x + PANEL_PAD_X, panel.y + 20.0, 24.0, YELLOW);
    let events = game.log();
    let start = events.len().saturating_sub(10);

    for (index, event) in events[start..].iter().enumerate() {
        let line = event_log_line(event);
        draw_text(
            &line,
            panel.x + PANEL_PAD_X,
            panel.y + 20.0 + (index as f32 + 1.0) * LINE_HEIGHT,
            18.0,
            LIGHTGRAY,
        );
    }
}

fn tile_glyph(tile: TileKind) -> &'static str {
    match tile {
        TileKind::Wall => "#",
        TileKind::Floor => ".",
        TileKind::ClosedDoor => "+",
        TileKind::DownStairs => ">",
    }
}

fn actor_glyph_and_color(kind: ActorKind) -> (&'static str, Color) {
    match kind {
        ActorKind::Player => ("@", GREEN),
        ActorKind::Goblin => ("g", RED),
        ActorKind::FeralHound => ("h", ORANGE),
        ActorKind::BloodAcolyte => ("a", RED),
        ActorKind::CorruptedGuard => ("C", BLUE),
        ActorKind::LivingArmor => ("A", LIGHTGRAY),
        ActorKind::Gargoyle => ("G", GRAY),
        ActorKind::ShadowStalker => ("S", PURPLE),
        ActorKind::AbyssalWarden => ("W", MAGENTA),
    }
}

#[cfg(test)]
mod tests;
