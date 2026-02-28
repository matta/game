//! Regression tests for simulation stepping, interruption flow, and intent updates.

#![allow(unused_imports)]

use super::*;
use crate::content::{ContentPack, keys};
use crate::game::test_support::*;
use crate::game::visibility::draw_map_diag;
use crate::mapgen::{BranchProfile, STARTING_FLOOR_INDEX};
use crate::*;

#[test]
fn starter_layout_has_expected_rooms_door_hazards_and_spawns() {
    let game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    assert_eq!(game.state.floor_index, STARTING_FLOOR_INDEX);
    assert_eq!(game.state.branch_profile, BranchProfile::Uncommitted);

    let expected_player = Pos { y: 5, x: 4 };
    assert_eq!(game.state.actors[game.state.player_id].pos, expected_player);

    let loot_positions: Vec<Pos> = game.state.items.iter().map(|(_, item)| item.pos).collect();
    assert_eq!(loot_positions, vec![Pos { y: 5, x: 6 }]);
    assert!(!loot_positions.contains(&expected_player));

    let goblin_positions: Vec<Pos> = game
        .state
        .actors
        .iter()
        .filter(|(id, actor)| *id != game.state.player_id && actor.kind == ActorKind::Goblin)
        .map(|(_, actor)| actor.pos)
        .collect();
    assert_eq!(goblin_positions.len(), 4, "starter layout should spawn four goblins");
    assert!(goblin_positions.contains(&Pos { y: 5, x: 11 }));
    assert!(goblin_positions.contains(&Pos { y: 6, x: 10 }));
    assert!(goblin_positions.contains(&Pos { y: 7, x: 9 }));
    assert!(goblin_positions.contains(&Pos { y: 11, x: 11 }));

    assert_eq!(game.state.map.tile_at(Pos { y: 5, x: 8 }), TileKind::ClosedDoor);
    assert_eq!(game.state.map.tile_at(Pos { y: 5, x: 7 }), TileKind::Floor);
    assert_eq!(game.state.map.tile_at(Pos { y: 8, x: 11 }), TileKind::Floor);
    assert_eq!(game.state.map.tile_at(Pos { y: 9, x: 11 }), TileKind::Floor);
    assert_eq!(game.state.map.tile_at(Pos { y: 11, x: 13 }), TileKind::DownStairs);

    for hazard in [Pos { y: 8, x: 11 }, Pos { y: 9, x: 11 }, Pos { y: 10, x: 11 }] {
        assert!(game.state.map.is_hazard(hazard), "expected hazard at {hazard:?}");
    }
}

#[test]
fn starter_layout_auto_flow_reaches_a_multi_enemy_encounter() {
    let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    let mut saw_multi_enemy_interrupt = false;
    let mut encounter_sizes: Vec<(u64, Pos, usize)> = Vec::new();

    while game.current_tick() <= 250 && !saw_multi_enemy_interrupt {
        match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) => {
                game.apply_choice(prompt_id, Choice::KeepLoot).expect("loot choice should apply");
            }
            AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => {
                game.apply_choice(prompt_id, Choice::OpenDoor).expect("door choice should apply");
            }
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                prompt_id,
                enemies,
                ..
            }) => {
                let player_pos = game.state.actors[game.state.player_id].pos;
                encounter_sizes.push((game.current_tick(), player_pos, enemies.len()));
                if enemies.len() >= 2 {
                    saw_multi_enemy_interrupt = true;
                }
                game.apply_choice(prompt_id, Choice::Fight).expect("fight choice should apply");
            }
            AdvanceStopReason::Interrupted(int @ Interrupt::FloorTransition { prompt_id, .. }) => {
                let choice = if matches!(
                    int,
                    Interrupt::FloorTransition { requires_branch_god_choice: true, .. }
                ) {
                    Choice::DescendBranchAVeil
                } else {
                    Choice::Descend
                };
                game.apply_choice(prompt_id, choice).expect("descend choice should apply");
            }
            AdvanceStopReason::Finished(_) => break,
            AdvanceStopReason::PausedAtBoundary { .. } | AdvanceStopReason::BudgetExhausted => {}
            AdvanceStopReason::EngineFailure(error) => {
                panic!("Engine failure in test: {:?}", error);
            }
        }
    }

    assert!(
        saw_multi_enemy_interrupt,
        "expected at least one multi-enemy encounter interrupt in starter layout auto-flow; encounters={encounter_sizes:?}"
    );
}

#[test]
fn run_does_not_end_only_because_tick_counter_grew() {
    let mut game = Game::new(55555, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);
    game.tick = 401;

    let result = game.advance(1);
    assert!(
        !matches!(result.stop_reason, AdvanceStopReason::Finished(_)),
        "run should not auto-finish from tick count"
    );
}

#[test]
fn no_progress_simulation_finishes_instead_of_spinning_budget_forever() {
    let mut game = Game::new(66666, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);

    let mut map = Map::new(7, 7);
    for y in 1..6 {
        for x in 1..6 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    let isolated = Pos { y: 3, x: 3 };
    map.set_tile(isolated, TileKind::Floor);
    map.discovered.fill(true);
    map.visible.fill(true);
    game.state.map = map;
    game.state.actors[game.state.player_id].pos = isolated;
    game.state.auto_intent = None;

    let result = game.advance(200);
    assert_eq!(
        result.simulated_ticks, MAX_NO_PROGRESS_TICKS,
        "stall watchdog should terminate within a fixed tick budget"
    );
    assert!(matches!(
        result.stop_reason,
        AdvanceStopReason::EngineFailure(crate::EngineFailureReason::StalledNoProgress)
    ));
}

#[test]
fn advance_uses_hazard_path_for_threat_avoidance_intent() {
    let mut game = Game::new(123, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);

    let mut map = Map::new(11, 9);
    for y in 1..8 {
        for x in 1..10 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 2..=8 {
        map.set_tile(Pos { y: 4, x }, TileKind::Floor);
    }
    map.discovered.fill(true);
    map.visible.fill(true);
    map.set_hazard(Pos { y: 4, x: 4 }, true);
    map.set_hazard(Pos { y: 4, x: 6 }, true);
    map.discovered[(4 * map.internal_width) + 1] = false;
    map.discovered[(4 * map.internal_width) + 9] = false;
    game.state.map = map;

    let start = Pos { y: 4, x: 5 };
    game.state.actors[game.state.player_id].pos = start;

    let result = game.advance(1);
    assert!(matches!(result.stop_reason, AdvanceStopReason::BudgetExhausted));
    assert_eq!(game.state.actors[game.state.player_id].pos, Pos { y: 4, x: 4 });
    assert_eq!(
        game.state.auto_intent.map(|intent| intent.reason),
        Some(AutoReason::ThreatAvoidance)
    );
}

#[test]
fn movement_updates_visibility_and_expands_discovery() {
    let mut game = Game::new(123, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);

    let mut map = Map::new(30, 10);
    for y in 1..9 {
        for x in 1..29 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 1..26 {
        map.set_tile(Pos { y: 4, x }, TileKind::Floor);
    }
    map.discovered.fill(false);
    game.state.map = map;

    let start = Pos { y: 4, x: 5 };
    game.state.actors[game.state.player_id].pos = start;
    compute_fov(&mut game.state.map, start, FOV_RADIUS);
    // Create visible frontier at (4,15) by leaving (4,16) unknown.
    game.state.map.discovered[(4 * game.state.map.internal_width) + 16] = false;
    let discovered_before = game.state.map.discovered.iter().filter(|&&known| known).count();

    let result = game.advance(1);
    assert!(matches!(result.stop_reason, AdvanceStopReason::BudgetExhausted));
    let moved_to = game.state.actors[game.state.player_id].pos;
    assert_eq!(manhattan(start, moved_to), 1, "player should move exactly one tile");
    let discovered_after = game.state.map.discovered.iter().filter(|&&known| known).count();
    assert!(
        discovered_after > discovered_before,
        "moving with FOV recompute should discover at least one new tile"
    );
}

#[test]
fn door_interrupt_and_open_flow() {
    let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);
    let (map, player_pos, door_pos) = closed_door_choke_fixture();
    game.state.map = map;
    game.state.actors[game.state.player_id].pos = player_pos;
    compute_fov(&mut game.state.map, player_pos, FOV_RADIUS);

    // Manually set intent to target the door (which is a frontier candidate).
    game.state.auto_intent =
        Some(AutoExploreIntent { target: door_pos, reason: AutoReason::Frontier, path_len: 1 });

    let result = game.advance(1);
    if let AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, pos }) =
        result.stop_reason
    {
        assert_eq!(pos, door_pos);
        game.apply_choice(prompt_id, Choice::OpenDoor).unwrap();
        assert_eq!(game.state.map.tile_at(door_pos), TileKind::Floor);
    } else {
        panic!(
            "Expected DoorBlocked at {:?}, got {:?}. Map:\n{}",
            door_pos,
            result.stop_reason,
            draw_map_diag(&game.state.map, player_pos)
        );
    }
}

#[test]
fn door_interrupt_open_then_resume_moves_forward() {
    let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);
    let (map, start, door) = closed_door_choke_fixture();
    game.state.map = map;
    game.state.actors[game.state.player_id].pos = start;
    compute_fov(&mut game.state.map, start, FOV_RADIUS);
    game.state.auto_intent =
        Some(AutoExploreIntent { target: door, reason: AutoReason::Door, path_len: 1 });

    let first = game.advance(1);
    let prompt_id = match first.stop_reason {
        AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => prompt_id,
        other => panic!("expected door interrupt, got {other:?}"),
    };
    game.apply_choice(prompt_id, Choice::OpenDoor).expect("open door");

    let second = game.advance(1);
    assert!(
        !matches!(
            second.stop_reason,
            AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { .. })
        ),
        "door should not immediately re-interrupt after opening"
    );
    assert_eq!(game.state.map.tile_at(door), TileKind::Floor);
}

#[test]
fn unchanged_intent_does_not_duplicate_reason_change_log() {
    let mut game = Game::new(123, &ContentPack::default(), GameMode::Ironman);
    if let AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) =
        game.advance(1).stop_reason
    {
        game.apply_choice(prompt_id, Choice::KeepLoot).unwrap();
    }
    let pos = game.state.actors[game.state.player_id].pos;
    compute_fov(&mut game.state.map, pos, FOV_RADIUS);
    game.plan_auto_intent(pos);
    let count_after_first = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_first, 1);
    game.plan_auto_intent(pos);
    let count_after_second = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_second, 1);
}

#[test]
fn path_len_only_change_does_not_emit_auto_reason_changed() {
    let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);
    let mut map = Map::new(12, 7);
    for y in 1..(map.internal_height - 1) {
        for x in 1..(map.internal_width - 1) {
            map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
        }
    }
    for x in 1..=9 {
        map.set_tile(Pos { y: 3, x }, TileKind::Floor);
    }
    map.discovered.fill(true);
    map.visible.fill(false);
    for x in 1..=8 {
        map.set_visible(Pos { y: 3, x }, true);
    }
    map.discovered[(3 * map.internal_width) + 9] = false;

    game.state.map = map;
    let first_pos = Pos { y: 3, x: 3 };
    game.state.actors[game.state.player_id].pos = first_pos;
    game.plan_auto_intent(first_pos);
    let previous_intent = game.state.auto_intent.unwrap_or_else(|| {
        panic!("No first intent! Map:\n{}", draw_map_diag(&game.state.map, first_pos));
    });
    assert_eq!(previous_intent.target, Pos { y: 3, x: 8 });

    let second_pos = Pos { y: 3, x: 4 };
    game.state.actors[game.state.player_id].pos = second_pos;
    game.plan_auto_intent(second_pos);
    let next_intent = game.state.auto_intent.unwrap();
    assert_eq!(previous_intent.target, next_intent.target);
    assert_ne!(previous_intent.path_len, next_intent.path_len);
    let count = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count, 1);
}

#[test]
fn auto_reason_changed_emits_only_on_reason_or_target_changes() {
    let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);
    let mut map = Map::new(10, 7);
    for y in 1..6 {
        for x in 1..9 {
            map.set_tile(Pos { y, x }, TileKind::Wall);
        }
    }
    for x in 1..=7 {
        map.set_tile(Pos { y: 3, x }, TileKind::Floor);
    }
    map.discovered.fill(true);
    map.visible.fill(true);
    map.discovered[(3 * map.internal_width) + 8] = false;
    game.state.map = map;

    let pos = Pos { y: 3, x: 2 };
    game.state.actors[game.state.player_id].pos = pos;
    game.plan_auto_intent(pos);
    let count_after_first = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_first, 1);

    // Same target, different reason due to hazard fallback.
    game.state.map.set_hazard(Pos { y: 3, x: 7 }, true);
    game.plan_auto_intent(pos);
    let count_after_reason_change = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_reason_change, 2);

    // No further reason/target change means no extra log.
    game.plan_auto_intent(pos);
    let count_after_repeat = game
        .log()
        .iter()
        .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
        .count();
    assert_eq!(count_after_repeat, 2);
}

#[test]
fn enemies_do_not_interrupt_when_player_is_on_sanctuary_tile() {
    let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
    game.state.items.clear();
    game.state.actors.retain(|id, _| id == game.state.player_id);

    let sanctuary = game.state.sanctuary_tile;
    let player = game.state.player_id;
    game.state.sanctuary_active = true;
    game.state.actors[player].pos = sanctuary;
    let _enemy = add_goblin(&mut game, Pos { y: sanctuary.y, x: sanctuary.x + 1 });
    game.suppressed_enemy = Some(EntityId::default());

    let result = game.advance(1);
    assert!(
        !matches!(
            result.stop_reason,
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { .. })
        ),
        "enemy encounter should be suppressed on sanctuary tile"
    );
    assert_eq!(game.suppressed_enemy, None, "sanctuary should purge stale threat state");
}
