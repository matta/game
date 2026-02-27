use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry};

use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;

use crate::content::{ContentPack, get_enemy_stats, keys};
use crate::floor::{BranchProfile, MAX_FLOORS, STARTING_FLOOR_INDEX, generate_floor};
use crate::state::{Actor, GameState, Item, Map};
use crate::types::*;

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingPromptKind {
    Loot {
        item: ItemId,
    },
    EnemyEncounter {
        enemies: Vec<EntityId>,
        primary_enemy: EntityId,
        retreat_eligible: bool,
        threat: ThreatSummary,
    },
    DoorBlocked {
        pos: Pos,
    },
    FloorTransition {
        current_floor: u8,
        next_floor: Option<u8>,
        requires_branch_god_choice: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingPrompt {
    id: ChoicePromptId,
    kind: PendingPromptKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OpenNode {
    f: u32,
    h: u32,
    y: i32,
    x: i32,
}

const FOV_RADIUS: i32 = 10;
const MAX_NO_PROGRESS_TICKS: u32 = 64;

pub struct Game {
    seed: u64,
    tick: u64,
    #[expect(dead_code)]
    rng: ChaCha8Rng,
    state: GameState,
    log: Vec<LogEvent>,
    next_input_seq: u64,
    pending_prompt: Option<PendingPrompt>,
    suppressed_enemy: Option<EntityId>,
    pause_requested: bool,
    at_pause_boundary: bool,
    finished_outcome: Option<RunOutcome>,
    no_progress_ticks: u32,
}

impl Game {
    pub fn new(seed: u64, _content: &ContentPack, _mode: GameMode) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(seed);
        let mut actors = slotmap::SlotMap::with_key();
        let player = Actor {
            id: EntityId::default(),
            kind: ActorKind::Player,
            pos: Pos { y: 5, x: 4 },
            hp: 20,
            max_hp: 20,
            attack: 5,
            defense: 0,
            active_weapon_slot: WeaponSlot::Primary,
            equipped_weapon: None,
            reserve_weapon: None,
            next_action_tick: 10,
            speed: 10,
        };
        let player_id = actors.insert(player);
        actors[player_id].id = player_id;

        let stats_a = get_enemy_stats(ActorKind::Goblin);
        let enemy_a = Actor {
            id: EntityId::default(),
            kind: ActorKind::Goblin,
            pos: Pos { y: 5, x: 11 },
            hp: stats_a.hp,
            max_hp: stats_a.hp,
            attack: stats_a.attack,
            defense: stats_a.defense,
            active_weapon_slot: WeaponSlot::Primary,
            equipped_weapon: None,
            reserve_weapon: None,
            next_action_tick: stats_a.speed as u64,
            speed: stats_a.speed,
        };
        let enemy_a_id = actors.insert(enemy_a);
        actors[enemy_a_id].id = enemy_a_id;

        let stats_b = get_enemy_stats(ActorKind::Goblin);
        let enemy_b = Actor {
            id: EntityId::default(),
            kind: ActorKind::Goblin,
            pos: Pos { y: 11, x: 11 },
            hp: stats_b.hp,
            max_hp: stats_b.hp,
            attack: stats_b.attack,
            defense: stats_b.defense,
            active_weapon_slot: WeaponSlot::Primary,
            equipped_weapon: None,
            reserve_weapon: None,
            next_action_tick: stats_b.speed as u64,
            speed: stats_b.speed,
        };
        let enemy_b_id = actors.insert(enemy_b);
        actors[enemy_b_id].id = enemy_b_id;

        let stats_c = get_enemy_stats(ActorKind::Goblin);
        let enemy_c = Actor {
            id: EntityId::default(),
            kind: ActorKind::Goblin,
            pos: Pos { y: 6, x: 10 },
            hp: stats_c.hp,
            max_hp: stats_c.hp,
            attack: stats_c.attack,
            defense: stats_c.defense,
            active_weapon_slot: WeaponSlot::Primary,
            equipped_weapon: None,
            reserve_weapon: None,
            next_action_tick: stats_c.speed as u64,
            speed: stats_c.speed,
        };
        let enemy_c_id = actors.insert(enemy_c);
        actors[enemy_c_id].id = enemy_c_id;

        let stats_d = get_enemy_stats(ActorKind::Goblin);
        let enemy_d = Actor {
            id: EntityId::default(),
            kind: ActorKind::Goblin,
            pos: Pos { y: 7, x: 9 },
            hp: stats_d.hp,
            max_hp: stats_d.hp,
            attack: stats_d.attack,
            defense: stats_d.defense,
            active_weapon_slot: WeaponSlot::Primary,
            equipped_weapon: None,
            reserve_weapon: None,
            next_action_tick: stats_d.speed as u64,
            speed: stats_d.speed,
        };
        let enemy_d_id = actors.insert(enemy_d);
        actors[enemy_d_id].id = enemy_d_id;

        let mut map = Map::new(20, 15);

        for y in 1..(map.internal_height - 1) {
            for x in 1..(map.internal_width - 1) {
                map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
            }
        }

        // Room A floor rectangle: x=2..6, y=3..7.
        for y in 3..=7 {
            for x in 2..=6 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        // Room B floor rectangle: x=9..13, y=3..7.
        for y in 3..=7 {
            for x in 9..=13 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        // Room C floor rectangle: x=9..13, y=9..13.
        for y in 9..=13 {
            for x in 9..=13 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }

        // Corridor A<->B with closed door at (8,5).
        map.set_tile(Pos { y: 5, x: 7 }, TileKind::Floor);
        map.set_tile(Pos { y: 5, x: 8 }, TileKind::ClosedDoor);

        // Corridor B<->C floor tiles.
        map.set_tile(Pos { y: 8, x: 11 }, TileKind::Floor);
        map.set_tile(Pos { y: 9, x: 11 }, TileKind::Floor);

        // Hazard lane tiles.
        map.set_hazard(Pos { y: 8, x: 11 }, true);
        map.set_hazard(Pos { y: 9, x: 11 }, true);
        map.set_hazard(Pos { y: 10, x: 11 }, true);

        // One-way descent anchor for floor 1.
        map.set_tile(Pos { y: 11, x: 13 }, TileKind::DownStairs);

        let mut items = slotmap::SlotMap::with_key();
        let item = Item {
            id: ItemId::default(),
            kind: ItemKind::Consumable(keys::CONSUMABLE_MINOR_HP_POT),
            pos: Pos { y: 5, x: 6 },
        };
        let item_id = items.insert(item);
        items[item_id].id = item_id;

        compute_fov(&mut map, actors[player_id].pos, FOV_RADIUS);

        Self {
            seed,
            tick: 0,
            rng,
            state: GameState {
                map,
                actors,
                items,
                player_id,
                sanctuary_tile: Pos { y: 5, x: 4 },
                sanctuary_active: false,
                floor_index: STARTING_FLOOR_INDEX,
                branch_profile: BranchProfile::Uncommitted,
                active_god: None,
                auto_intent: None,
                policy: Policy::default(),
                threat_trace: VecDeque::new(),
                active_perks: Vec::new(),
                kills_this_floor: 0,
            },
            log: Vec::new(),
            next_input_seq: 0,
            pending_prompt: None,
            suppressed_enemy: None,
            pause_requested: false,
            at_pause_boundary: true,
            finished_outcome: None,
            no_progress_ticks: 0,
        }
    }

    pub fn get_fov_radius(&self) -> i32 {
        if self.state.active_perks.contains(&keys::PERK_SCOUT) {
            FOV_RADIUS + 2
        } else {
            FOV_RADIUS
        }
    }

    fn effective_player_defense(&self) -> i32 {
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

    fn choose_blink_destination(&self, player_pos: Pos, avoid_hazards: bool) -> Option<Pos> {
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
                let r = self.get_fov_radius();
                compute_fov(&mut self.state.map, next_step, r);
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
            let p_hp_pct = (self.state.actors[self.state.player_id].hp * 100)
                / self.state.actors[self.state.player_id].max_hp;
            let retreat_triggered = p_hp_pct <= (self.state.policy.retreat_hp_threshold as i32)
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
                let mut p_attack = self.state.actors[self.state.player_id].attack;
                let _player_defense = self.effective_player_defense();

                let equipped = self.active_player_weapon();
                let ignores_armor = equipped == Some(keys::WEAPON_PHASE_DAGGER);
                let lifesteal = equipped == Some(keys::WEAPON_BLOOD_AXE);

                if let Some(w) = equipped {
                    if w == keys::WEAPON_RUSTY_SWORD {
                        p_attack += 2;
                    } else if w == keys::WEAPON_IRON_MACE {
                        p_attack += 4;
                    } else if w == keys::WEAPON_STEEL_LONGSWORD {
                        p_attack += 6;
                    } else if w == keys::WEAPON_PHASE_DAGGER {
                        p_attack += 3;
                    } else if w == keys::WEAPON_BLOOD_AXE {
                        p_attack += 6;
                    }
                }

                if self.state.active_perks.contains(&keys::PERK_RECKLESS_STRIKE) {
                    p_attack += 4;
                }
                if self.state.active_perks.contains(&keys::PERK_BERSERKER_RHYTHM)
                    && equipped.is_none()
                {
                    p_attack += 3;
                }

                match self.state.policy.stance {
                    Stance::Aggressive => {
                        p_attack += 2;
                    }
                    Stance::Balanced => {}
                    Stance::Defensive => {
                        p_attack -= 1;
                    }
                }

                let mut e_defense = self.state.actors[primary_enemy].defense;
                if ignores_armor {
                    e_defense = 0;
                }

                let damage = (p_attack.saturating_sub(e_defense)).max(1);

                let e_actor = self.state.actors.get_mut(primary_enemy).unwrap();
                e_actor.hp -= damage;

                self.log.push(LogEvent::EncounterResolved { enemy: primary_enemy, fought: true });

                if e_actor.hp <= 0 {
                    self.state.actors.remove(primary_enemy);
                    self.state.kills_this_floor += 1;

                    let has_bloodlust = self.state.active_perks.contains(&keys::PERK_BLOODLUST);
                    let player = self.state.actors.get_mut(self.state.player_id).unwrap();
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
                        self.state.actors.get_mut(self.state.player_id).unwrap().pos = best_pos;
                        let r = self.get_fov_radius();
                        compute_fov(&mut self.state.map, best_pos, r);
                        self.suppressed_enemy = None;
                    } else {
                        self.suppressed_enemy = Some(primary_enemy);
                    }
                } else if self.state.active_perks.contains(&keys::PERK_SHADOW_STEP) {
                    let best_pos =
                        self.choose_blink_destination(player_pos, false).unwrap_or(player_pos);
                    self.state.actors.get_mut(self.state.player_id).unwrap().pos = best_pos;
                    let r = self.get_fov_radius();
                    compute_fov(&mut self.state.map, best_pos, r);
                    self.suppressed_enemy = None;
                } else {
                    self.suppressed_enemy = Some(primary_enemy);
                }
                true
            }
            (PendingPromptKind::DoorBlocked { pos }, Choice::OpenDoor) => {
                self.state.map.set_tile(pos, TileKind::Floor);
                let r = self.get_fov_radius();
                compute_fov(&mut self.state.map, self.state.actors[self.state.player_id].pos, r);
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
                    let player = self.state.actors.get_mut(self.state.player_id).unwrap();
                    player.max_hp += 2;
                    player.hp = (player.hp + 2).min(player.max_hp);
                }
                if self.state.active_perks.contains(&keys::PERK_PACIFISTS_BOUNTY)
                    && self.state.kills_this_floor == 0
                {
                    let player = self.state.actors.get_mut(self.state.player_id).unwrap();
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
            PolicyUpdate::FightMode(m) => self.state.policy.fight_or_avoid = m,
            PolicyUpdate::Stance(s) => self.state.policy.stance = s,
            PolicyUpdate::TargetPriority(t) => self.state.policy.target_priority = t,
            PolicyUpdate::RetreatHpThreshold(h) => self.state.policy.retreat_hp_threshold = h,
            PolicyUpdate::AutoHealIfBelowThreshold(h) => {
                self.state.policy.auto_heal_if_below_threshold = h
            }
            PolicyUpdate::PositionIntent(i) => self.state.policy.position_intent = i,
            PolicyUpdate::ResourceAggression(a) => self.state.policy.resource_aggression = a,
            PolicyUpdate::ExplorationMode(e) => self.state.policy.exploration_mode = e,
        }
        self.no_progress_ticks = 0;
        Ok(())
    }

    pub fn apply_swap_weapon(&mut self) -> Result<(), GameError> {
        if !self.at_pause_boundary && self.pending_prompt.is_none() {
            return Err(GameError::NotAtPauseBoundary);
        }
        let player = self.state.actors.get_mut(self.state.player_id).unwrap();
        player.active_weapon_slot = match player.active_weapon_slot {
            WeaponSlot::Primary => WeaponSlot::Reserve,
            WeaponSlot::Reserve => WeaponSlot::Primary,
        };
        player.next_action_tick += 10;
        self.no_progress_ticks = 0;
        Ok(())
    }

    pub fn snapshot_hash(&self) -> u64 {
        use std::hash::Hasher;
        use xxhash_rust::xxh3::Xxh3;
        let mut hasher = Xxh3::new();
        hasher.write_u64(self.seed);
        hasher.write_u64(self.tick);
        hasher.write_u64(self.next_input_seq);
        hasher.write_u32(self.no_progress_ticks);
        hasher.write_u8(self.state.floor_index);
        hasher.write_u8(match self.state.branch_profile {
            BranchProfile::Uncommitted => 0,
            BranchProfile::BranchA => 1,
            BranchProfile::BranchB => 2,
        });
        hasher.write_u8(match self.state.active_god {
            None => 0,
            Some(GodId::Veil) => 1,
            Some(GodId::Forge) => 2,
        });
        let player = &self.state.actors[self.state.player_id];
        hasher.write_i32(player.pos.x);
        hasher.write_i32(player.pos.y);
        hasher.write_i32(self.state.sanctuary_tile.x);
        hasher.write_i32(self.state.sanctuary_tile.y);
        hasher.write_u8(u8::from(self.state.sanctuary_active));
        if let Some(intent) = self.state.auto_intent {
            hasher.write_i32(intent.target.x);
            hasher.write_i32(intent.target.y);
            hasher.write_u16(intent.path_len);
            hasher.write_u8(intent.reason as u8);
        }
        hasher.finish()
    }

    pub fn current_tick(&self) -> u64 {
        self.tick
    }
    pub fn request_pause(&mut self) {
        self.pause_requested = true;
    }
    pub fn state(&self) -> &GameState {
        &self.state
    }
    pub fn log(&self) -> &[LogEvent] {
        &self.log
    }

    fn active_player_weapon(&self) -> Option<&'static str> {
        let player = &self.state.actors[self.state.player_id];
        match player.active_weapon_slot {
            WeaponSlot::Primary => player.equipped_weapon,
            WeaponSlot::Reserve => player.reserve_weapon,
        }
    }

    fn visible_enemy_ids_sorted(&self, distance_from: Option<Pos>) -> Vec<EntityId> {
        let mut ids: Vec<EntityId> = self
            .state
            .actors
            .iter()
            .filter(|(id, actor)| {
                *id != self.state.player_id && self.state.map.is_visible(actor.pos)
            })
            .map(|(id, _)| id)
            .collect();
        ids.sort_by(|a_id, b_id| {
            let a = &self.state.actors[*a_id];
            let b = &self.state.actors[*b_id];

            let distance_cmp = distance_from.map_or(Ordering::Equal, |origin| {
                manhattan(origin, a.pos).cmp(&manhattan(origin, b.pos))
            });
            if distance_cmp != Ordering::Equal {
                return distance_cmp;
            }
            let y_cmp = a.pos.y.cmp(&b.pos.y);
            if y_cmp != Ordering::Equal {
                return y_cmp;
            }
            let x_cmp = a.pos.x.cmp(&b.pos.x);
            if x_cmp != Ordering::Equal {
                return x_cmp;
            }
            a.kind
                .cmp(&b.kind)
                .then(a.hp.cmp(&b.hp))
                .then(a.next_action_tick.cmp(&b.next_action_tick))
        });
        ids
    }

    fn apply_item_effect(&mut self, kind: ItemKind) {
        match kind {
            ItemKind::Weapon(id) => {
                let player = self.state.actors.get_mut(self.state.player_id).unwrap();
                if player.equipped_weapon.is_none() {
                    player.equipped_weapon = Some(id);
                } else if player.reserve_weapon.is_none() {
                    player.reserve_weapon = Some(id);
                } else {
                    match player.active_weapon_slot {
                        WeaponSlot::Primary => player.equipped_weapon = Some(id),
                        WeaponSlot::Reserve => player.reserve_weapon = Some(id),
                    }
                }
            }
            ItemKind::Perk(id) => {
                if !self.state.active_perks.contains(&id) {
                    self.state.active_perks.push(id);
                }
            }
            ItemKind::Consumable(id) => match id {
                keys::CONSUMABLE_MINOR_HP_POT => {
                    let player = self.state.actors.get_mut(self.state.player_id).unwrap();
                    player.hp = (player.hp + 10).min(player.max_hp);
                }
                keys::CONSUMABLE_MAJOR_HP_POT => {
                    let player = self.state.actors.get_mut(self.state.player_id).unwrap();
                    player.hp = (player.hp + 25).min(player.max_hp);
                }
                keys::CONSUMABLE_TELEPORT_RUNE => {
                    let player_pos = self.state.actors[self.state.player_id].pos;
                    let nearest =
                        self.visible_enemy_ids_sorted(Some(player_pos)).into_iter().next();
                    if let Some(e_id) = nearest {
                        let e_pos = self.state.actors[e_id].pos;
                        self.state.actors.get_mut(self.state.player_id).unwrap().pos = e_pos;
                        self.state.actors.get_mut(e_id).unwrap().pos = player_pos;
                    }
                }
                keys::CONSUMABLE_FORTIFICATION_SCROLL => {
                    let player_pos = self.state.actors[self.state.player_id].pos;
                    let occupied_positions: BTreeSet<Pos> =
                        self.state.actors.values().map(|actor| actor.pos).collect();
                    let mut fortified_map = self.state.map.clone();
                    let mut reachable_before =
                        reachable_discovered_walkable_tiles(&fortified_map, player_pos);
                    let had_intent_before =
                        choose_frontier_intent(&fortified_map, player_pos).is_some();

                    for neighbor in neighbors(player_pos) {
                        if !fortified_map.is_discovered_walkable(neighbor)
                            || fortified_map.tile_at(neighbor) == TileKind::DownStairs
                            || occupied_positions.contains(&neighbor)
                            || is_frontier_candidate(&fortified_map, neighbor)
                        {
                            continue;
                        }

                        let original_tile = fortified_map.tile_at(neighbor);
                        fortified_map.set_tile(neighbor, TileKind::Wall);

                        let reachable_after =
                            reachable_discovered_walkable_tiles(&fortified_map, player_pos);
                        let preserves_reachable_component = reachable_before
                            .iter()
                            .all(|pos| *pos == neighbor || reachable_after.contains(pos));
                        let preserves_progress_intent = !had_intent_before
                            || choose_frontier_intent(&fortified_map, player_pos).is_some();

                        if preserves_reachable_component && preserves_progress_intent {
                            reachable_before = reachable_after;
                        } else {
                            fortified_map.set_tile(neighbor, original_tile);
                        }
                    }
                    self.state.map = fortified_map;
                    let r = self.get_fov_radius();
                    compute_fov(
                        &mut self.state.map,
                        self.state.actors[self.state.player_id].pos,
                        r,
                    );
                }
                keys::CONSUMABLE_STASIS_HOURGLASS => {
                    for e_id in self.visible_enemy_ids_sorted(None) {
                        self.state.actors.get_mut(e_id).unwrap().next_action_tick += 50;
                    }
                }
                keys::CONSUMABLE_MAGNETIC_LURE => {
                    let player_pos = self.state.actors[self.state.player_id].pos;
                    let mut moves = Vec::new();
                    for e_id in self.visible_enemy_ids_sorted(Some(player_pos)) {
                        let actor_pos = self.state.actors[e_id].pos;
                        if let Some(path) = astar_path(&self.state.map, actor_pos, player_pos)
                            && let Some(next_step) = path.first().copied()
                        {
                            moves.push((e_id, actor_pos, next_step));
                        }
                    }
                    let mut occupied: BTreeSet<Pos> =
                        self.state.actors.values().map(|actor| actor.pos).collect();
                    for (e_id, from_pos, target_pos) in moves {
                        if !occupied.contains(&target_pos) {
                            occupied.remove(&from_pos);
                            occupied.insert(target_pos);
                            self.state.actors.get_mut(e_id).unwrap().pos = target_pos;
                        }
                    }
                }
                keys::CONSUMABLE_SMOKE_BOMB => {
                    self.state.threat_trace.clear();
                    self.suppressed_enemy = None;
                    for e_id in self.visible_enemy_ids_sorted(None) {
                        self.state.actors.get_mut(e_id).unwrap().next_action_tick += 20;
                    }
                }
                keys::CONSUMABLE_SHRAPNEL_BOMB => {
                    let mut to_remove = Vec::new();
                    for e_id in self.visible_enemy_ids_sorted(None) {
                        let actor = self.state.actors.get_mut(e_id).unwrap();
                        actor.hp -= 5;
                        if actor.hp <= 0 {
                            to_remove.push(e_id);
                        }
                    }
                    for e_id in to_remove {
                        self.state.actors.remove(e_id);
                    }
                }
                keys::CONSUMABLE_HASTE_POTION => {
                    let tick = self.tick;
                    let player = self.state.actors.get_mut(self.state.player_id).unwrap();
                    let target = player.next_action_tick.saturating_sub(50);
                    player.next_action_tick = target.max(tick + 1);
                }
                keys::CONSUMABLE_IRON_SKIN_POTION => {
                    let player = self.state.actors.get_mut(self.state.player_id).unwrap();
                    player.max_hp += 5;
                    player.hp += 5;
                }
                _ => {}
            },
        }
    }

    fn interrupt_loot(&mut self, item: ItemId, steps: u32) -> AdvanceResult {
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::Loot { item },
        };
        self.pending_prompt = Some(prompt.clone());
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }
    fn interrupt_enemy(
        &mut self,
        enemies: Vec<EntityId>,
        primary_enemy: EntityId,
        steps: u32,
    ) -> AdvanceResult {
        let player = &self.state.actors[self.state.player_id];
        let player_pos = player.pos;
        let hp_percent = (player.hp * 100) / player.max_hp;
        let retreat_eligible = hp_percent <= (self.state.policy.retreat_hp_threshold as i32);

        let mut tags = Vec::new();
        for &e_id in &enemies {
            if let Some(actor) = self.state.actors.get(e_id) {
                tags.extend(danger_tags_for_kind(actor.kind));
            }
        }
        tags.sort();
        tags.dedup();

        let visible_enemy_count = self
            .state
            .actors
            .iter()
            .filter(|(id, actor)| {
                *id != self.state.player_id && self.state.map.is_visible(actor.pos)
            })
            .count();
        let nearest_enemy_distance = self
            .state
            .actors
            .iter()
            .filter_map(|(id, actor)| {
                if id != self.state.player_id && self.state.map.is_visible(actor.pos) {
                    Some(manhattan(player_pos, actor.pos))
                } else {
                    None
                }
            })
            .min();
        let primary_enemy_kind = self.state.actors[primary_enemy].kind;

        let threat = ThreatSummary {
            danger_tags: tags,
            visible_enemy_count,
            nearest_enemy_distance,
            primary_enemy_kind,
        };

        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::EnemyEncounter {
                enemies,
                primary_enemy,
                retreat_eligible,
                threat: threat.clone(),
            },
        };
        self.pending_prompt = Some(prompt.clone());
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }
    fn interrupt_door(&mut self, pos: Pos, steps: u32) -> AdvanceResult {
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::DoorBlocked { pos },
        };
        self.pending_prompt = Some(prompt.clone());
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }
    fn interrupt_floor_transition(&mut self, steps: u32) -> AdvanceResult {
        let next_floor = if self.state.floor_index < MAX_FLOORS {
            Some(self.state.floor_index + 1)
        } else {
            None
        };
        let requires_branch_god_choice = self.state.floor_index == STARTING_FLOOR_INDEX
            && self.state.branch_profile == BranchProfile::Uncommitted
            && self.state.active_god.is_none()
            && next_floor.is_some();
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::FloorTransition {
                current_floor: self.state.floor_index,
                next_floor,
                requires_branch_god_choice,
            },
        };
        self.pending_prompt = Some(prompt.clone());
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }

    fn descend_to_floor(&mut self, floor_index: u8) {
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

    fn prompt_to_interrupt(&self, prompt: PendingPrompt) -> Interrupt {
        match prompt.kind {
            PendingPromptKind::Loot { item } => Interrupt::LootFound {
                prompt_id: prompt.id,
                item,
                kind: self.state.items[item].kind,
            },
            PendingPromptKind::EnemyEncounter {
                enemies,
                primary_enemy,
                retreat_eligible,
                threat,
            } => Interrupt::EnemyEncounter {
                prompt_id: prompt.id,
                enemies,
                primary_enemy,
                retreat_eligible,
                threat,
            },
            PendingPromptKind::DoorBlocked { pos } => {
                Interrupt::DoorBlocked { prompt_id: prompt.id, pos }
            }
            PendingPromptKind::FloorTransition {
                current_floor,
                next_floor,
                requires_branch_god_choice,
            } => Interrupt::FloorTransition {
                prompt_id: prompt.id,
                current_floor,
                next_floor,
                requires_branch_god_choice,
            },
        }
    }
    fn find_item_at(&self, pos: Pos) -> Option<ItemId> {
        self.state.items.iter().find(|(_, item)| item.pos == pos).map(|(id, _)| id)
    }
    fn find_adjacent_enemy_ids(&self, pos: Pos) -> Vec<EntityId> {
        let mut enemies: Vec<EntityId> = self
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

    fn clear_stale_suppressed_enemy(&mut self, player_pos: Pos) {
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

fn danger_tags_for_kind(kind: ActorKind) -> Vec<DangerTag> {
    match kind {
        ActorKind::Player => vec![],
        ActorKind::Goblin => vec![DangerTag::Melee],
        ActorKind::FeralHound => vec![DangerTag::Melee, DangerTag::Burst],
        ActorKind::BloodAcolyte => vec![DangerTag::Melee, DangerTag::Poison],
        ActorKind::CorruptedGuard => vec![DangerTag::Melee],
        ActorKind::LivingArmor => vec![DangerTag::Melee],
        ActorKind::Gargoyle => vec![DangerTag::Melee],
        ActorKind::ShadowStalker => vec![DangerTag::Melee, DangerTag::Burst],
        ActorKind::AbyssalWarden => vec![DangerTag::Melee, DangerTag::Burst],
    }
}

fn choose_frontier_intent(map: &Map, start: Pos) -> Option<AutoExploreIntent> {
    // Pass 1: BFS/Dijkstra avoiding hazards.
    if let Some(intent) = find_nearest_frontier(map, start, true) {
        return Some(intent);
    }

    // Pass 2: BFS/Dijkstra allowing hazards (fallback).
    if let Some(intent) = find_nearest_frontier(map, start, false) {
        return Some(AutoExploreIntent { reason: AutoReason::ThreatAvoidance, ..intent });
    }

    choose_downstairs_intent(map, start)
}

fn find_nearest_frontier(map: &Map, start: Pos, avoid_hazards: bool) -> Option<AutoExploreIntent> {
    find_nearest_auto_target(
        map,
        start,
        avoid_hazards,
        |current| is_frontier_candidate(map, current),
        |target| {
            if map.tile_at(target) == TileKind::ClosedDoor {
                AutoReason::Door
            } else {
                AutoReason::Frontier
            }
        },
    )
}

fn choose_downstairs_intent(map: &Map, start: Pos) -> Option<AutoExploreIntent> {
    // Pass 1: BFS/Dijkstra avoiding hazards.
    if let Some(intent) = find_nearest_downstairs(map, start, true) {
        return Some(intent);
    }

    // Pass 2: BFS/Dijkstra allowing hazards (fallback).
    if let Some(intent) = find_nearest_downstairs(map, start, false) {
        return Some(AutoExploreIntent { reason: AutoReason::ThreatAvoidance, ..intent });
    }

    None
}

fn find_nearest_downstairs(
    map: &Map,
    start: Pos,
    avoid_hazards: bool,
) -> Option<AutoExploreIntent> {
    find_nearest_auto_target(
        map,
        start,
        avoid_hazards,
        |current| map.tile_at(current) == TileKind::DownStairs && map.is_discovered(current),
        |_target| AutoReason::Frontier,
    )
}

fn find_nearest_auto_target<IsTarget, ReasonForTarget>(
    map: &Map,
    start: Pos,
    avoid_hazards: bool,
    is_target: IsTarget,
    reason_for_target: ReasonForTarget,
) -> Option<AutoExploreIntent>
where
    IsTarget: Fn(Pos) -> bool,
    ReasonForTarget: Fn(Pos) -> AutoReason,
{
    if !map.is_discovered_walkable(start) {
        return None;
    }

    let mut visited = BTreeMap::new();
    let mut queue = VecDeque::new();

    visited.insert(start, 0u16);
    queue.push_back(start);

    let mut best_target: Option<(u16, Pos)> = None;

    while let Some(current) = queue.pop_front() {
        let dist = *visited.get(&current).unwrap();

        if let Some((best_dist, _)) = best_target
            && dist > best_dist
        {
            break;
        }

        if current != start && is_target(current) {
            let is_better = match best_target {
                None => true,
                Some((best_dist, best_pos)) => {
                    dist < best_dist
                        || (dist == best_dist && (current.y, current.x) < (best_pos.y, best_pos.x))
                }
            };
            if is_better {
                best_target = Some((dist, current));
            }
        }

        // Neighbors in deterministic order: Up, Right, Down, Left
        for neighbor in neighbors(current) {
            if !map.is_discovered_walkable(neighbor) {
                continue;
            }
            if avoid_hazards && map.is_hazard(neighbor) {
                continue;
            }
            // Closed doors block transit.
            if map.tile_at(current) == TileKind::ClosedDoor {
                continue;
            }

            if let Entry::Vacant(entry) = visited.entry(neighbor) {
                entry.insert(dist + 1);
                queue.push_back(neighbor);
            }
        }
    }

    best_target.map(|(dist, target)| {
        let reason = reason_for_target(target);
        AutoExploreIntent { target, reason, path_len: dist }
    })
}

fn is_safe_frontier_candidate(map: &Map, pos: Pos) -> bool {
    is_frontier_candidate(map, pos) && !map.is_hazard(pos)
}

fn is_frontier_candidate(map: &Map, pos: Pos) -> bool {
    map.is_discovered(pos)
        && map.tile_at(pos) != TileKind::Wall
        && neighbors(pos).iter().any(|n| map.in_bounds(*n) && !map.is_discovered(*n))
}

fn is_intent_target_still_valid(map: &Map, intent: AutoExploreIntent) -> bool {
    match intent.reason {
        AutoReason::ThreatAvoidance => is_frontier_candidate(map, intent.target),
        _ => is_safe_frontier_candidate(map, intent.target),
    }
}

fn path_for_intent(map: &Map, start: Pos, intent: AutoExploreIntent) -> Option<Vec<Pos>> {
    match intent.reason {
        AutoReason::ThreatAvoidance => astar_path_allow_hazards(map, start, intent.target),
        _ => astar_path(map, start, intent.target),
    }
}

fn reachable_discovered_walkable_tiles(map: &Map, start: Pos) -> BTreeSet<Pos> {
    let mut visited = BTreeSet::new();
    if !map.is_discovered_walkable(start) {
        return visited;
    }

    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        for neighbor in neighbors(current) {
            if map.is_discovered_walkable(neighbor) && visited.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }

    visited
}

fn astar_path(map: &Map, start: Pos, goal: Pos) -> Option<Vec<Pos>> {
    astar_path_internal(map, start, goal, true, None, true)
}

fn astar_path_allow_hazards(map: &Map, start: Pos, goal: Pos) -> Option<Vec<Pos>> {
    astar_path_internal(map, start, goal, false, None, true)
}

fn enemy_path_to_player(
    map: &Map,
    enemy_pos: Pos,
    player_pos: Pos,
    sanctuary_tile: Option<Pos>,
) -> Option<Vec<Pos>> {
    astar_path_internal(map, enemy_pos, player_pos, false, sanctuary_tile, false)
}

fn astar_path_internal(
    map: &Map,
    start: Pos,
    goal: Pos,
    avoid_hazards: bool,
    blocked_tile: Option<Pos>,
    allow_goal_on_blocked_tile: bool,
) -> Option<Vec<Pos>> {
    if !map.is_discovered_walkable(start) || !map.is_discovered_walkable(goal) {
        return None;
    }
    if blocked_tile.is_some_and(|blocked| blocked == goal && !allow_goal_on_blocked_tile) {
        return None;
    }
    if start == goal {
        return Some(vec![]);
    }
    let mut open_set = BTreeSet::new();
    let mut g_score = BTreeMap::new();
    let mut came_from = BTreeMap::new();
    let h = manhattan(start, goal);
    open_set.insert(OpenNode { f: h, h, y: start.y, x: start.x });
    g_score.insert(start, 0);
    while let Some(curr) = open_set.pop_first() {
        let p = Pos { y: curr.y, x: curr.x };
        if p == goal {
            return Some(reconstruct_path(&came_from, start, goal));
        }
        let cur_g = *g_score.get(&p).unwrap();
        for n in neighbors_for_astar(p, blocked_tile) {
            if !is_astar_step_walkable(
                map,
                n,
                goal,
                avoid_hazards,
                blocked_tile,
                allow_goal_on_blocked_tile,
            ) {
                continue;
            }
            let tg = cur_g + 1;
            if tg < *g_score.get(&n).unwrap_or(&u32::MAX) {
                came_from.insert(n, p);
                g_score.insert(n, tg);
                let h = manhattan(n, goal);
                open_set.insert(OpenNode { f: tg + h, h, y: n.y, x: n.x });
            }
        }
    }
    None
}

fn is_astar_step_walkable(
    map: &Map,
    pos: Pos,
    goal: Pos,
    avoid_hazards: bool,
    blocked_tile: Option<Pos>,
    allow_goal_on_blocked_tile: bool,
) -> bool {
    if blocked_tile
        .is_some_and(|blocked| blocked == pos && (pos != goal || !allow_goal_on_blocked_tile))
    {
        return false;
    }
    if !map.is_discovered_walkable(pos) {
        return false;
    }
    if avoid_hazards && map.is_hazard(pos) {
        return false;
    }
    // Closed doors can be reached as an immediate target, but not traversed through.
    if map.tile_at(pos) == TileKind::ClosedDoor && pos != goal {
        return false;
    }
    true
}

fn neighbors_for_astar(p: Pos, blocked_tile: Option<Pos>) -> Vec<Pos> {
    neighbors(p).into_iter().filter(|next| Some(*next) != blocked_tile).collect()
}

fn reconstruct_path(came: &BTreeMap<Pos, Pos>, start: Pos, goal: Pos) -> Vec<Pos> {
    let mut p = goal;
    let mut res = vec![p];
    while p != start {
        p = *came.get(&p).unwrap();
        res.push(p);
    }
    res.reverse();
    res.remove(0);
    res
}

fn neighbors(p: Pos) -> [Pos; 4] {
    [
        Pos { y: p.y - 1, x: p.x },
        Pos { y: p.y, x: p.x + 1 },
        Pos { y: p.y + 1, x: p.x },
        Pos { y: p.y, x: p.x - 1 },
    ]
}

fn manhattan(a: Pos, b: Pos) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

fn transform_octant(orig: Pos, x: i32, y: i32, oct: u8) -> Pos {
    match oct {
        0 => Pos { y: orig.y - y, x: orig.x + x },
        1 => Pos { y: orig.y - x, x: orig.x + y },
        2 => Pos { y: orig.y - x, x: orig.x - y },
        3 => Pos { y: orig.y - y, x: orig.x - x },
        4 => Pos { y: orig.y + y, x: orig.x - x },
        5 => Pos { y: orig.y + x, x: orig.x - y },
        6 => Pos { y: orig.y + x, x: orig.x + y },
        7 => Pos { y: orig.y + y, x: orig.x + x },
        _ => orig,
    }
}

fn compute_fov(map: &mut Map, origin: Pos, range: i32) {
    let prev_discovered = map.discovered.clone();
    map.clear_visible();
    map.set_visible(origin, true);
    for octant in 0..8 {
        scan_octant(map, origin, range, 1, Slope::new(1, 1), Slope::new(0, 1), octant);
    }
    // Remove tiles that were marked visible but are blocked by a wall in a direct line.
    let min_y = (origin.y - range).max(0);
    let max_y = (origin.y + range + 1).min(map.internal_height as i32);
    let min_x = (origin.x - range).max(0);
    let max_x = (origin.x + range + 1).min(map.internal_width as i32);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = Pos { y, x };
            if p == origin || !map.is_visible(p) {
                continue;
            }
            if !has_direct_line_of_sight(map, origin, p) {
                let idx = (y as usize) * map.internal_width + (x as usize);
                map.visible[idx] = false;
                map.discovered[idx] = prev_discovered[idx];
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Slope {
    y: i32,
    x: i32,
}
impl Slope {
    fn new(y: i32, x: i32) -> Self {
        Self { y, x }
    }
    fn greater_or_equal(&self, other: &Slope) -> bool {
        self.y * other.x >= other.y * self.x
    }
    #[allow(dead_code)]
    fn greater_than(&self, other: &Slope) -> bool {
        self.y * other.x > other.y * self.x
    }
}

fn scan_octant(map: &mut Map, orig: Pos, range: i32, dist: i32, start: Slope, end: Slope, oct: u8) {
    if dist > range {
        return;
    }
    let range_u = u32::try_from(range).expect("FOV range must be non-negative");
    let mut blocked = false;
    let mut cur_start = start;
    for y in (0..=dist).rev() {
        let top = Slope::new(2 * y + 1, 2 * dist - 1);
        let bot = Slope::new(2 * y - 1, 2 * dist + 1);
        if cur_start.greater_or_equal(&bot) && top.greater_than(&end) {
            let p = transform_octant(orig, dist, y, oct);
            if manhattan(orig, p) <= range_u {
                map.set_visible(p, true);
            }
            let opaque = map.tile_at(p) == TileKind::Wall || map.tile_at(p) == TileKind::ClosedDoor;
            if opaque {
                if !blocked {
                    scan_octant(map, orig, range, dist + 1, cur_start, top, oct);
                    blocked = true;
                }
                // Tighten scan start to the bottom slope of the blocking tile.
                cur_start = bot;
            } else if blocked {
                blocked = false;
            }
        }
    }
    if !blocked {
        scan_octant(map, orig, range, dist + 1, cur_start, end, oct);
    }
}

fn has_direct_line_of_sight(map: &Map, origin: Pos, target: Pos) -> bool {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let sx = dx.signum();
    let sy = dy.signum();
    let total_dist_x = dx.abs();
    let total_dist_y = dy.abs();

    let mut x = origin.x;
    let mut y = origin.y;
    let mut current_step_x = 0;
    let mut current_step_y = 0;

    // Compare slopes to determine whether to step along the X or Y axis next.
    // We use integer arithmetic to avoid floating-point inaccuracies.
    while current_step_x < total_dist_x || current_step_y < total_dist_y {
        let lhs = (1 + 2 * current_step_x) * total_dist_y;
        let rhs = (1 + 2 * current_step_y) * total_dist_x;

        if lhs == rhs {
            x += sx;
            y += sy;
            current_step_x += 1;
            current_step_y += 1;
        } else if lhs < rhs {
            x += sx;
            current_step_x += 1;
        } else {
            y += sy;
            current_step_y += 1;
        }

        if x == target.x && y == target.y {
            break;
        }
        let tile = map.tile_at(Pos { y, x });
        if tile == TileKind::Wall || tile == TileKind::ClosedDoor {
            return false;
        }
    }
    true
}

#[allow(dead_code)]
fn draw_map_diag(map: &Map, player: Pos) -> String {
    let mut s = String::new();
    for y in 0..map.internal_height {
        for x in 0..map.internal_width {
            let p = Pos { y: y as i32, x: x as i32 };
            let c = if p == player {
                '@'
            } else if map.tile_at(p) == TileKind::Wall {
                '#'
            } else if map.tile_at(p) == TileKind::ClosedDoor {
                '+'
            } else if map.tile_at(p) == TileKind::DownStairs {
                '>'
            } else if is_safe_frontier_candidate(map, p) {
                'F'
            } else {
                '.'
            };
            let v = if map.is_visible(p) { 'v' } else { 'h' };
            let d = if map.is_discovered(p) { 'd' } else { 'u' };
            s.push_str(&format!("{c}{v}{d} "));
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_room_fixture() -> (Map, Pos) {
        let map = Map::new(10, 10);
        let origin = Pos { y: 5, x: 5 };
        (map, origin)
    }

    fn wall_occlusion_fixture() -> (Map, Pos) {
        let mut map = Map::new(11, 11);
        for y in 1..10 {
            for x in 1..10 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 1..10 {
            map.set_tile(Pos { y: 5, x }, TileKind::Floor);
        }
        map.set_tile(Pos { y: 5, x: 6 }, TileKind::Wall);
        (map, Pos { y: 5, x: 3 })
    }

    fn hazard_lane_fixture() -> (Map, Pos) {
        let mut map = Map::new(9, 9);
        for y in 1..8 {
            for x in 1..8 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 2..=5 {
            map.set_tile(Pos { y: 4, x }, TileKind::Floor);
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        (map, Pos { y: 4, x: 2 })
    }

    fn closed_door_choke_fixture() -> (Map, Pos, Pos) {
        let mut map = Map::new(10, 10);
        for x in 0..10 {
            for y in 0..10 {
                map.set_tile(Pos { y, x }, if y == 5 { TileKind::Floor } else { TileKind::Wall });
            }
        }
        map.discovered.fill(true);
        let door = Pos { y: 5, x: 6 };
        map.set_tile(door, TileKind::ClosedDoor);
        map.discovered[5 * 10 + 7] = false;
        (map, Pos { y: 5, x: 5 }, door)
    }

    fn corner_handle_fixture() -> (Map, Pos) {
        let mut map = Map::new(20, 15);
        for y in 1..(map.internal_height - 1) {
            for x in 1..(map.internal_width - 1) {
                map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
            }
        }
        // Room interior.
        for y in 3..=7 {
            for x in 2..=6 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        // Corridor + blocked door to reproduce right-facing corner geometry.
        map.set_tile(Pos { y: 5, x: 7 }, TileKind::Floor);
        map.set_tile(Pos { y: 5, x: 8 }, TileKind::ClosedDoor);
        (map, Pos { y: 5, x: 6 })
    }

    fn add_goblin(game: &mut Game, pos: Pos) -> EntityId {
        let enemy = Actor {
            id: EntityId::default(),
            kind: ActorKind::Goblin,
            pos,
            hp: 10,
            max_hp: 10,
            attack: 2,
            defense: 0,
            active_weapon_slot: WeaponSlot::Primary,
            equipped_weapon: None,
            reserve_weapon: None,
            next_action_tick: 12,
            speed: 12,
        };
        let id = game.state.actors.insert(enemy);
        game.state.actors[id].id = id;
        id
    }

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
    fn planner_targets_known_downstairs_when_no_frontier_remains() {
        let mut map = Map::new(12, 8);
        for y in 1..(map.internal_height - 1) {
            for x in 1..(map.internal_width - 1) {
                map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
            }
        }
        for x in 2..=9 {
            map.set_tile(Pos { y: 4, x }, TileKind::Floor);
        }
        let stairs = Pos { y: 4, x: 9 };
        map.set_tile(stairs, TileKind::DownStairs);
        map.discovered.fill(true);
        map.visible.fill(true);

        let start = Pos { y: 4, x: 3 };
        let intent = choose_frontier_intent(&map, start).expect("stairs should be selected");
        assert_eq!(intent.target, stairs);
    }

    #[test]
    fn downstairs_prefers_nearest_then_y_x_tie_break() {
        // Nearest downstairs should win.
        let mut map = Map::new(12, 8);
        for y in 1..(map.internal_height - 1) {
            for x in 1..(map.internal_width - 1) {
                map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
            }
        }
        for x in 2..=9 {
            map.set_tile(Pos { y: 4, x }, TileKind::Floor);
        }
        let near_stairs = Pos { y: 4, x: 4 };
        let far_stairs = Pos { y: 4, x: 8 };
        map.set_tile(near_stairs, TileKind::DownStairs);
        map.set_tile(far_stairs, TileKind::DownStairs);
        map.discovered.fill(true);
        map.visible.fill(true);

        let start = Pos { y: 4, x: 2 };
        let nearest_intent = choose_downstairs_intent(&map, start).expect("stairs should be found");
        assert_eq!(nearest_intent.target, near_stairs);
        assert_eq!(nearest_intent.path_len, 2);
        assert_eq!(nearest_intent.reason, AutoReason::Frontier);

        // If downstairs are tied by distance, pick lowest (y, x).
        let mut tie_map = Map::new(11, 11);
        for y in 1..10 {
            for x in 1..10 {
                tie_map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        let top_stairs = Pos { y: 3, x: 5 };
        let bottom_stairs = Pos { y: 5, x: 3 };
        tie_map.set_tile(top_stairs, TileKind::DownStairs);
        tie_map.set_tile(bottom_stairs, TileKind::DownStairs);
        tie_map.discovered.fill(true);
        tie_map.visible.fill(true);

        let tie_start = Pos { y: 4, x: 4 };
        let tie_intent =
            choose_downstairs_intent(&tie_map, tie_start).expect("tied stairs should be found");
        assert_eq!(tie_intent.target, top_stairs);
        assert_eq!(tie_intent.path_len, 2);
        assert_eq!(tie_intent.reason, AutoReason::Frontier);
    }

    #[test]
    fn downstairs_hazard_fallback_reports_threat_avoidance() {
        let (mut map, start) = hazard_lane_fixture();
        let stairs = Pos { y: 4, x: 5 };
        map.set_tile(stairs, TileKind::DownStairs);
        map.set_hazard(Pos { y: 4, x: 4 }, true);

        let intent = choose_downstairs_intent(&map, start).expect("hazard fallback intent");
        assert_eq!(intent.target, stairs);
        assert_eq!(intent.path_len, 3);
        assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
    }

    #[test]
    fn fov_open_room_visibility() {
        let (mut map, origin) = open_room_fixture();
        compute_fov(&mut map, origin, 3);
        assert!(map.is_visible(origin));
        assert!(map.is_visible(Pos { y: 5, x: 8 }));
        assert!(!map.is_visible(Pos { y: 1, x: 1 }));
    }

    #[test]
    fn fov_repeat_is_deterministic_for_same_state() {
        let (mut map, origin) = open_room_fixture();
        map.set_tile(Pos { y: 5, x: 7 }, TileKind::Wall);
        map.set_tile(Pos { y: 6, x: 7 }, TileKind::Wall);

        compute_fov(&mut map, origin, FOV_RADIUS);
        let first = map.visible.clone();
        compute_fov(&mut map, origin, FOV_RADIUS);
        let second = map.visible.clone();

        assert_eq!(first, second, "FOV result must be identical for same map/origin");
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
    fn fov_wall_occlusion_blocks_tiles_behind_wall_in_corridor() {
        let (mut map, origin) = wall_occlusion_fixture();
        compute_fov(&mut map, origin, 10);

        assert!(map.is_visible(Pos { y: 5, x: 5 }));
        assert!(map.is_visible(Pos { y: 5, x: 6 }));
        assert!(
            !map.is_visible(Pos { y: 5, x: 7 }),
            "tile directly behind corridor wall should be occluded"
        );
    }

    #[test]
    fn planner_avoids_hazard_route_when_safe_frontier_exists() {
        let (mut map, start) = hazard_lane_fixture();
        map.set_hazard(Pos { y: 4, x: 3 }, true);

        // Safe alternative route to a different frontier.
        for y in 2..=4 {
            map.set_tile(Pos { y, x: 2 }, TileKind::Floor);
        }
        for x in 2..=4 {
            map.set_tile(Pos { y: 2, x }, TileKind::Floor);
        }

        map.discovered.fill(true);
        map.visible.fill(true);
        // Frontier near hazard lane.
        map.discovered[(4 * map.internal_width) + 6] = false;
        // Safe frontier candidate.
        map.discovered[(2 * map.internal_width) + 5] = false;

        let intent = choose_frontier_intent(&map, start).expect("expected frontier intent");
        assert_eq!(intent.target, Pos { y: 2, x: 4 });
    }

    #[test]
    fn planner_reports_threat_avoidance_when_only_hazard_frontier_exists() {
        let (mut map, start) = hazard_lane_fixture();
        map.set_hazard(Pos { y: 4, x: 5 }, true);
        map.discovered[(4 * map.internal_width) + 6] = false;

        let intent = choose_frontier_intent(&map, start).expect("hazard fallback intent");
        assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
    }

    #[test]
    fn safe_frontier_reachable_only_through_hazards_uses_threat_avoidance() {
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

        let start = Pos { y: 4, x: 5 };
        let intent = choose_frontier_intent(&map, start).expect("fallback on safe frontier");
        assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(intent.target, Pos { y: 4, x: 2 });
    }

    #[test]
    fn threat_avoidance_intent_is_reused_without_retarget_replan() {
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

        let p1 = Pos { y: 4, x: 5 };
        game.state.actors[game.state.player_id].pos = p1;
        game.plan_auto_intent(p1);
        let first_intent = game.state.auto_intent.expect("first intent");
        assert_eq!(first_intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(first_intent.target, Pos { y: 4, x: 2 });
        let first_log_count =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(first_log_count, 1);

        // Move opposite the current target; replan would switch to x=8, reuse should not.
        let p2 = Pos { y: 4, x: 6 };
        game.state.actors[game.state.player_id].pos = p2;
        game.plan_auto_intent(p2);
        let second_intent = game.state.auto_intent.expect("second intent");
        assert_eq!(second_intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(second_intent.target, Pos { y: 4, x: 2 });
        let second_log_count =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(second_log_count, 1);
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
    fn frontier_selection_ignores_non_visible_frontiers() {
        let mut map = Map::new(10, 10);
        for y in 1..9 {
            for x in 1..9 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(false);
        for x in 2..=5 {
            map.set_visible(Pos { y: 4, x }, true);
        }
        map.discovered[(4 * map.internal_width) + 6] = false; // visible frontier
        map.discovered[(6 * map.internal_width) + 8] = false; // not visible frontier

        let start = Pos { y: 4, x: 3 };
        let intent = choose_frontier_intent(&map, start).expect("visible frontier");
        assert_eq!(intent.target, Pos { y: 4, x: 5 });
    }

    #[test]
    fn closed_door_is_not_used_as_path_transit_tile() {
        let mut map = Map::new(12, 6);
        for y in 1..5 {
            for x in 1..11 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 1..=10 {
            map.set_tile(Pos { y: 3, x }, TileKind::Floor);
        }
        map.set_tile(Pos { y: 3, x: 5 }, TileKind::ClosedDoor);
        map.discovered.fill(true);

        let start = Pos { y: 3, x: 2 };
        let beyond_door = Pos { y: 3, x: 8 };
        assert!(
            astar_path(&map, start, beyond_door).is_none(),
            "closed door should block traversal to tiles behind it"
        );
        assert!(
            astar_path(&map, start, Pos { y: 3, x: 5 }).is_some(),
            "closed door may still be targeted directly for interrupt handling"
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
    fn fov_does_not_leak_through_corners() {
        let mut map = Map::new(20, 20);
        // Create a 5x5 interior room from (5,5) to (9,9)
        // Walls at (4,4) to (10,10)
        let r_start = 4;
        let r_end = 10;
        for y in r_start..=r_end {
            for x in r_start..=r_end {
                if y == r_start || y == r_end || x == r_start || x == r_end {
                    map.set_tile(Pos { y, x }, TileKind::Wall);
                } else {
                    map.set_tile(Pos { y, x }, TileKind::Floor);
                }
            }
        }

        // Test with player at various positions inside the room
        for py in (r_start + 1)..r_end {
            for px in (r_start + 1)..r_end {
                let origin = Pos { y: py, x: px };
                compute_fov(&mut map, origin, 15);
                for y in 0..map.internal_height {
                    for x in 0..map.internal_width {
                        let p = Pos { y: y as i32, x: x as i32 };
                        if (p.y < r_start || p.y > r_end || p.x < r_start || p.x > r_end)
                            && map.is_visible(p)
                        {
                            panic!(
                                "Light leaked to {p:?} from origin {origin:?}\n{}",
                                draw_map_diag(&map, origin)
                            );
                        }
                    }
                }
            }
        }
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

    #[test]
    fn enemy_pathfinding_cannot_step_onto_sanctuary_tile() {
        let mut map = Map::new(10, 7);
        for y in 1..6 {
            for x in 1..9 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);

        let enemy_pos = Pos { y: 3, x: 6 };
        let sanctuary = Pos { y: 3, x: 4 };
        let player_on_sanctuary = sanctuary;

        let path = enemy_path_to_player(&map, enemy_pos, player_on_sanctuary, Some(sanctuary));
        assert!(
            path.is_none(),
            "enemy A* should treat sanctuary as non-walkable even when the player stands on it"
        );
    }

    #[test]
    fn fov_fixture_does_not_show_corner_handles() {
        let (mut map, origin) = corner_handle_fixture();
        compute_fov(&mut map, origin, FOV_RADIUS);

        // Cells that can be incorrectly discovered as "corner handles" should stay unknown.
        for p in [Pos { y: 1, x: 0 }, Pos { y: 9, x: 0 }, Pos { y: 1, x: 1 }, Pos { y: 9, x: 1 }] {
            assert!(
                !map.is_visible(p),
                "unexpected corner handle at {p:?}\n{}",
                draw_map_diag(&map, origin)
            );
            assert!(
                !map.is_discovered(p),
                "unexpected discovered corner handle at {p:?}\n{}",
                draw_map_diag(&map, origin)
            );
        }
    }

    #[test]
    fn fortification_scroll_never_walls_tile_occupied_by_actor() {
        let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let player_pos = game.state.actors[game.state.player_id].pos;
        let enemy_pos = Pos { y: player_pos.y, x: player_pos.x + 1 };
        let enemy_id = add_goblin(&mut game, enemy_pos);
        assert_eq!(game.state.map.tile_at(enemy_pos), TileKind::Floor);

        game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_FORTIFICATION_SCROLL));

        assert_eq!(
            game.state.map.tile_at(enemy_pos),
            TileKind::Floor,
            "fortification should not convert actor-occupied tiles into walls"
        );
        assert_eq!(game.state.actors[enemy_id].pos, enemy_pos);
    }

    #[test]
    fn fortification_scroll_preserves_an_adjacent_escape_tile() {
        let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let mut map = Map::new(9, 9);
        for y in 1..8 {
            for x in 1..8 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        game.state.map = map;

        let player_pos = Pos { y: 4, x: 4 };
        game.state.actors[game.state.player_id].pos = player_pos;

        game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_FORTIFICATION_SCROLL));

        let adjacent_walkable_count = neighbors(player_pos)
            .into_iter()
            .filter(|neighbor| game.state.map.is_discovered_walkable(*neighbor))
            .count();
        assert!(
            adjacent_walkable_count >= 1,
            "fortification must keep at least one adjacent walkable escape tile"
        );
    }

    #[test]
    fn teleport_rune_tie_break_uses_position_not_insertion_order() {
        let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let mut map = Map::new(12, 8);
        for y in 1..7 {
            for x in 1..11 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        game.state.map = map;

        let player_pos = Pos { y: 4, x: 5 };
        game.state.actors[game.state.player_id].pos = player_pos;

        let farther_in_sort_order =
            add_goblin(&mut game, Pos { y: player_pos.y + 1, x: player_pos.x + 1 });
        let nearer_in_sort_order =
            add_goblin(&mut game, Pos { y: player_pos.y - 1, x: player_pos.x + 1 });

        game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_TELEPORT_RUNE));

        assert_eq!(
            game.state.actors[game.state.player_id].pos,
            Pos { y: player_pos.y - 1, x: player_pos.x + 1 }
        );
        assert_eq!(game.state.actors[nearer_in_sort_order].pos, player_pos);
        assert_eq!(
            game.state.actors[farther_in_sort_order].pos,
            Pos { y: player_pos.y + 1, x: player_pos.x + 1 }
        );
    }

    #[test]
    fn test_magnetic_lure_synergy() {
        let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        let mut map = Map::new(12, 8);
        for y in 1..7 {
            for x in 1..11 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        game.state.map = map;

        let player_pos = Pos { y: 4, x: 3 };
        game.state.actors[game.state.player_id].pos = player_pos;
        let enemy_pos = Pos { y: 4, x: 8 };
        let enemy_id = add_goblin(&mut game, enemy_pos);

        // Apply magnetic lure
        game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_MAGNETIC_LURE));

        let new_enemy_pos = game.state.actors[enemy_id].pos;
        assert!(manhattan(player_pos, new_enemy_pos) < manhattan(player_pos, enemy_pos));
    }

    #[test]
    fn magnetic_lure_is_stable_across_enemy_insertion_order() {
        fn run_order(first: Pos, second: Pos) -> Vec<Pos> {
            let mut game = Game::new(1234, &ContentPack::default(), GameMode::Ironman);
            game.state.items.clear();
            game.state.actors.retain(|id, _| id == game.state.player_id);

            let mut map = Map::new(12, 8);
            for y in 1..7 {
                for x in 1..11 {
                    map.set_tile(Pos { y, x }, TileKind::Floor);
                }
            }
            map.discovered.fill(true);
            map.visible.fill(true);
            game.state.map = map;

            let player_pos = Pos { y: 4, x: 4 };
            game.state.actors[game.state.player_id].pos = player_pos;

            let enemy_a = add_goblin(&mut game, first);
            let enemy_b = add_goblin(&mut game, second);

            game.apply_item_effect(ItemKind::Consumable(keys::CONSUMABLE_MAGNETIC_LURE));

            let mut positions =
                vec![game.state.actors[enemy_a].pos, game.state.actors[enemy_b].pos];
            positions.sort_by_key(|p| (p.y, p.x));
            positions
        }

        let left = run_order(Pos { y: 4, x: 6 }, Pos { y: 4, x: 7 });
        let right = run_order(Pos { y: 4, x: 7 }, Pos { y: 4, x: 6 });

        assert_eq!(left, right, "magnetic lure results should not depend on insertion order");
        assert_eq!(left, vec![Pos { y: 4, x: 5 }, Pos { y: 4, x: 6 }]);
    }

    #[test]
    fn danger_tags_for_each_kind_are_deterministic_and_sorted() {
        let kinds = [
            ActorKind::Goblin,
            ActorKind::FeralHound,
            ActorKind::BloodAcolyte,
            ActorKind::CorruptedGuard,
            ActorKind::LivingArmor,
            ActorKind::Gargoyle,
            ActorKind::ShadowStalker,
            ActorKind::AbyssalWarden,
        ];
        for kind in kinds {
            let tags = danger_tags_for_kind(kind);
            assert!(!tags.is_empty(), "{kind:?} should have at least one danger tag");
            let mut sorted = tags.clone();
            sorted.sort();
            assert_eq!(tags, sorted, "{kind:?} tags should be pre-sorted");
        }
        // Player should have no danger tags
        assert!(danger_tags_for_kind(ActorKind::Player).is_empty());
    }

    #[test]
    fn encounter_interrupt_populates_static_threat_facts() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        // Run until an enemy encounter
        for _ in 0..250 {
            match game.advance(1).stop_reason {
                AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                    threat,
                    enemies,
                    ..
                }) => {
                    assert!(threat.visible_enemy_count > 0);
                    assert!(threat.nearest_enemy_distance.is_some());
                    assert_ne!(threat.primary_enemy_kind, ActorKind::Player);
                    assert!(!threat.danger_tags.is_empty());
                    // Verify tags are sorted and deduped
                    let mut sorted_tags = threat.danger_tags.clone();
                    sorted_tags.sort();
                    sorted_tags.dedup();
                    assert_eq!(threat.danger_tags, sorted_tags);
                    // Enemy count should be >= encounter list size
                    assert!(threat.visible_enemy_count >= enemies.len());
                    return;
                }
                AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::KeepLoot).unwrap();
                }
                AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::OpenDoor).unwrap();
                }
                AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                    prompt_id,
                    requires_branch_god_choice,
                    ..
                }) => {
                    let choice = if requires_branch_god_choice {
                        Choice::DescendBranchAVeil
                    } else {
                        Choice::Descend
                    };
                    game.apply_choice(prompt_id, choice).unwrap();
                }
                AdvanceStopReason::Finished(_) | AdvanceStopReason::EngineFailure(_) => break,
                _ => {}
            }
        }
        panic!("did not encounter an enemy within 250 ticks");
    }

    #[test]
    fn auto_explore_frontier_regression() {
        // 1. Open room with multiple frontiers.
        let (map, player_pos) = open_room_fixture();
        let mut map = map;
        map.discovered.fill(true);
        // Make (4,5) a frontier by setting its neighbor (3,5) to undiscovered.
        map.discovered[3 * map.internal_width + 5] = false;
        // Nearest frontier is at (4,5), distance 1.
        let intent = choose_frontier_intent(&map, player_pos).expect("frontier should be found");
        assert_eq!(intent.target, Pos { y: 4, x: 5 });
        assert_eq!(intent.path_len, 1);

        // 2. Maze-like layout requiring long paths.
        let mut map = Map::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        // Path: (1,1) -> (1,8) -> (8,8) -> (8,1)
        for x in 1..=8 {
            map.set_tile(Pos { y: 1, x }, TileKind::Floor);
        }
        for y in 2..=8 {
            map.set_tile(Pos { y, x: 8 }, TileKind::Floor);
        }
        for x in 1..=7 {
            map.set_tile(Pos { y: 8, x }, TileKind::Floor);
        }
        map.discovered.fill(true);
        // Frontier at (8,1) - its neighbor (9,1) is unknown.
        map.discovered[9 * 10 + 1] = false;
        let start = Pos { y: 1, x: 1 };
        let intent = choose_frontier_intent(&map, start).expect("frontier should be found in maze");
        assert_eq!(intent.target, Pos { y: 8, x: 1 });
        // Path: (1,2..8) [7 steps] + (2..8, 8) [7 steps] + (8, 7..1) [7 steps] = 21 steps.
        assert_eq!(intent.path_len, 21);

        // 3. Scenarios with hazards.
        let (mut map, start) = hazard_lane_fixture();
        // Neighbor of (4,5) is (4,6), make it unknown.
        map.discovered[4 * map.internal_width + 6] = false;
        // Set (4,4) as hazard. start is (4,2).
        map.set_hazard(Pos { y: 4, x: 4 }, true);
        // Only path to (4,5) is through (4,4).
        let intent = choose_frontier_intent(&map, start).expect("hazard fallback should work");
        assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(intent.target, Pos { y: 4, x: 5 });

        // 4. Scenarios with closed doors.
        let (map, start, door) = closed_door_choke_fixture();
        // door is a frontier candidate because its neighbor is unknown.
        let intent = choose_frontier_intent(&map, start).expect("door frontier should be found");
        assert_eq!(intent.target, door);
        assert_eq!(intent.reason, AutoReason::Door);
    }

    #[test]
    fn choose_frontier_intent_optimized_behavior() {
        // These tests will initially fail until choose_frontier_intent is optimized.
        // But since we are replacing the internal implementation, we can use the same
        // public API tests to verify the new behavior.

        // 1. Dijkstra correctly identifies distances.
        // (Handled by existing regression tests)

        // 2. Safe frontier preferred over hazard frontier.
        let (mut map, start) = hazard_lane_fixture();
        // Path to (4,3) is length 1 (safe).
        // Path to (4,5) is length 3 (via hazard (4,4)).
        // Make both frontiers.
        map.discovered[3 * map.internal_width + 3] = false; // neighbor of (4,3)
        map.discovered[4 * map.internal_width + 6] = false; // neighbor of (4,5)
        map.set_hazard(Pos { y: 4, x: 4 }, true);

        let intent = choose_frontier_intent(&map, start).expect("frontier should be found");
        assert_eq!(intent.target, Pos { y: 4, x: 3 }, "should prefer safe frontier");
        assert_eq!(intent.reason, AutoReason::Frontier);

        // 3. Hazard fallback is correctly triggered.
        // (Handled by existing regression tests case 3)
    }
}
