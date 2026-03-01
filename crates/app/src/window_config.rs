//! Window configuration for the desktop app.

use app::APP_NAME;
use app::ui_scale::resolve_ui_scale;
use macroquad::window::{Conf, screen_dpi_scale};
use std::env;

const DEFAULT_WINDOW_WIDTH: i32 = 1000;
const DEFAULT_WINDOW_HEIGHT: i32 = 750;

pub fn build_window_conf() -> Conf {
    Conf {
        window_title: APP_NAME.to_owned(),
        window_width: DEFAULT_WINDOW_WIDTH,
        window_height: DEFAULT_WINDOW_HEIGHT,
        // Linux desktop sessions may not scale low-DPI framebuffers automatically.
        // Request a high-DPI framebuffer so text and UI size track display scale.
        high_dpi: true,
        ..Default::default()
    }
}

pub fn current_dpi_scale() -> f32 {
    screen_dpi_scale()
}

pub fn runtime_ui_scale(persisted_ui_scale: Option<f32>) -> f32 {
    let dpi_scale = current_dpi_scale();
    let override_value = env::var("GAME_UI_SCALE").ok();
    resolve_ui_scale(dpi_scale, persisted_ui_scale, override_value.as_deref())
}

pub fn display_scale_notice(persisted_ui_scale: Option<f32>) -> String {
    let dpi_scale = current_dpi_scale();
    let override_value = env::var("GAME_UI_SCALE").ok();
    let ui_scale = resolve_ui_scale(dpi_scale, persisted_ui_scale, override_value.as_deref());
    let override_label = override_value.unwrap_or_else(|| "unset".to_string());
    let saved_label = persisted_ui_scale
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "unset".to_string());
    format!(
        "Display scale: dpi={dpi_scale:.2} ui={ui_scale:.2} saved={saved_label} GAME_UI_SCALE={override_label}"
    )
}

#[cfg(test)]
mod tests {
    use super::build_window_conf;

    #[test]
    fn enables_high_dpi_rendering() {
        let conf = build_window_conf();
        assert!(conf.high_dpi);
    }

    #[test]
    fn uses_expected_default_window_size() {
        let conf = build_window_conf();
        assert_eq!(conf.window_width, 1000);
        assert_eq!(conf.window_height, 750);
    }
}
