# Implementation Plan: Frontier Planner Performance Pass (Milestone 6e)

This plan outlines the steps for optimizing the auto-explore planner in the `core` simulation crate by replacing expensive A* searches with efficient single-source BFS/Dijkstra passes.

## Phase 1: Preparation & Regression Baseline [checkpoint: c5ced57]

Ensure a stable baseline and robust regression tests are in place to verify the new planner's behavioral consistency.

- [x] Task: Create a comprehensive regression test suite in `crates/core/src/game.rs` that captures the current frontier selection behavior (intent, target, path length) across a variety of map scenarios: b9d6378
    - [ ] Open room with multiple frontiers.
    - [ ] Maze-like layouts requiring long paths.
    - [ ] Scenarios with and without known hazards.
    - [ ] Scenarios with closed doors.
- [x] Task: Verify that all baseline tests pass with the current A*-based implementation. b9d6378
- [x] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md) b9d6378

## Phase 2: Implement Single-Source BFS/Dijkstra Planner [checkpoint: 6392c18]

Replace the current loop of A* scans with the optimized single-source searches in `choose_frontier_intent`.

- [x] Task: Write failing unit tests for the new BFS/Dijkstra implementation using synthetic map fixtures in `crates/core/src/game.rs`: 2f7bdee
    - [x] Test 1: Single-source Dijkstra correctly identifies distances to all reachable discovered tiles.
    - [x] Test 2: Safe frontier (no-hazard path) is preferred over a shorter hazard-containing path.
    - [x] Test 3: Hazard fallback is correctly triggered when no safe path to any frontier exists.
- [x] Task: Implement the optimized `choose_frontier_intent` using two primary BFS/Dijkstra passes: 2f7bdee
    - [x] Implement Pass 1: Dijkstra over discovered walkable tiles avoiding hazards.
    - [x] Implement Pass 2: Dijkstra over all discovered walkable tiles allowing hazards (only if Pass 1 fails).
- [x] Task: Ensure deterministic neighbor expansion order (`Up, Right, Down, Left`) and target ranking (`lowest length`, then `y`, then `x`) are correctly applied in the new implementation. 2f7bdee
- [x] Task: Run the newly written unit tests and confirm that the BFS/Dijkstra planner passes all scenarios. 2f7bdee
- [x] Task: Conductor - User Manual Verification 'Phase 2' (Protocol in workflow.md) 2f7bdee

## Phase 3: Final Verification & Performance Validation

Confirm that the new optimized planner remains behaviorally consistent with the original implementation and correctly handles all edge cases.

- [ ] Task: Run the Phase 1 baseline regression tests with the new BFS/Dijkstra implementation and confirm that all test cases produce identical results to the original A*-based implementation.
- [ ] Task: Verify that `AutoReason::Door` and `AutoReason::ThreatAvoidance` logic is correctly preserved in all relevant scenarios.
- [ ] Task: Perform a final performance check to ensure the new planner is responsive and efficient across large maps.
- [ ] Task: Conductor - User Manual Verification 'Phase 3' (Protocol in workflow.md)
