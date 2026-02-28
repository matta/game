//! Floor-change mechanics and generated floor state installation.
//! This module exists to isolate descent state mutation and spawn initialization rules.
//! It does not own prompt identity or top-level tick loop orchestration.

use super::*;
use crate::content::get_enemy_stats;
use crate::floor::generate_floor;
use crate::state::{Actor, Item, Map};

impl Game {
    pub(super) fn descend_to_floor(&mut self, floor_index: u8) {
        let generated = generate_floor(self.seed, floor_index, self.state.branch_profile);
        let mut map = Map::new(generated.width, generated.height);
        map.tiles = generated.tiles;
        map.hazards = generated.hazards;

        let player_id = self.state.player_id;
        self.state.actors.retain(|id, _| id == player_id);
        self.state.actors[player_id].pos = generated.entry_tile;

        for spawn in generated.enemy_spawns {
            let stats = get_enemy_stats(spawn.kind);
            let enemy = Actor {
                id: EntityId::default(),
                kind: spawn.kind,
                pos: spawn.pos,
                hp: stats.hp,
                max_hp: stats.hp,
                attack: stats.attack,
                defense: stats.defense,
                active_weapon_slot: WeaponSlot::Primary,
                equipped_weapon: None,
                reserve_weapon: None,
                next_action_tick: stats.speed as u64,
                speed: stats.speed,
            };
            let enemy_id = self.state.actors.insert(enemy);
            self.state.actors[enemy_id].id = enemy_id;
        }

        self.state.items.clear();
        for spawn in generated.item_spawns {
            let item = Item { id: ItemId::default(), kind: spawn.kind, pos: spawn.pos };
            let item_id = self.state.items.insert(item);
            self.state.items[item_id].id = item_id;
        }

        compute_fov(&mut map, generated.entry_tile, FOV_RADIUS);

        self.state.map = map;
        self.state.sanctuary_tile = generated.entry_tile;
        self.state.sanctuary_active = true;
        self.state.floor_index = floor_index;
        self.state.auto_intent = None;
        self.suppressed_enemy = None;
        self.no_progress_ticks = 0;
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::content::ContentPack;
    use crate::floor::{BranchProfile, STARTING_FLOOR_INDEX};
    use crate::game::test_support::*;
    use crate::*;

    #[test]
    fn descending_from_floor_one_loads_floor_two_with_different_map_state() {
        let mut game = Game::new(22222, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let floor_one_tiles = game.state.map.tiles.clone();
        game.state.actors[game.state.player_id].pos = Pos { y: 11, x: 13 };

        let prompt_id = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                prompt_id,
                current_floor,
                next_floor,
                requires_branch_god_choice,
            }) => {
                assert_eq!(current_floor, 1);
                assert_eq!(next_floor, Some(2));
                assert!(requires_branch_god_choice, "first descent should require branch choice");
                prompt_id
            }
            other => panic!("expected floor transition interrupt, got {other:?}"),
        };

        game.apply_choice(prompt_id, Choice::DescendBranchAVeil).expect("descend should apply");
        assert_eq!(game.state.floor_index, 2);
        assert_eq!(game.state.branch_profile, BranchProfile::BranchA);
        assert_eq!(game.state.active_god, Some(GodId::Veil));
        assert_ne!(floor_one_tiles, game.state.map.tiles);
    }

    #[test]
    fn floor_index_never_decreases_during_play() {
        let mut game = Game::new(33333, &ContentPack::default(), GameMode::Ironman);
        let mut last_floor = game.state.floor_index;

        for _ in 0..300 {
            let result = game.advance(8);
            match result.stop_reason {
                AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::KeepLoot).expect("keep loot");
                }
                AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::Fight).expect("fight");
                }
                AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::OpenDoor).expect("open door");
                }
                AdvanceStopReason::Interrupted(
                    int @ Interrupt::FloorTransition { prompt_id, .. },
                ) => {
                    let choice = if matches!(
                        int,
                        Interrupt::FloorTransition { requires_branch_god_choice: true, .. }
                    ) {
                        Choice::DescendBranchAVeil
                    } else {
                        Choice::Descend
                    };
                    game.apply_choice(prompt_id, choice).expect("descend");
                }
                AdvanceStopReason::Finished(_) => break,
                AdvanceStopReason::PausedAtBoundary { .. } | AdvanceStopReason::BudgetExhausted => {
                }
                AdvanceStopReason::EngineFailure(e) => panic!("Engine failure in test: {:?}", e),
            }

            assert!(game.state.floor_index >= last_floor, "floor index should never decrease");
            last_floor = game.state.floor_index;
        }
    }

    #[test]
    fn floor_transition_interrupt_uses_same_prompt_until_choice_is_applied() {
        let mut game = Game::new(44444, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        game.state.actors[game.state.player_id].pos = Pos { y: 11, x: 13 };

        let first_prompt = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition { prompt_id, .. }) => {
                prompt_id
            }
            other => panic!("expected floor transition interrupt, got {other:?}"),
        };

        let second_prompt = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition { prompt_id, .. }) => {
                prompt_id
            }
            other => panic!("expected floor transition interrupt while paused, got {other:?}"),
        };

        assert_eq!(first_prompt, second_prompt);
        game.apply_choice(first_prompt, Choice::DescendBranchAVeil).expect("descend should apply");
        assert_eq!(game.state.floor_index, 2);
    }

    #[test]
    fn branch_prompt_is_emitted_once_on_first_descent_only() {
        let mut game = Game::new(51_515, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        game.state.actors[game.state.player_id].pos = Pos { y: 11, x: 13 };

        let first_prompt = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                prompt_id,
                requires_branch_god_choice,
                ..
            }) => {
                assert!(
                    requires_branch_god_choice,
                    "first descent should require branch+god choice"
                );
                prompt_id
            }
            other => panic!("expected first floor-transition prompt, got {other:?}"),
        };
        game.apply_choice(first_prompt, Choice::DescendBranchAVeil).expect("select branch A");

        let mut stairs = None;
        for y in 0..game.state.map.internal_height {
            for x in 0..game.state.map.internal_width {
                let pos = Pos { y: y as i32, x: x as i32 };
                if game.state.map.tile_at(pos) == TileKind::DownStairs {
                    stairs = Some(pos);
                    break;
                }
            }
            if stairs.is_some() {
                break;
            }
        }
        let stairs = stairs.expect("floor 2 should have a stairs tile");
        game.state.actors[game.state.player_id].pos = stairs;

        match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                requires_branch_god_choice,
                ..
            }) => {
                assert!(
                    !requires_branch_god_choice,
                    "branch+god choice should not reappear after commitment"
                );
            }
            other => panic!("expected second floor-transition prompt, got {other:?}"),
        }
    }

    #[test]
    fn branch_choice_changes_later_floor_characteristics() {
        let content = ContentPack::default();
        let mut game_a = Game::new(42_424, &content, GameMode::Ironman);
        let mut game_b = Game::new(42_424, &content, GameMode::Ironman);
        for game in [&mut game_a, &mut game_b] {
            game.state.items.clear();
            game.state.actors.retain(|id, _| id == game.state.player_id);
            game.state.actors[game.state.player_id].pos = Pos { y: 11, x: 13 };
        }

        let prompt_a = match game_a.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                prompt_id,
                requires_branch_god_choice,
                ..
            }) => {
                assert!(requires_branch_god_choice);
                prompt_id
            }
            other => panic!("expected branch prompt in game A, got {other:?}"),
        };
        let prompt_b = match game_b.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                prompt_id,
                requires_branch_god_choice,
                ..
            }) => {
                assert!(requires_branch_god_choice);
                prompt_id
            }
            other => panic!("expected branch prompt in game B, got {other:?}"),
        };

        game_a.apply_choice(prompt_a, Choice::DescendBranchAVeil).expect("choose branch A");
        game_b.apply_choice(prompt_b, Choice::DescendBranchBForge).expect("choose branch B");

        let floor_a_enemy_count =
            game_a.state.actors.iter().filter(|(id, _)| *id != game_a.state.player_id).count();
        let floor_b_enemy_count =
            game_b.state.actors.iter().filter(|(id, _)| *id != game_b.state.player_id).count();
        let floor_a_hazard_count = game_a.state.map.hazards.iter().filter(|&&h| h).count();
        let floor_b_hazard_count = game_b.state.map.hazards.iter().filter(|&&h| h).count();

        assert!(
            floor_a_enemy_count > floor_b_enemy_count,
            "Branch A should create denser enemy floors"
        );
        assert!(
            floor_b_hazard_count > floor_a_hazard_count,
            "Branch B should create denser hazard floors"
        );
    }

    #[test]
    fn first_descent_rejects_plain_descend_and_requires_combined_choice() {
        let mut game = Game::new(112233, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        game.state.actors[game.state.player_id].pos = Pos { y: 11, x: 13 };

        let prompt_id = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                prompt_id,
                requires_branch_god_choice,
                ..
            }) => {
                assert!(requires_branch_god_choice);
                prompt_id
            }
            other => panic!("expected floor transition interrupt, got {other:?}"),
        };

        let result = game.apply_choice(prompt_id, Choice::Descend);
        assert!(matches!(result, Err(GameError::InvalidChoice)));

        game.apply_choice(prompt_id, Choice::DescendBranchAForge)
            .expect("combined branch+god choice should apply");
        assert_eq!(game.state.branch_profile, BranchProfile::BranchA);
        assert_eq!(game.state.active_god, Some(GodId::Forge));
    }

    #[test]
    fn non_first_descent_rejects_combined_choice_and_accepts_descend() {
        let mut game = Game::new(778899, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        game.state.actors[game.state.player_id].pos = Pos { y: 11, x: 13 };

        let first_prompt = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition { prompt_id, .. }) => {
                prompt_id
            }
            other => panic!("expected first floor transition interrupt, got {other:?}"),
        };
        game.apply_choice(first_prompt, Choice::DescendBranchBVeil)
            .expect("first descent combined choice should apply");

        let mut stairs = None;
        for y in 0..game.state.map.internal_height {
            for x in 0..game.state.map.internal_width {
                let pos = Pos { y: y as i32, x: x as i32 };
                if game.state.map.tile_at(pos) == TileKind::DownStairs {
                    stairs = Some(pos);
                    break;
                }
            }
            if stairs.is_some() {
                break;
            }
        }
        game.state.actors[game.state.player_id].pos = stairs.expect("floor 2 stairs");

        let prompt_id = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                prompt_id,
                requires_branch_god_choice,
                ..
            }) => {
                assert!(!requires_branch_god_choice);
                prompt_id
            }
            other => panic!("expected second floor transition interrupt, got {other:?}"),
        };
        let invalid = game.apply_choice(prompt_id, Choice::DescendBranchAForge);
        assert!(matches!(invalid, Err(GameError::InvalidChoice)));
        game.apply_choice(prompt_id, Choice::Descend).expect("plain descend should apply");
    }

    #[test]
    fn forge_choice_grants_hp_and_passive_defense() {
        let mut game = Game::new(332211, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        game.state.actors[game.state.player_id].pos = Pos { y: 11, x: 13 };
        let start_max_hp = game.state.actors[game.state.player_id].max_hp;
        let start_hp = game.state.actors[game.state.player_id].hp;

        let prompt_id = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                prompt_id,
                requires_branch_god_choice,
                ..
            }) => {
                assert!(requires_branch_god_choice);
                prompt_id
            }
            other => panic!("expected first floor transition interrupt, got {other:?}"),
        };
        game.apply_choice(prompt_id, Choice::DescendBranchAForge)
            .expect("forge choice should apply");

        assert_eq!(game.state.active_god, Some(GodId::Forge));
        assert_eq!(game.state.actors[game.state.player_id].max_hp, start_max_hp + 2);
        assert_eq!(
            game.state.actors[game.state.player_id].hp,
            (start_hp + 2).min(start_max_hp + 2)
        );
        assert_eq!(game.effective_player_defense(), 2);
    }
}
