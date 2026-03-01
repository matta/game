//! Public crate surface for the core simulation, map generation, and replay systems.

pub mod game;
pub mod journal;
pub mod journal_file;
pub mod mapgen;
pub mod replay;
pub mod state;
pub mod types;

pub use game::Game;
pub use journal::{InputJournal, InputPayload, InputRecord};
pub use journal_file::{JournalLoadError, JournalWriter, LoadedJournal, load_journal_from_file};
pub use mapgen::{
    BranchProfile, GeneratedFloor, MAX_FLOORS, MapGenerator, STARTING_FLOOR_INDEX, generate_floor,
};
pub use replay::*;
pub mod content;

pub use content::ContentPack;
pub use state::{GameState, Map};
pub use types::*;
