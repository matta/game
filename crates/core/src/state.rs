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

#[derive(Clone)]
pub struct Map {
    pub internal_width: usize,
    pub internal_height: usize,
    pub tiles: Vec<TileKind>,
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
}

pub struct GameState {
    pub map: Map,
    pub actors: SlotMap<EntityId, Actor>,
    pub items: SlotMap<ItemId, ()>, // Placeholder for items
    pub player_id: EntityId,
}
