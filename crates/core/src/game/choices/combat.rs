//! Combat-choice handlers and attack/defense math.
//! This module resolves fight outcomes and combat-side perk/weapon effects.

use super::*;
use crate::content::keys;

impl Game {
    pub(in crate::game) fn effective_player_defense(&self) -> i32 {
        let mut defense = self.state.actors[self.state.player_id].defense;
        if self.state.active_perks.contains(&keys::PERK_IRON_WILL) {
            defense += 2;
        }
        if self.state.active_perks.contains(&keys::PERK_RECKLESS_STRIKE) {
            defense -= 2;
        }
        match self.state.policy.stance {
            Stance::Aggressive => defense -= 1,
            Stance::Balanced => {}
            Stance::Defensive => defense += 2,
        }
        if self.state.active_god == Some(GodId::Forge) {
            defense += 2;
        }
        defense
    }

    pub(super) fn resolve_fight_choice(&mut self, primary_enemy: EntityId) {
        let mut player_attack = self.state.actors[self.state.player_id].attack;
        let _player_defense = self.effective_player_defense();

        let equipped = self.active_player_weapon();
        let ignores_armor = equipped == Some(keys::WEAPON_PHASE_DAGGER);
        let lifesteal = equipped == Some(keys::WEAPON_BLOOD_AXE);

        if let Some(weapon) = equipped {
            player_attack += Self::weapon_attack_bonus(weapon);
        }

        if self.state.active_perks.contains(&keys::PERK_RECKLESS_STRIKE) {
            player_attack += 4;
        }
        if self.state.active_perks.contains(&keys::PERK_BERSERKER_RHYTHM) && equipped.is_none() {
            player_attack += 3;
        }

        match self.state.policy.stance {
            Stance::Aggressive => {
                player_attack += 2;
            }
            Stance::Balanced => {}
            Stance::Defensive => {
                player_attack -= 1;
            }
        }

        let mut enemy_defense = self.state.actors[primary_enemy].defense;
        if ignores_armor {
            enemy_defense = 0;
        }

        let damage = (player_attack.saturating_sub(enemy_defense)).max(1);

        let enemy_actor =
            self.state.actors.get_mut(primary_enemy).expect("primary enemy should exist");
        enemy_actor.hp -= damage;

        self.log.push(LogEvent::EncounterResolved { enemy: primary_enemy, fought: true });

        if enemy_actor.hp <= 0 {
            self.state.actors.remove(primary_enemy);
            self.state.kills_this_floor += 1;

            let has_bloodlust = self.state.active_perks.contains(&keys::PERK_BLOODLUST);
            let player =
                self.state.actors.get_mut(self.state.player_id).expect("player should exist");
            if has_bloodlust {
                player.hp = (player.hp + 2).min(player.max_hp);
            }
            if lifesteal {
                player.hp = (player.hp + 1).min(player.max_hp);
            }
        }
    }

    fn weapon_attack_bonus(weapon: &'static str) -> i32 {
        match weapon {
            keys::WEAPON_RUSTY_SWORD => 2,
            keys::WEAPON_IRON_MACE => 4,
            keys::WEAPON_STEEL_LONGSWORD => 6,
            keys::WEAPON_PHASE_DAGGER => 3,
            keys::WEAPON_BLOOD_AXE => 6,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{ContentPack, keys};
    use crate::game::test_support::add_goblin;

    #[test]
    fn fighting_primary_enemy_leaves_other_enemy_to_interrupt_next_tick() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player = game.state.actors[game.state.player_id].pos;
        let second = add_goblin(&mut game, Pos { y: player.y + 1, x: player.x });
        let first = add_goblin(&mut game, Pos { y: player.y, x: player.x + 1 });
        game.state.actors[first].hp = 5;

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
        game.apply_choice(first_prompt, Choice::Fight).expect("fight should apply");
        assert!(!game.state.actors.contains_key(first), "primary enemy should be removed");
        assert!(game.state.actors.contains_key(second), "other enemy should remain");

        match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                primary_enemy,
                enemies,
                ..
            }) => {
                assert_eq!(primary_enemy, second);
                assert_eq!(enemies, vec![second]);
            }
            other => panic!("expected follow-up enemy encounter, got {other:?}"),
        }
    }

    #[test]
    fn stance_modifiers_affect_combat_damage() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player = game.state.actors[game.state.player_id].pos;
        let enemy = add_goblin(&mut game, Pos { y: player.y, x: player.x + 1 });
        game.state.actors[enemy].hp = 10;
        game.state.actors[enemy].max_hp = 10;
        game.state.actors[enemy].defense = 1;

        let prompt_balanced = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                prompt_id,
                primary_enemy,
                ..
            }) => {
                assert_eq!(primary_enemy, enemy);
                prompt_id
            }
            _ => panic!("missing encounter"),
        };

        game.apply_choice(prompt_balanced, Choice::Fight).unwrap();
        assert_eq!(game.state.actors[enemy].hp, 6);

        let prompt_aggressive = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { prompt_id, .. }) => {
                prompt_id
            }
            _ => panic!("missing encounter 2"),
        };

        game.apply_policy_update(PolicyUpdate::Stance(Stance::Aggressive)).unwrap();
        game.apply_choice(prompt_aggressive, Choice::Fight).unwrap();

        assert!(!game.state.actors.contains_key(enemy));
    }

    #[test]
    fn retreat_eligible_is_true_when_hp_percent_is_below_threshold() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        game.state.policy.retreat_hp_threshold = 50;

        let player = game.state.player_id;
        game.state.actors[player].max_hp = 20;

        let player_position = game.state.actors[player].pos;

        game.state.actors[player].hp = 11;
        let enemy1 = add_goblin(&mut game, Pos { y: player_position.y, x: player_position.x + 1 });
        game.state.actors[enemy1].hp = 5;

        let prompt1 = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                retreat_eligible,
                prompt_id,
                ..
            }) => {
                assert!(!retreat_eligible, "should not be eligible at 55% HP");
                prompt_id
            }
            other => panic!("missing encounter 1, got {other:?}"),
        };

        game.apply_choice(prompt1, Choice::Fight).unwrap();
        assert!(!game.state.actors.contains_key(enemy1));

        game.state.actors[player].hp = 10;
        let player_position = game.state.actors[player].pos;
        let _enemy2 = add_goblin(&mut game, Pos { y: player_position.y + 1, x: player_position.x });

        match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                retreat_eligible, ..
            }) => {
                assert!(retreat_eligible, "should be eligible at 50% HP");
            }
            _ => panic!("missing encounter 2"),
        }
    }

    #[test]
    fn swap_active_weapon_changes_combat_damage_output() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player_id = game.state.player_id;
        game.state.actors[player_id].equipped_weapon = Some(keys::WEAPON_RUSTY_SWORD);
        game.state.actors[player_id].reserve_weapon = Some(keys::WEAPON_STEEL_LONGSWORD);
        game.state.actors[player_id].active_weapon_slot = WeaponSlot::Primary;

        let player_pos = game.state.actors[player_id].pos;
        let enemy_id = add_goblin(&mut game, Pos { y: player_pos.y, x: player_pos.x + 1 });
        game.state.actors[enemy_id].hp = 20;
        game.state.actors[enemy_id].max_hp = 20;
        game.state.actors[enemy_id].defense = 0;

        game.apply_swap_weapon().expect("swap should be allowed at pause boundary");
        assert_eq!(game.state.actors[player_id].active_weapon_slot, WeaponSlot::Reserve);

        let prompt_id = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                prompt_id,
                primary_enemy,
                ..
            }) => {
                assert_eq!(primary_enemy, enemy_id);
                prompt_id
            }
            other => panic!("expected enemy encounter interrupt, got {other:?}"),
        };
        game.apply_choice(prompt_id, Choice::Fight).expect("fight choice should apply");

        assert_eq!(
            game.state.actors[enemy_id].hp, 9,
            "reserve weapon should be used after swap (20 - (5 + 6) = 9)"
        );
    }
}
