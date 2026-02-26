# Architectural and Design Risks

This document outlines the most significant ways in which the current technical implementation plan (`docs/plan.md`) diverges from or threatens the fundamental aesthetic and goals of the project vision (`docs/vision.md`).

## 1. Gameplay Loop & Player Trust

### The "Interrupt Fatigue" Risk
The core loop relies on an "Auto → Interrupt → Choose → Resume" flow. With a high density of interrupt triggers (loot, enemies, multi-enemies, hazards, perks, gods, checkpoints, retreat thresholds), there is a significant risk that the game devolves into modal-dialog whack-a-mole. This erodes the flow and momentum critical to the "30-minute session" fantasy. The plan lacks an explicit interrupt-frequency target to guard against this.

### The "Auto-Explore Trust" Risk
Because auto-explore and policy-driven combat are the core identity, player trust is paramount. If the pathing algorithm walks through avoidable hazards, walks into visible high-danger zones without pausing, or chooses a suboptimal frontier, players will blame the engine rather than their own policy. This fundamentally threatens the game's identity.

### The Undermining of "Policy Over Micromanagement"
**The Vision:** Section 3.2 defines the combat model entirely around policy knobs, explicitly including *"Consumable usage thresholds"* and *"Pre-combat loadout swap"*. The goal is to program an AI to execute tactical intent.
**The Plan:** DR-009 explicitly cuts auto-consumption of inventory and loadout rules, forcing the player to *"handle inventory usage manually during [...] interrupts."*
**The Risk:** By forcing manual potion drinking or weapon swapping every time an interrupt triggers, the plan reintroduces tactical micromanagement. It breaks the fantasy of being the "manager" of a build and shifts the burden back to discrete turn-based inputs, violating Vision Principle #1.

## 2. Combat & Survival Mechanics

### The Threat of "Sudden Opaque Deaths" via Spatial Naiveté
**The Vision:** Section 1.4 states that "Sudden opaque deaths" and "Random unavoidable outcomes" are unacceptable. Death should be avoidable with foresight.
**The Plan:** Section 4.6 enforces that multi-enemy encounters are "spatially naive" with restricted `PositionIntent` (Hold, Advance, Flee). DR-002 cuts tactical forecasting in favor of static threat facts.
**The Risk:** If auto-explore walks into a room with three ranged elites, the game pauses prior to a commitment. However, without complex positioning policies (like corner-peeking) and rigid auto-explore intent, the player might be doomed immediately. Without sophisticated escape policies or forecasting, death may feel like a random punishment for pressing auto-explore rather than a failure of policy.

### The "Retreat Illusion" Risk
The plan allows for a `FleeToNearestExploredTile` policy, but DR-003 explicitly cuts the overworld and ascending features in favor of a "strict one-way descent." Without multi-floor escapes or branch retreats, "fleeing" may simply mean stepping back three tiles and dying slightly slower. If a retreat does not materially change the state or offer a genuine escape vector, it becomes a placebo that undermines the fairness of the game.

## 3. Buildcrafting & Synergy

### Architectural Friction Against "Weird/Interesting Items"
**The Vision:** Section 1.1 demands "Weird/interesting items (not stat sticks)" with "deep buildcrafting with meaningful synergies."
**The Plan:** DR-008 decides to merge content and core logic, accepting the debt of hardcoding item effects directly into Rust functions rather than building an isolated data schema.
**The Risk:** Hardcoding item logic across engine callbacks often results in items bounded by immediate implementation ease (e.g., standard "deal X damage" stat sticks). Items that fundamentally alter game rules usually require a robust, decoupled event bus to interact synergistically. Hardcoding them risks creating a fragile web of conditions that limits true synergy.

### The "Synergy Testing Gap"
The plan heavily prioritizes determinism and engine-level testing, but lacks synergy density tests, build viability validation, or archetype diversity evaluation. With a constrained scope of only 15 items, each item must perform serious combinatorial work. If a significant fraction of those items are effectively independent and lack synergy, the game's buildcrafting graph collapses, violating the North-Star fun identified in the vision.

## 4. Architecture vs. Vision Prioritization

### Engine Architecture Eclipsing Buildcrafting
A massive percentage of the development roadmap is dedicated exclusively to perfect mathematical determinism, input journaling, tie-breaking logic, generating stable snapshot hashes, and headless test harnesses. With a strict 120-hour budget, the plan heavily indexes on "Architecture Astronautics" regarding determinism. This risks exhausting the time budget on a perfectly deterministic headless replay engine while starving the time needed to design the 15 items, 10 perks, and robust synergies that make the game fun.

### The "Checkpoint Emotional Dilution" Risk
The "Easy Mode" uses checkpoints with time travel to soften the game's difficulty. However, if checkpoints trigger too frequently (at every perk, every loot spike, or every branch), the game risks trivializing death tension. The vision’s failure philosophy depends heavily on near-death tension and earned consequences. To preserve the emotional stakes, the checkpoint cadence must be sparse and deeply meaningful.
