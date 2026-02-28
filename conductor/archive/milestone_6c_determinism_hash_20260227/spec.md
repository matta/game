# Specification: Milestone 6c — Determinism Hash Surfacing + Crash-Recovery State File

## Overview
Implement deterministic snapshot hash surfacing and a crash-recovery state file to enable reliable debugging and run reconstruction.

## Functional Requirements
1.  **Standardized Snapshot Hash Surfacing:**
    -   Ensure the `snapshot_hash` is rendered in the HUD Stats panel.
    -   Format MUST be `0x` followed by 16 lowercase hex digits (e.g., `0x0123456789abcdef`).
2.  **Persistent Run Diagnostics:**
    -   State file path: Use standard OS User Data directory (e.g., `~/Library/Application Support/Roguelike/` on macOS).
    -   Filename: `last_run_state.json`.
    -   JSON Schema (Version 1):
        - `format_version`: Integer
        - `run_seed`: u64
        - `snapshot_hash_hex`: String
        - `tick`: u64
        - `floor_index`: u8
        - `branch_profile`: String (e.g., "BranchA", "BranchB", or "None")
        - `active_god`: String (e.g., "Veil", "Forge", or "None")
        - `updated_at_unix_ms`: u64
3.  **Persistence Lifecycle:**
    -   **Creation:** Write the file immediately after the `Game` is instantiated.
    -   **Update:** Update the file after every simulation tick (`app_state.tick(...)`).
    -   **Atomic Write:** Use a temporary file and atomic rename (or equivalent) to prevent corruption during a crash.
4.  **Startup Recovery Hint:**
    -   On application launch, check if a valid `last_run_state.json` exists.
    -   If found, emit a recovery message to the Event Log: `Recovered last run: seed=<seed> hash=<hash>`.
5.  **Quick Resume Action:**
    -   While in the game, the player can press the **'R'** key to restart the game using the seed found in the `last_run_state.json` (if any).
    -   Pressing 'R' should re-instantiate the `Game` with the recovered seed and reset `AppState`.

## Non-Functional Requirements
- **Determinism:** The recovery process must not introduce any non-deterministic state into the new run.
- **IO Performance:** Persistence should be efficient enough to not cause frame drops (though synchronous writes of small JSON are generally acceptable for desktop targets, consider if throttling is needed if performance regresses).

## Acceptance Criteria
- [ ] Snapshot hash is displayed in the HUD stats panel in the correct hex format.
- [ ] `last_run_state.json` is successfully created and updated in the standard OS data directory.
- [ ] The JSON schema matches the specification exactly.
- [ ] A recovery message appears in the event log on startup if a prior state file exists.
- [ ] Pressing 'R' correctly restarts the game with the seed from the last run state.
- [ ] Determinism tests confirm that re-running a seed from a recovered state file produces the exact same snapshot hashes.

## Out of Scope
- Full game state snapshots (ironman/checkpoint restore via journal is the chosen model).
- Automatic resumption without user input.
