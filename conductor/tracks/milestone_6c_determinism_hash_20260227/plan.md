# Implementation Plan: Milestone 6c — Determinism Hash Surfacing + Crash-Recovery State File

## Phase 1: Standardize Snapshot Hash HUD Surfacing [checkpoint: 7a67f4d]
- [x] Task: Update `format_snapshot_hash` in `crates/app/src/main.rs` to ensure exact `0x` + 16 lowercase hex digits format (using `018x`). (71aeac8)
- [x] Task: Verify the HUD stats panel correctly displays the hash on every tick. (851c094)
- [x] Task: Conductor - User Manual Verification 'Standardize Snapshot Hash HUD Surfacing' (Protocol in workflow.md) (7a67f4d)

## Phase 2: RunState Diagnostics Module
- [ ] Task: Add `serde`, `serde_json`, and `directories` dependencies to `crates/app/Cargo.toml`.
- [ ] Task: Implement `RunStateFile` struct in a new module `crates/app/src/run_state_file.rs` with `serde` derivation and the specified schema.
- [ ] Task: Implement atomic write logic (using a `.tmp` file and `std::fs::rename`) for the `RunStateFile` within the standard OS user data directory.
- [ ] Task: Add unit tests in `crates/app/src/run_state_file.rs` for JSON round-trip and atomic write operations.
- [ ] Task: Conductor - User Manual Verification 'RunState Diagnostics Module' (Protocol in workflow.md)

## Phase 3: App Loop Integration & Resume Logic
- [ ] Task: Update `main.rs` startup logic to resolve the user data directory and check for an existing `last_run_state.json`.
- [ ] Task: If a state file is found, emit a recovery message to the Event Log on start (using a new `LogEvent` variant).
- [ ] Task: Implement `R` key resume logic in `main.rs`: if a recovered seed exists, restart the `Game` and reset `AppState` with that seed.
- [ ] Task: Update the main loop to persist the current game state to `last_run_state.json` after every `app_state.tick(...)` call.
- [ ] Task: Conductor - User Manual Verification 'App Loop Integration & Resume Logic' (Protocol in workflow.md)

## Phase 4: Final Verification & Determinism Check
- [ ] Task: Add a deterministic integration test in `crates/core/tests/determinism.rs` verifying that identical seeds and policy/inputs produce stable snapshot hashes across two separate runs.
- [ ] Task: Perform a full manual run to confirm:
    - [ ] Hash is correctly surfaced.
    - [ ] `last_run_state.json` is successfully created and updated in the user data directory.
    - [ ] 'R' key resume correctly restarts with the recovered seed.
    - [ ] Crash recovery hint appears correctly in the event log.
- [ ] Task: Conductor - User Manual Verification 'Final Verification & Determinism Check' (Protocol in workflow.md)
