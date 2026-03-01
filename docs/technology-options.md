# Technology Options

Status: Active record  
Current default: Macroquad  
Last reviewed: 2026-03-01

This document is a durable record of technology choices we may use now or revisit later.

---

## 1. Purpose

- Keep a stable history of engine and UI options.
- Record why each option exists on the table.
- Make revisit decisions explicit instead of ad hoc.

---

## 2. Current Decision

We are currently building with **Macroquad**.

Reason for now:
- Fast iteration for a small deterministic ASCII roguelike.
- Low setup and low overhead.
- Fits current scope and team size.

---

## 3. Revisit Triggers

Re-evaluate the engine choice only when one or more of these happen:

- Macroquad blocks a required feature we cannot implement reasonably.
- UI/layout needs become significantly more complex than current app architecture.
- Platform targets change in a way that Macroquad support cannot meet.
- Team size/content scale grows enough that tooling/editor workflows become a bottleneck.

If a trigger is hit, update this file with date, evidence, and decision.

---

## 4. Option Set

### 4.1 Macroquad (Current)

- Link: <https://github.com/not-fl3/macroquad>
- Status: Active choice
- Why keep it:
  - Lightweight and direct for current loop.
  - Good fit for deterministic simulation + simple rendering shell.
- Main tradeoffs:
  - Fewer built-in higher-level systems than bigger engines.

### 4.2 Bevy

- Link: <https://bevyengine.org/>
- Status: Candidate for future review
- Why consider it:
  - Larger ecosystem and tooling.
  - Strong long-term growth path if project complexity rises.
- Main tradeoffs:
  - Bigger architecture shift and migration cost.
  - Higher upfront complexity for current scope.

### 4.3 Fyrox

- Link: <https://fyrox.rs/>
- Status: Candidate for future review
- Why consider it:
  - Full-featured engine option if project needs expand.
- Main tradeoffs:
  - Migration and workflow cost not justified for current MVP direction.

### 4.4 ggez (+ mooeye for UI)

- Links:
  - ggez: <https://github.com/ggez/ggez>
  - mooeye: <https://github.com/mooeye/mooeye>
- Status: Candidate for future review
- Why consider it:
  - Alternate lightweight path with explicit UI helper option.
- Main tradeoffs:
  - Migration effort with unclear upside versus current Macroquad path.

---

## 5. Review Log

Use this format for each formal review:

- Date:
- Trigger:
- Options reviewed:
- Decision:
- Why:
- Follow-up tasks:

No formal reviews recorded yet.
