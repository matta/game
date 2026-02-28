//! Starting world construction and first-floor setup for a new run.
//! This module exists to isolate initialization details from runtime simulation flow.
//! It does not own tick advancement or choice resolution once a run has started.

use std::collections::VecDeque;

use rand_chacha::rand_core::SeedableRng;

use super::*;
use crate::content::{ContentPack, get_enemy_stats, keys};
use crate::floor::{BranchProfile, STARTING_FLOOR_INDEX};
use crate::state::{Actor, Item, Map};

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

        for y in 3..=7 {
            for x in 2..=6 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }

        for y in 3..=7 {
            for x in 9..=13 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }

        for y in 9..=13 {
            for x in 9..=13 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }

        map.set_tile(Pos { y: 5, x: 7 }, TileKind::Floor);
        map.set_tile(Pos { y: 5, x: 8 }, TileKind::ClosedDoor);

        map.set_tile(Pos { y: 8, x: 11 }, TileKind::Floor);
        map.set_tile(Pos { y: 9, x: 11 }, TileKind::Floor);

        map.set_hazard(Pos { y: 8, x: 11 }, true);
        map.set_hazard(Pos { y: 9, x: 11 }, true);
        map.set_hazard(Pos { y: 10, x: 11 }, true);

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
}
