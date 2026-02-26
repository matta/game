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
- Optional checkpoint-based easy mode
- Fully deterministic seed-based runs

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
The `core` crate uses Generational Arenas (`slotmap`) for **actor** storage (player + monsters). This avoids borrow checker conflicts and prevents use-after-free errors when actors are created and destroyed during the simulation, without resorting to complex ECS frameworks.

Map tiles and items-on-ground remain **dense arrays/structs** inside the map data structure. This keeps serialization simple and avoids unnecessary indirection for data that doesn't need generational identity.

---

# 3. Determinism Contract

Requirements to guarantee identical runs across macOS and Linux:
- Single RNG source: `rand_chacha::ChaCha8Rng` explicitly seeded at run start.
- No `thread_rng()` or system time access in `core`.
- Avoid floating-point nondeterminism (use integer math exclusively for logic/combat).
- All randomness injected explicitly.

Game struct sketch:

```rust
pub struct Game {
  pub seed: u64,
  pub tick: u64,
  pub rng: ChaCha8Rng,
  pub state: RunState,
  pub log: Vec<LogEvent>,
}
```

Core API:

```rust
impl Game {
  pub fn new(seed: u64, content: &ContentPack, mode: GameMode) -> Self;
  pub fn set_policy(&mut self, policy: Policy);
  pub fn step_until_interrupt(&mut self, max_steps: u32) -> Option<Interrupt>;
  pub fn apply_choice(&mut self, choice: Choice) -> Result<(), GameError>;
  pub fn is_finished(&self) -> Option<RunOutcome>;
  pub fn snapshot_hash(&self) -> u64;
}
```

## 3.1 Replay Specification

Replays record **seed + ordered choice log**, enabling deterministic replay of any run without Macroquad.

```rust
pub struct RunReplay {
  pub seed: u64,
  pub choices: Vec<ChoiceRecord>,
}

pub struct ChoiceRecord {
  pub prompt_id: ChoicePromptId, // validates correct choice at correct interrupt
  pub choice: Choice,
}
```

The `ChoicePromptId` is emitted by `core` with each `Interrupt`, allowing the replay runner to verify it is applying the right choice at the right time. This makes debugging and sharing runs trivial.

## 3.2 Core / App Communication Contract

- `app` provides **pure inputs**: `Choice`, `PolicyUpdate`.
- `core` returns **pure outputs**: `Interrupt`, `RenderState`, `LogEvent`.
- `app` never reads or writes `core` internals except through the public API.

The `app` crate controls the execution loop, deciding whether to auto-explore or wait for user input (e.g., when paused). The `core` only receives inputs that mutate the simulation state, remaining entirely ignorant of "Auto" versus "Manual" states.

This contract enables a headless `tools/replay_runner` that exercises `core` without any rendering dependency.

---

# 4. Core Simulation Design

## 4.1 Core Data Structures

- `EntityId` (managed via `slotmap`)
- `Actor` (player or monster)
- `Stats` (hp, atk, def, speed, limited resists)
- `Status` (poison, bleed, slow — minimal set)
- `Equipment` (small fixed slot set)
- `Perk`
- `God` (passive modifiers only)
- `Policy`

## 4.2 Policy Structure

```rust
pub struct Policy {
  pub fight_or_avoid: FightMode,
  pub stance: Stance,
  pub target_priority: Vec<TargetTag>,
  pub consume_hp_threshold: u8,
  pub retreat_hp_threshold: u8,
  pub position_intent: PositionIntent,
  pub resource_aggression: Aggro,
  pub exploration_mode: ExploreMode,
}
```

## 4.3 Spatial Systems
All spatial algorithms live strictly within `core`, with no dependency on external rendering libraries.

- **Pathing (Milestone 2a):** A* pathing on known-walkable tiles. Future-proofs the spatial architecture against weighted terrain.
- **FOV (Milestone 2b or later):** Field of View + frontier selection (unknown-adjacent tiles) added once the interrupt loop and keep/discard/fight/avoid flow are already fun. Defer unless exploration requires it sooner.

---

# 5. Interrupt Model

Interrupt types (MVP subset):

- LootFound
- EnemyEncounter
- BranchChoice
- GodOffer
- CampChoice
- PerkChoice
- BossEncounter
- CheckpointAvailable (easy mode)

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

---

# 8. Milestone Roadmap

## Milestone 0 — Workspace Setup (3–5 hrs)
- Create 4-crate workspace (`core`, `content`, `app`, `tools`).
- Add rustfmt + clippy.
- Basic CI (test + lint).
- README.
*Done when: `cargo test` passes cleanly.*

## Milestone 1 — CoreSim Skeleton & Initial UI (10–12 hrs)
- Set up `slotmap` (actors only) in `core` and `ChaCha8Rng`.
- Implement `RunState` and basic map structure (dense tile arrays).
- Build the minimal Macroquad `app` shell to render a simple grid, proving the core/app communication contract: app sends pure inputs, core returns pure outputs.
- Implement Player + 1 enemy, Turn engine, and a fake loot interrupt.
*Done when: identical logs for identical seeds, and it visually renders.*

## Milestone 2a — Basic Pathing & Interrupt Loop (10–12 hrs)
- Implement A* pathing on known-walkable tiles in `core` (to support future weighted terrain logic).
- Render ASCII map and display event log in `app`.
- Implement the `step_until_interrupt()` API for auto-explore.
- Implement keep/discard and fight vs avoid interrupt panels.
*Done when: 5-minute auto-exploring run is playable and pauses on interrupts.*

## Milestone 2b — FOV & Exploration Intelligence (6–8 hrs)
- Implement Field of View in `core`.
- Add frontier selection: pathing toward unknown-adjacent tiles.
- Handle edge cases: unknown tiles, doors, hazards, soft-danger avoidance.
*Done when: explore feels intentional — the player understands why the character moved where it did.*

## Milestone 3 — Combat + Policy (15–18 hrs)
- Multi-enemy encounters.
- Implement the full `Policy` struct (Target priority, Stance modifiers, Consumable thresholds, Retreat logic).
- Implement a micro-set of test content (2 weapons, 1 consumable, 2 perks) to validate policy behaviors.
- Wire UI to update policy knobs.
*Done when: automated combat resolves based on policy choices and builds feel distinct.*

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
- Threat summaries.
- Death cause stack.
- Seed display + copy.
- Determinism hash.
*Done when: deaths are reproducible and explainable to the player.*

## Milestone 7 — Saving Modes (8–10 hrs)
- Implement append-only `RunReplay` logging. Load games by fast-forwarding the replay payload.
- Checkpoint easy mode.
- Snapshot system.
*Done when: rewind works reliably.*

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
