//! Encounter-avoidance handling and blink destination selection.
//! This module resolves `Choice::Avoid` outcomes and Veil/Shadow Step movement.

use std::collections::BTreeSet;

use super::*;
use crate::content::keys;

impl Game {
    pub(super) fn choose_blink_destination(
        &self,
        player_pos: Pos,
        avoid_hazards: bool,
    ) -> Option<Pos> {
        let occupied: BTreeSet<Pos> = self.state.actors.values().map(|actor| actor.pos).collect();
        let mut best: Option<(u32, Pos)> = None;
        for y in (player_pos.y - 3)..=(player_pos.y + 3) {
            for x in (player_pos.x - 3)..=(player_pos.x + 3) {
                let pos = Pos { y, x };
                if !self.state.map.is_discovered_walkable(pos)
                    || self.state.map.tile_at(pos) == TileKind::ClosedDoor
                    || occupied.contains(&pos)
                {
                    continue;
                }
                if avoid_hazards && self.state.map.is_hazard(pos) {
                    continue;
                }
                let distance = manhattan(player_pos, pos);
                let is_better = match best {
                    None => true,
                    Some((best_distance, best_pos)) => {
                        distance > best_distance
                            || (distance == best_distance
                                && (pos.y, pos.x) < (best_pos.y, best_pos.x))
                    }
                };
                if is_better {
                    best = Some((distance, pos));
                }
            }
        }
        best.map(|(_, pos)| pos)
    }

    pub(super) fn resolve_avoid_choice(&mut self, primary_enemy: EntityId) {
        let player_pos = self.state.actors[self.state.player_id].pos;
        if self.state.active_god == Some(GodId::Veil) {
            if let Some(best_pos) = self.choose_blink_destination(player_pos, true) {
                self.state.actors.get_mut(self.state.player_id).expect("player should exist").pos =
                    best_pos;
                let radius = self.get_fov_radius();
                compute_fov(&mut self.state.map, best_pos, radius);
                self.suppressed_enemy = None;
            } else {
                self.suppressed_enemy = Some(primary_enemy);
            }
        } else if self.state.active_perks.contains(&keys::PERK_SHADOW_STEP) {
            let best_pos = self.choose_blink_destination(player_pos, false).unwrap_or(player_pos);
            self.state.actors.get_mut(self.state.player_id).expect("player should exist").pos =
                best_pos;
            let radius = self.get_fov_radius();
            compute_fov(&mut self.state.map, best_pos, radius);
            self.suppressed_enemy = None;
        } else {
            self.suppressed_enemy = Some(primary_enemy);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Map;
    use crate::content::ContentPack;
    use crate::game::test_support::add_goblin;

    #[test]
    fn veil_avoid_blinks_to_farthest_safe_tile() {
        let mut game = Game::new(998877, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        game.state.active_god = Some(GodId::Veil);

        let mut map = Map::new(9, 9);
        for y in 1..8 {
            for x in 1..8 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        map.set_hazard(Pos { y: 7, x: 7 }, true);
        game.state.map = map;

        let player_pos = Pos { y: 4, x: 4 };
        game.state.actors[game.state.player_id].pos = player_pos;
        let enemy_id = add_goblin(&mut game, Pos { y: 4, x: 5 });
        game.state.actors[enemy_id].hp = 99;

        let prompt_id = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { prompt_id, .. }) => {
                prompt_id
            }
            other => panic!("expected enemy encounter interrupt, got {other:?}"),
        };
        game.apply_choice(prompt_id, Choice::Avoid).expect("avoid should apply");

        assert_eq!(game.state.actors[game.state.player_id].pos, Pos { y: 1, x: 1 });
    }

    #[test]
    fn veil_avoid_falls_back_to_suppression_when_no_safe_blink_exists() {
        let mut game = Game::new(445566, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        game.state.active_god = Some(GodId::Veil);

        let mut map = Map::new(7, 7);
        for y in 1..6 {
            for x in 1..6 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        let player_pos = Pos { y: 3, x: 3 };
        let enemy_pos = Pos { y: 3, x: 4 };
        map.set_tile(player_pos, TileKind::Floor);
        map.set_tile(enemy_pos, TileKind::Floor);
        map.discovered.fill(true);
        map.visible.fill(true);
        game.state.map = map;
        game.state.actors[game.state.player_id].pos = player_pos;
        let enemy_id = add_goblin(&mut game, enemy_pos);

        let prompt_id = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { prompt_id, .. }) => {
                prompt_id
            }
            other => panic!("expected enemy encounter interrupt, got {other:?}"),
        };
        game.apply_choice(prompt_id, Choice::Avoid).expect("avoid should apply");

        assert_eq!(game.state.actors[game.state.player_id].pos, player_pos);
        assert_eq!(game.suppressed_enemy, Some(enemy_id));
    }

    #[test]
    fn avoid_suppresses_only_primary_enemy_and_still_interrupts_on_other_enemy() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player = game.state.actors[game.state.player_id].pos;
        let second = add_goblin(&mut game, Pos { y: player.y + 1, x: player.x });
        let first = add_goblin(&mut game, Pos { y: player.y, x: player.x + 1 });

        let first_prompt = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                prompt_id,
                primary_enemy,
                ..
            }) => {
                assert_eq!(primary_enemy, first);
                prompt_id
            }
            other => panic!("expected first enemy encounter, got {other:?}"),
        };
        game.apply_choice(first_prompt, Choice::Avoid).expect("avoid should apply");

        match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                primary_enemy,
                enemies,
                ..
            }) => {
                assert_eq!(primary_enemy, second, "second enemy should now be primary");
                assert_eq!(enemies, vec![second], "suppressed enemy should be omitted");
            }
            other => panic!("expected second enemy encounter, got {other:?}"),
        }
    }

    #[test]
    fn suppressed_enemy_clears_after_it_is_no_longer_adjacent() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player = game.state.actors[game.state.player_id].pos;
        let enemy = add_goblin(&mut game, Pos { y: player.y, x: player.x + 1 });

        let prompt_id = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                prompt_id,
                primary_enemy,
                ..
            }) => {
                assert_eq!(primary_enemy, enemy);
                prompt_id
            }
            other => panic!("expected enemy encounter, got {other:?}"),
        };
        game.apply_choice(prompt_id, Choice::Avoid).expect("avoid should apply");
        assert_eq!(game.suppressed_enemy, Some(enemy));

        game.state.actors[game.state.player_id].pos = Pos { y: player.y - 1, x: player.x - 1 };
        let _ = game.advance(1);
        assert_eq!(game.suppressed_enemy, None);
    }
}
