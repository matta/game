use slotmap::SlotMap;

use crate::types::*;

pub struct ContentPack {
    // Hardcoded content schemas will go here.
}

#[derive(Clone, Debug)]
pub struct Actor {
    pub id: EntityId,
    pub kind: ActorKind,
    pub pos: Pos,
    pub hp: i32,
    pub max_hp: i32,
    pub next_action_tick: u64,
    pub speed: u32,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: ItemId,
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
            && (self.tile_at(pos) == TileKind::Floor || self.tile_at(pos) == TileKind::ClosedDoor)
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
    pub auto_intent: Option<AutoExploreIntent>,
}
