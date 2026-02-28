//! Per-tick simulation loop and stop-reason handling for the game engine.

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
            self.record_threat_trace();

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

    fn record_threat_trace(&mut self) {
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
        let retreat_triggered = player_hp_pct <= (self.state.policy.retreat_hp_threshold as i32)
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
    }
}
