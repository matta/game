//! Layout model for the game's on-screen panels.

use taffy::TaffyTree;
use taffy::prelude::*;

pub struct LayoutNodes {
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

#[derive(Clone, Copy)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct FrameLayout {
    pub status: PanelRect,
    pub map: PanelRect,
    pub stats: PanelRect,
    pub policy: PanelRect,
    pub threat: PanelRect,
    pub event_log: PanelRect,
}

pub fn setup_layout(taffy: &mut TaffyTree<()>) -> LayoutNodes {
    let status = taffy
        .new_leaf(Style {
            size: Size { width: percent(1.0), height: length(40.0) },
            margin: taffy::Rect { left: zero(), right: zero(), top: zero(), bottom: length(20.0) },
            ..Default::default()
        })
        .expect("status node");
    let map = taffy
        .new_leaf(Style {
            flex_grow: 1.0,
            margin: taffy::Rect { left: zero(), right: zero(), top: zero(), bottom: length(20.0) },
            ..Default::default()
        })
        .expect("map node");
    let stats = taffy.new_leaf(Style { flex_grow: 1.8, ..Default::default() }).expect("stats node");
    let policy = taffy
        .new_leaf(Style {
            flex_grow: 1.1,
            margin: taffy::Rect { left: length(15.0), right: zero(), top: zero(), bottom: zero() },
            ..Default::default()
        })
        .expect("policy node");
    let threat = taffy
        .new_leaf(Style {
            flex_grow: 1.0,
            margin: taffy::Rect { left: length(15.0), right: zero(), top: zero(), bottom: zero() },
            ..Default::default()
        })
        .expect("threat node");
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
        .expect("bottom info node");
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
        .expect("left column node");
    let event_log = taffy
        .new_leaf(Style {
            flex_grow: 1.0,
            margin: taffy::Rect { left: length(20.0), right: zero(), top: zero(), bottom: zero() },
            ..Default::default()
        })
        .expect("event log node");
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
        .expect("main row node");
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
        .expect("root node");
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

pub fn compute_frame_layout(
    taffy: &mut TaffyTree<()>,
    nodes: &LayoutNodes,
    viewport_width: f32,
    viewport_height: f32,
) -> FrameLayout {
    let available_size = Size {
        width: AvailableSpace::Definite(viewport_width),
        height: AvailableSpace::Definite(viewport_height),
    };
    taffy.compute_layout(nodes.root, available_size).expect("compute layout");

    let l_root = taffy.layout(nodes.root).expect("root layout");
    let l_status = taffy.layout(nodes.status).expect("status layout");
    let l_main = taffy.layout(nodes.main_row).expect("main layout");
    let l_left = taffy.layout(nodes.left_col).expect("left layout");
    let l_map = taffy.layout(nodes.map).expect("map layout");
    let l_bottom = taffy.layout(nodes.bottom_info).expect("bottom layout");
    let l_stats = taffy.layout(nodes.stats).expect("stats layout");
    let l_policy = taffy.layout(nodes.policy).expect("policy layout");
    let l_threat = taffy.layout(nodes.threat).expect("threat layout");
    let l_event = taffy.layout(nodes.event_log).expect("event layout");

    FrameLayout {
        status: panel_rect(l_status, &[l_root]),
        map: panel_rect(l_map, &[l_root, l_main, l_left]),
        stats: panel_rect(l_stats, &[l_root, l_main, l_left, l_bottom]),
        policy: panel_rect(l_policy, &[l_root, l_main, l_left, l_bottom]),
        threat: panel_rect(l_threat, &[l_root, l_main, l_left, l_bottom]),
        event_log: panel_rect(l_event, &[l_root, l_main]),
    }
}

fn panel_rect(layout: &taffy::Layout, parents: &[&taffy::Layout]) -> PanelRect {
    let mut x = layout.location.x;
    let mut y = layout.location.y;
    for parent in parents {
        x += parent.location.x;
        y += parent.location.y;
    }

    PanelRect { x, y, width: layout.size.width, height: layout.size.height }
}
