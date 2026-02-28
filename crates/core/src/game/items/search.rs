//! Enemy search primitives used by item effects.

use std::cmp::Ordering;

use super::*;

impl Game {
    pub(super) fn visible_enemy_ids_sorted(&self, distance_from: Option<Pos>) -> Vec<EntityId> {
        let mut ids: Vec<EntityId> = self
            .state
            .actors
            .iter()
            .filter(|(id, actor)| {
                *id != self.state.player_id && self.state.map.is_visible(actor.pos)
            })
            .map(|(id, _)| id)
            .collect();
        ids.sort_by(|a_id, b_id| {
            let a = &self.state.actors[*a_id];
            let b = &self.state.actors[*b_id];

            let distance_cmp = distance_from.map_or(Ordering::Equal, |origin| {
                manhattan(origin, a.pos).cmp(&manhattan(origin, b.pos))
            });
            if distance_cmp != Ordering::Equal {
                return distance_cmp;
            }
            let y_cmp = a.pos.y.cmp(&b.pos.y);
            if y_cmp != Ordering::Equal {
                return y_cmp;
            }
            let x_cmp = a.pos.x.cmp(&b.pos.x);
            if x_cmp != Ordering::Equal {
                return x_cmp;
            }
            a.kind
                .cmp(&b.kind)
                .then(a.hp.cmp(&b.hp))
                .then(a.next_action_tick.cmp(&b.next_action_tick))
        });
        ids
    }
}
