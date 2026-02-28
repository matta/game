//! Main simulation stepping loop and pause/interrupt control flow.
//! This module exists to keep tick advancement and stop-reason logic centralized.
//! It does not own floor generation internals or item effect definitions.

use super::*;

impl Game {
    pub fn advance(&mut self, max_steps: u32) -> AdvanceResult {
        self.at_pause_boundary = false;
        let mut steps = 0;
        if let Some(outcome) = self.finished_outcome {
            return AdvanceResult {
                simulated_ticks: 0,
                stop_reason: AdvanceStopReason::Finished(outcome),
            };
        }
        if let Some(prompt) = self.pending_prompt.clone() {
            return AdvanceResult {
                simulated_ticks: 0,
                stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
            };
        }

        while steps < max_steps {
            if self.pause_requested {
                self.pause_requested = false;
                self.at_pause_boundary = true;
                return AdvanceResult {
                    simulated_ticks: steps,
                    stop_reason: AdvanceStopReason::PausedAtBoundary { tick: self.tick },
                };
            }

            let player_pos = self.state.actors[self.state.player_id].pos;
            if self.state.map.tile_at(player_pos) == TileKind::DownStairs {
                return self.interrupt_floor_transition(steps);
            }
            if self.state.sanctuary_active && player_pos == self.state.sanctuary_tile {
                self.suppressed_enemy = None;
            } else {
                self.clear_stale_suppressed_enemy(player_pos);
                let adjacent = self.find_adjacent_enemy_ids(player_pos);
                if let Some(primary_enemy) = adjacent.first().copied() {
                    self.log.push(LogEvent::EnemyEncountered { enemy: primary_enemy });
                    return self.interrupt_enemy(adjacent, primary_enemy, steps);
                }
            }
            if let Some(item_id) = self.find_item_at(player_pos) {
                return self.interrupt_loot(item_id, steps);
            }

            self.plan_auto_intent(player_pos);
            let mut player_moved = false;

            if let Some(intent) = self.state.auto_intent
                && intent.path_len > 0
                && let Some(path) = path_for_intent(&self.state.map, player_pos, intent)
                && let Some(next_step) = path.first().copied()
            {
                if self.state.map.tile_at(next_step) == TileKind::ClosedDoor {
                    return self.interrupt_door(next_step, steps);
                }
                self.state.actors[self.state.player_id].pos = next_step;
                let radius = self.get_fov_radius();
                compute_fov(&mut self.state.map, next_step, radius);
                player_moved = true;
            }

            self.tick += 1;
            steps += 1;

            let visible_enemy_count = self
                .state
                .actors
                .iter()
                .filter(|(id, actor)| {
                    *id != self.state.player_id && self.state.map.is_visible(actor.pos)
                })
                .count();
            let min_enemy_distance = self
                .state
                .actors
                .iter()
                .filter_map(|(id, actor)| {
                    if id != self.state.player_id && self.state.map.is_visible(actor.pos) {
                        Some(manhattan(self.state.actors[self.state.player_id].pos, actor.pos))
                    } else {
                        None
                    }
                })
                .min();
            let player_hp_pct = (self.state.actors[self.state.player_id].hp * 100)
                / self.state.actors[self.state.player_id].max_hp;
            let retreat_triggered = player_hp_pct
                <= (self.state.policy.retreat_hp_threshold as i32)
                && visible_enemy_count > 0;
            self.state.threat_trace.push_front(ThreatTrace {
                tick: self.tick,
                visible_enemy_count,
                min_enemy_distance,
                retreat_triggered,
            });
            if self.state.threat_trace.len() > 32 {
                self.state.threat_trace.pop_back();
            }

            if player_moved {
                self.no_progress_ticks = 0;
            } else {
                self.no_progress_ticks = self.no_progress_ticks.saturating_add(1);
                if self.no_progress_ticks >= MAX_NO_PROGRESS_TICKS {
                    return AdvanceResult {
                        simulated_ticks: steps,
                        stop_reason: AdvanceStopReason::EngineFailure(
                            EngineFailureReason::StalledNoProgress,
                        ),
                    };
                }
            }
        }
        AdvanceResult { simulated_ticks: steps, stop_reason: AdvanceStopReason::BudgetExhausted }
    }

    pub fn plan_auto_intent(&mut self, player_pos: Pos) {
        let mut needs_replan = true;
        if let Some(intent) = self.state.auto_intent {
            if player_pos == intent.target {
                needs_replan = true;
            } else if is_intent_target_still_valid(&self.state.map, intent)
                && let Some(path) = path_for_intent(&self.state.map, player_pos, intent)
            {
                let new_len = path.len() as u16;
                if new_len != intent.path_len {
                    self.state.auto_intent =
                        Some(AutoExploreIntent { path_len: new_len, ..intent });
                }
                needs_replan = false;
            }
        }
        if needs_replan {
            let next_intent = choose_frontier_intent(&self.state.map, player_pos);
            let changed = self.state.auto_intent.map(|i| i.reason) != next_intent.map(|i| i.reason);
            if changed && let Some(intent) = next_intent {
                self.log.push(LogEvent::AutoReasonChanged {
                    reason: intent.reason,
                    target: intent.target,
                    path_len: intent.path_len,
                });
            }
            self.state.auto_intent = next_intent;
        }
    }

    pub(super) fn find_item_at(&self, pos: Pos) -> Option<ItemId> {
        self.state.items.iter().find(|(_, item)| item.pos == pos).map(|(id, _)| id)
    }

    pub(super) fn find_adjacent_enemy_ids(&self, pos: Pos) -> Vec<EntityId> {
        let enemies: Vec<EntityId> = self
            .state
            .actors
            .iter()
            .filter(|(id, actor)| {
                if *id == self.state.player_id {
                    return false;
                }
                let sanctuary = self.state.sanctuary_active.then_some(self.state.sanctuary_tile);
                enemy_path_to_player(&self.state.map, actor.pos, pos, sanctuary).is_some()
            })
            .filter(|(id, actor)| {
                Some(*id) != self.suppressed_enemy
                    && *id != self.state.player_id
                    && manhattan(pos, actor.pos) == 1
            })
            .map(|(id, _)| id)
            .collect();
        self.sort_adjacent_enemies_by_policy(pos, enemies)
    }

    pub(super) fn clear_stale_suppressed_enemy(&mut self, player_pos: Pos) {
        let Some(enemy_id) = self.suppressed_enemy else {
            return;
        };
        let should_clear = match self.state.actors.get(enemy_id) {
            Some(actor) => manhattan(player_pos, actor.pos) != 1,
            None => true,
        };
        if should_clear {
            self.suppressed_enemy = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::content::{ContentPack, keys};
    use crate::floor::{BranchProfile, STARTING_FLOOR_INDEX};
    use crate::game::test_support::*;
    use crate::game::visibility::draw_map_diag;
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
                    game.apply_choice(prompt_id, Choice::KeepLoot)
                        .expect("loot choice should apply");
                }
                AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::OpenDoor)
                        .expect("door choice should apply");
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
                    game.apply_choice(prompt_id, choice).expect("descend choice should apply");
                }
                AdvanceStopReason::Finished(_) => break,
                AdvanceStopReason::PausedAtBoundary { .. } | AdvanceStopReason::BudgetExhausted => {
                }
                AdvanceStopReason::EngineFailure(e) => panic!("Engine failure in test: {:?}", e),
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
        assert_eq!(game.state.auto_intent.map(|i| i.reason), Some(AutoReason::ThreatAvoidance));
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
        let discovered_before = game.state.map.discovered.iter().filter(|&&d| d).count();

        let result = game.advance(1);
        assert!(matches!(result.stop_reason, AdvanceStopReason::BudgetExhausted));
        let moved_to = game.state.actors[game.state.player_id].pos;
        assert_eq!(manhattan(start, moved_to), 1, "player should move exactly one tile");
        let discovered_after = game.state.map.discovered.iter().filter(|&&d| d).count();
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
        let (map, pp, dp) = closed_door_choke_fixture();
        game.state.map = map;
        game.state.actors[game.state.player_id].pos = pp;
        compute_fov(&mut game.state.map, pp, FOV_RADIUS);

        // Manually set intent to target the door (which is a frontier candidate)
        game.state.auto_intent =
            Some(AutoExploreIntent { target: dp, reason: AutoReason::Frontier, path_len: 1 });

        let res = game.advance(1);
        if let AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, pos }) =
            res.stop_reason
        {
            assert_eq!(pos, dp);
            game.apply_choice(prompt_id, Choice::OpenDoor).unwrap();
            assert_eq!(game.state.map.tile_at(dp), TileKind::Floor);
        } else {
            panic!(
                "Expected DoorBlocked at {:?}, got {:?}. Map:\n{}",
                dp,
                res.stop_reason,
                draw_map_diag(&game.state.map, pp)
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
        let cnt1 =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(cnt1, 1);
        game.plan_auto_intent(pos);
        let cnt2 =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(cnt2, 1);
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
        let p1 = Pos { y: 3, x: 3 };
        game.state.actors[game.state.player_id].pos = p1;
        game.plan_auto_intent(p1);
        let prev_intent = game.state.auto_intent.unwrap_or_else(|| {
            panic!("No first intent! Map:\n{}", draw_map_diag(&game.state.map, p1));
        });
        assert_eq!(prev_intent.target, Pos { y: 3, x: 8 });

        let p2 = Pos { y: 3, x: 4 };
        game.state.actors[game.state.player_id].pos = p2;
        game.plan_auto_intent(p2);
        let next_intent = game.state.auto_intent.unwrap();
        assert_eq!(prev_intent.target, next_intent.target);
        assert_ne!(prev_intent.path_len, next_intent.path_len);
        let cnt =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(cnt, 1);
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
        let count_after_first =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(count_after_first, 1);

        // Same target, different reason due to hazard fallback.
        game.state.map.set_hazard(Pos { y: 3, x: 7 }, true);
        game.plan_auto_intent(pos);
        let count_after_reason_change =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(count_after_reason_change, 2);

        // No further reason/target change => no extra log.
        game.plan_auto_intent(pos);
        let count_after_repeat =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
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
}
