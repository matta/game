# Roguelike Project — Interrogation Phase Results (MVP v1)

Author: Matt Armstrong  
Status: Frozen (Archival, Non-Normative)  
Scope: 1.0 MVP (20–40 minute runs)

Normative source for active implementation decisions: `plan.md`.

---

# 1. Core Identity

## 1.1 Flavor Targets (ADOM/DCSS Inspired)
Selected qualities:
- Lethal-but-fair gameplay
- Deep buildcrafting with meaningful synergies
- Weird/interesting items (not stat sticks)
- Gods/pacts reshape runs
- Branching dungeon/world structure
- Decisions matter more than grind
- Learn-the-system mastery depth

## 1.2 North-Star Fun
Primary driver: **Buildcrafting**

The game is about assembling synergistic systems, not manual tactical micro-play.

## 1.3 Great 30-Minute Session
- Descend into dungeon
- Near-death tension moments
- Discover rare build-defining item
- Synergize item with chosen god
- End session with strong forward momentum
- Feeling: “I now have a real shot at winning the full run.”

## 1.4 Failure Philosophy
Unacceptable:
- Sudden opaque deaths
- Random unavoidable outcomes

Desired death model:
- Primary: Mostly avoidable with foresight
- Secondary: Slow bleed attrition
- Rare spice: A bad call while weakened can cascade into death

Death should feel earned through inefficiency or misunderstanding, not randomness.

---

# 2. Core Loop Design

## 2.1 One-Sentence Loop
The player repeatedly hits auto-explore, encounters a tactical interrupt (item, enemy, branch, god, etc.), refines build or policy, sets a new goal, and continues until reaching the bottom.

## 2.2 Interrupt Types
- Loot found (keep/discard)
- Enemy encounter
- Multi-enemy encounter
- Environmental hazard
- Shrine / God interaction
- Branch choice
- Descend / Ascend
- NPC interaction
- Camp / Rest decision
- Build breakpoint (perk choice)
- Goal drift suggestion
- Retreat threshold triggered
- Boss encounter
- Time travel (future system hook)

## 2.3 Time Model
- Strict turn-based simulation
- Auto-explore advances invisible turns until interrupt
- Deterministic and replayable

---

# 3. Combat Model

## 3.1 Spatial Model
- Full grid simulation
- Auto-movement only (no manual tactical micromovement)

## 3.2 Combat Control Surface (Player Policy Knobs)
- Target priority ordering
- Consumable usage thresholds
- Retreat conditions
- Combat stance (aggressive/defensive/etc.)
- Positioning intent (close distance / kite / hold)
- Resource aggression (burn vs conserve)
- Pre-combat loadout swap
- Fight vs avoid toggle
- Exploration strategy (stealth vs thorough)

Combat is expressed through policy, not manual tile-by-tile input.

---

# 4. World Structure

## 4.1 Map Model
Hybrid:
- Central stacked dungeon spine
- Surface/overworld layer
- Optional side branches
- Order of exploration matters

## 4.2 Content Generation Model
- Procedural layouts
- Small number of authored vaults/events
- Author rules, not handcrafted levels

## 4.3 MVP Run Length
- 20–40 minutes

---

# 5. Build System (MVP Scope)

## 5.1 Primary Build Axes
- Equipment-centric
- Skill/perk-based

Gods and companions deferred to later depth layers.

## 5.2 Complexity Level (MVP)
- ~10–20 items
- ~10 perks

Small system with real synergy density.

## 5.3 Loot Model
- Keep/Discard only
- No inventory grid
- No weight management
- No stash

Future potential: auto-salvage rules layered on top.

## 5.4 Gods (MVP Depth)
- Light
- Passive modifiers only
- Influence build direction
- No piety economy yet

---

# 6. Platform & Technical Constraints

## 6.1 Target Platform
- Desktop (macOS + Linux first)

## 6.2 Language & Stack
- Rust
- Macroquad (lightweight rendering loop)
- ASCII / glyph grid
- No audio

## 6.3 Saving Model
- Ironman-only in MVP
- Append-only input journal persists accepted inputs during play
- Replay from journal is the only MVP recovery path
- Optional checkpoint/time-travel mode is deferred post-MVP

## 6.4 Determinism
Required:
- Seed-based runs
- Deterministic simulation
- Reproducible replays
- Single explicit RNG source

This is mandatory for debugging, balance, and fairness validation.

## 6.5 Open Source
- Project will be open source from inception.

---

# 7. Development Constraints

## 7.1 Time Budget
- 10 hours/week
- ~120 hours total for MVP

## 7.2 Development Philosophy
Balanced:
- Clean architecture
- Clear simulation boundaries
- No architecture astronautics
- MVP completion prioritized

---

# 8. Design Principles (Derived)

1. Policy over micromovement
2. Determinism over spectacle
3. Build identity emerges early
4. Resource efficiency determines survival
5. No opaque randomness
6. Systems > content volume
7. Clear roguelike identity with automation twist

---

# Status

Interrogation Phase: COMPLETE  
