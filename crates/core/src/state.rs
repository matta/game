//! Runtime world state and storage structures used by the simulation engine.

use std::collections::VecDeque;

use slotmap::SlotMap;

use crate::mapgen::BranchProfile;
use crate::types::*;

#[derive(Clone, Debug)]
pub struct Actor {
    pub id: EntityId,
    pub kind: ActorKind,
    pub pos: Pos,
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub active_weapon_slot: WeaponSlot,
    pub equipped_weapon: Option<&'static str>,
    pub reserve_weapon: Option<&'static str>,
    pub next_action_tick: u64,
    pub speed: u32,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: ItemId,
    pub kind: ItemKind,
    pub pos: Pos,
}

#[derive(Clone)]
pub struct Map {
    pub internal_width: usize,
    pub internal_height: usize,
    pub tiles: Vec<TileKind>,
    pub discovered: Vec<bool>,
    pub visible: Vec<bool>,
    pub hazards: Vec<bool>,
}

impl Map {
    pub fn new(width: usize, height: usize) -> Self {
        let mut tiles = vec![TileKind::Floor; width * height];
        for x in 0..width {
            tiles[x] = TileKind::Wall;
            tiles[(height - 1) * width + x] = TileKind::Wall;
        }
        for y in 0..height {
            tiles[y * width] = TileKind::Wall;
            tiles[y * width + (width - 1)] = TileKind::Wall;
        }
        Self {
            internal_width: width,
            internal_height: height,
            tiles,
            discovered: vec![false; width * height],
            visible: vec![false; width * height],
            hazards: vec![false; width * height],
        }
    }

    pub fn tile_at(&self, pos: Pos) -> TileKind {
        if pos.x < 0 || pos.y < 0 {
            return TileKind::Wall;
        }
        let xu = pos.x as usize;
        let yu = pos.y as usize;
        if xu >= self.internal_width || yu >= self.internal_height {
            return TileKind::Wall;
        }
        self.tiles[yu * self.internal_width + xu]
    }

    pub fn in_bounds(&self, pos: Pos) -> bool {
        pos.x >= 0
            && pos.y >= 0
            && (pos.x as usize) < self.internal_width
            && (pos.y as usize) < self.internal_height
    }

    pub fn set_tile(&mut self, pos: Pos, tile: TileKind) {
        if !self.in_bounds(pos) {
            return;
        }
        let idx = self.index(pos);
        self.tiles[idx] = tile;
    }

    pub fn reveal(&mut self, pos: Pos) {
        if !self.in_bounds(pos) {
            return;
        }
        let idx = self.index(pos);
        self.discovered[idx] = true;
    }

    pub fn is_discovered(&self, pos: Pos) -> bool {
        if !self.in_bounds(pos) {
            return false;
        }
        self.discovered[self.index(pos)]
    }

    pub fn is_discovered_walkable(&self, pos: Pos) -> bool {
        self.is_discovered(pos)
            && (self.tile_at(pos) == TileKind::Floor
                || self.tile_at(pos) == TileKind::ClosedDoor
                || self.tile_at(pos) == TileKind::DownStairs)
    }

    pub fn clear_visible(&mut self) {
        self.visible.fill(false);
    }

    pub fn set_visible(&mut self, pos: Pos, val: bool) {
        if !self.in_bounds(pos) {
            return;
        }
        let idx = self.index(pos);
        self.visible[idx] = val;
        if val {
            self.discovered[idx] = true;
        }
    }

    pub fn is_visible(&self, pos: Pos) -> bool {
        if !self.in_bounds(pos) {
            return false;
        }
        self.visible[self.index(pos)]
    }

    pub fn set_hazard(&mut self, pos: Pos, val: bool) {
        if !self.in_bounds(pos) {
            return;
        }
        let idx = self.index(pos);
        self.hazards[idx] = val;
    }

    pub fn is_hazard(&self, pos: Pos) -> bool {
        if !self.in_bounds(pos) {
            return false;
        }
        self.hazards[self.index(pos)]
    }

    pub fn is_discovered_walkable_safe(&self, pos: Pos) -> bool {
        self.is_discovered_walkable(pos) && !self.is_hazard(pos)
    }

    fn index(&self, pos: Pos) -> usize {
        (pos.y as usize) * self.internal_width + (pos.x as usize)
    }
}

pub struct GameState {
    pub map: Map,
    pub actors: SlotMap<EntityId, Actor>,
    pub items: SlotMap<ItemId, Item>,
    pub player_id: EntityId,
    pub sanctuary_tile: Pos,
    pub sanctuary_active: bool,
    pub floor_index: u8,
    pub branch_profile: BranchProfile,
    pub active_god: Option<GodId>,
    pub auto_intent: Option<AutoExploreIntent>,
    pub policy: Policy,
    pub threat_trace: VecDeque<ThreatTrace>,
    pub active_perks: Vec<&'static str>,
    pub kills_this_floor: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_helpers_handle_bounds_and_clear() {
        let mut map = Map::new(5, 5);
        let in_bounds = Pos { y: 2, x: 2 };
        let out_of_bounds = Pos { y: -1, x: 2 };

        assert!(!map.is_visible(in_bounds));
        assert!(!map.is_visible(out_of_bounds));

        map.set_visible(in_bounds, true);
        assert!(map.is_visible(in_bounds));
        assert!(map.is_discovered(in_bounds));

        map.set_visible(out_of_bounds, true);
        assert!(!map.is_visible(out_of_bounds));

        map.clear_visible();
        assert!(!map.is_visible(in_bounds));
        assert!(map.is_discovered(in_bounds), "clear_visible should not erase discovery");
    }
}
