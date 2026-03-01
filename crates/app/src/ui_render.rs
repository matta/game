//! Rendering for the main game frame and UI panels.

use crate::game_layout::{FrameLayout, PanelRect};
use crate::ui_text::{event_log_line, finished_recap_lines, stats_panel_lines, status_text};
use app::app_loop::{AppMode, AppState};
use core::{ActorKind, Game, GameState, Map, Pos, TileKind};
use macroquad::prelude::*;

const BORDER_COLOR: Color = Color { r: 0.2, g: 0.2, b: 0.2, a: 1.0 };
const BORDER_THICKNESS: f32 = 1.0;
const PANEL_PAD_X: f32 = 15.0;
const PANEL_PAD_Y: f32 = 25.0;
const LINE_HEIGHT: f32 = 18.0;
const STATS_FONT_SIZE: f32 = 16.0;
const STATS_LINE_STEP: f32 = 15.0;
type OverlayCell = (&'static str, Color);

pub fn draw_frame(
    game: &Game,
    app_state: &AppState,
    run_seed: u64,
    layout: &FrameLayout,
    ui_scale: f32,
) {
    draw_panel_borders(layout, ui_scale);
    draw_ascii_map(game, layout.map, ui_scale);
    draw_event_log(game, layout.event_log, ui_scale);
    draw_status_panel(app_state, layout.status, ui_scale);
    draw_stats_panel(game, app_state, run_seed, layout.stats, ui_scale);
    draw_policy_panel(game, app_state, layout.policy, ui_scale);
    draw_threat_panel(game, layout.threat, ui_scale);
}

fn draw_panel_borders(layout: &FrameLayout, ui_scale: f32) {
    for panel in
        [layout.status, layout.map, layout.stats, layout.policy, layout.threat, layout.event_log]
    {
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.width,
            panel.height,
            scaled(BORDER_THICKNESS, ui_scale),
            BORDER_COLOR,
        );
    }
}

fn draw_status_panel(app_state: &AppState, panel: PanelRect, ui_scale: f32) {
    let status = status_text(&app_state.mode);
    draw_text(
        &status,
        panel.x + scaled(PANEL_PAD_X, ui_scale),
        panel.y + scaled(PANEL_PAD_Y, ui_scale),
        scaled(20.0, ui_scale),
        WHITE,
    );
}

fn draw_stats_panel(
    game: &Game,
    app_state: &AppState,
    run_seed: u64,
    panel: PanelRect,
    ui_scale: f32,
) {
    let text_x = panel.x + scaled(PANEL_PAD_X, ui_scale);
    let mut text_y = panel.y + scaled(PANEL_PAD_Y, ui_scale);

    if let AppMode::Finished(completion) = &app_state.mode {
        for line in finished_recap_lines(game, run_seed, completion) {
            draw_text(&line, text_x, text_y, scaled(20.0, ui_scale), WHITE);
            text_y += scaled(20.0, ui_scale);
        }
        return;
    }

    let raw_lines = stats_panel_lines(game, run_seed);
    let visible_lines = fit_lines_to_panel(
        &raw_lines,
        panel.height,
        scaled(STATS_LINE_STEP, ui_scale),
        scaled(PANEL_PAD_Y, ui_scale),
    );
    for line in visible_lines {
        draw_text(&line, text_x, text_y, scaled(STATS_FONT_SIZE, ui_scale), WHITE);
        text_y += scaled(STATS_LINE_STEP, ui_scale);
    }
}

fn fit_lines_to_panel(
    lines: &[String],
    panel_height: f32,
    line_step: f32,
    panel_pad_y: f32,
) -> Vec<String> {
    if line_step <= 0.0 {
        return Vec::new();
    }

    let usable_height = (panel_height - panel_pad_y).max(0.0);
    let max_lines = (usable_height / line_step).floor() as usize;
    if lines.len() <= max_lines {
        return lines.to_vec();
    }
    if max_lines == 0 {
        return Vec::new();
    }
    if max_lines == 1 {
        return vec![format!("... and {} more", lines.len())];
    }

    let hidden_count = lines.len() - (max_lines - 1);
    let mut fitted_lines = lines[..max_lines - 1].to_vec();
    fitted_lines.push(format!("... and {hidden_count} more"));
    fitted_lines
}

fn draw_policy_panel(game: &Game, app_state: &AppState, panel: PanelRect, ui_scale: f32) {
    let text_x = panel.x + scaled(PANEL_PAD_X, ui_scale);
    let mut text_y = panel.y + scaled(PANEL_PAD_Y, ui_scale);
    let policy = &game.state().policy;

    if matches!(app_state.mode, AppMode::Finished(_)) {
        draw_text("Policy: run ended", text_x, text_y, scaled(20.0, ui_scale), YELLOW);
        return;
    }

    draw_text("Policy:", text_x, text_y, scaled(20.0, ui_scale), YELLOW);
    text_y += scaled(20.0, ui_scale);
    draw_text(
        &format!("[M]ode: {:?}", policy.fight_or_avoid),
        text_x,
        text_y,
        scaled(18.0, ui_scale),
        LIGHTGRAY,
    );
    text_y += scaled(20.0, ui_scale);
    draw_text(
        &format!("s[T]ance: {:?}", policy.stance),
        text_x,
        text_y,
        scaled(18.0, ui_scale),
        LIGHTGRAY,
    );
    text_y += scaled(20.0, ui_scale);
    draw_text(
        &format!("[P]riority: {:?}", policy.target_priority),
        text_x,
        text_y,
        scaled(18.0, ui_scale),
        LIGHTGRAY,
    );
    text_y += scaled(20.0, ui_scale);
    draw_text(
        &format!("[R]etreat HP: {}%", policy.retreat_hp_threshold),
        text_x,
        text_y,
        scaled(18.0, ui_scale),
        LIGHTGRAY,
    );
    text_y += scaled(20.0, ui_scale);
    draw_text(
        &format!("[H]eal: {:?}", policy.auto_heal_if_below_threshold),
        text_x,
        text_y,
        scaled(18.0, ui_scale),
        LIGHTGRAY,
    );
    text_y += scaled(20.0, ui_scale);
    draw_text(
        &format!("[I]ntent: {:?}", policy.position_intent),
        text_x,
        text_y,
        scaled(18.0, ui_scale),
        LIGHTGRAY,
    );
    text_y += scaled(20.0, ui_scale);
    draw_text(
        &format!("[E]xplore: {:?}", policy.exploration_mode),
        text_x,
        text_y,
        scaled(18.0, ui_scale),
        LIGHTGRAY,
    );
    text_y += scaled(20.0, ui_scale);
    draw_text(
        &format!("[G]reed: {:?}", policy.resource_aggression),
        text_x,
        text_y,
        scaled(18.0, ui_scale),
        LIGHTGRAY,
    );
}

fn draw_threat_panel(game: &Game, panel: PanelRect, ui_scale: f32) {
    let text_x = panel.x + scaled(PANEL_PAD_X, ui_scale);
    let mut text_y = panel.y + scaled(PANEL_PAD_Y, ui_scale);

    draw_text("Threat Trace:", text_x, text_y, scaled(20.0, ui_scale), RED);
    text_y += scaled(20.0, ui_scale);

    for (index, trace) in game.state().threat_trace.iter().take(5).enumerate() {
        let description = format!(
            "T{}: {} vis, dist {:?}",
            trace.tick, trace.visible_enemy_count, trace.min_enemy_distance
        );
        let color = if trace.retreat_triggered { ORANGE } else { LIGHTGRAY };
        draw_text(
            &description,
            text_x,
            text_y + index as f32 * scaled(20.0, ui_scale),
            scaled(18.0, ui_scale),
            color,
        );
    }
}

fn draw_ascii_map(game: &Game, panel: PanelRect, ui_scale: f32) {
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
                panel.x + scaled(PANEL_PAD_X, ui_scale) + x as f32 * scaled(11.0, ui_scale)
            } else {
                panel.x + scaled(PANEL_PAD_X, ui_scale) + x as f32 * scaled(10.0, ui_scale)
            };

            draw_text(
                glyph,
                text_x,
                panel.y + scaled(20.0, ui_scale) + y as f32 * scaled(LINE_HEIGHT, ui_scale),
                scaled(22.0, ui_scale),
                final_color,
            );
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

fn draw_event_log(game: &Game, panel: PanelRect, ui_scale: f32) {
    draw_text(
        "Event log",
        panel.x + scaled(PANEL_PAD_X, ui_scale),
        panel.y + scaled(20.0, ui_scale),
        scaled(24.0, ui_scale),
        YELLOW,
    );
    let events = game.log();
    let start = events.len().saturating_sub(10);

    for (index, event) in events[start..].iter().enumerate() {
        let line = event_log_line(event);
        draw_text(
            &line,
            panel.x + scaled(PANEL_PAD_X, ui_scale),
            panel.y + scaled(20.0, ui_scale) + (index as f32 + 1.0) * scaled(LINE_HEIGHT, ui_scale),
            scaled(18.0, ui_scale),
            LIGHTGRAY,
        );
    }
}

fn scaled(value: f32, ui_scale: f32) -> f32 {
    value * ui_scale
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
