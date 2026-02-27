# Specification: Frontier Planner Performance Pass (Milestone 6e)

## Overview
Optimize the auto-explore planner in the Roguelike MVP's `core` simulation by replacing expensive, per-candidate A* scans in `choose_frontier_intent` with two efficient, single-source searches (BFS/Dijkstra). This optimization reduces the complexity of finding the nearest exploration target from $O(Candidates \cdot A*)$ to $O(MapCells)$.

## Functional Requirements
1.  **Single-Source BFS/Dijkstra Implementation:**
    -   In `crates/core/src/game.rs`, replace the loop of per-candidate A* scans with two primary BFS/Dijkstra passes from the player's current position.
    -   **Pass 1:** Dijkstra over discovered walkable tiles while **avoiding known hazards**.
    -   **Pass 2:** Dijkstra over all discovered walkable tiles **allowing hazards** (used only if Pass 1 finds no reachable frontiers).
2.  **Deterministic Neighbor Expansion:**
    -   Maintain the strict deterministic neighbor expansion order: `Up, Right, Down, Left`.
3.  **Target Ranking & Tie-Breaking:**
    -   Preserve existing target ranking semantics:
        -   Lowest path length first.
        -   Tie-break by `(y, x)` coordinate ordering.
4.  **Preserve Auto-Explore Intent Reason logic:**
    -   Closed-door targets still yield `AutoReason::Door`.
    -   Hazard-fallback paths still yield `AutoReason::ThreatAvoidance`.
5.  **Maintain Pathing Execution:**
    -   The actual movement execution pathing (`path_for_intent`) should remain unchanged, ensuring that the character's step-by-step movement follows the chosen intent.

## Acceptance Criteria
-   **Behavioral Consistency:** Auto-explore behavior remains identical to the original A*-based implementation for all currently tested scenarios.
-   **Safe Frontier Preference:** The planner correctly chooses a safe frontier over a hazardous one when both are reachable.
-   **Hazard Fallback:** The planner correctly falls back to a hazardous path when no safe route to any frontier exists.
-   **Deterministic Stability:** Identical seeds and map layouts result in identical frontier selection and pathing.
-   **No Regressions:** All existing unit and integration tests pass without modification.
-   **Targeted Regression Tests:** New tests verify the BFS/Dijkstra implementation specifically for safe/hazard scenarios and deterministic tie-breaking.
