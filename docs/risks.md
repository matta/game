# Roguelike Project - Pivot Risk Register

Status: Active  
Scope: Durable progression pivot (`docs/vision.md` + `docs/plan.md`)

This document tracks the highest-risk failure modes for the current direction and how we will detect and respond to them.

---

## 1. Manual Loop Feels Like Busywork

**Risk**  
Manual play becomes repetitive key-pressing instead of meaningful tactical play.

**Early warning signs**
- Playtests report that turns feel "obvious" instead of interesting.
- Players hold `Wait` or repeat the same movement pattern in most rooms.
- Runs feel slower without feeling more strategic.

**Mitigation path**
- Milestone A: keep one-input-one-tick clarity and make `Rest` useful.
- Milestone B: remove modal enemy stops that break flow.
- Milestone E: add anti-stall pressure so repeated no-op play is not optimal.

---

## 2. Non-Modal Combat Readability Failure

**Risk**  
Removing enemy encounter popups hides important threat context, causing confusion.

**Early warning signs**
- Players miss first contact and take avoidable hits.
- Players cannot explain why a fight went bad.
- Threat overlays feel noisy or easy to ignore.

**Mitigation path**
- Milestone B: add clear HUD/log first-contact alerts.
- Keep threat summary visible in persistent UI.
- Add replay-stable tests for first-sighting agency at tick boundaries.

---

## 3. Auto-Explore Trust Regression

**Risk**  
Auto-explore, now a support feature, still takes actions that feel reckless.

**Early warning signs**
- Auto steps adjacent to known hostiles.
- Auto pathing enters avoidable high-risk tiles.
- Players stop using auto because it feels unsafe.

**Mitigation path**
- Milestone F: conservative safe-auto rules.
- Immediate stop conditions on danger/damage/high-value discovery.
- Keep optional explainability traces for debugging trust issues.

---

## 4. Forgiveness Window Overtuned or Undertuned

**Risk**  
Durability mechanics either remove tension or fail to protect run investment.

**Early warning signs**
- Overtuned: mistakes feel free, HP rarely matters.
- Undertuned: 2-3 minor errors still end runs quickly.
- Defensive tools feel random instead of predictable.

**Mitigation path**
- Milestone C: explicit Guard rules with deterministic tests.
- Keep HP as long-run failure channel.
- Tune Guard recovery with clear player-facing conditions.

---

## 5. Degenerate Defensive Play (Infinite Stalling)

**Risk**  
Door play and waiting dominate as the best strategy with little tradeoff.

**Early warning signs**
- Best play is waiting for perfect conditions every fight.
- Runs are safe but slow and low-interest.
- Tactical retreat and indefinite stalling look identical.

**Mitigation path**
- Milestone E: add readable anti-turtle pressure.
- Preserve real retreat tools while penalizing indefinite loops.
- Add bounded-run tests to catch stall exploits.

---

## 6. Progression Does Not Create Identity

**Risk**  
Level-ups and stats do not change play style enough to feel meaningful.

**Early warning signs**
- Mid-run characters feel similar to run start.
- Stat choices feel like minor number bumps.
- Players cannot describe build identity by early-mid run.

**Mitigation path**
- Milestone D: stat choices must map to defensive behavior changes.
- Add HUD cues showing derived defenses and identity direction.
- Add deterministic tests proving stat choices change outcomes.

---

## 7. Content Shape Conflicts With Pivot

**Risk**  
Existing items/enemies still reward burst or chaos over durable control.

**Early warning signs**
- Early floors produce sudden opaque deaths.
- Loot often feels off-theme for defensive progression.
- Recovery moments are rare compared to run-ending spikes.

**Mitigation path**
- Milestone G: retune item and enemy roles around stabilization.
- Keep rare run-changers, but reduce early volatility.
- Add curated seed playtests for uncertainty -> stabilization -> recovery arc.

---

## 8. Determinism Regressions During Loop Refactor

**Risk**  
Manual input and new durability systems accidentally break replay stability.

**Early warning signs**
- Same seed + same input journal produces drift.
- Flaky tests appear in movement/combat/replay paths.
- New systems depend on non-deterministic iteration.

**Mitigation path**
- Preserve current determinism invariants from `docs/plan.md`.
- Add milestone smoke tests for manual control and durability rules.
- Keep `make check` and `make test` as required gates.

---

## 9. Active Monitoring Cadence

Update this file when:
- A risk becomes real in playtests.
- A mitigation ships and risk level drops.
- New pivot-critical risk appears.

Retire risks only after the behavior is test-covered and stable across multiple curated seeds.
