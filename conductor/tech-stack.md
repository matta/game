# Technology Stack: Roguelike MVP

## Language
- **Rust:** The primary programming language for the entire project, chosen for its safety, performance, and memory management features.

## Frontend & Rendering
- **Macroquad:** A lightweight, easy-to-use Rust library for 2D graphics and input, perfect for the project's ASCII-grid visual style.

## Core Simulation & Logic
- **Crate-based Architecture:** The project is organized into three distinct crates to ensure a clean separation of concerns and deterministic simulation:
    - **core:** The pure, headless deterministic simulation engine.
    - **app:** The Macroquad-based frontend handling rendering, UI, and input translation.
    - **tools:** Headless utilities for balance testing and deterministic replay verification.

## Entity Management
- **slotmap:** Used for managing entities (actors, items) within the `core` crate, providing stable identifiers and efficient memory management without the overhead of a full ECS.

## Determinism & Randomness
- **rand_chacha (ChaCha8Rng):** The single, explicitly seeded RNG source for the simulation, ensuring identical runs across different platforms and sessions.
- **Deterministic Containers:** Use `Vec` with explicit sorting or `BTreeMap`/`BTreeSet` in simulation-critical paths to maintain a predictable iteration order.
- **Fixed-Point Arithmetic:** Rely on integer math for all gameplay-critical logic to avoid floating-point non-determinism.

## Serialization & Persistence
- **Input Journal (Custom):** A versioned, append-only log of player choices and policy updates, used to reconstruct the game state from tick 0 for save/load and replays.
- **No Full-State Serialization:** The project avoids the complexity of full-state snapshots (serde), relying instead on fast-forward simulation from the input journal.
