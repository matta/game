use crate::types::ActorKind;

pub mod keys {
    pub const WEAPON_RUSTY_SWORD: &str = "weapon_rusty_sword";
    pub const WEAPON_IRON_MACE: &str = "weapon_iron_mace";
    pub const WEAPON_STEEL_LONGSWORD: &str = "weapon_steel_longsword";
    pub const WEAPON_PHASE_DAGGER: &str = "weapon_phase_dagger";
    pub const WEAPON_BLOOD_AXE: &str = "weapon_blood_axe";

    pub const CONSUMABLE_MINOR_HP_POT: &str = "consumable_minor_hp_pot";
    pub const CONSUMABLE_MAJOR_HP_POT: &str = "consumable_major_hp_pot";
    pub const CONSUMABLE_TELEPORT_RUNE: &str = "consumable_teleport_rune";
    pub const CONSUMABLE_FORTIFICATION_SCROLL: &str = "consumable_fortification_scroll";
    pub const CONSUMABLE_STASIS_HOURGLASS: &str = "consumable_stasis_hourglass";
    pub const CONSUMABLE_MAGNETIC_LURE: &str = "consumable_magnetic_lure";
    pub const CONSUMABLE_SMOKE_BOMB: &str = "consumable_smoke_bomb";
    pub const CONSUMABLE_SHRAPNEL_BOMB: &str = "consumable_shrapnel_bomb";
    pub const CONSUMABLE_HASTE_POTION: &str = "consumable_haste_potion";
    pub const CONSUMABLE_IRON_SKIN_POTION: &str = "consumable_iron_skin_potion";

    pub const PERK_TOUGHNESS: &str = "perk_toughness";
    pub const PERK_SWIFT: &str = "perk_swift";
    pub const PERK_BERSERKER_RHYTHM: &str = "perk_berserker_rhythm";
    pub const PERK_PACIFISTS_BOUNTY: &str = "perk_pacifists_bounty";
    pub const PERK_SNIPERS_EYE: &str = "perk_snipers_eye";
    pub const PERK_IRON_WILL: &str = "perk_iron_will";
    pub const PERK_BLOODLUST: &str = "perk_bloodlust";
    pub const PERK_SCOUT: &str = "perk_scout";
    pub const PERK_RECKLESS_STRIKE: &str = "perk_reckless_strike";
    pub const PERK_SHADOW_STEP: &str = "perk_shadow_step";

    pub const GOD_VEIL: &str = "god_veil";
    pub const GOD_FORGE: &str = "god_forge";
}

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

pub struct God {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub struct ContentPack {
    pub weapons: Vec<Weapon>,
    pub consumables: Vec<Consumable>,
    pub perks: Vec<Perk>,
    pub gods: Vec<God>,
}

impl ContentPack {
    pub fn build_default() -> Self {
        Self {
            weapons: vec![
                Weapon { id: keys::WEAPON_RUSTY_SWORD, name: "Rusty Sword", attack_bonus: 2 },
                Weapon { id: keys::WEAPON_IRON_MACE, name: "Iron Mace", attack_bonus: 4 },
                Weapon {
                    id: keys::WEAPON_STEEL_LONGSWORD,
                    name: "Steel Longsword",
                    attack_bonus: 6,
                },
                Weapon { id: keys::WEAPON_PHASE_DAGGER, name: "Phase Dagger", attack_bonus: 3 }, // Weird: ignores armor
                Weapon { id: keys::WEAPON_BLOOD_AXE, name: "Blood Axe", attack_bonus: 6 }, // Weird: lifesteal
            ],
            consumables: vec![
                Consumable {
                    id: keys::CONSUMABLE_MINOR_HP_POT,
                    name: "Minor Health Potion",
                    heal_amount: 10,
                },
                Consumable {
                    id: keys::CONSUMABLE_MAJOR_HP_POT,
                    name: "Major Health Potion",
                    heal_amount: 25,
                },
                Consumable {
                    id: keys::CONSUMABLE_TELEPORT_RUNE,
                    name: "Teleport Rune",
                    heal_amount: 0,
                }, // Weird: Swap with enemy
                Consumable {
                    id: keys::CONSUMABLE_FORTIFICATION_SCROLL,
                    name: "Fortification Scroll",
                    heal_amount: 0,
                }, // Weird: Wall spawn
                Consumable {
                    id: keys::CONSUMABLE_STASIS_HOURGLASS,
                    name: "Stasis Hourglass",
                    heal_amount: 0,
                }, // Weird: Freeze
                Consumable {
                    id: keys::CONSUMABLE_MAGNETIC_LURE,
                    name: "Magnetic Lure",
                    heal_amount: 0,
                }, // Weird: Pull
                Consumable { id: keys::CONSUMABLE_SMOKE_BOMB, name: "Smoke Bomb", heal_amount: 0 }, // Weird: Blind/Drop aggro
                Consumable {
                    id: keys::CONSUMABLE_SHRAPNEL_BOMB,
                    name: "Shrapnel Bomb",
                    heal_amount: 0,
                }, // Standard: AOE damage
                Consumable {
                    id: keys::CONSUMABLE_HASTE_POTION,
                    name: "Potion of Haste",
                    heal_amount: 0,
                }, // Standard: speed buff
                Consumable {
                    id: keys::CONSUMABLE_IRON_SKIN_POTION,
                    name: "Iron Skin Potion",
                    heal_amount: 0,
                }, // Standard: def buff
            ],
            perks: vec![
                Perk {
                    id: keys::PERK_TOUGHNESS,
                    name: "Toughness",
                    description: "Increases max HP by 5.",
                },
                Perk {
                    id: keys::PERK_SWIFT,
                    name: "Swift",
                    description: "Action tick cost reduced by 1.",
                },
                Perk {
                    id: keys::PERK_BERSERKER_RHYTHM,
                    name: "Berserker Rhythm",
                    description: "Attacks deal +3 damage if unarmed.",
                },
                Perk {
                    id: keys::PERK_PACIFISTS_BOUNTY,
                    name: "Pacifist's Bounty",
                    description: "Gain max HP and full heal when you descend with 0 kills.",
                },
                Perk {
                    id: keys::PERK_SNIPERS_EYE,
                    name: "Sniper's Eye",
                    description: "First strike on a floor deals +3 damage (Not Imp).",
                },
                Perk { id: keys::PERK_IRON_WILL, name: "Iron Will", description: "+2 Defense." },
                Perk {
                    id: keys::PERK_BLOODLUST,
                    name: "Bloodlust",
                    description: "Heal 2 HP on kill.",
                },
                Perk {
                    id: keys::PERK_SCOUT,
                    name: "Scout",
                    description: "Increases FOV radius by 2.",
                },
                Perk {
                    id: keys::PERK_RECKLESS_STRIKE,
                    name: "Reckless Strike",
                    description: "+4 Attack, -2 Defense.",
                },
                Perk {
                    id: keys::PERK_SHADOW_STEP,
                    name: "Shadow Step",
                    description: "Choosing 'Avoid' teleports you.",
                },
            ],
            gods: vec![
                God {
                    id: keys::GOD_VEIL,
                    name: "Veil",
                    description: "Avoid blinks you to the farthest nearby safe tile.",
                },
                God {
                    id: keys::GOD_FORGE,
                    name: "Forge",
                    description: "Gain +2 max HP, heal +2, and +2 passive defense.",
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
