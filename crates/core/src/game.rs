use std::collections::{BTreeMap, BTreeSet};

use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;

use crate::state::{Actor, ContentPack, GameState, Item, Map};
use crate::types::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingPromptKind {
    Loot { item: ItemId },
    EnemyEncounter { enemy: EntityId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingPrompt {
    id: ChoicePromptId,
    kind: PendingPromptKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OpenNode {
    f: u32,
    h: u32,
    y: i32,
    x: i32,
}

pub struct Game {
    seed: u64,
    tick: u64,
    #[expect(dead_code)]
    rng: ChaCha8Rng,
    state: GameState,
    log: Vec<LogEvent>,
    next_input_seq: u64,
    pending_prompt: Option<PendingPrompt>,
    // Enemy temporarily ignored after an Avoid choice to prevent immediate re-trigger loops.
    suppressed_enemy: Option<EntityId>,
    pause_requested: bool,
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
            pos: Pos { y: 5, x: 10 },
            hp: 10,
            max_hp: 10,
            next_action_tick: 12,
            speed: 12,
        };
        let enemy_id = actors.insert(enemy);
        actors[enemy_id].id = enemy_id;

        let mut map = Map::new(20, 15);
        map.set_tile(Pos { y: 5, x: 8 }, TileKind::Wall);
        map.set_tile(Pos { y: 6, x: 8 }, TileKind::Wall);

        let mut items = slotmap::SlotMap::with_key();
        let item = Item { id: ItemId::default(), pos: Pos { y: 5, x: 5 } };
        let item_id = items.insert(item);
        items[item_id].id = item_id;

        reveal_radius(&mut map, actors[player_id].pos, 3);

        Self {
            seed,
            tick: 0,
            rng,
            state: GameState { map, actors, items, player_id, auto_intent: None },
            log: Vec::new(),
            next_input_seq: 0,
            pending_prompt: None,
            suppressed_enemy: None,
            pause_requested: false,
        }
    }

    pub fn advance(&mut self, max_steps: u32) -> AdvanceResult {
        let mut steps = 0;

        if let Some(prompt) = self.pending_prompt {
            return AdvanceResult {
                simulated_ticks: 0,
                stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
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

            let player_pos = self.state.actors[self.state.player_id].pos;
            self.clear_suppressed_enemy_if_separated(player_pos);

            if let Some((enemy_id, _)) = self.find_adjacent_enemy(player_pos) {
                self.log.push(LogEvent::EnemyEncountered { enemy: enemy_id });
                return self.interrupt_enemy(enemy_id, steps);
            }

            if let Some(item_id) = self.find_item_at(player_pos) {
                return self.interrupt_loot(item_id, steps);
            }

            self.plan_auto_intent(player_pos);

            if let Some(intent) = self.state.auto_intent
                && intent.path_len > 0
                && let Some(path) = astar_path(&self.state.map, player_pos, intent.target)
                && let Some(next_step) = path.first().copied()
            {
                self.state.actors[self.state.player_id].pos = next_step;
                reveal_radius(&mut self.state.map, next_step, 3);
            }

            self.tick += 1;
            steps += 1;

            if self.tick > 400 {
                return AdvanceResult {
                    simulated_ticks: steps,
                    stop_reason: AdvanceStopReason::Finished(RunOutcome::Victory),
                };
            }
        }

        AdvanceResult { simulated_ticks: steps, stop_reason: AdvanceStopReason::BudgetExhausted }
    }

    pub fn request_pause(&mut self) {
        self.pause_requested = true;
    }

    pub fn apply_choice(
        &mut self,
        prompt_id: ChoicePromptId,
        choice: Choice,
    ) -> Result<(), GameError> {
        let Some(prompt) = self.pending_prompt else {
            return Err(GameError::PromptMismatch);
        };

        if prompt.id != prompt_id {
            return Err(GameError::PromptMismatch);
        }

        let handled = match (prompt.kind, choice) {
            (PendingPromptKind::Loot { item }, Choice::KeepLoot) => {
                self.state.items.remove(item);
                self.log.push(LogEvent::ItemPickedUp);
                true
            }
            (PendingPromptKind::Loot { item }, Choice::DiscardLoot) => {
                self.state.items.remove(item);
                self.log.push(LogEvent::ItemDiscarded);
                true
            }
            (PendingPromptKind::EnemyEncounter { enemy }, Choice::Fight) => {
                self.state.actors.remove(enemy);
                if self.suppressed_enemy == Some(enemy) {
                    self.suppressed_enemy = None;
                }
                self.log.push(LogEvent::EncounterResolved { enemy, fought: true });
                true
            }
            (PendingPromptKind::EnemyEncounter { enemy }, Choice::Avoid) => {
                self.suppressed_enemy = Some(enemy);
                self.try_step_away_from_enemy(enemy);
                self.log.push(LogEvent::EncounterResolved { enemy, fought: false });
                true
            }
            _ => false,
        };

        if !handled {
            return Err(GameError::InvalidChoice);
        }

        self.pending_prompt = None;
        self.next_input_seq += 1;
        Ok(())
    }

    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn log(&self) -> &[LogEvent] {
        &self.log
    }

    pub fn snapshot_hash(&self) -> u64 {
        use std::hash::Hasher;
        use xxhash_rust::xxh3::Xxh3;

        let mut hasher = Xxh3::new();
        hasher.write_u64(self.seed);
        hasher.write_u64(self.tick);
        hasher.write_u64(self.next_input_seq);

        // Hash canonical basic state.
        let player = &self.state.actors[self.state.player_id];
        hasher.write_i32(player.pos.x);
        hasher.write_i32(player.pos.y);

        if let Some(intent) = self.state.auto_intent {
            hasher.write_i32(intent.target.x);
            hasher.write_i32(intent.target.y);
            hasher.write_u16(intent.path_len);
            hasher.write_u8(intent.reason as u8);
        }

        hasher.finish()
    }

    fn plan_auto_intent(&mut self, player_pos: Pos) {
        let next_intent = choose_frontier_intent(&self.state.map, player_pos);
        let reason_or_target_changed =
            !auto_reason_and_target_equal(self.state.auto_intent, next_intent);
        if reason_or_target_changed && let Some(intent) = next_intent {
            self.log.push(LogEvent::AutoReasonChanged {
                reason: intent.reason,
                target: intent.target,
                path_len: intent.path_len,
            });
        }
        self.state.auto_intent = next_intent;
    }

    fn next_prompt_id(&self) -> ChoicePromptId {
        ChoicePromptId(self.next_input_seq)
    }

    fn interrupt_loot(&mut self, item: ItemId, simulated_ticks: u32) -> AdvanceResult {
        let prompt =
            PendingPrompt { id: self.next_prompt_id(), kind: PendingPromptKind::Loot { item } };
        self.pending_prompt = Some(prompt);
        AdvanceResult {
            simulated_ticks,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }

    fn interrupt_enemy(&mut self, enemy: EntityId, simulated_ticks: u32) -> AdvanceResult {
        let prompt = PendingPrompt {
            id: self.next_prompt_id(),
            kind: PendingPromptKind::EnemyEncounter { enemy },
        };
        self.pending_prompt = Some(prompt);
        AdvanceResult {
            simulated_ticks,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }

    fn prompt_to_interrupt(&self, prompt: PendingPrompt) -> Interrupt {
        match prompt.kind {
            PendingPromptKind::Loot { item } => Interrupt::LootFound { prompt_id: prompt.id, item },
            PendingPromptKind::EnemyEncounter { enemy } => {
                Interrupt::EnemyEncounter { prompt_id: prompt.id, enemy }
            }
        }
    }

    fn find_item_at(&self, pos: Pos) -> Option<ItemId> {
        self.state.items.iter().find(|(_, item)| item.pos == pos).map(|(id, _)| id)
    }

    fn find_adjacent_enemy(&self, player_pos: Pos) -> Option<(EntityId, &Actor)> {
        self.state
            .actors
            .iter()
            .filter(|(id, actor)| {
                *id != self.state.player_id
                    && actor.kind != ActorKind::Player
                    && Some(*id) != self.suppressed_enemy
            })
            .find(|(_, actor)| manhattan(player_pos, actor.pos) == 1)
    }

    fn clear_suppressed_enemy_if_separated(&mut self, player_pos: Pos) {
        let Some(enemy_id) = self.suppressed_enemy else {
            return;
        };

        let Some(enemy) = self.state.actors.get(enemy_id) else {
            self.suppressed_enemy = None;
            return;
        };

        if manhattan(player_pos, enemy.pos) > 1 {
            // Clear suppression once distance is re-established so normal encounters can resume.
            self.suppressed_enemy = None;
        }
    }

    fn try_step_away_from_enemy(&mut self, enemy_id: EntityId) {
        let Some(enemy_pos) = self.state.actors.get(enemy_id).map(|actor| actor.pos) else {
            self.suppressed_enemy = None;
            return;
        };

        let player_pos = self.state.actors[self.state.player_id].pos;
        let current_distance = manhattan(player_pos, enemy_pos);

        let mut best_step: Option<(Pos, u32)> = None;
        for candidate in neighbors(player_pos) {
            if !self.state.map.is_discovered_walkable(candidate)
                || self.is_occupied_by_actor(candidate)
            {
                continue;
            }

            let candidate_distance = manhattan(candidate, enemy_pos);
            let better = match best_step {
                None => true,
                Some((_, best_distance)) => candidate_distance > best_distance,
            };
            if better {
                best_step = Some((candidate, candidate_distance));
            }
        }

        if let Some((next_pos, next_distance)) = best_step
            && next_distance >= current_distance
        {
            // Resolve Avoid by stepping away when possible, instead of pausing in place.
            self.state.actors[self.state.player_id].pos = next_pos;
            reveal_radius(&mut self.state.map, next_pos, 3);
        }
    }

    fn is_occupied_by_actor(&self, pos: Pos) -> bool {
        self.state.actors.iter().any(|(id, actor)| id != self.state.player_id && actor.pos == pos)
    }
}

fn auto_reason_and_target_equal(
    left: Option<AutoExploreIntent>,
    right: Option<AutoExploreIntent>,
) -> bool {
    match (left, right) {
        (Some(lhs), Some(rhs)) => lhs.reason == rhs.reason && lhs.target == rhs.target,
        (None, None) => true,
        _ => false,
    }
}

fn choose_frontier_intent(map: &Map, start: Pos) -> Option<AutoExploreIntent> {
    let mut best: Option<(Pos, usize)> = None;

    for y in 0..map.internal_height {
        for x in 0..map.internal_width {
            let pos = Pos { y: y as i32, x: x as i32 };
            if pos == start {
                continue;
            }
            if !is_frontier_candidate(map, pos) {
                continue;
            }

            if let Some(path) = astar_path(map, start, pos) {
                let len = path.len();
                let better = match best {
                    None => true,
                    Some((best_pos, best_len)) => {
                        len < best_len
                            || (len == best_len && (pos.y, pos.x) < (best_pos.y, best_pos.x))
                    }
                };

                if better {
                    best = Some((pos, len));
                }
            }
        }
    }

    best.map(|(target, path_len)| AutoExploreIntent {
        target,
        reason: AutoReason::Frontier,
        path_len: path_len as u16,
    })
}

fn is_frontier_candidate(map: &Map, pos: Pos) -> bool {
    map.is_discovered_walkable(pos)
        && neighbors(pos).iter().any(|n| map.in_bounds(*n) && !map.is_discovered(*n))
}

fn astar_path(map: &Map, start: Pos, goal: Pos) -> Option<Vec<Pos>> {
    if !map.is_discovered_walkable(start) || !map.is_discovered_walkable(goal) {
        return None;
    }

    if start == goal {
        return Some(Vec::new());
    }

    let mut open_set = BTreeSet::new();
    let mut open_entries: BTreeMap<Pos, OpenNode> = BTreeMap::new();
    let mut came_from: BTreeMap<Pos, Pos> = BTreeMap::new();
    let mut g_score: BTreeMap<Pos, u32> = BTreeMap::new();

    let start_h = manhattan(start, goal);
    let start_node = OpenNode { f: start_h, h: start_h, y: start.y, x: start.x };
    open_set.insert(start_node);
    open_entries.insert(start, start_node);
    g_score.insert(start, 0);

    while let Some(current_node) = open_set.pop_first() {
        let current = Pos { y: current_node.y, x: current_node.x };
        open_entries.remove(&current);

        if current == goal {
            return Some(reconstruct_path(&came_from, start, goal));
        }

        let current_g = *g_score.get(&current).unwrap_or(&u32::MAX);
        if current_g == u32::MAX {
            continue;
        }

        for neighbor in neighbors(current) {
            if !map.is_discovered_walkable(neighbor) {
                continue;
            }

            let tentative_g = current_g.saturating_add(1);
            let existing_g = g_score.get(&neighbor).copied().unwrap_or(u32::MAX);
            if tentative_g >= existing_g {
                continue;
            }

            if let Some(existing_node) = open_entries.remove(&neighbor) {
                open_set.remove(&existing_node);
            }

            came_from.insert(neighbor, current);
            g_score.insert(neighbor, tentative_g);

            let h = manhattan(neighbor, goal);
            let f = tentative_g.saturating_add(h);
            let node = OpenNode { f, h, y: neighbor.y, x: neighbor.x };
            open_set.insert(node);
            open_entries.insert(neighbor, node);
        }
    }

    None
}

fn reconstruct_path(came_from: &BTreeMap<Pos, Pos>, start: Pos, goal: Pos) -> Vec<Pos> {
    let mut path = vec![goal];
    let mut current = goal;

    while current != start {
        let Some(prev) = came_from.get(&current).copied() else {
            return Vec::new();
        };
        current = prev;
        path.push(current);
    }

    path.reverse();
    path.remove(0);
    path
}

fn neighbors(pos: Pos) -> [Pos; 4] {
    [
        Pos { y: pos.y - 1, x: pos.x },
        Pos { y: pos.y, x: pos.x + 1 },
        Pos { y: pos.y + 1, x: pos.x },
        Pos { y: pos.y, x: pos.x - 1 },
    ]
}

fn reveal_radius(map: &mut Map, center: Pos, radius: i32) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let p = Pos { y: center.y + dy, x: center.x + dx };
            if map.in_bounds(p) {
                map.reveal(p);
            }
        }
    }
}

fn manhattan(a: Pos, b: Pos) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovered_floor_map(width: usize, height: usize) -> Map {
        let mut map = Map::new(width, height);
        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                map.reveal(Pos { y: y as i32, x: x as i32 });
            }
        }
        map
    }

    #[test]
    fn astar_straight_line_path_has_expected_length() {
        let map = discovered_floor_map(7, 7);
        let path = astar_path(&map, Pos { y: 3, x: 2 }, Pos { y: 3, x: 5 }).expect("path");
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], Pos { y: 3, x: 3 });
        assert_eq!(path[2], Pos { y: 3, x: 5 });
    }

    #[test]
    fn astar_tie_break_uses_deterministic_order() {
        let mut map = discovered_floor_map(7, 7);
        map.set_tile(Pos { y: 3, x: 3 }, TileKind::Wall);
        let path = astar_path(&map, Pos { y: 3, x: 2 }, Pos { y: 3, x: 4 }).expect("path");
        assert_eq!(path[0], Pos { y: 2, x: 2 });
    }

    #[test]
    fn astar_does_not_walk_unknown_or_walls() {
        let mut map = discovered_floor_map(7, 7);
        map.discovered.fill(false);
        map.reveal(Pos { y: 3, x: 2 });
        map.reveal(Pos { y: 3, x: 3 });
        map.reveal(Pos { y: 3, x: 4 });
        map.set_tile(Pos { y: 3, x: 3 }, TileKind::Wall);

        let path = astar_path(&map, Pos { y: 3, x: 2 }, Pos { y: 3, x: 4 });
        assert!(path.is_none());
    }

    #[test]
    fn astar_unreachable_returns_none() {
        let mut map = discovered_floor_map(7, 7);
        map.set_tile(Pos { y: 2, x: 3 }, TileKind::Wall);
        map.set_tile(Pos { y: 3, x: 2 }, TileKind::Wall);
        map.set_tile(Pos { y: 3, x: 4 }, TileKind::Wall);
        map.set_tile(Pos { y: 4, x: 3 }, TileKind::Wall);

        let path = astar_path(&map, Pos { y: 3, x: 3 }, Pos { y: 1, x: 1 });
        assert!(path.is_none());
    }

    #[test]
    fn frontier_selection_picks_nearest_then_pos() {
        let mut map = discovered_floor_map(8, 8);
        map.discovered.fill(false);
        for y in 2..=4 {
            for x in 2..=4 {
                map.reveal(Pos { y, x });
            }
        }

        let start = Pos { y: 3, x: 3 };
        let intent = choose_frontier_intent(&map, start).expect("intent");
        assert_eq!(intent.path_len, 1);
        assert_eq!(intent.target, Pos { y: 2, x: 3 });
    }

    #[test]
    fn no_frontier_candidate_returns_none() {
        let mut map = Map::new(6, 6);
        for y in 0..map.internal_height {
            for x in 0..map.internal_width {
                map.reveal(Pos { y: y as i32, x: x as i32 });
            }
        }
        let start = Pos { y: 3, x: 3 };
        assert_eq!(choose_frontier_intent(&map, start), None);
    }

    #[test]
    fn unchanged_intent_does_not_duplicate_reason_change_log() {
        let content = ContentPack {};
        let mut game = Game::new(123, &content, GameMode::Ironman);

        if let AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) =
            game.advance(1).stop_reason
        {
            game.apply_choice(prompt_id, Choice::KeepLoot).expect("resolve loot");
        }

        let player_pos = game.state.actors[game.state.player_id].pos;
        game.plan_auto_intent(player_pos);
        let first_count = game
            .log()
            .iter()
            .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
            .count();
        assert_eq!(first_count, 1);

        game.plan_auto_intent(player_pos);
        let second_count = game
            .log()
            .iter()
            .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
            .count();
        assert_eq!(second_count, 1);
    }

    #[test]
    fn path_len_only_change_does_not_emit_auto_reason_changed() {
        let content = ContentPack {};
        let mut game = Game::new(123, &content, GameMode::Ironman);

        // Build a deterministic map where only one frontier candidate exists at (5, 7).
        let mut map = Map::new(12, 12);
        for y in 1..(map.internal_height - 1) {
            for x in 1..(map.internal_width - 1) {
                map.reveal(Pos { y: y as i32, x: x as i32 });
            }
        }
        let unknown = Pos { y: 5, x: 8 };
        let unknown_idx = unknown.y as usize * map.internal_width + unknown.x as usize;
        map.discovered[unknown_idx] = false;
        map.set_tile(Pos { y: 4, x: 8 }, TileKind::Wall);
        map.set_tile(Pos { y: 6, x: 8 }, TileKind::Wall);
        map.set_tile(Pos { y: 5, x: 9 }, TileKind::Wall);

        game.state.map = map;
        game.state.actors[game.state.player_id].pos = Pos { y: 5, x: 3 };

        let start_pos = game.state.actors[game.state.player_id].pos;
        game.plan_auto_intent(start_pos);
        let first_intent = game.state.auto_intent.expect("first intent");
        assert_eq!(first_intent.target, Pos { y: 5, x: 7 });
        assert_eq!(first_intent.path_len, 4);

        game.state.actors[game.state.player_id].pos = Pos { y: 5, x: 4 };
        game.plan_auto_intent(Pos { y: 5, x: 4 });
        let second_intent = game.state.auto_intent.expect("second intent");
        assert_eq!(second_intent.target, first_intent.target);
        assert_eq!(second_intent.path_len, 3);

        let reason_change_count = game
            .log()
            .iter()
            .filter(|event| matches!(event, LogEvent::AutoReasonChanged { .. }))
            .count();
        assert_eq!(reason_change_count, 1);
    }

    #[test]
    fn avoid_choice_does_not_immediately_retrigger_same_enemy() {
        let content = ContentPack {};
        let mut game = Game::new(123, &content, GameMode::Ironman);

        if let AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) =
            game.advance(1).stop_reason
        {
            game.apply_choice(prompt_id, Choice::KeepLoot).expect("resolve loot");
        }

        let enemy_id = game
            .state
            .actors
            .iter()
            .find(|(id, actor)| *id != game.state.player_id && actor.kind == ActorKind::Goblin)
            .map(|(id, _)| id)
            .expect("enemy");
        let enemy_pos = game.state.actors[enemy_id].pos;
        let player_pos = Pos { y: enemy_pos.y, x: enemy_pos.x - 1 };
        game.state.actors[game.state.player_id].pos = player_pos;
        game.state.map.reveal(player_pos);

        let first = game.advance(1);
        let prompt_id = match first.stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { prompt_id, enemy }) => {
                assert_eq!(enemy, enemy_id);
                prompt_id
            }
            other => panic!("expected enemy interrupt, got {other:?}"),
        };
        game.apply_choice(prompt_id, Choice::Avoid).expect("resolve avoid");

        let second = game.advance(1);
        if let AdvanceStopReason::Interrupted(Interrupt::EnemyEncounter { enemy, .. }) =
            second.stop_reason
        {
            assert_ne!(enemy, enemy_id, "same enemy immediately re-triggered after Avoid");
        }
    }
}
