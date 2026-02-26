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
