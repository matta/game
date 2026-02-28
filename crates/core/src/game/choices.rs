//! Choice application, combat resolution, and pause-bound policy updates.
//! This module exists to separate decision consequences from prompt generation and tick stepping.
//! It does not own frontier planning primitives or floor construction internals.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::*;
use crate::content::keys;
use crate::game::prompts::PendingPromptKind;
use crate::mapgen::BranchProfile;

impl Game {
    pub(super) fn effective_player_defense(&self) -> i32 {
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

    pub fn apply_choice(
        &mut self,
        prompt_id: ChoicePromptId,
        choice: Choice,
    ) -> Result<(), GameError> {
        let Some(prompt) = self.pending_prompt.clone() else {
            return Err(GameError::PromptMismatch);
        };
        if prompt.id != prompt_id {
            return Err(GameError::PromptMismatch);
        }
        let handled = match (prompt.kind, choice) {
            (PendingPromptKind::Loot { item }, Choice::KeepLoot) => {
                let kind = self.state.items[item].kind;
                self.apply_item_effect(kind);
                self.state.items.remove(item);
                self.log.push(LogEvent::ItemPickedUp { kind });
                true
            }
            (PendingPromptKind::Loot { item }, Choice::DiscardLoot) => {
                let kind = self.state.items[item].kind;
                self.state.items.remove(item);
                self.log.push(LogEvent::ItemDiscarded { kind });
                true
            }
            (PendingPromptKind::EnemyEncounter { primary_enemy, .. }, Choice::Fight) => {
                let mut player_attack = self.state.actors[self.state.player_id].attack;
                let _player_defense = self.effective_player_defense();

                let equipped = self.active_player_weapon();
                let ignores_armor = equipped == Some(keys::WEAPON_PHASE_DAGGER);
                let lifesteal = equipped == Some(keys::WEAPON_BLOOD_AXE);

                if let Some(weapon) = equipped {
                    if weapon == keys::WEAPON_RUSTY_SWORD {
                        player_attack += 2;
                    } else if weapon == keys::WEAPON_IRON_MACE {
                        player_attack += 4;
                    } else if weapon == keys::WEAPON_STEEL_LONGSWORD {
                        player_attack += 6;
                    } else if weapon == keys::WEAPON_PHASE_DAGGER {
                        player_attack += 3;
                    } else if weapon == keys::WEAPON_BLOOD_AXE {
                        player_attack += 6;
                    }
                }

                if self.state.active_perks.contains(&keys::PERK_RECKLESS_STRIKE) {
                    player_attack += 4;
                }
                if self.state.active_perks.contains(&keys::PERK_BERSERKER_RHYTHM)
                    && equipped.is_none()
                {
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
                    let player = self
                        .state
                        .actors
                        .get_mut(self.state.player_id)
                        .expect("player should exist");
                    if has_bloodlust {
                        player.hp = (player.hp + 2).min(player.max_hp);
                    }
                    if lifesteal {
                        player.hp = (player.hp + 1).min(player.max_hp);
                    }
                }

                true
            }
            (PendingPromptKind::EnemyEncounter { primary_enemy, .. }, Choice::Avoid) => {
                let player_pos = self.state.actors[self.state.player_id].pos;
                if self.state.active_god == Some(GodId::Veil) {
                    if let Some(best_pos) = self.choose_blink_destination(player_pos, true) {
                        self.state
                            .actors
                            .get_mut(self.state.player_id)
                            .expect("player should exist")
                            .pos = best_pos;
                        let radius = self.get_fov_radius();
                        compute_fov(&mut self.state.map, best_pos, radius);
                        self.suppressed_enemy = None;
                    } else {
                        self.suppressed_enemy = Some(primary_enemy);
                    }
                } else if self.state.active_perks.contains(&keys::PERK_SHADOW_STEP) {
                    let best_pos =
                        self.choose_blink_destination(player_pos, false).unwrap_or(player_pos);
                    self.state
                        .actors
                        .get_mut(self.state.player_id)
                        .expect("player should exist")
                        .pos = best_pos;
                    let radius = self.get_fov_radius();
                    compute_fov(&mut self.state.map, best_pos, radius);
                    self.suppressed_enemy = None;
                } else {
                    self.suppressed_enemy = Some(primary_enemy);
                }
                true
            }
            (PendingPromptKind::DoorBlocked { pos }, Choice::OpenDoor) => {
                self.state.map.set_tile(pos, TileKind::Floor);
                let radius = self.get_fov_radius();
                compute_fov(
                    &mut self.state.map,
                    self.state.actors[self.state.player_id].pos,
                    radius,
                );
                true
            }
            (
                PendingPromptKind::FloorTransition {
                    current_floor,
                    next_floor,
                    requires_branch_god_choice,
                },
                choice,
            ) if matches!(
                choice,
                Choice::Descend
                    | Choice::DescendBranchAVeil
                    | Choice::DescendBranchAForge
                    | Choice::DescendBranchBVeil
                    | Choice::DescendBranchBForge
            ) =>
            {
                if self.state.floor_index != current_floor {
                    return Err(GameError::InvalidChoice);
                }
                if requires_branch_god_choice
                    && !matches!(
                        choice,
                        Choice::DescendBranchAVeil
                            | Choice::DescendBranchAForge
                            | Choice::DescendBranchBVeil
                            | Choice::DescendBranchBForge
                    )
                {
                    return Err(GameError::InvalidChoice);
                }
                if !requires_branch_god_choice && !matches!(choice, Choice::Descend) {
                    return Err(GameError::InvalidChoice);
                }
                match &choice {
                    Choice::DescendBranchAVeil => {
                        self.state.branch_profile = BranchProfile::BranchA;
                        self.state.active_god = Some(GodId::Veil);
                    }
                    Choice::DescendBranchAForge => {
                        self.state.branch_profile = BranchProfile::BranchA;
                        self.state.active_god = Some(GodId::Forge);
                    }
                    Choice::DescendBranchBVeil => {
                        self.state.branch_profile = BranchProfile::BranchB;
                        self.state.active_god = Some(GodId::Veil);
                    }
                    Choice::DescendBranchBForge => {
                        self.state.branch_profile = BranchProfile::BranchB;
                        self.state.active_god = Some(GodId::Forge);
                    }
                    Choice::Descend => {
                        if self.state.branch_profile == BranchProfile::Uncommitted
                            || self.state.active_god.is_none()
                        {
                            return Err(GameError::InvalidChoice);
                        }
                    }
                    _ => {}
                }
                if requires_branch_god_choice && self.state.active_god == Some(GodId::Forge) {
                    let player = self
                        .state
                        .actors
                        .get_mut(self.state.player_id)
                        .expect("player should exist");
                    player.max_hp += 2;
                    player.hp = (player.hp + 2).min(player.max_hp);
                }
                if self.state.active_perks.contains(&keys::PERK_PACIFISTS_BOUNTY)
                    && self.state.kills_this_floor == 0
                {
                    let player = self
                        .state
                        .actors
                        .get_mut(self.state.player_id)
                        .expect("player should exist");
                    player.max_hp += 5;
                    player.hp = player.max_hp;
                }
                self.state.kills_this_floor = 0;
                match next_floor {
                    Some(next_index) => self.descend_to_floor(next_index),
                    None => {
                        self.finished_outcome = Some(RunOutcome::Victory);
                    }
                }
                true
            }
            _ => false,
        };
        if !handled {
            return Err(GameError::InvalidChoice);
        }
        self.pending_prompt = None;
        self.next_input_seq += 1;
        self.no_progress_ticks = 0;
        Ok(())
    }

    pub fn apply_policy_update(&mut self, update: PolicyUpdate) -> Result<(), GameError> {
        if !self.at_pause_boundary && self.pending_prompt.is_none() {
            return Err(GameError::NotAtPauseBoundary);
        }
        match update {
            PolicyUpdate::FightMode(mode) => self.state.policy.fight_or_avoid = mode,
            PolicyUpdate::Stance(stance) => self.state.policy.stance = stance,
            PolicyUpdate::TargetPriority(target_priority) => {
                self.state.policy.target_priority = target_priority
            }
            PolicyUpdate::RetreatHpThreshold(threshold) => {
                self.state.policy.retreat_hp_threshold = threshold
            }
            PolicyUpdate::AutoHealIfBelowThreshold(threshold) => {
                self.state.policy.auto_heal_if_below_threshold = threshold
            }
            PolicyUpdate::PositionIntent(intent) => self.state.policy.position_intent = intent,
            PolicyUpdate::ResourceAggression(aggression) => {
                self.state.policy.resource_aggression = aggression
            }
            PolicyUpdate::ExplorationMode(mode) => self.state.policy.exploration_mode = mode,
        }
        self.no_progress_ticks = 0;
        Ok(())
    }

    pub fn apply_swap_weapon(&mut self) -> Result<(), GameError> {
        if !self.at_pause_boundary && self.pending_prompt.is_none() {
            return Err(GameError::NotAtPauseBoundary);
        }
        let player = self.state.actors.get_mut(self.state.player_id).expect("player should exist");
        player.active_weapon_slot = match player.active_weapon_slot {
            WeaponSlot::Primary => WeaponSlot::Reserve,
            WeaponSlot::Reserve => WeaponSlot::Primary,
        };
        player.next_action_tick += 10;
        self.no_progress_ticks = 0;
        Ok(())
    }

    pub(super) fn sort_adjacent_enemies_by_policy(
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
    #![allow(unused_imports)]

    use super::*;
    use crate::content::{ContentPack, keys};
    use crate::game::test_support::*;
    use crate::*;

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
    fn multi_enemy_interrupt_orders_enemies_and_sets_primary() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player = game.state.actors[game.state.player_id].pos;
        // Distance is identical, so ordering falls back to y then x.
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

        // Update policy to LowestHP first
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
    fn fighting_primary_enemy_leaves_other_enemy_to_interrupt_next_tick() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player = game.state.actors[game.state.player_id].pos;
        let second = add_goblin(&mut game, Pos { y: player.y + 1, x: player.x });
        let first = add_goblin(&mut game, Pos { y: player.y, x: player.x + 1 });
        game.state.actors[first].hp = 5; // Die in 1 hit

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

        // Player default atk = 5. Default stance = Balanced (+0atk).
        // Damage = 5 - 1 = 4.
        let prompt_balanced = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                prompt_id,
                primary_enemy,
                ..
            }) => {
                assert_eq!(primary_enemy, enemy);
                prompt_id
            }
            _ => panic!("Missing encounter"),
        };

        game.apply_choice(prompt_balanced, Choice::Fight).unwrap();
        assert_eq!(game.state.actors[enemy].hp, 6); // 10 - 4

        // Enemy still alive, advance again should trigger another encounter immediately.
        let prompt_aggressive = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { prompt_id, .. }) => {
                prompt_id
            }
            _ => panic!("Missing encounter 2"),
        };

        // At Pause boundary, update policy to Aggressive (+2atk). Damage = 7 - 1 = 6.
        game.apply_policy_update(PolicyUpdate::Stance(Stance::Aggressive)).unwrap();
        game.apply_choice(prompt_aggressive, Choice::Fight).unwrap();

        // Enemy should take 6 damage, leaving 0 HP, getting removed.
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

        let p_pos = game.state.actors[player].pos;

        // Above threshold: 11/20 = 55%
        game.state.actors[player].hp = 11;
        let enemy1 = add_goblin(&mut game, Pos { y: p_pos.y, x: p_pos.x + 1 });
        game.state.actors[enemy1].hp = 5; // Die in 1 hit

        let prompt1 = match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                retreat_eligible,
                prompt_id,
                ..
            }) => {
                assert!(!retreat_eligible, "Should not be eligible at 55% HP");
                prompt_id
            }
            other => panic!("Missing encounter 1, got {other:?}"),
        };

        game.apply_choice(prompt1, Choice::Fight).unwrap();
        assert!(!game.state.actors.contains_key(enemy1));

        // At threshold: 10/20 = 50%
        game.state.actors[player].hp = 10;
        let p_pos = game.state.actors[player].pos;
        let _enemy2 = add_goblin(&mut game, Pos { y: p_pos.y + 1, x: p_pos.x });

        match game.advance(1).stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                retreat_eligible, ..
            }) => {
                assert!(retreat_eligible, "Should be eligible at 50% HP");
            }
            _ => panic!("Missing encounter 2"),
        }
    }

    #[test]
    fn swap_active_weapon_toggles_slot_and_consumes_ticks() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let p_id = game.state.player_id;

        // Assert initial state
        assert_eq!(game.state.actors[p_id].active_weapon_slot, WeaponSlot::Primary);
        let start_ticks = game.state.actors[p_id].next_action_tick;

        // Perform swap
        game.apply_swap_weapon().expect("Swap should succeed at pause boundary");

        // Verify slot and ticks
        assert_eq!(game.state.actors[p_id].active_weapon_slot, WeaponSlot::Reserve);
        assert_eq!(game.state.actors[p_id].next_action_tick, start_ticks + 10);

        // Swap back
        game.apply_swap_weapon().unwrap();
        assert_eq!(game.state.actors[p_id].active_weapon_slot, WeaponSlot::Primary);
        assert_eq!(game.state.actors[p_id].next_action_tick, start_ticks + 20);
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

        // Move away so the suppressed enemy is no longer adjacent, then advance one tick.
        game.state.actors[game.state.player_id].pos = Pos { y: player.y - 1, x: player.x - 1 };
        let _ = game.advance(1);
        assert_eq!(game.suppressed_enemy, None);
    }
}
