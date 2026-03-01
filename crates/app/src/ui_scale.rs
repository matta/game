//! Shared UI scale model and operations.

pub const DEFAULT_UI_SCALE: f32 = 1.0;
pub const MIN_UI_SCALE: f32 = 0.5;
pub const MAX_UI_SCALE: f32 = 4.0;
pub const UI_SCALE_STEP: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiScaleAction {
    Increase,
    Decrease,
    Reset,
}

pub fn clamp_ui_scale(value: f32) -> f32 {
    if !value.is_finite() {
        return DEFAULT_UI_SCALE;
    }
    value.clamp(MIN_UI_SCALE, MAX_UI_SCALE)
}

pub fn increase_ui_scale(current: f32) -> f32 {
    clamp_ui_scale(current + UI_SCALE_STEP)
}

pub fn decrease_ui_scale(current: f32) -> f32 {
    clamp_ui_scale(current - UI_SCALE_STEP)
}

pub fn reset_ui_scale() -> f32 {
    DEFAULT_UI_SCALE
}

pub fn resolve_ui_scale(
    dpi_scale: f32,
    persisted_ui_scale: Option<f32>,
    ui_scale_override: Option<&str>,
) -> f32 {
    let override_scale =
        ui_scale_override.and_then(|raw| raw.parse::<f32>().ok()).map(clamp_ui_scale);
    if let Some(scale) = override_scale {
        return scale;
    }
    if let Some(scale) = persisted_ui_scale {
        return clamp_ui_scale(scale);
    }
    if dpi_scale.is_finite() && dpi_scale > 1.0 {
        return clamp_ui_scale(dpi_scale);
    }
    DEFAULT_UI_SCALE
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_UI_SCALE, MIN_UI_SCALE, clamp_ui_scale, decrease_ui_scale, increase_ui_scale,
        reset_ui_scale, resolve_ui_scale,
    };

    #[test]
    fn resolve_ui_scale_uses_dpi_when_override_missing() {
        assert_eq!(resolve_ui_scale(2.0, None, None), 2.0);
    }

    #[test]
    fn resolve_ui_scale_defaults_to_one_for_invalid_dpi() {
        assert_eq!(resolve_ui_scale(0.0, None, None), 1.0);
    }

    #[test]
    fn resolve_ui_scale_prefers_env_override() {
        assert_eq!(resolve_ui_scale(2.0, Some(1.2), Some("1.5")), 1.5);
    }

    #[test]
    fn resolve_ui_scale_ignores_invalid_override() {
        assert_eq!(resolve_ui_scale(2.0, None, Some("abc")), 2.0);
    }

    #[test]
    fn resolve_ui_scale_uses_persisted_value_when_no_override() {
        assert_eq!(resolve_ui_scale(1.0, Some(1.7), None), 1.7);
    }

    #[test]
    fn clamp_ui_scale_respects_bounds() {
        assert_eq!(clamp_ui_scale(0.1), MIN_UI_SCALE);
        assert_eq!(clamp_ui_scale(9.9), MAX_UI_SCALE);
    }

    #[test]
    fn increase_and_decrease_ui_scale_use_fractional_steps() {
        assert!((increase_ui_scale(1.0) - 1.1).abs() < 0.0001);
        assert!((decrease_ui_scale(1.0) - 0.9).abs() < 0.0001);
    }

    #[test]
    fn reset_ui_scale_returns_default() {
        assert_eq!(reset_ui_scale(), 1.0);
    }
}
