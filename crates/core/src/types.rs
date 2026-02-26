use slotmap::new_key_type;

new_key_type! {
    pub struct EntityId;
    pub struct ItemId;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub y: i32,
    pub x: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TileKind {
    Wall,
    Floor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActorKind {
    Player,
    Goblin,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChoicePromptId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Choice {
    KeepLoot,
    DiscardLoot,
    Fight,
    Avoid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Victory,
    Defeat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interrupt {
    LootFound { prompt_id: ChoicePromptId, item: ItemId },
    EnemyEncounter { prompt_id: ChoicePromptId, enemy: EntityId },
}

#[derive(Clone, Debug)]
pub enum AdvanceStopReason {
    Interrupted(Interrupt),
    PausedAtBoundary { tick: u64 },
    Finished(RunOutcome),
    BudgetExhausted,
}

#[derive(Clone, Debug)]
pub struct AdvanceResult {
    pub simulated_ticks: u32,
    pub stop_reason: AdvanceStopReason,
}

#[derive(Debug, Clone)]
pub enum GameError {
    InvalidChoice,
    PromptMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogEvent {
    AutoReasonChanged { reason: AutoReason, target: Pos, path_len: u16 },
    EnemyEncountered { enemy: EntityId },
    ItemPickedUp,
    ItemDiscarded,
    EncounterResolved { enemy: EntityId, fought: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoReason {
    Frontier,
    Loot,
    ThreatAvoidance,
    Stuck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoExploreIntent {
    pub target: Pos,
    pub reason: AutoReason,
    pub path_len: u16,
}

pub enum GameMode {
    Ironman,
    Easy,
}
