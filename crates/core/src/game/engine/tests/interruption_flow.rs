//! Tests for interruption handling around doors and sanctuary suppression.

use super::support::*;

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
