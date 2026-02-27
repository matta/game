# Roguelike Project — MVP 1.0 Complete Integrated Development Plan

Author: Matt Armstrong  
Stack: Rust + Macroquad  
Target: Desktop (macOS + Linux)  
Run Length: 20–40 minutes  
Time Budget: ~120 hours (10 hrs/week × 12 weeks)
Determinism Budget: Capped at 25% of total hours (~30 hours)

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
- Branching dungeon structure (strict one-way descent)
- Ironman primary mode
- Optional checkpoint-based easy mode (time travel via full replay from tick 0)
- Journal-based save/load (same-build compatibility only)
- Fully deterministic seed-based runs

Deferred post-MVP:
- Snapshot-accelerated reloads (fast checkpoint restore without full replay)
- Best-effort replay migration across code/content versions

---

# 2. Workspace Architecture

Cargo workspace layout utilizes three distinct crates to enforce strict separation of concerns and protect the deterministic core simulation:

```text
/Cargo.toml
crates/
  core/      # Deterministic simulation engine + Hardcoded Content
  app/       # Macroquad frontend (UI shell and input translation)
  tools/     # Optional balance/replay tools
```

## 2.1 Memory Management Strategy
The `core` crate uses Generational Arenas (`slotmap`) for **actor** storage (player + monsters) and **item instance** storage (stable `ItemId`). This avoids borrow checker conflicts and prevents use-after-free errors when entities are created and destroyed during simulation, without resorting to complex ECS frameworks.

Map tiles remain dense arrays/structs; each tile stores stable IDs/flags rather than positional indices for player-facing references. Static content definitions (item/perk/god blueprints) remain dense arrays in `core::content`.

---

# 3. Determinism Contract

Requirements to guarantee identical runs across macOS and Linux:
- Single RNG source: `rand_chacha::ChaCha8Rng` explicitly seeded at run start.
- No `thread_rng()` or system time access in `core`.
- Avoid floating-point nondeterminism (use integer math exclusively for logic/combat).
- All randomness injected explicitly.
- Deterministic containers/iteration: avoid `HashMap`/`HashSet` in simulation-critical paths unless keys are copied into a sorted buffer before iteration. Prefer `Vec` + explicit sort, or `BTreeMap`/`BTreeSet`.
- `slotmap` iteration order is treated as non-contractual; deterministic systems/hashing must not depend on raw arena iteration order.
- To avoid fragility, do not use monotonic `spawn_seq` counters for canonical state or hashing; minor cosmetic changes might drift the count and break replays.
- For whole-world passes (turn queue rebuild, threat scans, hashing), iterate actors/items sorted by stable visible identifiers (e.g., `(Pos.y, Pos.x, EntityKind)`).
- Deterministic tie-breakers:
  - Turn order: `(next_action_tick, speed_desc, pos.y, pos.x, entity_kind)`; do not rely on generational key ordering for ties.
  - Path tie resolution: fixed neighbor expansion order `Up, Right, Down, Left`.
- Stable snapshot hash: `xxh3_64` over canonical binary encoding of minimal deterministic state only:
  - `seed`, `tick`
  - map tiles + deterministic tile metadata
  - actor arena contents sorted by stable `(Pos, Kind)` keys
  - item arena contents sorted by stable `(Pos, Kind)` keys
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

pub struct AdvanceResult {
  pub simulated_ticks: u32,
  pub stop_reason: AdvanceStopReason,
}

impl Game {
  pub fn new(seed: u64, content: &ContentPack, mode: GameMode) -> Self;
  pub fn advance(&mut self, max_steps: u32) -> AdvanceResult;
  pub fn request_pause(&mut self);
  pub fn apply_choice(
    &mut self,
    prompt_id: ChoicePromptId,
    choice: Choice
  ) -> Result<(), GameError>;
  pub fn apply_policy_update(&mut self, update: PolicyUpdate) -> Result<(), GameError>;
  pub fn current_tick(&self) -> u64;
  pub fn state(&self) -> &GameState;
  pub fn snapshot_hash(&self) -> u64;
}
```

Execution rule:
- MVP threading model is single-threaded: Macroquad app loop and `Game` simulation run on the same main thread.
- `app` auto mode advances with synchronous batch stepping: `advance(max_steps)`.
- `core` still advances one tick at a time internally and may return early on interrupt, finish, pending pause, or batch budget exhaustion.
- Once the batch is complete, the `app` immediately renders the final state.
- Because the trace queue has been dropped for the MVP, the player will see discrete leaps in state during auto-explore.
- User pause requests are latched via `request_pause()` (called by the app after input polling on the same thread) and honored at the next tick boundary (`PausedAtBoundary`).

## 3.1 Input Journal Specification (Replay-Only MVP)

To minimize complexity and enforce determinism, the MVP uses **exactly one** state restoration mechanism: a versioned, append-only input journal. There is no `GameState` snapshot serialization.

- **Saving (ironman):** Write `{seed, inputs_so_far}` to disk.
- **Loading:** Instantiate from `seed`, replay all inputs to reconstruct state.
- **Easy Mode:** Load the same file, truncate inputs to a checkpoint marker, and replay from tick 0.
- **Debugging:** Export the same file to reproduce bugs perfectly.

```rust
pub struct InputJournal {
  pub format_version: u16,
  pub build_id: String,      // git SHA or build fingerprint
  pub content_hash: u64,     // content pack fingerprint
  pub seed: u64,
  pub inputs: Vec<InputRecord>,
  pub checkpoints: Vec<CheckpointMarker>, // easy-mode restore candidates
}

pub struct InputRecord {
  pub seq: u64,
  pub payload: InputPayload,
}

pub enum InputPayload {
  Choice { prompt_id: ChoicePromptId, choice: Choice },
  PolicyUpdate(PolicyUpdate), // takes effect on the next simulation tick
}

pub struct CheckpointMarker {
  pub id: u32,
  pub input_seq: u64,      // restore by truncating inputs beyond this sequence
  pub reason: CheckpointReason,
}

pub enum CheckpointReason {
  GodChoiceResolved,
  BranchCommitted,
  CampResolved,
  PerkChosen,
  BuildDefiningLootResolved,
}
```

MVP Disk I/O Semantics:
- The journal is held in memory and written to disk only when an `InputRecord` is appended (i.e., upon resolving an interrupt or updating a policy).
- Writing uses a simple write-to-temp-file and atomic-rename to prevent corruption. No continuous per-tick disk syncing.
- `ChoicePromptId` is emitted by `core` with each `Interrupt`, and must match on `apply_choice(...)`.
- MVP compatibility guarantee: journal replay is supported only when `build_id` and `content_hash` match.

## 3.2 Core / App Communication Contract

- `app` provides **pure inputs**: `InputPayload`.
- `core` returns **pure outputs**: `AdvanceResult`, `LogEvent`, and provides immutable view access via `state()`.
- `app` never writes `core` internals except through the public API mutations (`apply_choice`, etc.).

The `app` crate controls the execution loop, deciding whether to auto-explore or wait for user input (e.g., when paused). The `core` only receives inputs that mutate simulation state, remaining entirely ignorant of UI concepts.
The boundary between simulation logic and presentation is enforced by **Cargo dependencies**. The `core` crate does not depend on `macroquad` or any UI crates, ensuring simulation data structures (`Actor`, `Tile`, etc.) remain pure. The `app` crate reads `&GameState` directly and is fully responsible for matching logical state to rendering output (glyphs, colors, positions).

`AutoExploreIntent` is a required explainability artifact for pathing/frontier decisions, available on `&GameState`.
To prevent log spam while maintaining explainability, `core` emits `LogEvent::AutoSegmentStarted` only when the auto-explore planner selects a *new* target or fundamentally changes its reason for moving. It does not emit per-tick distance countdowns.
The `app` UI keeps this trace hidden by default during normal play, but it can be toggled via an "Inspect" overlay (e.g. `L` key) to help players understand *why* their current policy resulted in the game's routing decisions.

```rust
pub enum LogEvent {
  // ... other gameplay events
  AutoSegmentStarted {
    reason: AutoReason,
    target: Pos,
    planned_len: u16,
  },
}
```

Policy updates in MVP are applied only at tick boundaries while paused (from interrupt or user pause), and every accepted update is journaled with its `tick_boundary`.
Action-cost requirement: loadout swaps are manual in-world actions that consume ticks; no free pre-combat equipment changes.
MVP policy timing split:
- Loadout-affecting updates (equip/swap) are issued manually during an interrupt pause and consume time via `SwapActiveWeapon` action(s).
- Policy knob edits (priority/stance/thresholds) are applied at pause boundaries without direct tick cost.

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
- Each `advance(...)` call simulates up to N ticks synchronously before returning to the app for immediate rendering.
- `core` checks pending pause between internal ticks and exits on the next boundary.
- Pause changes control flow only and does not mutate simulation state.
- When a hostile is first seen in FOV during auto-explore, `core` emits `EnemyEncounter` before committing opening combat actions.
- Auto-explore halts at that interrupt so the player can inspect threat summary and adjust policy/loadout.
- While paused, the user may issue one or more policy updates.
- On resume, the next tick uses the latest accepted policy state.
- Replay applies policy updates at the recorded boundary before simulating the next tick.
- Interrupts/finish are surfaced as stop reasons; app shows related UI immediately.

## 3.4 Easy Mode Checkpoint Model

- Easy mode uses engine-authored checkpoint markers at deterministic boundaries (e.g., resolved god choice, branch commitment, resolved camp), omitting per-level checkpoints to preserve tension.
- Build-identity checkpoints are also supported (`PerkChosen`, `BuildDefiningLootResolved`).
- On death, player may select any unlocked checkpoint.
- **Restore mechanic:** Load the `InputJournal`, truncate the `inputs` list to the checkpoint marker's `input_seq`, and replay deterministically from tick 0.
- Because replay involves only discrete `InputRecord` evaluation, simulation-from-zero performs fast enough for an MVP without the complexity of building a separate `GameState` snapshot/serde system.
- To avoid checkpoint spam, build-defining loot checkpoints should be gated by explicit content flags/rarity tiers rather than firing for every pickup/equip.

---

# 4. Core Simulation Design

## 4.1 Core Data Structures

- `EntityId` (managed via `slotmap`)
- `ItemId` (managed via `slotmap`; stable in interrupt/replay flows)
- `Actor` (player or monster)
- `Stats` (hp, atk, def, speed, limited resists)
- `Status` (poison, bleed, slow — minimal set)
- `Equipment` (small fixed slot set)
- `ItemInstance`
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
  pub retreat_hp_threshold: u8,
  pub auto_heal_if_below_threshold: Option<u8>,
  pub position_intent: PositionIntent, // MVP-restricted intent set
  pub resource_aggression: Aggro,
  pub exploration_mode: ExploreMode,
}

pub enum PositionIntent {
  HoldGround,
  AdvanceToMelee,
  FleeToNearestExploredTile, // Utilizes "doorway bias" heuristic
}
```

## 4.3 Spatial Systems
All spatial algorithms live strictly within `core`, with no dependency on external rendering libraries.

- **Pathing + minimal exploration intent (Milestone 2a):** A* pathing on discovered known-walkable tiles plus a simple frontier target selection (nearest unknown-adjacent discovered tile), with `AutoExploreIntent { target, reason, path_len }` produced as a first-class output.
- **FOV minimum viable pass (Milestone 2b):** Simple deterministic FOV + frontier refinement + hazard-avoidance v0 (avoid known hazard tiles).

## 4.4 Deterministic Traversal Rules

- Never rely on raw `slotmap` iteration order for simulation decisions, logging semantics, or hashing.
- Before deterministic global passes, obtain active IDs and evaluate records ordered by stable sorting keys (e.g. `(Pos.y, Pos.x, Kind)`).
- Do not silently fallback to generational key order for tie-breaking; if stable sorting keys collide, use a deterministic secondary attribute.

## 4.5 Loadout Action Semantics (MVP)

- `SwapActiveWeapon` is a first-class simulation action with tick cost (same timing model as other actor actions).
- `SwapActiveWeapon` is strictly a manual command issued by the player upon an interrupt pause; auto behavior never bypasses turn economy to swap gear.
- Enemy reactions occur according to normal turn ordering; swapping can expose player to incoming actions.
- Emit deterministic log events for executed swaps so replay/debug can explain pre-combat openings.

## 4.6 MVP Combat Positioning Scope

- Multi-enemy encounter handling is intentionally spatially naive in MVP.
- `PositionIntent` is restricted to `HoldGround`, `AdvanceToMelee`, and `FleeToNearestExploredTile`.
- **"Doorway Bias" Heuristic:** Fleeing utilizes a single deterministic spatial heuristic: If ≥2 visible ranged enemies and current tile has ≥2 walkable neighbors, prefer stepping backward to a chokepoint.
- **Sanctuary Zones:** Stair tiles (and the first tile of a new floor) are absolute safe zones (no enemy entry). Fleeing successfully to the stairs offers a genuine escape vector and resets an encounter.
- No advanced tactical repositioning in MVP: no kiting logic, no deliberate LOS-break maneuvers, no corner-peeking planner.
- Enemy selection and threat handling still use deterministic target-priority and retreat thresholds.

---

# 5. Interrupt Model

Interrupt types (MVP subset):

- LootFound (Auto-discard trivial items. Only interrupt on "build-relevant" or rarity-gated loot. Reduce interrupt frequency to maintain flow.)
- EnemyEncounter (first-sighting pre-commit stop)
- HpThresholdReached (drops below `retreat_hp_threshold`, pausing for manual intervention)
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
  pub danger_tags: Vec<DangerTag>, // e.g., Poison, Burst, Ranged
}

pub struct OpeningActionPreview {
  pub action: OpeningAction,
  pub tick_cost: u8,
}

pub enum OpeningAction {
  SwapActiveWeapon { to: LoadoutId },
  HoldCurrentLoadout,
}
```

MVP guidance:
- The threat summary relies on static facts (e.g. tags, basic stats). No speculative damage forecasting or heuristics in MVP.
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
- Overlay: Inspect Trace panel (hidden by default)

Input:
- Space: toggle auto
- Esc: menu
- Number keys: interrupt choices
- Tab: cycle policy presets
- L: toggle Auto Trace inspect panel

## 7.1 Main Loop Sketch (Single Thread)

```rust
#[macroquad::main("Roguelike")]
async fn main() {
  let mut game = Game::new(seed, &content, mode);
  let mut auto_enabled = true;
  let mut presented = PresentedState::from_state(game.state());

  loop {
    // 1) Poll user input (Macroquad keyboard/mouse APIs)
    if is_key_pressed(KeyCode::Space) { auto_enabled = !auto_enabled; }
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

    // 2) Batch simulation
    if auto_enabled {
      let batch = game.advance(MAX_STEPS_PER_CALL);
      
      match batch.stop_reason {
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
      
      // Keep presented state aligned.
      presented.sync_from_state(game.state());
    }

    // 3) Render
    render_presented_state(&presented);
    next_frame().await;
  }
}
```

---

# 8. Milestone Roadmap

## Milestone 0 — Workspace Setup (3–5 hrs)
- [x] Create 3-crate workspace (`core`, `app`, `tools`).
- [x] Add rustfmt + clippy.
- [x] Basic CI (test + lint).
- [x] README.
**Exit Criteria:**
- **a) User Experience:** None. This is pure developer scaffolding.
- **b) Progress toward vision:** Sets up the open-source baseline (Vision 6.5).
- **c) Architecture & Maintainability:** Establishes the 3-crate layout protecting the deterministic layer (`core`). CI/CD pipelines, cargo formatting, and basic linting are in place, confirming a sustainable development environment.

## Milestone 1 — CoreSim Skeleton & Initial UI (10–12 hrs)
- [x] Set up `slotmap` for actors + item instances in `core`, and `ChaCha8Rng`.
- [x] Implement `RunState` and basic map structure (dense tile arrays).
- [x] Implement `advance(...)` API (`AdvanceResult`, stop reasons) and prompt-bound `apply_choice`.
- [x] Implement headless replay API in `core` (`replay_to_end(content, journal) -> ReplayResult`).
- [x] Add thin `tools/replay_runner` CLI wrapper that prints final `snapshot_hash`/outcome from a journal file.
- [x] Build the minimal Macroquad `app` shell to render a simple grid, proving the core/app communication contract.
- [x] Implement single-clock auto loop (synchronous batch stepping) with pause-at-next-tick-boundary behavior.
- [x] Implement Player + 1 enemy, turn engine, and fake loot interrupt.
**Exit Criteria:**
- **a) User Experience:** A rudimentary static ASCII grid appears, rendering a player, an enemy, and simulating an interrupt. Visuals are purely functional debug outputs.
- **b) Progress toward vision:** Validates the strict turn-based simulation model (Vision 2.3) and the technical constraint of seed-based determinism (Vision 6.4).
- **c) Architecture & Maintainability:** The generative arena memory management (`slotmap`) is in place. Crucially, the headless replay API and determinism contract are proven strong, meaning any future bug can be perfectly reproduced by copying the input journal.

## Milestone 2a — Basic Pathing & Interrupt Loop (10–12 hrs)
- [x] Add `core` test fixtures/helpers to build tiny discovered/walkable map layouts for pathing/frontier tests.
- [x] Implement deterministic neighbor expansion helper in fixed order: `Up, Right, Down, Left`.
- [x] Implement A* pathing over discovered known-walkable tiles only.
- [x] Add deterministic tie-break ordering inside A* open-set selection (`f`, then `h`, then `y`, then `x`).
- [x] Return `None` when no valid route exists to the selected frontier target.
- [x] Add unit test: straight-line path over discovered walkable tiles is found with expected length.
- [x] Add unit test: tie case chooses the deterministic first route (matching neighbor/tie ordering).
- [x] Add unit test: unknown or blocked tiles are never used by the generated path.
- [x] Add unit test: unreachable target yields no path and does not panic.
- [x] Implement frontier candidate collection: discovered tiles adjacent to at least one unknown tile.
- [x] Implement frontier selection: choose nearest candidate by path length, then deterministic `(y, x)` tie-break.
- [x] Add unit test: nearest frontier is selected when multiple candidates exist.
- [x] Add unit test: no frontier candidate results in a safe "exploration complete/stuck" outcome.
- [x] Populate `AutoExploreIntent { target, reason, path_len }` from current planner output.
- [x] Recompute intent only when target/reason/path actually changes.
- [x] Emit `LogEvent::AutoReasonChanged { reason, target, path_len }` exactly when intent changes.
- [x] Add unit test: unchanged intent across ticks does not emit duplicate reason-change log entries.
- [x] Render discovered map tiles and entities as ASCII glyphs in `app`.
- [x] Add a bounded event log panel in `app` and show newest events in stable order.
- [x] Display current auto-explore intent summary (`reason`, `target`, `path_len`) in UI debug text/log.
- [x] Implement keep/discard interrupt panel bound to stable item IDs.
- [x] Implement fight-vs-avoid interrupt panel bound to stable actor/encounter IDs.
- [x] Add integration test (or scripted harness) for loop: auto-explore -> interrupt -> choice -> resume.
- [x] Add deterministic smoke check for the milestone flow by replaying a fixed seed and asserting stable intent/log sequence.
**Exit Criteria:**
- **a) User Experience:** The player can watch a character automatically traverse a dungeon, pausing natively for rudimentary item, enemy, or choice pop-up panels. Intent tracing is visible in the UI event log.
- **b) Progress toward vision:** The "One-Sentence Loop" (auto-explore, event interrupt) is visibly functioning (Vision 2.1).
- **c) Architecture & Maintainability:** Pathing and frontier selection are decoupled from UI frames. The `AutoExploreIntent` logging mechanism ensures complex AI decisions can be analyzed and logged without console spam.

## Milestone 2b — FOV & Exploration Intelligence (6–8 hrs)
- [x] Add test fixtures for FOV cases: open room, wall occlusion, hazard lane, and closed-door choke.
- [x] Add map visibility helpers (`clear_visible`, `set_visible`, `is_visible`) and bounds tests.
- [x] Add unit test: deterministic FOV in an open room returns the expected visible tile set.
- [x] Add unit test: wall occlusion blocks visibility behind the wall.
- [x] Add unit test: running FOV twice on identical state yields identical visible results.
- [x] Implement deterministic FOV pass in `core` using a fixed traversal order and integer math only.
- [x] Recompute visible tiles at run start and after each player movement tick.
- [x] Promote currently visible tiles to `discovered` while preserving prior discoveries.
- [x] Add integration test: moving one tile updates visibility and expands discovery deterministically.
- [x] Add unit test: frontier selection ignores frontier tiles outside current visibility.
- [x] Update frontier candidate filter to require visibility (visible frontier only).
- [x] Keep deterministic tie-breaks for visible frontier selection (`path_len`, then `y`, then `x`).
- [x] Add hazard metadata for tiles in `core` map fixtures/content hooks (known hazard = discovered hazardous tile).
- [x] Add unit test: planner picks a non-hazard path when a safe visible alternative exists.
- [x] Add unit test: planner returns no safe frontier intent when all visible options require known hazards.
- [x] Implement danger scoring v0 by excluding known hazard tiles from auto-explore path candidates.
- [x] Add `TileKind::ClosedDoor` (or equivalent closed-door marker) to map semantics.
- [x] Add unit test: closed doors are treated as non-walkable for FOV frontier pathing.
- [x] Add interrupt emission when a closed door blocks the chosen auto path segment.
- [x] Add minimal interrupt resolution for doors: open door tile and resume (no advanced door simulation).
- [x] Add integration test: closed-door interrupt -> open -> auto-explore resumes through the doorway.
- [x] Expand `AutoReason` variants for FOV/hazard/door-driven decisions.
- [x] Add unit test: `AutoReasonChanged` emits only when reason/target class changes under FOV/hazard/door scenarios.
- [x] Update app log text mappings for new `AutoReason` values.
- [x] Update app rendering so unknown tiles remain obscured and current visibility is reflected consistently.
**Exit Criteria:**
- **a) User Experience:** The map obscures unknown tiles, bringing a feeling of discovery. Movement looks intentional despite constrained vision. Hazards are avoided naturally.
- **b) Progress toward vision:** Reinforces the "decisions matter" pillar (Vision 1.1). Managing incomplete information becomes part of the core fantasy.
- **c) Architecture & Maintainability:** FOV algorithms remain fully deterministic within `core`. Spatial algorithms are abstracted to allow test injection of fake hazard layouts.

Scope guard: advanced door/hazard simulation and richer danger scoring defer to Milestone 6 or post-MVP.

Post-2a review carry-over:
- [ ] Low-priority cleanup (defer unless it blocks clarity): either emit `AutoReason::Stuck` on no-frontier/no-path states or remove the unused variant.
- [ ] Post-MVP door-state semantics: preserve door identity when opened (e.g., `ClosedDoor -> OpenDoor`) instead of collapsing to `Floor`.

## Milestone 3 — Combat + Policy (15–18 hrs)

Execution guardrails for all Milestone 3 passes:
- [ ] Execute passes strictly in order: `3a -> 3b -> 3c -> 3d -> 3e`.
- [ ] Use TDD in every pass: add failing test(s) first, then implement the minimum code to pass.
- [ ] After each pass, run all required checks with no warnings/errors: `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`.
- [ ] Do not redesign policy architecture during implementation; use the contracts already defined in sections **4.2**, **4.5**, **4.6**, and **5**.

### Milestone 3a — Starter Dungeon Coverage Baseline (3–4 hrs)
- [x] Replace the current single-room default map in `Game::new` with a deterministic starter layout on a `20x15` map using exact coordinates:
- [x] Room A floor rectangle: `x=2..6`, `y=3..7` (player start room; player starts at `(x=4, y=5)`).
- [x] Room B floor rectangle: `x=9..13`, `y=3..7`.
- [x] Room C floor rectangle: `x=9..13`, `y=9..13`.
- [x] Corridor A<->B floor tiles: `(x=7, y=5)` and `(x=8, y=5)` with `ClosedDoor` at `(x=8, y=5)`.
- [x] Corridor B<->C floor tiles: `(x=11, y=8)` and `(x=11, y=9)`.
- [x] Hazard lane tiles (all hazardous): `(x=11, y=8)`, `(x=11, y=9)`, `(x=11, y=10)`.
- [x] Spawn at least two goblins for deterministic multi-enemy setup at fixed positions `(x=11, y=5)` and `(x=11, y=11)`.
- [x] Ensure the default starter auto-flow can reach at least one true multi-enemy encounter (interrupt payload contains `enemies.len() >= 2`) without test-only actor injection.
- [x] Add a starter-layout unit test that asserts door placement, hazard placement, and actor spawn coordinates exactly.
- [x] Add an integration smoke test on fixed seed that confirms normal auto-run can hit both `DoorBlocked` interrupt and `AutoReason::ThreatAvoidance` within `<= 250` ticks.
**Pass 3a Exit Criteria:**
- **a) User Experience:** A fresh run no longer feels like a single test chamber; the player traverses multiple connected rooms and can naturally encounter a door stop and hazard-influenced exploration.
- **b) Progress toward vision:** Promotes exploration from a synthetic demo to a representative dungeon slice and verifies Milestone 2b systems (door/hazard/FOV intenting) in normal play flow.
- **c) Architecture & Maintainability:** Starter layout is fully deterministic and locked by coordinate-based tests, so future combat/policy work can rely on a stable integration map.

### Milestone 3b — Multi-Enemy Encounter Interrupt Semantics (3–4 hrs)
- [x] Expand enemy encounter collection to support multiple hostiles in a single interrupt event.
- [x] Deterministic enemy ordering rule for encounter lists: sort by `(distance_to_player, pos.y, pos.x, actor_kind)`.
- [x] Define `primary_enemy` as index `0` from the sorted encounter list; `Fight`/`Avoid` applies to that enemy only.
- [x] Ensure first-sighting `EnemyEncounter` interrupt is emitted before opening combat actions or auto movement for that tick.
- [x] Keep `Avoid` suppression scoped to one enemy (`primary_enemy`) and clear suppression when that enemy is no longer adjacent or no longer exists.
- [x] Add unit test: two adjacent enemies produce a stable sorted encounter list and stable `primary_enemy`.
- [x] Add unit test: `Avoid` suppresses only the selected enemy and still allows interrupts from another adjacent enemy.
- [x] Add integration test: resolving one enemy in a multi-enemy encounter leaves remaining enemies active and interrupt-capable.
**Pass 3b Exit Criteria:**
- **a) User Experience:** Multi-enemy contact is understandable and consistent: the player gets a clear encounter pause with deterministic enemy ordering and predictable follow-up behavior after `Fight` or `Avoid`.
- **b) Progress toward vision:** Removes the hidden single-enemy assumption and establishes the minimum viable multi-hostile combat entry point needed for policy-driven encounters.
- **c) Architecture & Maintainability:** Encounter selection/suppression semantics are explicit and test-locked, preventing nondeterministic target flipping or regressions in interrupt priority.

### Milestone 3c — Policy Model + Boundary-Safe Update Pipeline (3–4 hrs)
- [x] Implement `Policy` state in `GameState` using the schema from section **4.2** (no extra fields).
- [x] Add concrete default policy values:
- [x] `fight_or_avoid = Fight`.
- [x] `stance = Balanced`.
- [x] `target_priority = [Nearest, LowestHp]`.
- [x] `retreat_hp_threshold = 35`.
- [x] `auto_heal_if_below_threshold = None`.
- [x] `position_intent = HoldGround`.
- [x] `resource_aggression = Conserve`.
- [x] `exploration_mode = Thorough`.
- [x] Add `PolicyUpdate` payload variants to mutate each policy field independently.
- [x] Add `Game::apply_policy_update` and enforce timing rule: updates are accepted only while paused at a tick boundary (`Interrupted` or `PausedAtBoundary` state), otherwise rejected.
- [x] Extend journal input payloads with `PolicyUpdate { tick_boundary, update }` and append every accepted update with current boundary tick.
- [x] Extend replay to apply journaled policy updates at the recorded boundary before the next simulation tick.
- [x] Add tests for accepted/rejected timing and replay equivalence (`same seed + same journal => same final hash`).
**Pass 3c Exit Criteria:**
- **a) User Experience:** While paused, the player can change policy knobs and trust that updates apply exactly when intended (not mid-tick), with invalid timing rejected cleanly.
- **b) Progress toward vision:** Policy control becomes a concrete game mechanism rather than a design placeholder, and replay/save flows now include policy decisions as first-class inputs.
- **c) Architecture & Maintainability:** Boundary-gated policy updates plus journal/replay coverage preserve determinism and prevent state corruption from out-of-band policy edits.

### Milestone 3d — Policy Effects + SwapActiveWeapon + Micro Content (3–4 hrs)
- [x] Implement policy-driven target selection using `target_priority` when multiple enemies are in range; fallback tie-breaker remains `(distance, y, x, kind)`.
- [x] Implement stance modifiers with fixed deterministic combat deltas:
- [x] `Aggressive`: `+2 atk`, `-1 def`.
- [x] `Balanced`: `+0 atk`, `+0 def`.
- [x] `Defensive`: `-1 atk`, `+2 def`.
- [x] Implement retreat trigger check at encounter start: if `hp_percent <= retreat_hp_threshold`, mark encounter as retreat-eligible for policy/UI.
- [x] Implement `SwapActiveWeapon` as a simulation action with fixed tick cost `10`. Since there is no inventory grid, this toggles between a primary and reserve weapon. Available anytime the game is paused, but typically used during an enemy encounter.
- [x] Ensure `SwapActiveWeapon` execution is journaled and replayed deterministically like other player inputs.
- [x] Add micro validation content in `core::content`: exactly `2` weapons, `1` consumable, `2` perks with deterministic numeric effects (no proc chance).
- [x] Add tests verifying swap tick cost is applied and affects turn order deterministically.
- [x] Add tests verifying stance and target priority change deterministic combat outcomes in fixture encounters.
**Pass 3d Exit Criteria:**
- **a) User Experience:** None directly. The combat simulator natively understands how to interpret policy commands and weapon swaps exist as discrete time-consuming actions, but the UI to control these does not yet exist.
- **b) Progress toward vision:** Foundational. Provides the numerical and state-machine backing for the "policy over micromovement" combat loop, but the loop is not "end-to-end" until the UI is wired in Pass 3e.
- **c) Architecture & Maintainability:** Combat deltas and swap timing are explicit numeric rules with deterministic tests, reducing ambiguity and making future balancing/refactors safer.

### Milestone 3e — UI Wiring + Fairness Instrumentation (2–3 hrs)
- [x] Add app controls to edit all MVP policy knobs while paused; disallow edits while running.
- [x] Add encounter panel rendering for deterministic `ThreatSummary` with sorted danger tags.
- [x] Add baseline death-cause reason codes and surface the reason code in defeat UI/log output.
- [x] Add compact per-turn threat trace ring buffer (latest `32` entries) with deterministic fields: `tick`, `visible_enemy_count`, `min_enemy_distance`, `retreat_triggered`.
- [x] Add tests ensuring `ThreatSummary` tag ordering is deterministic and stable across repeated runs with identical inputs.
- [x] Add integration test validating policy edit -> resume -> resulting behavior and logs are replay-stable.
**Pass 3e Exit Criteria:**
- **a) User Experience:** The player can fully configure policy from pause UI, inspect threat context before committing, and receive actionable defeat explanations instead of opaque losses. Policy choices have immediate, visible combat impact (targeting, stance behavior, retreat eligibility), and pre-combat loadout swaps feel fair because they consume deterministic time.
- **b) Progress toward vision:** Completes the milestone’s fairness and transparency layer so policy decisions are explainable, debuggable, and consistent with the intended combat control surface. Delivers the first end-to-end “policy over micromovement” combat loop, backed by minimal content that proves knob effects are meaningful in actual encounters.
- **c) Architecture & Maintainability:** Threat/death instrumentation uses deterministic ordering and bounded trace storage, enabling stable replay comparisons and low-noise debugging workflows.

**Exit Criteria:**
- **a) User Experience:** The player explores a richer starter dungeon, meets multi-enemy encounters, can tune policy/loadout while paused, and receives readable threat/death explanations.
- **b) Progress toward vision:** Combat policy controls (Vision 3.2) are concretely implemented and no longer abstract placeholders.
- **c) Architecture & Maintainability:** Each pass lands with deterministic tests, boundary-safe policy journaling, and replay-consistent outcomes.

Scope guard: advanced tactical repositioning (kiting/LOS-breaking/corner play) and predictive combat forecasting defer to post-MVP.

## Milestone 4 — Floors + Branching (12–15 hrs)

Execution guardrails for all Milestone 4 passes:
- [ ] Execute passes strictly in order: `4a -> 4b -> 4c -> 4d -> 4e`.
- [ ] Use TDD in every pass: add failing test(s) first, then implement the minimum code to pass.
- [ ] After each pass, run all required checks with no warnings/errors: `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`.
- [ ] Preserve DR-003 constraints throughout: strict one-way descent, no overworld selector, no ascending mechanics.

### Milestone 4a — Floor State + Deterministic Generator Contract (3–4 hrs)
- [x] Introduce explicit floor progression state in `core` with fixed MVP constants: `MAX_FLOORS = 3`, starting floor index `1`.
- [x] Add deterministic floor-generation API in `core` with full input contract: `(run_seed, floor_index, branch_profile) -> generated_floor`.
- [x] Define generated floor payload to include at minimum: map tiles, player entry tile, one down-stairs tile, enemy/item spawn placements.
- [x] Keep only one active floor state in memory at a time (prior floor state discarded on descent; no backtracking storage).
- [x] Enforce deterministic floor seed derivation from `(run_seed, floor_index, branch_profile)` using integer-only logic.
- [x] Add unit test: identical `(seed, floor, branch)` inputs produce byte-identical generated floor outputs.
- [x] Add unit test: changing `floor_index` changes generated floor output for fixed seed/branch.
- [x] Add unit test: generated floor always has a walkable route from entry tile to down-stairs tile.
**Pass 4a Exit Criteria:**
- **a) User Experience:** None directly yet; floor transitions may still be single-floor in runtime behavior, but generation primitives are in place.
- **b) Progress toward vision:** Establishes concrete multi-floor infrastructure instead of a single static map.
- **c) Architecture & Maintainability:** Floor generation becomes deterministic, testable, and isolated from UI concerns.

### Milestone 4b — One-Way Descent Runtime Flow (3–4 hrs)
- [x] Add explicit down-stairs semantics to runtime map flow (`TileKind` or equivalent) and place exactly one reachable down-stairs per floor.
- [x] Implement descent trigger: when player reaches down-stairs, pause via a deterministic floor-transition interrupt before generating the next floor.
- [x] Implement `Descend` choice handling: increment floor index, generate next floor, relocate player to next floor entry tile, recompute FOV/discovery.
- [x] Enforce one-way rule in runtime APIs and input handling: no `Ascend` choice, no up-stairs traversal, no return to prior floors.
- [x] On floor `MAX_FLOORS`, resolve descent target as run completion (or boss-floor completion path) with deterministic finish behavior.
- [x] Add integration test: fixed-seed run descends from floor 1 to floor 2 and receives a different deterministic map state.
- [x] Add integration test: no legal input path can move from floor N back to floor N-1.
- [x] Add integration test: floor transition interrupt occurs once per descent event and does not spam while paused on stairs.
- [x] Add deterministic simulation-stall watchdog in `core::Game::advance` so no-progress states terminate quickly with an explicit defeat reason (instead of spinning `BudgetExhausted` forever).
- [x] Add bounded replay watchdog + regression assertions so replay/test loops fail fast on non-terminating simulation regressions.
**Pass 4b Exit Criteria:**
- **a) User Experience:** The run now has visible forward momentum: reaching stairs moves the player into a new floor context.
- **b) Progress toward vision:** Implements strict one-way dungeon descent as a core loop element, consistent with DR-003.
- **c) Architecture & Maintainability:** Transition behavior is explicit and regression-tested, avoiding hidden floor-state coupling.

### Milestone 4c — Branch Choice + Persistent Branch Profile (3–4 hrs)
- [x] Introduce exactly one MVP branch commitment point: branch prompt appears on first descent only (floor 1 -> floor 2 transition). Merge the branch selection prompt directly into the descent interrupt to avoid back-to-back modal popups (mitigating Interrupt Fatigue).
- [x] Define two fixed branch profiles for MVP and freeze their modifiers:
- [x] `Branch A`: increases enemy density on floors 2–3 (`+1` enemy group spawn attempt per floor).
- [x] `Branch B`: increases hazard density on floors 2–3 (`+3` hazard tiles per floor, deterministic placement rules).
- [x] Persist branch commitment in run state and thread it into deterministic floor generation for all subsequent floors.
- [ ] Issue a checkpoint marker on Branch Commitment, but omit per-level checkpoints to preserve tension and mitigate the Emotional Dilution risk. Deferred to Milestone 6 journal/checkpoint implementation.
- [x] Ensure branch prompt is emitted exactly once per run and never re-offered after commitment.
- [x] Add unit test: same seed + same branch => identical floor 2/3 generation.
- [x] Add unit test: same seed + different branch => measurable deterministic difference in floor 2/3 spawn/hazard outputs.
- [x] Add integration test: branch prompt appears at the intended transition and branch choice changes later floor characteristics.
**Pass 4c Exit Criteria:**
- **a) User Experience:** The player makes a meaningful branch decision that visibly changes upcoming floors.
- **b) Progress toward vision:** Delivers branching dungeon structure in MVP scope without requiring overworld navigation.
- **c) Architecture & Maintainability:** Branch effects are explicit, bounded, and deterministic rather than ad-hoc map mutations.

### Milestone 4d — Floor Safety/Retreat Guarantees + Spawn Rules (1–2 hrs)
- [x] Implement stair-adjacent sanctuary spawn rule per floor transition: no enemy can spawn on entry tile or immediate adjacent tiles.
- [x] Implement runtime sanctuary rule: enemies treat the stair/'entry' tile as strictly non-walkable during A* and neighbor expansion.
- [x] Implement encounter-reset logic: if the player flees onto a sanctuary tile, purge the active threat state and transition out of combat mode.
- [x] Ensure down-stairs tile is never blocked by walls, hazards, or enemy spawn occupancy at generation time.
- [x] Add deterministic spawn ordering/tie-break rules for per-floor actor placement to prevent cross-platform drift.
- [x] Add unit test: sanctuary rule holds across generated floors for multiple fixed seeds.
- [x] Add unit test: stairs tile remains reachable and unoccupied at floor start.
- [x] Add unit test: enemies cannot pathfind onto the sanctuary tile even if the player is standing on it.
**Pass 4d Exit Criteria:**
- **a) User Experience:** Entering a new floor feels fair; the player gets a stable foothold instead of immediate unavoidable collapse.
- **b) Progress toward vision:** Supports lethal-but-fair pacing while preserving forward progression.
- **c) Architecture & Maintainability:** Generator invariants are encoded as tests, reducing regressions from future content tuning.

### Milestone 4e — App/UI Wiring + Multi-Floor Smoke Coverage (1–2 hrs)
- [ ] Surface current floor index and committed branch in app HUD/log output.
- [ ] Add branch-choice and descent prompt handling in app loop with explicit key bindings.
- [ ] Add end-to-end smoke test: fixed seed run reaches floor 3 through one branch path and remains replay-stable.
- [ ] Add companion smoke test for the alternate branch path confirming deterministic divergence in floor characteristics.
- [ ] Add regression test: app exposes no overworld selector and no ascend action bindings.
**Pass 4e Exit Criteria:**
- **a) User Experience:** Multi-floor progression and branch identity are visible and understandable during normal play.
- **b) Progress toward vision:** Completes milestone-level playability for floors + branches, not just headless core behavior.
- **c) Architecture & Maintainability:** App/core contract for floor transitions is validated by deterministic integration tests.

**Exit Criteria:**
- **a) User Experience:** Player traverses multiple distinct floors with one-way descent and makes a branch commitment that materially alters subsequent floors.
- **b) Progress toward vision:** Advances the 20–40 minute session arc with structured floor progression while honoring DR-003 one-way constraints.
- **c) Architecture & Maintainability:** Floor generation and transition logic are deterministic, bounded, and covered by unit + integration regressions.

### Milestone 4f — Minimal Headless Fuzzer + CI Integration (1–2 hrs)
- [x] Create `crates/tools/src/bin/fuzz.rs` to instantiate headless simulation loops.
- [x] Implement semantic fuzzing oracle: picks random valid choices upon encountering interrupts (Loot, Enemies, Doors, Floor Transitions).
- [x] Add inline invariant assertions on `&GameState` (e.g. valid HP paths, actor bounds, valid tiles) inside the fuzz loop.
- [x] Add CI step in `.github/workflows/ci.yml` to run the fuzzer for 1,000 steps on every push.
**Pass 4f Exit Criteria:**
- **a) User Experience:** None directly.
- **b) Progress toward vision:** Mechanically guarantees Vision 6.4 (Determinism + Fairness Validation) by fuzzing semantic paths to catch edge bugs, dead states, and panic conditions immediately.
- **c) Architecture & Maintainability:** Invariant stress testing is operational and wired into CI, protecting structural integrity continuously from future rapid prototyping and balance changes.

## Milestone 5 — Content Pass (15–18 hrs)
- [ ] Implement topological floor generation: replace the placeholder MVP tunnel with random non-overlapping `Rect` room placement and deterministic corridor routing to create actual chokepoints.
- [ ] 3–5 vault templates (e.g., Shrine rooms, Hazard corridors, Elite ambushes) stamped deterministically over the generated floors.
- [ ] Populate `core::content`: ~15 items, ~10 perks, 2 gods.
- [ ] 6–8 enemy types, 1 boss.
- [ ] **Weirdness Quota:** At least 5 items that modify rule systems (not just stat sticks). At least 3 perks that alter core mechanics (timing, targeting, economy).
**Exit Criteria:**
- **a) User Experience:** Substantial content density. The player encounters bosses, diverse enemies, and items with weird, rule-breaking properties, enabling wildly asymmetric build synergies.
- **b) Progress toward vision:** "Deep buildcrafting" (Vision 1.2) is tangibly achieved. Items feel unique, not like stat sticks (Vision 1.1).
- **c) Architecture & Maintainability:** Proves viability of defining engine logic alongside content (DR-008). Tests whether hardcoding item behaviors locally scales maintainably across the MVP boundaries.

## Milestone 6 — Fairness Tooling (8–10 hrs)
- [ ] Refine threat tags and static encounter facts.
- [ ] Seed display + copy.
- [ ] Determinism hash.
- [ ] Death-recap UI using reason codes from Milestone 3.
- [ ] Replace repeated per-candidate A* frontier scans with a single-pass BFS/Dijkstra nearest-frontier search.
- [ ] Reduce ASCII render complexity from per-cell entity scanning to an `O(MapCells + Entities)` pass (spatial lookup or per-entity overlay pass).
**Exit Criteria:**
- **a) User Experience:** Gameplay feels meticulously fair. The player can easily copy a run seed and review exact reason codes for their death.
- **b) Progress toward vision:** "Lethal-but-fair gameplay" and "No opaque randomness" (Vision 1.4, 8.5) physically realized.
- **c) Architecture & Maintainability:** Deepens determinism tooling. Ensures the game is fundamentally debuggable using player-submitted crash seeds as reliable regression tests.

## Milestone 7 — Persistence, Replay, and Easy Mode Checkpoints (8–10 hrs)
- [ ] Implement append-only `InputJournal` logging in memory, writing atomically on new inputs.
- [ ] Load games by replaying journal events from tick 0.
- [ ] Add deterministic checkpoint marker generation at engine-authored boundaries.
- [ ] Implement death flow to select checkpoint and restore via truncating journal and replaying from tick 0.
**Exit Criteria:**
- **a) User Experience:** Player can safely close the game and perfectly resume their run. If playing "easy mode," deaths gracefully bounce the player back to a natural progression checkpoint to try a different policy.
- **b) Progress toward vision:** Finalizes the targeted Saving Model (Ironman vs Checkpoint) (Vision 6.3).
- **c) Architecture & Maintainability:** Proves the append-only `InputJournal` replay loop in practice. Fast-forwarding simulation entirely removes the maintenance burden of generic state serialization (serde) snapshots.

## Milestone 8 — Release Packaging (8–10 hrs)
- [ ] Versioning and final balance pass.
- [ ] macOS + Linux builds.
- [ ] Run summary screen.
- [ ] GitHub release or Itch upload.
**Exit Criteria:**
- **a) User Experience:** A cohesive, polished client. Clean fonts, finalized colors, smooth input polling, and an intuitive run-summary screen. Completely playable as a standalone desktop app.
- **b) Progress toward vision:** Finalizes the Desktop target (Vision 6.1). Achieves the fully integrated MVP session length.
- **c) Architecture & Maintainability:** CI pipelines successfully yield distributable binaries. Final profiling guarantees UI responsiveness never stutters during synchronous auto-explore batches.

---

# 9. Deployment Plan

None.

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

**Hard Cap on Determinism Engineering:** Determinism engineering budget is strictly capped at 25% of total hours (~30 hours). Once replay works, snapshot hashing is stable, and there is one determinism smoke test — stop. No more determinism work until content is complete.

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

---

# 13. Decision Records

## DR-001: Synchronous Batch Simulation over Trace Animation
**Context:** Initial plan proposed a two-clock system where simulation ran decoupled from presentation, buffering `TickFrame` traces to animate auto-explore steps.
**Decision:** Dropped two-clock trace queue for MVP. We will execute synchronous simulation batches and immediately render the resulting state.
**Rationale:** MVP focus is on buildcrafting, not micromovement spectacle. Animating deterministic tick-by-tick steps introduces heavy async/state-management costs into the UI that don't serve the core gameplay loop.

## DR-002: Static Threat Display over Tactical Forecasting
**Context:** Initial plan required heuristic simulation of future outcomes (`expected_damage_band`, `escape_feasibility`) during enemy encounters.
**Decision:** Replaced dynamic forecasting with exposing static facts (danger tags, attack/defense stats).
**Rationale:** Automated tactical forecasting is an implementation risk, prone to bugs and high maintenance as content changes. Mastery should rely on the player learning the system, not an in-game analyst.

## DR-003: Strict One-Way Descent over Overworld Branches
**Context:** Goal was to have multiple floors, an overworld selector, and ascending/descending mechanics.
**Decision:** Cut the overworld and ascending features in favor of a strict one-way descent.
**Rationale:** Supporting two-way staircases means carrying complex world states (multiple floors) in memory or disk. A strict descent eliminates this massive state management burden and reinforces the game's core loop of "forward momentum".

## DR-004: Stable Identifiers over Monotonic Spawn Sequences
**Context:** Proposed caching chronological `spawn_seq` properties for deterministic entity hashing and traversal tie-breakers.
**Decision:** Removed `spawn_seq` counters from canonical state. Tie-breakers now prioritize stable, visible identifiers (e.g. `(Pos.y, Pos.x, EntityKind)`).
**Rationale:** Relying on creation sequence for simulation logic makes replays extremely fragile; a cosmetic tweak that creates an invisible particle effect or drops an item out-of-order would permanently break the run seed determinism.

## DR-005: Replay-Only State Restoration over Snapshots
**Context:** Needed a state restoration system for save/load, easy-mode time travel, and debugging. Considered implementing serde snapshots to avoid the presumed performance cost of replaying from tick 0.
**Decision:** Rejected snapshots. The MVP will use exactly one restoration mechanism: a lightweight Input Journal (`seed` + player choices/policies). Time travel is achieved by truncating the journal and re-simulating from tick 0.
**Rationale:** Having exactly one restoration mechanism drastically reduces complexity. The performance cost of re-simulating an ASCII run without rendering is presumed fast enough for an MVP, making the optimization of snapshots premature. Crucially, a journal provides a perfect test harness for our strict determinism contract and trivial, shareable debug replays.

## DR-006: Direct State Access over DTO Mapping
**Context:** Needed to prevent presentation logic (glyphs, colors) from bleeding into the `core` simulation engine. Initial plan used `ObservationRef` DTOs to encapsulate `core` state before passing it to `app`.
**Decision:** Removed the DTO layer. The `app` will directly read the internal `&GameState` data structures via an immutable reference.
**Rationale:** In a single-threaded runtime where `app` and `core` are cleanly separated by Cargo crates, the strict dependency direction (`app` -> `core`) naturally prevents UI from polluting the core. A 1:1 DTO mapping requires massive boilerplate whenever a generic simulation attribute evaluates (e.g. adding a new StatusEffect type). Direct immutable access maintains architectural purity without slowing feature velocity.

## DR-007: Segmented Trace over Continuous Auto-Explore Logging
**Context:** Need a way to explain auto-explore decisions to the player so deaths don't feel "unfair", but without spamming the UI with continuous pathing updates.
**Decision:** Auto-explore acts as a planner emitting chunky `AutoSegmentStarted` log events only when deciding on a new target or objective. The UI hides this trace by default but allows the player to inspect the recent planner history on demand via a dedicated panel.
**Rationale:** Fully black-boxing auto-explore makes policy tuning feel random. But logging every step creates unreadable noise. A segmented trace (logging the "intent" of the next N steps) satisfies the need for "why did it do that?" debuggability while remaining lightweight to implement and out of the player's way during normal play.

## DR-008: Merging Content and Core for Rapid MVP Development
**Context:** The initial plan proposed a distinct `content` crate to cleanly separate pure data schemas (Items, Perks) from the `core` simulation engine.
**Decision:** The `content` crate was dropped and its responsibilities merged into a `core::content` module. Data specifications are allowed to contain hardcoded engine logic directly rather than relying on a pure, data-driven enum schema.
**Rationale:** Maintaining a separate `content` crate forces the developer to build a rigid, generic data-driven architecture (e.g. `enum ItemEffect { DealDamage, ApplyStatus }`) to avoid circular Cargo dependencies. During an MVP, this is a time sink. By merging them, we accept the technical debt of mixing data definitions with engine logic. For example:
```rust
pub fn sword_of_healing() -> ItemDef {
    ItemDef {
        name: "Sword of Healing",
        on_hit: |game, target, attacker| {
            // Debt accepted: Direct engine mutation mixed into item definition!
            attacker.heal(5);
            target.take_damage(10);
        }
    }
}
```
This is a calculated risk. It borders on "OOP morass" where entities own their own logic, breaking strict data/logic separation. However, given the tiny MVP scope (~15 items, ~10 perks), the velocity gained by avoiding a complex data schema engine far outweighs the structural impurity. If the game succeeds and scaling to 200+ items is required, this debt must be paid down by extracting a pure data schema.

## DR-009: Constrained Auto-Consumption over Full AI Evaluation
**Context:** The `Policy` struct initially included `loadout_rule` (when to swap weapons) and `consume_hp_threshold` (when to automatically drink a potion).
**Decision:** Cut full evaluator AI and loadout swapping, but retain soft, rigid auto-consumption (`auto_heal_if_below_threshold: Option<u8>`). Auto-consumption is highly constrained: it triggers only once per encounter, requires a matching potion type, and never triggers during multi-enemy combats unless retreating.
**Rationale:** Building an AI logic block to safely evaluate when it is "worth" spending a turn to swap a weapon or drink a limited consumable is complex and brittle. However, completely removing auto-consumption reintroduces tactical micromanagement, violating the core "policy over micromanagement" vision. By allowing a single, deterministic heuristic for emergency potion usage, we preserve the policy fantasy, maintain low implementation complexity, and avoid forcing the player to manually drink potions every fight.
