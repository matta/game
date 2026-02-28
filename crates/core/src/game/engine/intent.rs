//! Auto-explore intent upkeep for each simulation step.

use super::*;

impl Game {
    pub fn plan_auto_intent(&mut self, player_pos: Pos) {
        let mut needs_replan = true;
        if let Some(intent) = self.state.auto_intent {
            if player_pos == intent.target {
                needs_replan = true;
            } else if is_intent_target_still_valid(&self.state.map, intent)
                && let Some(path) = path_for_intent(&self.state.map, player_pos, intent)
            {
                let new_len = path.len() as u16;
                if new_len != intent.path_len {
                    self.state.auto_intent =
                        Some(AutoExploreIntent { path_len: new_len, ..intent });
                }
                needs_replan = false;
            }
        }
        if needs_replan {
            let next_intent = choose_frontier_intent(&self.state.map, player_pos);
            let changed = self.state.auto_intent.map(|intent| intent.reason)
                != next_intent.map(|intent| intent.reason);
            if changed && let Some(intent) = next_intent {
                self.log.push(LogEvent::AutoReasonChanged {
                    reason: intent.reason,
                    target: intent.target,
                    path_len: intent.path_len,
                });
            }
            self.state.auto_intent = next_intent;
        }
    }
}
