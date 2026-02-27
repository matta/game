use app::{
    app_loop::{AppCompletion, AppMode, AppState},
    format_snapshot_hash,
    seed::{generate_runtime_seed, resolve_seed_from_args},
};
use core::{ContentPack, Game, GameMode, Interrupt, LogEvent, Pos, TileKind};
use macroquad::prelude::*;
use macroquad::window::Conf;
use std::{env, process::exit};
use taffy::TaffyTree;
use taffy::prelude::*;

struct LayoutNodes {
    root: NodeId,
    status: NodeId,
    main_row: NodeId,
    left_col: NodeId,
    map: NodeId,
    bottom_info: NodeId,
    stats: NodeId,
    policy: NodeId,
    threat: NodeId,
    event_log: NodeId,
}

fn setup_layout(taffy: &mut TaffyTree<()>) -> LayoutNodes {
    let status = taffy
        .new_leaf(Style {
            size: Size { width: percent(1.0), height: length(40.0) },
            margin: taffy::Rect { left: zero(), right: zero(), top: zero(), bottom: length(20.0) },
            ..Default::default()
        })
        .unwrap();
    let map = taffy
        .new_leaf(Style {
            flex_grow: 1.0,
            margin: taffy::Rect { left: zero(), right: zero(), top: zero(), bottom: length(20.0) },
            ..Default::default()
        })
        .unwrap();
    let stats = taffy.new_leaf(Style { flex_grow: 1.8, ..Default::default() }).unwrap();
    let policy = taffy
        .new_leaf(Style {
            flex_grow: 1.1,
            margin: taffy::Rect { left: length(15.0), right: zero(), top: zero(), bottom: zero() },
            ..Default::default()
        })
        .unwrap();
    let threat = taffy
        .new_leaf(Style {
            flex_grow: 1.0,
            margin: taffy::Rect { left: length(15.0), right: zero(), top: zero(), bottom: zero() },
            ..Default::default()
        })
        .unwrap();
    let bottom_info = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                size: Size { width: percent(1.0), height: length(240.0) },
                flex_grow: 0.0,
                ..Default::default()
            },
            &[stats, policy, threat],
        )
        .unwrap();
    let left_col = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                flex_grow: 2.0,
                margin: taffy::Rect {
                    left: zero(),
                    right: length(20.0),
                    top: zero(),
                    bottom: zero(),
                },
                ..Default::default()
            },
            &[map, bottom_info],
        )
        .unwrap();
    let event_log = taffy
        .new_leaf(Style {
            flex_grow: 1.0,
            margin: taffy::Rect { left: length(20.0), right: zero(), top: zero(), bottom: zero() },
            ..Default::default()
        })
        .unwrap();
    let main_row = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                size: Size { width: percent(1.0), height: percent(1.0) },
                flex_grow: 1.0,
                ..Default::default()
            },
            &[left_col, event_log],
        )
        .unwrap();
    let root = taffy
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size { width: percent(1.0), height: percent(1.0) },
                padding: taffy::Rect {
                    left: length(20.0),
                    right: length(20.0),
                    top: length(20.0),
                    bottom: length(20.0),
                },
                ..Default::default()
            },
            &[status, main_row],
        )
        .unwrap();
    LayoutNodes {
        root,
        status,
        main_row,
        left_col,
        map,
        bottom_info,
        stats,
        policy,
        threat,
        event_log,
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Roguelike".to_owned(),
        window_width: 1000,
        window_height: 750,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let generated_seed = generate_runtime_seed();
    let selected_seed = match resolve_seed_from_args(&args, generated_seed) {
        Ok(seed_choice) => seed_choice,
        Err(message) => {
            let program_name = args.first().map_or("game", String::as_str);
            eprintln!("Error: {message}");
            eprintln!("Usage: {program_name} [--seed <u64>]");
            exit(2);
        }
    };
    let run_seed = selected_seed.value();

    let content = ContentPack::default();
    let mut game = Game::new(run_seed, &content, GameMode::Ironman);

    let mut app_state = AppState::default();

    let mut taffy: TaffyTree<()> = TaffyTree::new();
    let nodes = setup_layout(&mut taffy);

    loop {
        clear_background(BLACK);

        let mut keys_pressed = vec![];
        if is_key_pressed(KeyCode::Space) {
            keys_pressed.push(KeyCode::Space);
        }
        if is_key_pressed(KeyCode::Right) {
            keys_pressed.push(KeyCode::Right);
        }
        for key in [
            KeyCode::L,
            KeyCode::D,
            KeyCode::F,
            KeyCode::A,
            KeyCode::B,
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::O,
            KeyCode::C,
            KeyCode::M,
            KeyCode::T,
            KeyCode::P,
            KeyCode::R,
            KeyCode::H,
            KeyCode::I,
            KeyCode::E,
            KeyCode::G,
        ] {
            if is_key_pressed(key) {
                keys_pressed.push(key);
            }
        }

        app_state.tick(&mut game, &keys_pressed);

        let available_size = Size {
            width: AvailableSpace::Definite(screen_width()),
            height: AvailableSpace::Definite(screen_height()),
        };
        taffy.compute_layout(nodes.root, available_size).unwrap();

        let l_root = taffy.layout(nodes.root).unwrap();
        let l_status = taffy.layout(nodes.status).unwrap();
        let l_main = taffy.layout(nodes.main_row).unwrap();
        let l_left = taffy.layout(nodes.left_col).unwrap();
        let l_map = taffy.layout(nodes.map).unwrap();
        let l_bottom = taffy.layout(nodes.bottom_info).unwrap();
        let l_stats = taffy.layout(nodes.stats).unwrap();
        let l_policy = taffy.layout(nodes.policy).unwrap();
        let l_threat = taffy.layout(nodes.threat).unwrap();
        let l_event = taffy.layout(nodes.event_log).unwrap();

        let get_abs = |lyt: &taffy::Layout, parents: &[&taffy::Layout]| -> (f32, f32) {
            let mut x = lyt.location.x;
            let mut y = lyt.location.y;
            for p in parents {
                x += p.location.x;
                y += p.location.y;
            }
            (x, y)
        };

        let pos_status = get_abs(l_status, &[l_root]);
        let pos_map = get_abs(l_map, &[l_root, l_main, l_left]);
        let pos_stats = get_abs(l_stats, &[l_root, l_main, l_left, l_bottom]);
        let pos_policy = get_abs(l_policy, &[l_root, l_main, l_left, l_bottom]);
        let pos_threat = get_abs(l_threat, &[l_root, l_main, l_left, l_bottom]);
        let pos_event = get_abs(l_event, &[l_root, l_main]);

        let border_color = Color::new(0.2, 0.2, 0.2, 1.0);
        let border_thickness = 1.0;
        draw_rectangle_lines(
            pos_status.0,
            pos_status.1,
            l_status.size.width,
            l_status.size.height,
            border_thickness,
            border_color,
        );
        draw_rectangle_lines(
            pos_map.0,
            pos_map.1,
            l_map.size.width,
            l_map.size.height,
            border_thickness,
            border_color,
        );
        draw_rectangle_lines(
            pos_stats.0,
            pos_stats.1,
            l_stats.size.width,
            l_stats.size.height,
            border_thickness,
            border_color,
        );
        draw_rectangle_lines(
            pos_policy.0,
            pos_policy.1,
            l_policy.size.width,
            l_policy.size.height,
            border_thickness,
            border_color,
        );
        draw_rectangle_lines(
            pos_threat.0,
            pos_threat.1,
            l_threat.size.width,
            l_threat.size.height,
            border_thickness,
            border_color,
        );
        draw_rectangle_lines(
            pos_event.0,
            pos_event.1,
            l_event.size.width,
            l_event.size.height,
            border_thickness,
            border_color,
        );

        let line_height = 18.0;
        let pad_x = 15.0;
        let pad_y = 25.0;

        draw_ascii_map(&game, pos_map.0 + pad_x, pos_map.1, line_height);
        draw_event_log(&game, pos_event.0 + pad_x, pos_event.1, line_height);

        let status = match &app_state.mode {
            AppMode::PendingPrompt { interrupt, .. } => prompt_text(interrupt),
            AppMode::Finished(completion) => {
                format!("Finished: {}", completion_reason_code(completion.clone()))
            }
            AppMode::AutoPlay => "Auto-Explore ON (Space to pause)".to_string(),
            AppMode::Paused => "Paused (Space to Auto-Explore, Right to step)".to_string(),
        };
        draw_text(&status, pos_status.0 + pad_x, pos_status.1 + pad_y, 20.0, WHITE);

        let mut stats_y = pos_stats.1 + pad_y;
        let p_x = pos_stats.0 + pad_x;
        if let AppMode::Finished(completion) = &app_state.mode {
            let recap_lines = build_finished_recap_lines(&game, run_seed, completion.clone());
            for line in recap_lines {
                draw_text(&line, p_x, stats_y, 20.0, WHITE);
                stats_y += 20.0;
            }
        } else {
            draw_text(&format!("Tick: {}", game.current_tick()), p_x, stats_y, 20.0, WHITE);
            stats_y += 20.0;
            draw_text(&format!("Seed: {run_seed}"), p_x, stats_y, 20.0, WHITE);
            stats_y += 20.0;
            draw_text(
                &format!("Floor: {} / 5", game.state().floor_index),
                p_x,
                stats_y,
                20.0,
                WHITE,
            );
            stats_y += 20.0;
            draw_text(
                &format!("Branch: {:?}", game.state().branch_profile),
                p_x,
                stats_y,
                20.0,
                WHITE,
            );
            stats_y += 20.0;
            draw_text(&format!("God: {:?}", game.state().active_god), p_x, stats_y, 20.0, WHITE);
            stats_y += 20.0;
            draw_text(
                &format!("Hash: {}", format_snapshot_hash(game.snapshot_hash())),
                p_x,
                stats_y,
                20.0,
                WHITE,
            );
            stats_y += 20.0;

            let intent_text = if let Some(intent) = game.state().auto_intent {
                format!(
                    "Intent: {:?} target=({}, {}) path_len={}",
                    intent.reason, intent.target.x, intent.target.y, intent.path_len
                )
            } else {
                "Intent: none".to_string()
            };
            draw_text(&intent_text, p_x, stats_y, 20.0, WHITE);
        }

        let policy = &game.state().policy;
        let mut pol_y = pos_policy.1 + pad_y;
        let pol_x = pos_policy.0 + pad_x;
        if matches!(app_state.mode, AppMode::Finished(_)) {
            draw_text("Policy: run ended", pol_x, pol_y, 20.0, YELLOW);
        } else {
            draw_text("Policy: ", pol_x, pol_y, 20.0, YELLOW);
            pol_y += 20.0;
            draw_text(
                &format!("[M]ode: {:?}", policy.fight_or_avoid),
                pol_x,
                pol_y,
                18.0,
                LIGHTGRAY,
            );
            pol_y += 20.0;
            draw_text(&format!("s[T]ance: {:?}", policy.stance), pol_x, pol_y, 18.0, LIGHTGRAY);
            pol_y += 20.0;
            draw_text(
                &format!("[P]riority: {:?}", policy.target_priority),
                pol_x,
                pol_y,
                18.0,
                LIGHTGRAY,
            );
            pol_y += 20.0;
            draw_text(
                &format!("[R]etreat HP: {}%", policy.retreat_hp_threshold),
                pol_x,
                pol_y,
                18.0,
                LIGHTGRAY,
            );
            pol_y += 20.0;
            draw_text(
                &format!("[H]eal: {:?}", policy.auto_heal_if_below_threshold),
                pol_x,
                pol_y,
                18.0,
                LIGHTGRAY,
            );
            pol_y += 20.0;
            draw_text(
                &format!("[I]ntent: {:?}", policy.position_intent),
                pol_x,
                pol_y,
                18.0,
                LIGHTGRAY,
            );
            pol_y += 20.0;
            draw_text(
                &format!("[E]xplore: {:?}", policy.exploration_mode),
                pol_x,
                pol_y,
                18.0,
                LIGHTGRAY,
            );
            pol_y += 20.0;
            draw_text(
                &format!("[G]reed: {:?}", policy.resource_aggression),
                pol_x,
                pol_y,
                18.0,
                LIGHTGRAY,
            );
        }

        let mut thr_y = pos_threat.1 + pad_y;
        let thr_x = pos_threat.0 + pad_x;
        draw_text("Threat Trace:", thr_x, thr_y, 20.0, RED);
        thr_y += 20.0;
        for (i, trace) in game.state().threat_trace.iter().take(5).enumerate() {
            let desc = format!(
                "T{}: {} vis, dist {:?}",
                trace.tick, trace.visible_enemy_count, trace.min_enemy_distance
            );
            let color = if trace.retreat_triggered { ORANGE } else { LIGHTGRAY };
            draw_text(&desc, thr_x, thr_y + (i as f32 * 20.0), 18.0, color);
        }

        next_frame().await
    }
}

fn draw_ascii_map(game: &Game, left: f32, top: f32, line_height: f32) {
    let state = game.state();
    let map = &state.map;
    for y in 0..map.internal_height {
        for x in 0..map.internal_width {
            let pos = Pos { x: x as i32, y: y as i32 };
            if !map.is_discovered(pos) {
                draw_text(
                    " ",
                    left + x as f32 * 10.0,
                    top + 20.0 + y as f32 * line_height,
                    22.0,
                    LIGHTGRAY,
                );
                continue;
            }

            let mut glyph = match map.tile_at(pos) {
                TileKind::Wall => "#",
                TileKind::Floor => ".",
                TileKind::ClosedDoor => "+",
                TileKind::DownStairs => ">",
            };

            let color = if map.is_visible(pos) { WHITE } else { GRAY };

            let mut final_color = color;

            for (_, item) in &state.items {
                if item.pos == pos && map.is_visible(pos) {
                    glyph = "!";
                    final_color = YELLOW;
                    break;
                }
            }

            for (_, actor) in &state.actors {
                if actor.pos == pos && map.is_visible(pos) {
                    glyph = match actor.kind {
                        core::ActorKind::Player => "@",
                        core::ActorKind::Goblin => "g",
                        core::ActorKind::FeralHound => "h",
                        core::ActorKind::BloodAcolyte => "a",
                        core::ActorKind::CorruptedGuard => "C",
                        core::ActorKind::LivingArmor => "A",
                        core::ActorKind::Gargoyle => "G",
                        core::ActorKind::ShadowStalker => "S",
                        core::ActorKind::AbyssalWarden => "W",
                    };
                    final_color = match actor.kind {
                        core::ActorKind::Player => GREEN,
                        core::ActorKind::Goblin => RED,
                        core::ActorKind::FeralHound => ORANGE,
                        core::ActorKind::BloodAcolyte => RED,
                        core::ActorKind::CorruptedGuard => BLUE,
                        core::ActorKind::LivingArmor => LIGHTGRAY,
                        core::ActorKind::Gargoyle => GRAY,
                        core::ActorKind::ShadowStalker => PURPLE,
                        core::ActorKind::AbyssalWarden => MAGENTA,
                    };
                    break;
                }
            }

            draw_text(
                glyph,
                left + x as f32 * 11.0,
                top + 20.0 + y as f32 * line_height,
                22.0,
                final_color,
            );
        }
    }
}

fn draw_event_log(game: &Game, left: f32, top: f32, line_height: f32) {
    draw_text("Event log", left, top + 20.0, 24.0, YELLOW);
    let events = game.log();
    let start = events.len().saturating_sub(10);
    for (idx, event) in events[start..].iter().enumerate() {
        let line = match event {
            LogEvent::AutoReasonChanged { reason, .. } => auto_reason_text(*reason).to_string(),
            LogEvent::EnemyEncountered { enemy } => format!("enemy encountered {:?}", enemy),
            LogEvent::ItemPickedUp { kind: _ } => "picked up item".to_string(),
            LogEvent::ItemDiscarded { kind: _ } => "discarded item".to_string(),
            LogEvent::EncounterResolved { enemy, fought } => {
                format!("encounter {:?} resolved fought={}", enemy, fought)
            }
        };
        draw_text(&line, left, top + 20.0 + ((idx + 1) as f32 * line_height), 18.0, LIGHTGRAY);
    }
}

fn prompt_text(interrupt: &Interrupt) -> String {
    match interrupt {
        Interrupt::LootFound { .. } => "INTERRUPT: Loot found (L=keep, D=discard)".to_string(),
        Interrupt::EnemyEncounter { threat, .. } => {
            let dist_text = match threat.nearest_enemy_distance {
                Some(d) => format!("{d}"),
                None => "?".to_string(),
            };
            format!(
                "INTERRUPT: {:?} sighted (F=fight, A=avoid) {} visible, nearest={}, Tags: {:?}",
                threat.primary_enemy_kind,
                threat.visible_enemy_count,
                dist_text,
                threat.danger_tags
            )
        }
        Interrupt::DoorBlocked { .. } => "INTERRUPT: Door blocked (O=open)".to_string(),
        Interrupt::FloorTransition { next_floor, requires_branch_god_choice, .. } => {
            if *requires_branch_god_choice {
                "INTERRUPT: Choose pact (1=A+Veil, 2=A+Forge, 3=B+Veil, 4=B+Forge)".to_string()
            } else {
                match next_floor {
                    Some(floor) => {
                        format!("INTERRUPT: Stairs reached (C=descend to floor {floor})")
                    }
                    None => "INTERRUPT: Final stairs reached (C=finish run)".to_string(),
                }
            }
        }
    }
}

fn auto_reason_text(reason: core::AutoReason) -> &'static str {
    match reason {
        core::AutoReason::Frontier => "Exploring the unknown...",
        core::AutoReason::Loot => "Moving to collect loot...",
        core::AutoReason::ThreatAvoidance => "Pathing around threats...",
        core::AutoReason::Stuck => "Auto-explore is stuck.",
        core::AutoReason::Door => "Moving to open a door...",
    }
}

fn completion_reason_code(completion: AppCompletion) -> &'static str {
    match completion {
        AppCompletion::Outcome(core::RunOutcome::Victory) => "WIN_CLEAR",
        AppCompletion::Outcome(core::RunOutcome::Defeat(core::DeathCause::Damage)) => "DMG_HP_ZERO",
        AppCompletion::Outcome(core::RunOutcome::Defeat(core::DeathCause::Poison)) => "PSN_HP_ZERO",
        AppCompletion::EngineFailure(core::EngineFailureReason::StalledNoProgress) => {
            "ENG_STALLED_NO_PROGRESS"
        }
    }
}

fn build_finished_recap_lines(
    game: &Game,
    run_seed: u64,
    completion: AppCompletion,
) -> Vec<String> {
    let mut lines = vec![
        "Run recap:".to_string(),
        format!("Reason: {}", completion_reason_code(completion)),
        format!("Seed: {run_seed}"),
        format!("Snapshot: {}", format_snapshot_hash(game.snapshot_hash())),
        format!(
            "Floor/Branch/God: {}/{:?}/{:?}",
            game.state().floor_index,
            game.state().branch_profile,
            game.state().active_god
        ),
        format!("Tick: {}", game.current_tick()),
        "Threat trace (latest 5):".to_string(),
    ];

    for trace in game.state().threat_trace.iter().take(5) {
        lines.push(format!(
            "T{} vis={} min_dist={:?} retreat={}",
            trace.tick,
            trace.visible_enemy_count,
            trace.min_enemy_distance,
            trace.retreat_triggered
        ));
    }

    lines
}
