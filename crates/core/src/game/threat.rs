//! Threat-tag classification helpers for encounter summaries.
//! This module exists to keep enemy danger taxonomy centralized and deterministic.
//! It does not own prompt lifecycle or combat resolution flow.

use crate::types::{ActorKind, DangerTag};

pub(super) fn danger_tags_for_kind(kind: ActorKind) -> Vec<DangerTag> {
    match kind {
        ActorKind::Player => vec![],
        ActorKind::Goblin => vec![DangerTag::Melee],
        ActorKind::FeralHound => vec![DangerTag::Melee, DangerTag::Burst],
        ActorKind::BloodAcolyte => vec![DangerTag::Melee, DangerTag::Poison],
        ActorKind::CorruptedGuard => vec![DangerTag::Melee],
        ActorKind::LivingArmor => vec![DangerTag::Melee],
        ActorKind::Gargoyle => vec![DangerTag::Melee],
        ActorKind::ShadowStalker => vec![DangerTag::Melee, DangerTag::Burst],
        ActorKind::AbyssalWarden => vec![DangerTag::Melee, DangerTag::Burst],
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::content::ContentPack;
    use crate::game::test_support::*;
    use crate::*;

    #[test]
    fn danger_tags_for_each_kind_are_deterministic_and_sorted() {
        let kinds = [
            ActorKind::Goblin,
            ActorKind::FeralHound,
            ActorKind::BloodAcolyte,
            ActorKind::CorruptedGuard,
            ActorKind::LivingArmor,
            ActorKind::Gargoyle,
            ActorKind::ShadowStalker,
            ActorKind::AbyssalWarden,
        ];
        for kind in kinds {
            let tags = danger_tags_for_kind(kind);
            assert!(!tags.is_empty(), "{kind:?} should have at least one danger tag");
            let mut sorted = tags.clone();
            sorted.sort();
            assert_eq!(tags, sorted, "{kind:?} tags should be pre-sorted");
        }
        // Player should have no danger tags
        assert!(danger_tags_for_kind(ActorKind::Player).is_empty());
    }

    #[test]
    fn encounter_interrupt_populates_static_threat_facts() {
        let mut game = Game::new(12345, &ContentPack::default(), GameMode::Ironman);
        // Run until an enemy encounter
        for _ in 0..250 {
            match game.advance(1).stop_reason {
                AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter {
                    threat,
                    enemies,
                    ..
                }) => {
                    assert!(threat.visible_enemy_count > 0);
                    assert!(threat.nearest_enemy_distance.is_some());
                    assert_ne!(threat.primary_enemy_kind, ActorKind::Player);
                    assert!(!threat.danger_tags.is_empty());
                    // Verify tags are sorted and deduped
                    let mut sorted_tags = threat.danger_tags.clone();
                    sorted_tags.sort();
                    sorted_tags.dedup();
                    assert_eq!(threat.danger_tags, sorted_tags);
                    // Enemy count should be >= encounter list size
                    assert!(threat.visible_enemy_count >= enemies.len());
                    return;
                }
                AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::KeepLoot).unwrap();
                }
                AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => {
                    game.apply_choice(prompt_id, Choice::OpenDoor).unwrap();
                }
                AdvanceStopReason::Interrupted(Interrupt::FloorTransition {
                    prompt_id,
                    requires_branch_god_choice,
                    ..
                }) => {
                    let choice = if requires_branch_god_choice {
                        Choice::DescendBranchAVeil
                    } else {
                        Choice::Descend
                    };
                    game.apply_choice(prompt_id, choice).unwrap();
                }
                AdvanceStopReason::Finished(_) | AdvanceStopReason::EngineFailure(_) => break,
                _ => {}
            }
        }
        panic!("did not encounter an enemy within 250 ticks");
    }
}
