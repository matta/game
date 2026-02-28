//! Tests for starter layout structure and early automatic progression.

use super::support::*;

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
