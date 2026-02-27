# Product Guidelines: Roguelike MVP

## Prose Style
- **Plain English:** Use clear, simple language in all in-game text and documentation. Avoid jargon and technical terms that would be unfamiliar to a high school or university student unless they are essential to the game's mechanics.
- **Explain with Clarity:** When a rule or mechanic is described, it should be done in a way that is easy to understand, even if the underlying system is complex.
- **Consistent Tone:** Maintain a consistent, professional, yet approachable tone across all communications and player-facing content.

## Branding & Tone
- **Experimental & Weird:** Emphasize the unique, rule-breaking synergies and rule-bending items that define the Roguelike MVP. The world should feel slightly off-kilter, encouraging experimentation and unexpected combinations.
- **Asymmetric Power:** Highlight the potential for the player to become vastly overpowered through clever buildcrafting, while maintaining the lethal-but-fair stakes of the dungeon.
- **Aesthetic Focus:** The ASCII grid should be treated with care, using color and layout to create a distinct, modern-feeling "retro" aesthetic.

## UX & Interaction Principles
- **High Explainability:** The game must never leave the player wondering "why did that happen?". Every simulation halt, death, or major event should be accompanied by clear, deterministic feedback.
- **Minimalist & Functional:** Prioritize a clean, functional interface that provides the player with exactly the information they need to make policy decisions, without unnecessary clutter.
- **Interactive Feedback:** Ensure the UI is responsive to player changes. When a policy is adjusted, the player should see immediate, clear confirmation of the new state.
- **Policy over Micro:** The user interface should focus on the "big picture" strategy rather than manual tile-by-tile movement.

## Feedback Model
- **Transparent & Deterministic:** Never hide the "why" behind an outcome. If the player fails, they should be able to trace the failure back to a specific policy choice or a lack of resource efficiency.
- **No Opaque Randomness:** While procedural generation is a core feature, the results of the player's choices and the game's systems must always be predictable and reproducible.
- **Actionable Defeat:** A death should be a learning moment. The game should provide enough information for the player to refine their strategy for the next run.
