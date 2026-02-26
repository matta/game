# Roguelike Project — MVP 1.0 Complete Integrated Development Plan

Author: Matt Armstrong  
Stack: Rust + Macroquad  
Target: Desktop (macOS + Linux)  
Run Length: 20–40 minutes  
Time Budget: ~120 hours (10 hrs/week × 12 weeks)

---

# 1. MVP Definition

A deterministic, ASCII-grid roguelike with:

- Grid-based simulation (auto-movement only)
- Auto-explore with interrupt-driven decisions
- Equipment-centric + perk-based builds
- ~15 items
- ~10 perks
- 2 passive gods
- Procedural dungeon floors + small authored vault templates
- Branching dungeon structure
- Ironman primary mode
- Optional checkpoint-based easy mode (time travel via full replay from tick 0)
- Journal-based save/load (same-build compatibility only)
- Fully deterministic seed-based runs

Deferred post-MVP:
- Snapshot-accelerated reloads (fast checkpoint restore without full replay)
- Best-effort replay migration across code/content versions

---

# 2. Workspace Architecture

Cargo workspace layout utilizes four distinct crates to enforce strict separation of concerns and protect the deterministic core simulation:

```text
/Cargo.toml
crates/
  core/      # Deterministic simulation engine (Logic only, no OS/Render)
  content/   # Data definitions (Items, Perks, Gods) + validation
  app/       # Macroquad frontend (UI shell and input translation)
  tools/     # Optional balance/replay tools
```

## 2.1 Memory Management Strategy
The `core` crate uses Generational Arenas (`slotmap`) for **actor** storage (player + monsters) and **item instance** storage (stable `ItemId`). This avoids borrow checker conflicts and prevents use-after-free errors when entities are created and destroyed during simulation, without resorting to complex ECS frameworks.

Map tiles remain dense arrays/structs; each tile stores stable IDs/flags rather than positional indices for player-facing references. Static content definitions (item/perk/god blueprints) remain dense arrays in `content`.

---

# 3. Determinism Contract

Requirements to guarantee identical runs across macOS and Linux:
- Single RNG source: `rand_chacha::ChaCha8Rng` explicitly seeded at run start.
- No `thread_rng()` or system time access in `core`.
- Avoid floating-point nondeterminism (use integer math exclusively for logic/combat).
- All randomness injected explicitly.
- Deterministic containers/iteration: avoid `HashMap`/`HashSet` in simulation-critical paths unless keys are copied into a sorted buffer before iteration. Prefer `Vec` + explicit sort, or `BTreeMap`/`BTreeSet`.
- `slotmap` iteration order is treated as non-contractual; deterministic systems/hashing must not depend on raw arena iteration order.
- Actor and item records carry monotonic `spawn_seq` values assigned at creation; this is the primary stable ordering key.
- For whole-world passes (turn queue rebuild, threat scans, hashing), iterate actors/items in `spawn_seq` ascending order.
- Deterministic tie-breakers:
  - Turn order: `(next_action_tick, speed_desc, spawn_seq)`; do not rely on generational key ordering for ties.
  - Path tie resolution: fixed neighbor expansion order `Up, Right, Down, Left`.
- Stable snapshot hash: `xxh3_64` over canonical binary encoding of minimal deterministic state only:
  - `seed`, `tick`
  - map tiles + deterministic tile metadata
  - actor arena contents in `spawn_seq` ascending order
  - item arena contents in `spawn_seq` ascending order
  - RNG internal state
  - `next_input_seq`
  - policy state
  - pending `prompt_id`/interrupt context (if any)
- Explicitly excluded from snapshot hash: `log`, UI histories, replay trace buffers, and any presentation/transient app state.

Game struct sketch:

```rust
pub struct Game {
  seed: u64,
  tick: u64,
  rng: ChaCha8Rng,
  state: RunState,
  log: Vec<LogEvent>,
  next_input_seq: u64,
}
```

Core API:

```rust
pub enum AdvanceStopReason {
  Interrupted(Interrupt),
  PausedAtBoundary { tick: u64 },
  Finished(RunOutcome),
  BudgetExhausted,
}

pub struct TickFrame {
  pub tick: u64,
  pub player_pos: Pos,
  pub visible_actor_positions: Vec<(EntityId, Pos)>,
  pub auto_intent: Option<AutoExploreIntent>,
}

pub struct AdvanceResult {
  pub simulated_ticks: u32,
  pub trace: Vec<TickFrame>, // deterministic order, ascending tick
  pub stop_reason: AdvanceStopReason,
}

impl Game {
  pub fn new(seed: u64, content: &ContentPack, mode: GameMode) -> Self;
  pub fn advance(&mut self, max_steps: u32, max_trace_frames: usize) -> AdvanceResult;
  pub fn request_pause(&mut self);
  pub fn apply_choice(
    &mut self,
    prompt_id: ChoicePromptId,
    choice: Choice
  ) -> Result<(), GameError>;
  pub fn apply_policy_update(&mut self, update: PolicyUpdate) -> Result<(), GameError>;
  pub fn current_tick(&self) -> u64;
  pub fn observation(&self) -> ObservationRef<'_>;
  pub fn observation_owned(&self) -> ObservationOwned; // tools/debug export path
  pub fn snapshot_hash(&self) -> u64;
}
```

Execution rule:
- MVP threading model is single-threaded: Macroquad app loop and `Game` simulation run on the same main thread.
- `app` auto mode advances with batch stepping: `advance(max_steps, max_trace_frames)`.
- `core` still advances one tick at a time internally and may return early on interrupt, finish, pending pause, or batch budget exhaustion.
- `app` runs simulation and presentation on two clocks:
  - Simulation clock: fill a trace queue using `advance(...)` up to a CPU time budget.
  - Presentation clock: consume queued `TickFrame`s at a configurable visual speed (ticks/second).
- Simulation can run far ahead of rendering, but auto-stepping stops filling when the trace queue reaches a configured high-water mark.
- User pause requests are latched via `request_pause()` (called by the app after input polling on the same thread) and honored at the next tick boundary (`PausedAtBoundary`).

Why this complexity is intentional:
- A Rust `core` can simulate hundreds/thousands of ticks in one render frame; a naive loop would visually skip meaningful movement.
- Players need controllable perceived speed (watchable motion, readable interrupts) without sacrificing fast simulation throughput.
- The trace queue decouples simulation speed from presentation speed while preserving deterministic tick order.
- `TickFrame` data is derived output, not an input source, so replay determinism still depends only on seed + input journal.

## 3.1 Input Journal Specification

Replays are represented as a versioned append-only **input journal** (all core inputs in order), enabling deterministic replay without Macroquad.

```rust
pub struct InputJournal {
  pub format_version: u16,
  pub build_id: String,      // git SHA or build fingerprint
  pub content_hash: u64,     // content pack fingerprint
  pub seed: u64,
  pub events: Vec<InputEvent>,
  pub checkpoints: Vec<CheckpointMarker>, // easy-mode restore candidates
}

pub struct InputEvent {
  pub seq: u64,
  pub tick_boundary: u64, // boundary where event is accepted
  pub payload: InputPayload,
}

pub enum InputPayload {
  Choice { prompt_id: ChoicePromptId, choice: Choice },
  PolicyUpdate(PolicyUpdate), // takes effect on the next simulation tick
}

pub struct CheckpointMarker {
  pub id: u32,
  pub tick_boundary: u64,
  pub input_seq: u64,      // restore by replaying events up to this sequence
  pub reason: CheckpointReason,
}

pub enum CheckpointReason {
  GodChoiceResolved,
  LevelChanged,
  BranchCommitted,
  CampResolved,
  PerkChosen,
  BuildDefiningLootResolved,
}
```

`ChoicePromptId` is emitted by `core` with each `Interrupt`, and must match on `apply_choice(...)`.  
MVP compatibility guarantee: journal replay is supported only when `build_id` and `content_hash` match.

## 3.2 Core / App Communication Contract

- `app` provides **pure inputs**: `InputPayload`.
- `core` returns **pure outputs**: `AdvanceResult` (`TickFrame` + `AdvanceStopReason`), `ObservationRef`, `LogEvent`.
- `app` never reads or writes `core` internals except through the public API.

The `app` crate controls the execution loop, deciding whether to auto-explore or wait for user input (e.g., when paused). The `core` only receives inputs that mutate simulation state, remaining entirely ignorant of UI concepts.

`ObservationRef` is strictly engine-agnostic state and is returned as a borrow (no per-frame deep copy):

```rust
pub struct ObservationRef<'a> {
  pub visible_tiles: &'a [TileObservation],    // TileKind only; no glyph/color
  pub visible_actors: &'a [ActorObservation],  // logical entities in view
  pub player: PlayerObservation,               // logical stats and statuses
  pub current_auto_intent: Option<AutoExploreIntent>, // why auto-explore is moving
}

pub struct TileObservation {
  pub pos: Pos,
  pub tile: TileKind,
}

pub struct ActorObservation {
  pub id: EntityId,
  pub kind: ActorKind,
  pub pos: Pos,
  pub status_bits: StatusBits,
}

pub struct PlayerObservation {
  pub hp: i32,
  pub max_hp: i32,
  pub status_bits: StatusBits,
}

pub struct AutoExploreIntent {
  pub target: Pos,
  pub reason: AutoReason,
  pub path_len: u16,
}

pub enum AutoReason {
  Frontier,
  Loot,
  BranchGoal,
  ThreatAvoidance,
  ReturnToSafe,
}

pub struct ObservationOwned {
  pub visible_tiles: Vec<TileObservation>,
  pub visible_actors: Vec<ActorObservation>,
  pub player: PlayerObservation,
  pub current_auto_intent: Option<AutoExploreIntent>,
}

pub enum LogEvent {
  // ... other gameplay events
  AutoReasonChanged {
    reason: AutoReason,
    target: Pos,
    path_len: u16,
  },
}
```

Explicitly excluded from `ObservationRef`/`ObservationOwned`: glyphs, colors, fonts, panel layout, animation/motion cues, and any UI-specific formatting.
Default render path should use `observation()` (`ObservationRef`) to avoid per-frame heap churn; `observation_owned()` is for tooling/debug paths where ownership is useful.

`TickFrame` is also engine-agnostic (logical positions/targets only), and is used to pace presentation. Rendering style remains entirely in `app`.
`AutoExploreIntent` is a required explainability artifact for pathing/frontier decisions.
`core` emits `LogEvent::AutoReasonChanged` whenever the auto-explore target or reason changes.
`path_len` is the planned shortest-path length from current player position to `target` at decision time.
Emission rule: emit on `(target, reason)` transition; do not spam events for per-tick `path_len` shrink while following the same intent.

Policy updates in MVP are applied only at tick boundaries while paused (from interrupt or user pause), and every accepted update is journaled with its `tick_boundary`.
Action-cost requirement: automatic loadout swaps are in-world actions that consume ticks; no free pre-combat equipment changes.
MVP policy timing split:
- Loadout-affecting updates (equip/swap) consume time via `SwapLoadout` action(s).
- Non-loadout policy knob edits (priority/stance/thresholds) are applied at pause boundaries without direct tick cost.

This contract enables a headless `tools/replay_runner` that exercises `core` without any rendering dependency.

Replay execution surface (MVP):

```rust
pub struct ReplayResult {
  pub final_outcome: RunOutcome,
  pub final_snapshot_hash: u64,
  pub final_tick: u64,
}

pub fn replay_to_end(
  content: &ContentPack,
  journal: &InputJournal
) -> Result<ReplayResult, ReplayError>;
```

- Primary contract is the Rust API in `core` (`replay_to_end`) used by tests/CI.
- `tools/replay_runner` is a thin CLI wrapper around that API for manual verification and debugging.
- CLI minimum behavior: load journal, run replay headlessly, print final `snapshot_hash` and outcome.

## 3.3 Pause/Policy Timing Model

- Auto-explore runs by repeated batch stepping, while simulation logic remains tick-granular internally.
- Each `advance(...)` call emits an ordered `TickFrame` trace that the app can play back at any visual speed.
- `core` checks pending pause between internal ticks and exits on the next boundary.
- Pause changes control flow only and does not mutate simulation state.
- When a hostile is first seen in FOV during auto-explore, `core` emits `EnemyEncounter` before committing opening combat actions.
- Auto-explore halts at that interrupt so the player can inspect threat summary and adjust policy/loadout.
- While paused, the user may issue one or more policy updates.
- On resume, the next tick uses the latest accepted policy state.
- Replay applies policy updates at the recorded boundary before simulating the next tick.
- Interrupts/finish are surfaced as stop reasons; app shows related UI once visual playback has consumed all prior trace frames.

## 3.4 Easy Mode Checkpoint Model

- Easy mode uses engine-authored checkpoint markers at deterministic boundaries (e.g., resolved god choice, level change, branch commitment, resolved camp).
- Build-identity checkpoints are also supported (`PerkChosen`, `BuildDefiningLootResolved`).
- On death, player may select any unlocked checkpoint.
- Restore always replays from tick `0` to the checkpoint marker's `input_seq`; no state snapshot fast-path in MVP.
- Checkpoint markers are persisted in the run save and must be consistent with deterministic replay.
- To avoid checkpoint spam, build-defining loot checkpoints should be gated by explicit content flags/rarity tiers rather than firing for every pickup/equip.

---

# 4. Core Simulation Design

## 4.1 Core Data Structures

- `EntityId` (managed via `slotmap`)
- `ItemId` (managed via `slotmap`; stable in interrupt/replay flows)
- `Actor` (player or monster; includes monotonic `spawn_seq: u64`)
- `Stats` (hp, atk, def, speed, limited resists)
- `Status` (poison, bleed, slow — minimal set)
- `Equipment` (small fixed slot set)
- `ItemInstance` (includes monotonic `spawn_seq: u64`)
- `Perk`
- `God` (passive modifiers only)
- `Policy`
- `ActiveActorIds` (`Vec<EntityId>`) for deterministic whole-world traversal
- `ActiveItemIds` (`Vec<ItemId>`) for deterministic whole-world traversal

## 4.2 Policy Structure

```rust
pub struct Policy {
  pub fight_or_avoid: FightMode,
  pub stance: Stance,
  pub target_priority: Vec<TargetTag>,
  pub consume_hp_threshold: u8,
  pub retreat_hp_threshold: u8,
  pub position_intent: PositionIntent, // MVP-restricted intent set
  pub resource_aggression: Aggro,
  pub exploration_mode: ExploreMode,
  pub loadout_rule: LoadoutRule, // may trigger time-costing SwapLoadout actions
}

pub enum PositionIntent {
  HoldGround,
  AdvanceToMelee,
  FleeToNearestExploredTile,
}
```

## 4.3 Spatial Systems
All spatial algorithms live strictly within `core`, with no dependency on external rendering libraries.

- **Pathing + minimal exploration intent (Milestone 2a):** A* pathing on discovered known-walkable tiles plus a simple frontier target selection (nearest unknown-adjacent discovered tile), with `AutoExploreIntent { target, reason, path_len }` produced as a first-class output.
- **FOV minimum viable pass (Milestone 2b):** Simple deterministic FOV + frontier refinement + hazard-avoidance v0 (avoid known hazard tiles).

## 4.4 Deterministic Traversal Rules

- Never rely on raw `slotmap` iteration order for simulation decisions, logging semantics, or hashing.
- Before deterministic global passes, obtain active IDs and evaluate records in `spawn_seq` ascending order.
- If `spawn_seq` ever collides due to bug/corruption, fail fast in debug builds (do not silently fallback to generational key order).

## 4.5 Loadout Action Semantics (MVP)

- `SwapLoadout` is a first-class simulation action with tick cost (same timing model as other actor actions).
- Auto behavior may choose `SwapLoadout` based on `loadout_rule`, but it never bypasses turn economy.
- Enemy reactions occur according to normal turn ordering; swapping can expose player to incoming actions.
- Emit deterministic log events for executed swaps so replay/debug can explain pre-combat openings.

## 4.6 MVP Combat Positioning Scope

- Multi-enemy encounter handling is intentionally spatially naive in MVP.
- `PositionIntent` is restricted to `HoldGround`, `AdvanceToMelee`, and `FleeToNearestExploredTile`.
- No advanced tactical repositioning in MVP: no kiting logic, no deliberate LOS-break maneuvers, no corner-peeking planner.
- Enemy selection and threat handling still use deterministic target-priority and retreat thresholds.

---

# 5. Interrupt Model

Interrupt types (MVP subset):

- LootFound
- EnemyEncounter (first-sighting pre-commit stop)
- BranchChoice
- GodOffer
- CampChoice
- PerkChoice
- BossEncounter
- CheckpointAvailable (easy mode only)

Enemy encounter fairness contract (MVP):

```rust
pub struct EnemyEncounterInterrupt {
  pub enemies: Vec<EntityId>,
  pub threat: ThreatSummary,
  pub opening_preview: Option<OpeningActionPreview>,
}

pub struct ThreatSummary {
  pub horizon_ticks: u8,           // fixed small N (e.g., 3)
  pub expected_damage_band: (u16, u16), // rough lower/upper estimate over N ticks
  pub danger_tags: Vec<DangerTag>, // e.g., Poison, Burst, Ranged
  pub escape_feasibility: EscapeFeasibility,
}

pub enum EscapeFeasibility {
  Likely,
  Uncertain,
  Unlikely,
}

pub struct OpeningActionPreview {
  pub action: OpeningAction,
  pub tick_cost: u8,
}

pub enum OpeningAction {
  SwapLoadout { to: LoadoutId },
  HoldCurrentLoadout,
}
```

MVP guidance:
- This summary can be heuristic and conservative; it does not need perfect tactical forecasting.
- Compute deterministically from current known state/policy so equal seeds and inputs produce equal summaries.
- `EnemyEncounter` is emitted on first hostile sighting during auto-explore, before any opening swap/attack is executed.

---

# 6. Content System

## 6.1 Procedural Generation

- Rectangular rooms + corridors (simplest readable ASCII).
- Weighted spawn tables by depth.
- 2–4 vault templates per floor.

## 6.2 Vault Templates

Examples:
- Shrine room
- Treasure guard room
- Hazard corridor
- Elite ambush room

Author rules, not handcrafted layouts.

---

# 7. Frontend (Macroquad)

Rendering:
- ASCII glyph grid
- Fixed-width font
- Grid cell = (char, fg, bg)

Layout:
- Center: map
- Right: player stats + policy
- Bottom: event log
- Modal: interrupt panels

Input:
- Space: toggle auto
- Esc: menu
- Number keys: interrupt choices
- Tab: cycle policy presets
- `[` / `]`: decrease/increase auto-explore visual speed (ticks/sec)

## 7.1 Main Loop Sketch (Single Thread)

```rust
#[macroquad::main("Roguelike")]
async fn main() {
  let mut game = Game::new(seed, &content, mode);
  let mut auto_enabled = true;
  let mut visual_ticks_per_sec = 30.0_f32;
  let mut visual_tick_accum = 0.0_f32;
  // Buffer of deterministic per-tick facts for smooth playback when simulation outruns rendering.
  let mut trace_queue: VecDeque<TickFrame> = VecDeque::new();
  // Stop reasons are delayed until buffered frames are shown, so UI doesn't jump ahead of animation.
  let mut pending_stop: Option<AdvanceStopReason> = None;
  let mut presented = PresentedState::from_observation(game.observation());

  loop {
    let dt = get_frame_time();

    // 1) Poll user input (Macroquad keyboard/mouse APIs)
    if is_key_pressed(KeyCode::Space) { auto_enabled = !auto_enabled; }
    if is_key_pressed(KeyCode::LeftBracket)  { visual_ticks_per_sec *= 0.8; }
    if is_key_pressed(KeyCode::RightBracket) { visual_ticks_per_sec *= 1.25; }
    if is_key_pressed(KeyCode::P) { game.request_pause(); }

    if let Some(update) = poll_policy_update_from_ui() {
      // Valid only while paused at a tick boundary.
      if game.apply_policy_update(update).is_ok() {
        journal_append_policy(update, game.current_tick());
      }
    }

    if let Some((prompt_id, choice)) = poll_interrupt_choice_from_ui() {
      if game.apply_choice(prompt_id, choice).is_ok() {
        journal_append_choice(prompt_id, choice, game.current_tick());
      }
    }

    // 2) Fill simulation trace queue (simulation clock)
    if auto_enabled && pending_stop.is_none() && trace_queue.len() < TRACE_HIGH_WATER {
      let frame_start = get_time();
      while get_time() - frame_start < SIM_BUDGET_SECS && trace_queue.len() < TRACE_HIGH_WATER {
        let batch = game.advance(MAX_STEPS_PER_CALL, TRACE_CHUNK_MAX);
        trace_queue.extend(batch.trace);
        match batch.stop_reason {
          AdvanceStopReason::BudgetExhausted => {}
          stop => {
            pending_stop = Some(stop);
            break;
          }
        }
      }
    }

    // 3) Consume trace queue at configurable visual speed (presentation clock)
    visual_tick_accum += dt * visual_ticks_per_sec;
    while visual_tick_accum >= 1.0 && !trace_queue.is_empty() {
      let tick_frame = trace_queue.pop_front().unwrap();
      presented.apply_tick_frame(&tick_frame);
      visual_tick_accum -= 1.0;
    }

    // 4) Only show interrupt/game-over after matching visual playback catches up
    if trace_queue.is_empty() {
      if let Some(stop) = pending_stop.take() {
        match stop {
          AdvanceStopReason::Interrupted(interrupt) => {
            show_interrupt_ui(interrupt);
            auto_enabled = false;
          }
          AdvanceStopReason::PausedAtBoundary { .. } => {
            auto_enabled = false;
          }
          AdvanceStopReason::Finished(outcome) => {
            show_game_over_ui(outcome);
            auto_enabled = false;
          }
          AdvanceStopReason::BudgetExhausted => {}
        }
      } else {
        // Keep presented state aligned when no buffered trace remains.
        presented.sync_from_observation(game.observation());
      }
    }

    // 5) Render
    render_presented_state(&presented);
    next_frame().await;
  }
}
```

---

# 8. Milestone Roadmap

## Milestone 0 — Workspace Setup (3–5 hrs)
- Create 4-crate workspace (`core`, `content`, `app`, `tools`).
- Add rustfmt + clippy.
- Basic CI (test + lint).
- README.
*Done when: `cargo test` passes cleanly.*

## Milestone 1 — CoreSim Skeleton & Initial UI (10–12 hrs)
- Set up `slotmap` for actors + item instances in `core`, and `ChaCha8Rng`.
- Implement `RunState` and basic map structure (dense tile arrays).
- Implement `advance(...)` API (`AdvanceResult`, `TickFrame`, stop reasons) and prompt-bound `apply_choice`.
- Implement headless replay API in `core` (`replay_to_end(content, journal) -> ReplayResult`).
- Add thin `tools/replay_runner` CLI wrapper that prints final `snapshot_hash`/outcome from a journal file.
- Build the minimal Macroquad `app` shell to render a simple grid, proving the core/app communication contract.
- Implement two-clock auto loop (simulation fill + playback consume) with pause-at-next-tick-boundary behavior.
- Implement Player + 1 enemy, turn engine, and fake loot interrupt.
*Done when: repeated runs with the same seed/journal produce identical final snapshot hash via headless replay API.*

## Milestone 2a — Basic Pathing & Interrupt Loop (10–12 hrs)
- Implement A* pathing on discovered known-walkable tiles with fixed tie-break order.
- Implement minimal frontier selection (nearest unknown-adjacent discovered tile).
- Implement `AutoExploreIntent { target, reason, path_len }` as required core output.
- Emit `LogEvent::AutoReasonChanged { reason, target, path_len }` whenever target/reason changes.
- Render ASCII map and display event log in `app`.
- Implement trace-driven playback controls (visual ticks/sec) independent of simulation throughput.
- Implement keep/discard and fight-vs-avoid interrupt panels using stable IDs.
*Done when: 5-minute auto-exploring run is playable, pauses on interrupts, and intent/reason changes are inspectable in the event log.*

## Milestone 2b — FOV & Exploration Intelligence (6–8 hrs)
- Implement minimum viable deterministic FOV in `core` (simple shadowcasting or equivalent simple method).
- Improve frontier selection using visible frontier only.
- Implement danger scoring v0: avoid known hazard tiles only.
- Treat closed doors as walls until explicitly opened through an interrupt (no full door simulation in 2b).
- Expand `AutoReason` usage for FOV/hazard-driven decisions.
*Done when: explore remains coherent under FOV constraints and hazard avoidance v0, without complex door/hazard simulation.*
Scope guard: advanced door/hazard simulation and richer danger scoring defer to Milestone 6 or post-MVP.

## Milestone 3 — Combat + Policy (15–18 hrs)
- Multi-enemy encounters.
- Implement MVP `Policy` controls (Target priority, Stance modifiers, Consumable thresholds, Retreat logic, restricted `PositionIntent`, pre-combat loadout rule).
- Restrict policy updates to paused tick boundaries and journal every accepted update with boundary tick.
- Implement `SwapLoadout` as a time-costing simulation action (including auto-trigger from `loadout_rule`).
- Ensure first-sighting `EnemyEncounter` interrupts occur before opening combat actions.
- Implement a micro-set of test content (2 weapons, 1 consumable, 2 perks) to validate policy behaviors.
- Wire UI to update policy knobs.
- Add baseline fairness instrumentation: death-cause reason codes, enemy-encounter `ThreatSummary`, and a compact per-turn threat trace.
*Done when: automated combat resolves from policy/build choices, opening swap costs are explicit/time-costed, encounter threat summaries are visible pre-commit, and death traces explain what happened with spatially naive positioning.*
Scope guard: advanced tactical repositioning (kiting/LOS-breaking/corner play) defers to post-MVP.

## Milestone 4 — Floors + Branching (12–15 hrs)
- Multiple floors.
- Descend/ascend mechanics.
- Overworld selector.
- Branch modifies spawn tables.
*Done when: route choice matters.*

## Milestone 5 — Content Pass (15–18 hrs)
- Populate `content` crate: ~15 items, ~10 perks, 2 gods.
- 6–8 enemy types, 1 boss.
- 3–5 vault templates.
*Done when: 3+ viable archetypes exist.*

## Milestone 6 — Fairness Tooling (8–10 hrs)
- Expand/retune threat summaries beyond MVP heuristics.
- Seed display + copy.
- Determinism hash.
- Death-recap UI using reason codes from Milestone 3.
*Done when: deaths are reproducible and explainable to the player.*

## Milestone 7 — Persistence, Replay, and Easy Mode Checkpoints (8–10 hrs)
- Implement append-only `InputJournal` logging.
- Load games by fast-forwarding journal events.
- Add deterministic checkpoint marker generation at engine-authored boundaries (including perk/build-defining loot events).
- Implement death flow to select checkpoint and restore via replay from tick 0 to marker `input_seq`.
- Use atomic writes + checksums for crash-safe persistence.
*Done when: save/load, replay, and checkpoint time travel are reliable within the same build/content fingerprint.*

## Milestone 8 — Release Packaging (8–10 hrs)
- Versioning and final balance pass.
- macOS + Linux builds.
- Run summary screen.
- GitHub release or Itch upload.
*Done when: a friend can download and play.*

---

# 9. Deployment Plan

Primary:
- GitHub Releases

Secondary:
- Itch.io distribution

Windows support: optional later.

---

# 10. Design Principles

1. Policy over micromovement
2. Determinism over spectacle
3. Build identity emerges early
4. Resource efficiency determines survival
5. No opaque randomness
6. Systems over content volume
7. Roguelike identity with automation twist

---

# 11. Test Strategy and Complexity Analysis (MVP)

Goal: maximize bug-catching per engineering hour, given a ~120 hour budget.

| Test Layer | Value | Cost to Build | Ongoing Maintenance | MVP Decision |
| --- | --- | --- | --- | --- |
| Unit tests for combat, pathing, interrupt validation | High | Low | Low | Include |
| Headless replay executor (`core` API + thin CLI wrapper) | High | Low-Medium | Low | Include |
| Determinism smoke tests (same seed => same hash/interrupt trace in same build) | High | Low | Low | Include |
| Property tests (`proptest`) for invariants (no negative HP, no invalid target IDs, replay/apply equivalence) | High | Medium | Low-Medium | Include (small targeted set) |
| Golden replay files committed to repo | Medium | Medium-High | High (content churn rewrites files) | Defer |
| Cross-version replay compatibility tests | Low for MVP | High | High | Defer post-MVP |

Rationale:
- Golden replay files are expensive during rapid content iteration because expected outputs churn frequently.
- Small, focused property tests catch broad logic bugs without coupling tests to unstable content details.
- Determinism smoke tests and replay round-trip tests (including checkpoint restore equivalence) provide core confidence with low maintenance.
- The replay executor is API-first (better for CI and library-level tests); CLI remains a convenience layer, not the core contract.
- Add batching-equivalence tests (`N` single-tick advances vs one `advance(N, ..)`) to protect performance optimizations without risking behavior drift.

---

# 12. Post-MVP Backlog

- Snapshot-accelerated checkpoint restore for faster reloads.
- Best-effort migration layer for replay files across content/code versions.
- Optional golden replay corpus once content format stabilizes.
