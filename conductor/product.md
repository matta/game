# Initial Concept

A deterministic, ASCII-grid roguelike built in Rust using Macroquad, focusing on policy-based combat over manual micromovement.

# Product Definition: Roguelike MVP

## Vision
A deterministic, ASCII-grid roguelike designed for **Roguelike Veterans** who value deep buildcrafting and the tactical challenge of lethal-but-fair gameplay. The project's unique "automation twist" removes the manual micro-management of tile-by-tile movement, focusing instead on high-level combat policies and synergistic build construction.

## Core Pillars
- **Policy over Micro:** The player's influence is expressed through high-level policy knobs (stances, thresholds, targeting) rather than direct tactical movement.
- **Strict Determinism:** Every run is seed-based and perfectly reproducible. This guarantees fairness and enables a robust replay/debugging system.
- **Deep Buildcrafting:** The game centers on discovering and combining items, perks, and god-pacts that break standard rules and create asymmetric synergies.
- **Core Fun:** Above all, the automation must not feel like a "solved" game. Decisions must be impactful, and the player's strategy must directly result in either victory or defeat.

## Key Features (MVP)
- **Auto-Explore Planning:** An interrupt-driven simulation that handles pathing and basic exploration, halting only when the player's intervention is required.
- **Combat Policy Knobs:** A detailed control surface allowing the player to tune their character's behavior (e.g., target priority, stance, retreat thresholds).
- **Branching Dungeon:** A multi-floor dungeon with a strict one-way descent and meaningful branch choices that alter the run's characteristics.
- **Deterministic Replays:** The ability to reconstruct any run tick-by-tick from an input journal for verification and balance testing.
- **Crash-Recoverable Diagnostics:** Automatic persistence of run metadata (seed, hash, tick) enabling immediate run identification and seed-based recovery after a failure.

## Success Criteria
- **Replay Verification:** 100% stable replays for all MVP sessions across different builds and platforms.
- **Balanced 1.0 Release:** A focused vertical slice with ~15 items and ~10 perks that offers real tactical depth and replayability.
- **Playable Fun:** The game's automation and policy layers create a compelling, high-stakes experience that keeps players coming back.
