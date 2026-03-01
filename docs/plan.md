# Roguelike Project — Active Plan (Pivot Integrated)

Author: Matt Armstrong  
Stack: Rust + Macroquad  
Target: Desktop (macOS + Linux)  
Run Length: 20–40 minutes  
Status: Active

This is now the single active implementation plan. It replaces the old MVP 1.0 milestone history and merges the intent of the former `docs/plan2.md`.

---

## 1. Current Baseline (Already Implemented)

The following is shipped and should not be re-planned as new work:

- Deterministic simulation in `core` and rendering/input in `app`.
- Seeded deterministic runs with stable tie-break rules and integer simulation logic.
- Append-only input journaling and replay for crash reproduction.
- Multi-floor, one-way descent with branch commitment.
- Auto-explore with FOV-aware pathing, hazard awareness, and prompt-based interrupts.
- Enemy encounters, policy editing, loadout swap action cost, and threat/death diagnostics.
- Procedural floor generation, vaults, and a content baseline (enemies, items, perks, gods).
- CI/check gates and broad test coverage across unit/integration/smoke/fuzz layers.

This baseline is valuable. Keep it stable while pivoting the game loop.

---

## 2. Pivot Goal

Align implementation with `docs/vision.md`:

- Default play is manual and continuous.
- Survival has a clear forgiveness window.
- Character growth compounds into defensive stability.
- Exploration and positioning matter every run.
- Auto-explore is support tooling, not the core identity.

---

## 3. Keep These Invariants

These remain mandatory unless they actively block fun:

- Determinism contract (`core` remains replay-stable).
- Journal-based crash repro/replay.
- Clear separation of simulation and presentation.
- One explicit RNG source and deterministic ordering rules.
- Quality gates stay green (`make check`, `make test`).

---

## 4. Active Roadmap

### Milestone 0 — Status HUD Foundation
Goal: make the HUD reflect the data already tracked by the simulation (`GameState::actors`) so players always know their durability and combat posture before the manual-control work begins.

- [x] Confirm that the player `Actor` (see `crates/core/src/state.rs`) exposes `hp`, `max_hp`, `attack`, `defense`, `active_weapon_slot`, `equipped_weapon`, `reserve_weapon`, and `speed`, and assign them to a coherent HUD row with matched color/spacing.
- [x] Show live `hp`/`max_hp` numbers with visual cues whenever they change, relying on the existing actor fields instead of derived heuristics.
- [x] Surface auxiliary trackers stored in `GameState` such as `active_perks`, `kills_this_floor`, and `policy` thresholds so their effects (e.g., `PERK_SCOUT` + FOV range) are obvious alongside the health info.
- [x] Document that the code currently has no `level`/XP field; keep a placeholder row or note so Milestone D can later populate it once character progression is implemented rather than inventing mock data now.
- [x] Add targeted smoke/visual checks that step through actor damage/heal flows to prove the HUD refreshes each metric in real time and update `docs/vision.md` or similar docs to describe where each tracked stat appears.
- [x] Ensure the stats panel never prints past its visible height; when space is tight, show an explicit overflow line so hidden entries are clear.
- [x] Enable high-DPI window rendering so UI text keeps expected physical size on Linux HiDPI displays.
- [x] Add a UI scale fallback path and on-screen scale diagnostics (`dpi` and `GAME_UI_SCALE`) so Linux scaling issues can be debugged and overridden.
- [x] Add in-app UI scale controls with fractional steps and persistent save/load so Linux users can tune readability without environment variables.

Exit criteria:
- [x] Players can glance at the HUD and immediately read their current and max health, combat stats, weapon status, and any perks/kills that impact their posture; the plan also records that level data is pending Milestone D so the HUD can grow into that slot later.

### Milestone A — Manual Control Baseline
Goal: make continuous manual play the default experience.

- [ ] Add new simulation inputs for direct control:
  - [ ] `Move { dir }` (4-way)
  - [ ] `Wait`
  - [ ] `Rest` (repeat wait until interrupted by new info or recovery condition)
- [ ] Update app loop so one manual input advances one simulation tick.
- [ ] Keep auto-explore as an explicit optional action.
- [ ] Auto-explore must stop immediately on:
  - [ ] Enemy entering FOV
  - [ ] Player taking damage
  - [ ] New high-value loot discovery
  - [ ] User stop request
- [ ] Journal/replay support for the new manual input payloads.
- [ ] Add manual-loop deterministic smoke test coverage.

Exit criteria:
- [ ] Player can clear and reposition through fights without modal dependence.
- [ ] Auto-explore is optional convenience, not required gameplay.

### Milestone B — Continuous Combat (No Modal Enemy Encounter Stop)
Goal: remove enemy-sighting modal flow as the default combat entry pattern.

- [ ] Replace `EnemyEncounter` modal interrupt with non-modal alerting:
  - [ ] Log event
  - [ ] HUD/overlay threat indicator
- [ ] Keep threat summary data, but render it in persistent UI rather than pause panel.
- [ ] Preserve fairness: first sighting must land on a tick boundary where player still has agency on next input.
- [ ] Add regression tests proving enemy sighting does not force a modal stop.

Exit criteria:
- [ ] Enemy contact keeps the player in continuous play.
- [ ] Player can act immediately after first contact.

### Milestone C — Forgiveness Window (Durability v1)
Goal: prevent runs from collapsing from a few minor mistakes.

Primary choice: **Guard meter**.

- [ ] Add player `guard` resource with clear deterministic rules.
- [ ] Incoming damage consumes Guard first, overflow to HP.
- [ ] Add deterministic Guard recovery rules (out-of-pressure regen and/or defensive action regen).
- [ ] Add UI for Guard value, cap, and recovery conditions.
- [ ] Add deterministic tests for:
  - [ ] Damage routing (Guard then HP)
  - [ ] Regen triggers
  - [ ] Replay stability

Exit criteria:
- [ ] Minor mistakes usually drain Guard, not the entire run.
- [ ] HP remains meaningful long-run danger.

### Milestone D — Character Development (XP + Level Choices)
Goal: create obvious run-to-run build identity and compounding stability.

- [ ] Add XP progression from combat/exploration milestones.
- [ ] Add level-up flow with one meaningful choice per level.
- [ ] First-pass stat set and defensive meaning:
  - [ ] STR: guard/armor leaning
  - [ ] DEX: evasion/initiative/retreat reliability leaning
  - [ ] WIS: regen/status-resist/resource-efficiency leaning
- [ ] Surface identity cues in HUD (stat totals and derived defenses).
- [ ] Deterministic tests for level-up and stat effects.

Exit criteria:
- [ ] By early-mid run, build identity is visible and gameplay-relevant.
- [ ] Leveling feels like commitment, not tiny stat drift.

### Milestone E — Anti-Turtle Balance Pass
Goal: keep defensive play strong without making infinite stalling optimal.

- [ ] Add 1–2 deterministic anti-stall pressures (choose simple, readable versions):
  - [ ] Noise/attention over time
  - [ ] Hunger/fatigue clock
  - [ ] Timed threat escalation
- [ ] Preserve fair retreat tools (doors, chokepoints, limited emergency options).
- [ ] Add tests that stall loops are bounded and forward play is more efficient.

Exit criteria:
- [ ] Defensive play is valid but not degenerate.
- [ ] Runs naturally move forward.

### Milestone F — Auto-Explore as Supportive Automation
Goal: keep convenience while preserving trust and readability.

- [ ] During auto-explore, render movement in readable steps (avoid hard "teleport" feel).
- [ ] Stop auto-explore immediately on danger, damage, or high-value discovery.
- [ ] Add conservative safe-auto rules:
  - [ ] Never step adjacent to known hostiles.
  - [ ] Avoid known high-risk tiles by rule.
- [ ] Keep explainability trace optional but available.

Exit criteria:
- [ ] Auto-explore feels safe and optional.
- [ ] Players can trust when and why it stops.

### Milestone G — Content Retune for Durable Progression
Goal: retune existing content to fit the new loop before adding large new content sets.

- [ ] Reclassify existing items/perks into:
  - [ ] Stabilizers
  - [ ] Identity formers
  - [ ] Rare run changers
- [ ] Reduce early-floor chaos spikes; keep rare transformations later.
- [ ] Retune enemies toward readable sustained pressure over sudden opaque spikes.
- [ ] Add curated thin-slice seed set for manual playtesting:
  - [ ] Early danger
  - [ ] Mid-run stabilization
  - [ ] Near-collapse recovery
- [ ] Carry over unresolved quality item from prior plan:
  - [ ] Add synergy test fixtures for weird item/perk combinations.

Exit criteria:
- [ ] Most drops reinforce or cleanly redirect build identity.
- [ ] At least one curated seed reliably demonstrates uncertainty -> stabilization -> recovery -> renewed momentum.

---

## 5. Updated Risk Register (Pivot)

Top active risks:

1. Tedium without escalation.
2. Degenerate defense loops.
3. Over-forgiveness (tension collapse).
4. Under-forgiveness (investment feels brittle).
5. Auto-explore trust regression.

Risk handling is tied to Milestones C, E, F, and G.

---

## 6. Test Strategy Updates

Keep existing determinism and replay tests. Add pivot-focused coverage:

- [ ] Manual control smoke: scripted moves/waits/fights produce stable outcomes.
- [ ] Durability invariants: Guard rules deterministic and replay-stable.
- [ ] Stall prevention: bounded-run tests for anti-turtle systems.
- [ ] Curated seed regression: fixed seeds reach expected pacing checkpoints.

Quality gates for each milestone increment:

- [ ] `make check`
- [ ] `make test`

---

## 7. Decision Record Updates Needed

Add or amend DRs during implementation:

- [ ] DR-010: Manual play is the default mode.
- [ ] DR-011: Enemy sightings are non-modal by default.
- [ ] DR-012: Forgiveness window mechanic (Guard-first) is required before content expansion.
- [ ] Mark old automation-first assumptions as superseded where they conflict with this pivot.

---

## 8. Next Slice (Execution Order)

Immediate order:

1. Milestone A (manual control baseline)
2. Milestone B (continuous combat presentation)
3. Milestone C (forgiveness window)

Do not start large content growth before A+B+C are playable and fun.

---

## 9. Working Rules

- Use TDD for new features and bug fixes.
- Keep language plain and names readable.
- Keep determinism guarantees intact while changing the play loop.
- Keep `docs/plan.md` as the source of truth and update checkboxes as tasks land.

---

## 10. Checklist Template (Use For Task Closures)

- [ ] Use @docs/plan.md for planning
- [ ] Keep @docs/plan.md checkmarks up to date.
- [ ] Use a TDD approach to implement new features, and fix bugs.
- [ ] Ensure `make check` passes with no warnings or errors.
- [ ] Ensure `make test` passes with no failures.
- [ ] Prefer "Plain English" and jargon-free explanations in documentation, comments, names in code, and commit messages. Exception: technical terms High School students would understand are fine, as are Computer Science terms a university student would understand.
- [ ] Avoid cryptic variable names; prefer words.
