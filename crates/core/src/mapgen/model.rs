//! Public data models for generated maps, enemy spawns, and item spawns.

use crate::types::{ActorKind, ItemKind, Pos, TileKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnemySpawn {
    pub kind: ActorKind,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSpawn {
    pub kind: ItemKind,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedFloor {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<TileKind>,
    pub hazards: Vec<bool>,
    pub entry_tile: Pos,
    pub down_stairs_tile: Pos,
    pub enemy_spawns: Vec<EnemySpawn>,
    pub item_spawns: Vec<ItemSpawn>,
}

impl GeneratedFloor {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend((self.width as u32).to_le_bytes());
        bytes.extend((self.height as u32).to_le_bytes());
        for tile in &self.tiles {
            bytes.push(match tile {
                TileKind::Wall => 0,
                TileKind::Floor => 1,
                TileKind::ClosedDoor => 2,
                TileKind::DownStairs => 3,
            });
        }
        for &hazard in &self.hazards {
            bytes.push(u8::from(hazard));
        }
        bytes.extend(self.entry_tile.y.to_le_bytes());
        bytes.extend(self.entry_tile.x.to_le_bytes());
        bytes.extend(self.down_stairs_tile.y.to_le_bytes());
        bytes.extend(self.down_stairs_tile.x.to_le_bytes());

        bytes.extend((self.enemy_spawns.len() as u32).to_le_bytes());
        for spawn in &self.enemy_spawns {
            bytes.push(match spawn.kind {
                ActorKind::Player => 0,
                ActorKind::Goblin => 1,
                ActorKind::FeralHound => 2,
                ActorKind::BloodAcolyte => 3,
                ActorKind::CorruptedGuard => 4,
                ActorKind::LivingArmor => 5,
                ActorKind::Gargoyle => 6,
                ActorKind::ShadowStalker => 7,
                ActorKind::AbyssalWarden => 8,
            });
            bytes.extend(spawn.pos.y.to_le_bytes());
            bytes.extend(spawn.pos.x.to_le_bytes());
        }

        bytes.extend((self.item_spawns.len() as u32).to_le_bytes());
        for spawn in &self.item_spawns {
            bytes.extend(spawn.pos.y.to_le_bytes());
            bytes.extend(spawn.pos.x.to_le_bytes());
        }

        bytes
    }

    pub fn tile_at(&self, pos: Pos) -> TileKind {
        if pos.x < 0 || pos.y < 0 {
            return TileKind::Wall;
        }
        let x = pos.x as usize;
        let y = pos.y as usize;
        if x >= self.width || y >= self.height {
            return TileKind::Wall;
        }
        self.tiles[y * self.width + x]
    }
}
