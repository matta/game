use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::state::{Actor, ContentPack, GameState, Map};
use crate::types::*;

pub struct Game {
    seed: u64,
    tick: u64,
    #[expect(dead_code)]
    rng: ChaCha8Rng,
    state: GameState,
    log: Vec<LogEvent>,
    next_input_seq: u64,
    pending_prompt: Option<ChoicePromptId>,
    pause_requested: bool,
    
    // For M1 testing
    fake_loot_thrown: bool,
}

impl Game {
    pub fn new(seed: u64, _content: &ContentPack, _mode: GameMode) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(seed);
        
        let mut actors = slotmap::SlotMap::with_key();
        
        let player = Actor {
            id: EntityId::default(), // Will be overwritten
            kind: ActorKind::Player,
            pos: Pos { y: 5, x: 5 },
            hp: 20,
            max_hp: 20,
            next_action_tick: 10,
            speed: 10,
        };
        let player_id = actors.insert(player);
        actors[player_id].id = player_id;
        
        let enemy = Actor {
            id: EntityId::default(),
            kind: ActorKind::Goblin,
            pos: Pos { y: 6, x: 7 },
            hp: 10,
            max_hp: 10,
            next_action_tick: 12,
            speed: 12,
        };
        let enemy_id = actors.insert(enemy);
        actors[enemy_id].id = enemy_id;

        Self {
            seed,
            tick: 0,
            rng,
            state: GameState {
                map: Map::new(20, 15),
                actors,
                items: slotmap::SlotMap::with_key(),
                player_id,
            },
            log: Vec::new(),
            next_input_seq: 0,
            pending_prompt: None,
            pause_requested: false,
            fake_loot_thrown: false,
        }
    }

    pub fn advance(&mut self, max_steps: u32) -> AdvanceResult {
        let mut steps = 0;
        
        if let Some(prompt_id) = self.pending_prompt {
           return AdvanceResult {
               simulated_ticks: 0,
               stop_reason: AdvanceStopReason::Interrupted(Interrupt::LootFound(prompt_id)),
           };
        }

        while steps < max_steps {
            if self.pause_requested {
                self.pause_requested = false;
                return AdvanceResult {
                    simulated_ticks: steps,
                    stop_reason: AdvanceStopReason::PausedAtBoundary { tick: self.tick },
                };
            }

            // Milestone 1 fake logic
            self.tick += 1;
            steps += 1;

            if self.tick.is_multiple_of(50) && !self.fake_loot_thrown {
                self.fake_loot_thrown = true;
                let prompt_id = ChoicePromptId(self.next_input_seq);
                self.pending_prompt = Some(prompt_id);
                return AdvanceResult {
                    simulated_ticks: steps,
                    stop_reason: AdvanceStopReason::Interrupted(Interrupt::LootFound(prompt_id)),
                };
            }
            
            // Advance the player arbitrarily to simulate auto explore
            if self.tick.is_multiple_of(10) {
                let mut p = self.state.actors[self.state.player_id].pos;
                if p.x < 15 {
                    p.x += 1;
                    self.state.actors[self.state.player_id].pos = p;
                }
            }
            
            if self.tick > 200 {
                return AdvanceResult {
                    simulated_ticks: steps,
                    stop_reason: AdvanceStopReason::Finished(RunOutcome::Victory),
                };
            }
        }

        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::BudgetExhausted,
        }
    }

    pub fn request_pause(&mut self) {
        self.pause_requested = true;
    }

    pub fn apply_choice(
        &mut self,
        prompt_id: ChoicePromptId,
        choice: Choice,
    ) -> Result<(), GameError> {
        if Some(prompt_id) != self.pending_prompt {
            return Err(GameError::PromptMismatch);
        }
        
        self.pending_prompt = None;
        self.next_input_seq += 1;
        
        if choice == Choice::KeepLoot {
            self.log.push(LogEvent::ItemPickedUp);
        }
        
        Ok(())
    }

    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn snapshot_hash(&self) -> u64 {
        use std::hash::Hasher;
        use xxhash_rust::xxh3::Xxh3;
        
        let mut hasher = Xxh3::new();
        hasher.write_u64(self.seed);
        hasher.write_u64(self.tick);
        hasher.write_u64(self.next_input_seq);
        
        // Hash canonical basic state
        let player = &self.state.actors[self.state.player_id];
        hasher.write_i32(player.pos.x);
        hasher.write_i32(player.pos.y);
        
        hasher.finish()
    }
}
