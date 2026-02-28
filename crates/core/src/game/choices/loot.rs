//! Loot-choice handlers.
//! This module applies keep/discard outcomes for prompted loot pickups.

use super::*;

impl Game {
    pub(super) fn resolve_keep_loot_choice(&mut self, item: ItemId) {
        let kind = self.state.items[item].kind;
        self.apply_item_effect(kind);
        self.state.items.remove(item);
        self.log.push(LogEvent::ItemPickedUp { kind });
    }

    pub(super) fn resolve_discard_loot_choice(&mut self, item: ItemId) {
        let kind = self.state.items[item].kind;
        self.state.items.remove(item);
        self.log.push(LogEvent::ItemDiscarded { kind });
    }
}
