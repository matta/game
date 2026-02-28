//! Enemy-target ordering logic for encounter prompts.
//! This module sorts adjacent enemies according to policy priority tags and stable tie-breakers.

use std::cmp::Ordering;

use super::*;

impl Game {
    pub(in crate::game) fn sort_adjacent_enemies_by_policy(
        &self,
        pos: Pos,
        mut enemies: Vec<EntityId>,
    ) -> Vec<EntityId> {
        enemies.sort_by(|a_id, b_id| {
            let a = &self.state.actors[*a_id];
            let b = &self.state.actors[*b_id];

            for tag in &self.state.policy.target_priority {
                let cmp = match tag {
                    TargetTag::Nearest => manhattan(pos, a.pos).cmp(&manhattan(pos, b.pos)),
                    TargetTag::LowestHp => a.hp.cmp(&b.hp),
                };
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }

            let dist_cmp = manhattan(pos, a.pos).cmp(&manhattan(pos, b.pos));
            if dist_cmp != Ordering::Equal {
                return dist_cmp;
            }
            let y_cmp = a.pos.y.cmp(&b.pos.y);
            if y_cmp != Ordering::Equal {
                return y_cmp;
            }
            let x_cmp = a.pos.x.cmp(&b.pos.x);
            if x_cmp != Ordering::Equal {
                return x_cmp;
            }
            a.kind.cmp(&b.kind)
        });
        enemies
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentPack;
    use crate::game::test_support::add_goblin;

    #[test]
    fn multi_enemy_interrupt_orders_enemies_and_sets_primary() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player = game.state.actors[game.state.player_id].pos;
        let second = add_goblin(&mut game, Pos { y: player.y + 1, x: player.x });
        let first = add_goblin(&mut game, Pos { y: player.y, x: player.x + 1 });

        let result = game.advance(1);
        assert_eq!(result.simulated_ticks, 0, "interrupt should occur before movement");
        match result.stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                enemies,
                primary_enemy,
                ..
            }) => {
                assert_eq!(enemies, vec![first, second]);
                assert_eq!(primary_enemy, first);
            }
            other => panic!("expected enemy encounter interrupt, got {other:?}"),
        }
    }

    #[test]
    fn policy_driven_target_selection_by_lowest_hp() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        game.state.policy.target_priority = vec![TargetTag::LowestHp, TargetTag::Nearest];

        let player = game.state.actors[game.state.player_id].pos;

        let high_hp_enemy = add_goblin(&mut game, Pos { y: player.y, x: player.x + 1 });
        game.state.actors[high_hp_enemy].hp = 10;

        let low_hp_enemy = add_goblin(&mut game, Pos { y: player.y + 1, x: player.x });
        game.state.actors[low_hp_enemy].hp = 3;

        let result = game.advance(1);
        match result.stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                enemies,
                primary_enemy,
                ..
            }) => {
                assert_eq!(primary_enemy, low_hp_enemy);
                assert_eq!(enemies, vec![low_hp_enemy, high_hp_enemy]);
            }
            other => panic!("expected enemy encounter interrupt, got {other:?}"),
        }
    }
}
