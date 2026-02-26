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
    ClosedDoor,
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
    OpenDoor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Victory,
    Defeat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interrupt {
    LootFound { prompt_id: ChoicePromptId, item: ItemId },
    EnemyEncounter { prompt_id: ChoicePromptId, enemies: Vec<EntityId>, primary_enemy: EntityId },
    DoorBlocked { prompt_id: ChoicePromptId, pos: Pos },
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
    NotAtPauseBoundary,
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
    Door,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FightMode {
    Fight,
    Avoid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    Aggressive,
    Balanced,
    Defensive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetTag {
    Nearest,
    LowestHp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionIntent {
    HoldGround,
    AdvanceToMelee,
    FleeToNearestExploredTile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Aggro {
    Conserve,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExploreMode {
    Thorough,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub fight_or_avoid: FightMode,
    pub stance: Stance,
    pub target_priority: Vec<TargetTag>,
    pub retreat_hp_threshold: u8,
    pub auto_heal_if_below_threshold: Option<u8>,
    pub position_intent: PositionIntent,
    pub resource_aggression: Aggro,
    pub exploration_mode: ExploreMode,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            fight_or_avoid: FightMode::Fight,
            stance: Stance::Balanced,
            target_priority: vec![TargetTag::Nearest, TargetTag::LowestHp],
            retreat_hp_threshold: 35,
            auto_heal_if_below_threshold: None,
            position_intent: PositionIntent::HoldGround,
            resource_aggression: Aggro::Conserve,
            exploration_mode: ExploreMode::Thorough,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyUpdate {
    FightMode(FightMode),
    Stance(Stance),
    TargetPriority(Vec<TargetTag>),
    RetreatHpThreshold(u8),
    AutoHealIfBelowThreshold(Option<u8>),
    PositionIntent(PositionIntent),
    ResourceAggression(Aggro),
    ExplorationMode(ExploreMode),
}
