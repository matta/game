use crate::types::ActorKind;

pub struct EnemyStats {
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: u32,
}

pub fn get_enemy_stats(kind: ActorKind) -> EnemyStats {
    match kind {
        ActorKind::Goblin => EnemyStats { hp: 10, attack: 2, defense: 0, speed: 12 },
        ActorKind::FeralHound => EnemyStats { hp: 6, attack: 3, defense: 0, speed: 15 },
        ActorKind::BloodAcolyte => EnemyStats { hp: 12, attack: 5, defense: 0, speed: 10 },
        ActorKind::CorruptedGuard => EnemyStats { hp: 18, attack: 4, defense: 2, speed: 9 },
        ActorKind::LivingArmor => EnemyStats { hp: 25, attack: 3, defense: 4, speed: 5 },
        ActorKind::Gargoyle => EnemyStats { hp: 20, attack: 4, defense: 3, speed: 8 },
        ActorKind::ShadowStalker => EnemyStats { hp: 14, attack: 4, defense: 1, speed: 12 },
        ActorKind::AbyssalWarden => EnemyStats { hp: 80, attack: 8, defense: 3, speed: 9 },
        ActorKind::Player => EnemyStats { hp: 20, attack: 5, defense: 0, speed: 10 },
    }
}

pub struct Weapon {
    pub id: &'static str,
    pub name: &'static str,
    pub attack_bonus: i32,
}

pub struct Consumable {
    pub id: &'static str,
    pub name: &'static str,
    pub heal_amount: i32,
}

pub struct Perk {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub struct ContentPack {
    pub weapons: Vec<Weapon>,
    pub consumables: Vec<Consumable>,
    pub perks: Vec<Perk>,
}

impl ContentPack {
    pub fn build_default() -> Self {
        Self {
            weapons: vec![
                Weapon { id: "w_rusty_sword", name: "Rusty Sword", attack_bonus: 2 },
                Weapon { id: "w_iron_mace", name: "Iron Mace", attack_bonus: 4 },
            ],
            consumables: vec![Consumable {
                id: "c_minor_hp_pot",
                name: "Minor Health Potion",
                heal_amount: 10,
            }],
            perks: vec![
                Perk {
                    id: "p_toughness",
                    name: "Toughness",
                    description: "Increases max HP by 5.",
                },
                Perk {
                    id: "p_swift",
                    name: "Swift",
                    description: "Action tick cost reduced by 1.",
                },
            ],
        }
    }
}

impl Default for ContentPack {
    fn default() -> Self {
        Self::build_default()
    }
}
