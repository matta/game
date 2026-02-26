use std::collections::{BTreeMap, BTreeSet};

use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;

use crate::state::{Actor, ContentPack, GameState, Item, Map};
use crate::types::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingPromptKind {
    Loot { item: ItemId },
    EnemyEncounter { enemy: EntityId },
    DoorBlocked { pos: Pos },
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

const FOV_RADIUS: i32 = 10;

pub struct Game {
    seed: u64,
    tick: u64,
    #[expect(dead_code)]
    rng: ChaCha8Rng,
    state: GameState,
    log: Vec<LogEvent>,
    next_input_seq: u64,
    pending_prompt: Option<PendingPrompt>,
    suppressed_enemy: Option<EntityId>,
    pause_requested: bool,
}

impl Game {
    pub fn new(seed: u64, _content: &ContentPack, _mode: GameMode) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(seed);
        let mut actors = slotmap::SlotMap::with_key();
        let player = Actor {
            id: EntityId::default(),
            kind: ActorKind::Player,
            pos: Pos { y: 5, x: 4 },
            hp: 20,
            max_hp: 20,
            next_action_tick: 10,
            speed: 10,
        };
        let player_id = actors.insert(player);
        actors[player_id].id = player_id;

        let enemy_a = Actor {
            id: EntityId::default(),
            kind: ActorKind::Goblin,
            pos: Pos { y: 5, x: 11 },
            hp: 10,
            max_hp: 10,
            next_action_tick: 12,
            speed: 12,
        };
        let enemy_a_id = actors.insert(enemy_a);
        actors[enemy_a_id].id = enemy_a_id;

        let enemy_b = Actor {
            id: EntityId::default(),
            kind: ActorKind::Goblin,
            pos: Pos { y: 11, x: 11 },
            hp: 10,
            max_hp: 10,
            next_action_tick: 12,
            speed: 12,
        };
        let enemy_b_id = actors.insert(enemy_b);
        actors[enemy_b_id].id = enemy_b_id;

        let mut map = Map::new(20, 15);

        for y in 1..(map.internal_height - 1) {
            for x in 1..(map.internal_width - 1) {
                map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
            }
        }

        // Room A floor rectangle: x=2..6, y=3..7.
        for y in 3..=7 {
            for x in 2..=6 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        // Room B floor rectangle: x=9..13, y=3..7.
        for y in 3..=7 {
            for x in 9..=13 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        // Room C floor rectangle: x=9..13, y=9..13.
        for y in 9..=13 {
            for x in 9..=13 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }

        // Corridor A<->B with closed door at (8,5).
        map.set_tile(Pos { y: 5, x: 7 }, TileKind::Floor);
        map.set_tile(Pos { y: 5, x: 8 }, TileKind::ClosedDoor);

        // Corridor B<->C floor tiles.
        map.set_tile(Pos { y: 8, x: 11 }, TileKind::Floor);
        map.set_tile(Pos { y: 9, x: 11 }, TileKind::Floor);

        // Hazard lane tiles.
        map.set_hazard(Pos { y: 8, x: 11 }, true);
        map.set_hazard(Pos { y: 9, x: 11 }, true);
        map.set_hazard(Pos { y: 10, x: 11 }, true);

        let mut items = slotmap::SlotMap::with_key();
        let item = Item { id: ItemId::default(), pos: Pos { y: 5, x: 6 } };
        let item_id = items.insert(item);
        items[item_id].id = item_id;

        compute_fov(&mut map, actors[player_id].pos, FOV_RADIUS);

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
                && let Some(path) = path_for_intent(&self.state.map, player_pos, intent)
                && let Some(next_step) = path.first().copied()
            {
                if self.state.map.tile_at(next_step) == TileKind::ClosedDoor {
                    return self.interrupt_door(next_step, steps);
                }
                self.state.actors[self.state.player_id].pos = next_step;
                compute_fov(&mut self.state.map, next_step, FOV_RADIUS);
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
            let changed = self.state.auto_intent.map(|i| i.reason) != next_intent.map(|i| i.reason);
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
                true
            }
            (PendingPromptKind::Loot { item }, Choice::DiscardLoot) => {
                self.state.items.remove(item);
                true
            }
            (PendingPromptKind::EnemyEncounter { enemy }, Choice::Fight) => {
                self.state.actors.remove(enemy);
                true
            }
            (PendingPromptKind::EnemyEncounter { enemy }, Choice::Avoid) => {
                self.suppressed_enemy = Some(enemy);
                true
            }
            (PendingPromptKind::DoorBlocked { pos }, Choice::OpenDoor) => {
                self.state.map.set_tile(pos, TileKind::Floor);
                compute_fov(
                    &mut self.state.map,
                    self.state.actors[self.state.player_id].pos,
                    FOV_RADIUS,
                );
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

    pub fn snapshot_hash(&self) -> u64 {
        use std::hash::Hasher;
        use xxhash_rust::xxh3::Xxh3;
        let mut hasher = Xxh3::new();
        hasher.write_u64(self.seed);
        hasher.write_u64(self.tick);
        hasher.write_u64(self.next_input_seq);
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

    pub fn current_tick(&self) -> u64 {
        self.tick
    }
    pub fn request_pause(&mut self) {
        self.pause_requested = true;
    }
    pub fn state(&self) -> &GameState {
        &self.state
    }
    pub fn log(&self) -> &[LogEvent] {
        &self.log
    }

    fn interrupt_loot(&mut self, item: ItemId, steps: u32) -> AdvanceResult {
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::Loot { item },
        };
        self.pending_prompt = Some(prompt);
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }
    fn interrupt_enemy(&mut self, enemy: EntityId, steps: u32) -> AdvanceResult {
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::EnemyEncounter { enemy },
        };
        self.pending_prompt = Some(prompt);
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }
    fn interrupt_door(&mut self, pos: Pos, steps: u32) -> AdvanceResult {
        let prompt = PendingPrompt {
            id: ChoicePromptId(self.next_input_seq),
            kind: PendingPromptKind::DoorBlocked { pos },
        };
        self.pending_prompt = Some(prompt);
        AdvanceResult {
            simulated_ticks: steps,
            stop_reason: AdvanceStopReason::Interrupted(self.prompt_to_interrupt(prompt)),
        }
    }
    fn prompt_to_interrupt(&self, prompt: PendingPrompt) -> Interrupt {
        match prompt.kind {
            PendingPromptKind::Loot { item } => Interrupt::LootFound { prompt_id: prompt.id, item },
            PendingPromptKind::EnemyEncounter { enemy } => {
                Interrupt::EnemyEncounter { prompt_id: prompt.id, enemy }
            }
            PendingPromptKind::DoorBlocked { pos } => {
                Interrupt::DoorBlocked { prompt_id: prompt.id, pos }
            }
        }
    }
    fn find_item_at(&self, pos: Pos) -> Option<ItemId> {
        self.state.items.iter().find(|(_, item)| item.pos == pos).map(|(id, _)| id)
    }
    fn find_adjacent_enemy(&self, pos: Pos) -> Option<(EntityId, &Actor)> {
        self.state
            .actors
            .iter()
            .filter(|(id, _)| Some(*id) != self.suppressed_enemy && *id != self.state.player_id)
            .find(|(_, a)| manhattan(pos, a.pos) == 1)
    }
}

fn choose_frontier_intent(map: &Map, start: Pos) -> Option<AutoExploreIntent> {
    let mut best_safe: Option<(Pos, usize)> = None;
    let mut best_hazard: Option<(Pos, usize)> = None;
    for y in 0..map.internal_height {
        for x in 0..map.internal_width {
            let p = Pos { y: y as i32, x: x as i32 };
            if p == start || !is_frontier_candidate_visible(map, p) {
                continue;
            }
            if !map.is_hazard(p) {
                if let Some(path) = astar_path(map, start, p) {
                    let len = path.len();
                    let is_better = match best_safe {
                        None => true,
                        Some((best_pos, best_len)) => {
                            len < best_len
                                || (len == best_len && (p.y, p.x) < (best_pos.y, best_pos.x))
                        }
                    };
                    if is_better {
                        best_safe = Some((p, len));
                    }
                } else if let Some(path) = astar_path_allow_hazards(map, start, p) {
                    // Safe frontier target reachable only by traversing hazard tiles.
                    let len = path.len();
                    let is_better = match best_hazard {
                        None => true,
                        Some((best_pos, best_len)) => {
                            len < best_len
                                || (len == best_len && (p.y, p.x) < (best_pos.y, best_pos.x))
                        }
                    };
                    if is_better {
                        best_hazard = Some((p, len));
                    }
                }
            } else if let Some(path) = astar_path_allow_hazards(map, start, p) {
                let len = path.len();
                let is_better = match best_hazard {
                    None => true,
                    Some((best_pos, best_len)) => {
                        len < best_len || (len == best_len && (p.y, p.x) < (best_pos.y, best_pos.x))
                    }
                };
                if is_better {
                    best_hazard = Some((p, len));
                }
            }
        }
    }

    if let Some((t, l)) = best_safe {
        let reason = if map.tile_at(t) == TileKind::ClosedDoor {
            AutoReason::Door
        } else {
            AutoReason::Frontier
        };
        return Some(AutoExploreIntent { target: t, reason, path_len: l as u16 });
    }

    best_hazard.map(|(t, l)| AutoExploreIntent {
        target: t,
        reason: AutoReason::ThreatAvoidance,
        path_len: l as u16,
    })
}

fn is_safe_frontier_candidate(map: &Map, pos: Pos) -> bool {
    is_frontier_candidate_visible(map, pos) && !map.is_hazard(pos)
}

fn is_frontier_candidate_visible(map: &Map, pos: Pos) -> bool {
    map.is_visible(pos)
        && map.tile_at(pos) != TileKind::Wall
        && neighbors(pos).iter().any(|n| map.in_bounds(*n) && !map.is_discovered(*n))
}

fn is_intent_target_still_valid(map: &Map, intent: AutoExploreIntent) -> bool {
    match intent.reason {
        AutoReason::ThreatAvoidance => is_frontier_candidate_visible(map, intent.target),
        _ => is_safe_frontier_candidate(map, intent.target),
    }
}

fn path_for_intent(map: &Map, start: Pos, intent: AutoExploreIntent) -> Option<Vec<Pos>> {
    match intent.reason {
        AutoReason::ThreatAvoidance => astar_path_allow_hazards(map, start, intent.target),
        _ => astar_path(map, start, intent.target),
    }
}

fn astar_path(map: &Map, start: Pos, goal: Pos) -> Option<Vec<Pos>> {
    astar_path_internal(map, start, goal, true)
}

fn astar_path_allow_hazards(map: &Map, start: Pos, goal: Pos) -> Option<Vec<Pos>> {
    astar_path_internal(map, start, goal, false)
}

fn astar_path_internal(map: &Map, start: Pos, goal: Pos, avoid_hazards: bool) -> Option<Vec<Pos>> {
    if !map.is_discovered_walkable(start) || !map.is_discovered_walkable(goal) {
        return None;
    }
    if start == goal {
        return Some(vec![]);
    }
    let mut open_set = BTreeSet::new();
    let mut g_score = BTreeMap::new();
    let mut came_from = BTreeMap::new();
    let h = manhattan(start, goal);
    open_set.insert(OpenNode { f: h, h, y: start.y, x: start.x });
    g_score.insert(start, 0);
    while let Some(curr) = open_set.pop_first() {
        let p = Pos { y: curr.y, x: curr.x };
        if p == goal {
            return Some(reconstruct_path(&came_from, start, goal));
        }
        let cur_g = *g_score.get(&p).unwrap();
        for n in neighbors(p) {
            if !is_astar_step_walkable(map, n, goal, avoid_hazards) {
                continue;
            }
            let tg = cur_g + 1;
            if tg < *g_score.get(&n).unwrap_or(&u32::MAX) {
                came_from.insert(n, p);
                g_score.insert(n, tg);
                let h = manhattan(n, goal);
                open_set.insert(OpenNode { f: tg + h, h, y: n.y, x: n.x });
            }
        }
    }
    None
}

fn is_astar_step_walkable(map: &Map, pos: Pos, goal: Pos, avoid_hazards: bool) -> bool {
    if !map.is_discovered_walkable(pos) {
        return false;
    }
    if avoid_hazards && map.is_hazard(pos) {
        return false;
    }
    // Closed doors can be reached as an immediate target, but not traversed through.
    if map.tile_at(pos) == TileKind::ClosedDoor && pos != goal {
        return false;
    }
    true
}

fn reconstruct_path(came: &BTreeMap<Pos, Pos>, start: Pos, goal: Pos) -> Vec<Pos> {
    let mut p = goal;
    let mut res = vec![p];
    while p != start {
        p = *came.get(&p).unwrap();
        res.push(p);
    }
    res.reverse();
    res.remove(0);
    res
}

fn neighbors(p: Pos) -> [Pos; 4] {
    [
        Pos { y: p.y - 1, x: p.x },
        Pos { y: p.y, x: p.x + 1 },
        Pos { y: p.y + 1, x: p.x },
        Pos { y: p.y, x: p.x - 1 },
    ]
}

fn manhattan(a: Pos, b: Pos) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

fn transform_octant(orig: Pos, x: i32, y: i32, oct: u8) -> Pos {
    match oct {
        0 => Pos { y: orig.y - y, x: orig.x + x },
        1 => Pos { y: orig.y - x, x: orig.x + y },
        2 => Pos { y: orig.y - x, x: orig.x - y },
        3 => Pos { y: orig.y - y, x: orig.x - x },
        4 => Pos { y: orig.y + y, x: orig.x - x },
        5 => Pos { y: orig.y + x, x: orig.x - y },
        6 => Pos { y: orig.y + x, x: orig.x + y },
        7 => Pos { y: orig.y + y, x: orig.x + x },
        _ => orig,
    }
}

fn compute_fov(map: &mut Map, origin: Pos, range: i32) {
    let prev_discovered = map.discovered.clone();
    map.clear_visible();
    map.set_visible(origin, true);
    for octant in 0..8 {
        scan_octant(map, origin, range, 1, Slope::new(1, 1), Slope::new(0, 1), octant);
    }
    // Remove tiles that were marked visible but are blocked by a wall in a direct line.
    let min_y = (origin.y - range).max(0);
    let max_y = (origin.y + range + 1).min(map.internal_height as i32);
    let min_x = (origin.x - range).max(0);
    let max_x = (origin.x + range + 1).min(map.internal_width as i32);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = Pos { y, x };
            if p == origin || !map.is_visible(p) {
                continue;
            }
            if !has_direct_line_of_sight(map, origin, p) {
                let idx = (y as usize) * map.internal_width + (x as usize);
                map.visible[idx] = false;
                map.discovered[idx] = prev_discovered[idx];
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Slope {
    y: i32,
    x: i32,
}
impl Slope {
    fn new(y: i32, x: i32) -> Self {
        Self { y, x }
    }
    fn greater_or_equal(&self, other: &Slope) -> bool {
        self.y * other.x >= other.y * self.x
    }
    #[allow(dead_code)]
    fn greater_than(&self, other: &Slope) -> bool {
        self.y * other.x > other.y * self.x
    }
}

fn scan_octant(map: &mut Map, orig: Pos, range: i32, dist: i32, start: Slope, end: Slope, oct: u8) {
    if dist > range {
        return;
    }
    let range_u = u32::try_from(range).expect("FOV range must be non-negative");
    let mut blocked = false;
    let mut cur_start = start;
    for y in (0..=dist).rev() {
        let top = Slope::new(2 * y + 1, 2 * dist - 1);
        let bot = Slope::new(2 * y - 1, 2 * dist + 1);
        if cur_start.greater_or_equal(&bot) && top.greater_than(&end) {
            let p = transform_octant(orig, dist, y, oct);
            if manhattan(orig, p) <= range_u {
                map.set_visible(p, true);
            }
            let opaque = map.tile_at(p) == TileKind::Wall || map.tile_at(p) == TileKind::ClosedDoor;
            if opaque {
                if !blocked {
                    scan_octant(map, orig, range, dist + 1, cur_start, top, oct);
                    blocked = true;
                }
                // Tighten scan start to the bottom slope of the blocking tile.
                cur_start = bot;
            } else if blocked {
                blocked = false;
            }
        }
    }
    if !blocked {
        scan_octant(map, orig, range, dist + 1, cur_start, end, oct);
    }
}

fn has_direct_line_of_sight(map: &Map, origin: Pos, target: Pos) -> bool {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let sx = dx.signum();
    let sy = dy.signum();
    let total_dist_x = dx.abs();
    let total_dist_y = dy.abs();

    let mut x = origin.x;
    let mut y = origin.y;
    let mut current_step_x = 0;
    let mut current_step_y = 0;

    // Compare slopes to determine whether to step along the X or Y axis next.
    // We use integer arithmetic to avoid floating-point inaccuracies.
    while current_step_x < total_dist_x || current_step_y < total_dist_y {
        let lhs = (1 + 2 * current_step_x) * total_dist_y;
        let rhs = (1 + 2 * current_step_y) * total_dist_x;

        if lhs == rhs {
            x += sx;
            y += sy;
            current_step_x += 1;
            current_step_y += 1;
        } else if lhs < rhs {
            x += sx;
            current_step_x += 1;
        } else {
            y += sy;
            current_step_y += 1;
        }

        if x == target.x && y == target.y {
            break;
        }
        let tile = map.tile_at(Pos { y, x });
        if tile == TileKind::Wall || tile == TileKind::ClosedDoor {
            return false;
        }
    }
    true
}

#[allow(dead_code)]
fn draw_map_diag(map: &Map, player: Pos) -> String {
    let mut s = String::new();
    for y in 0..map.internal_height {
        for x in 0..map.internal_width {
            let p = Pos { y: y as i32, x: x as i32 };
            let c = if p == player {
                '@'
            } else if map.tile_at(p) == TileKind::Wall {
                '#'
            } else if map.tile_at(p) == TileKind::ClosedDoor {
                '+'
            } else if is_safe_frontier_candidate(map, p) {
                'F'
            } else {
                '.'
            };
            let v = if map.is_visible(p) { 'v' } else { 'h' };
            let d = if map.is_discovered(p) { 'd' } else { 'u' };
            s.push_str(&format!("{c}{v}{d} "));
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_room_fixture() -> (Map, Pos) {
        let map = Map::new(10, 10);
        let origin = Pos { y: 5, x: 5 };
        (map, origin)
    }

    fn wall_occlusion_fixture() -> (Map, Pos) {
        let mut map = Map::new(11, 11);
        for y in 1..10 {
            for x in 1..10 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 1..10 {
            map.set_tile(Pos { y: 5, x }, TileKind::Floor);
        }
        map.set_tile(Pos { y: 5, x: 6 }, TileKind::Wall);
        (map, Pos { y: 5, x: 3 })
    }

    fn hazard_lane_fixture() -> (Map, Pos) {
        let mut map = Map::new(9, 9);
        for y in 1..8 {
            for x in 1..8 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 2..=5 {
            map.set_tile(Pos { y: 4, x }, TileKind::Floor);
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        (map, Pos { y: 4, x: 2 })
    }

    fn closed_door_choke_fixture() -> (Map, Pos, Pos) {
        let mut map = Map::new(10, 10);
        for x in 0..10 {
            for y in 0..10 {
                map.set_tile(Pos { y, x }, if y == 5 { TileKind::Floor } else { TileKind::Wall });
            }
        }
        map.discovered.fill(true);
        let door = Pos { y: 5, x: 6 };
        map.set_tile(door, TileKind::ClosedDoor);
        map.discovered[5 * 10 + 7] = false;
        (map, Pos { y: 5, x: 5 }, door)
    }

    fn corner_handle_fixture() -> (Map, Pos) {
        let mut map = Map::new(20, 15);
        for y in 1..(map.internal_height - 1) {
            for x in 1..(map.internal_width - 1) {
                map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
            }
        }
        // Room interior.
        for y in 3..=7 {
            for x in 2..=6 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        // Corridor + blocked door to reproduce right-facing corner geometry.
        map.set_tile(Pos { y: 5, x: 7 }, TileKind::Floor);
        map.set_tile(Pos { y: 5, x: 8 }, TileKind::ClosedDoor);
        (map, Pos { y: 5, x: 6 })
    }

    #[test]
    fn starter_layout_has_expected_rooms_door_hazards_and_spawns() {
        let game = Game::new(12345, &ContentPack {}, GameMode::Ironman);

        let expected_player = Pos { y: 5, x: 4 };
        assert_eq!(game.state.actors[game.state.player_id].pos, expected_player);

        let loot_positions: Vec<Pos> = game.state.items.iter().map(|(_, item)| item.pos).collect();
        assert_eq!(loot_positions, vec![Pos { y: 5, x: 6 }]);
        assert!(!loot_positions.contains(&expected_player));

        let goblin_positions: Vec<Pos> = game
            .state
            .actors
            .iter()
            .filter(|(id, actor)| *id != game.state.player_id && actor.kind == ActorKind::Goblin)
            .map(|(_, actor)| actor.pos)
            .collect();
        assert_eq!(goblin_positions.len(), 2, "starter layout should spawn two goblins");
        assert!(goblin_positions.contains(&Pos { y: 5, x: 11 }));
        assert!(goblin_positions.contains(&Pos { y: 11, x: 11 }));

        assert_eq!(game.state.map.tile_at(Pos { y: 5, x: 8 }), TileKind::ClosedDoor);
        assert_eq!(game.state.map.tile_at(Pos { y: 5, x: 7 }), TileKind::Floor);
        assert_eq!(game.state.map.tile_at(Pos { y: 8, x: 11 }), TileKind::Floor);
        assert_eq!(game.state.map.tile_at(Pos { y: 9, x: 11 }), TileKind::Floor);

        for hazard in [Pos { y: 8, x: 11 }, Pos { y: 9, x: 11 }, Pos { y: 10, x: 11 }] {
            assert!(game.state.map.is_hazard(hazard), "expected hazard at {hazard:?}");
        }
    }

    #[test]
    fn fov_open_room_visibility() {
        let (mut map, origin) = open_room_fixture();
        compute_fov(&mut map, origin, 3);
        assert!(map.is_visible(origin));
        assert!(map.is_visible(Pos { y: 5, x: 8 }));
        assert!(!map.is_visible(Pos { y: 1, x: 1 }));
    }

    #[test]
    fn fov_repeat_is_deterministic_for_same_state() {
        let (mut map, origin) = open_room_fixture();
        map.set_tile(Pos { y: 5, x: 7 }, TileKind::Wall);
        map.set_tile(Pos { y: 6, x: 7 }, TileKind::Wall);

        compute_fov(&mut map, origin, FOV_RADIUS);
        let first = map.visible.clone();
        compute_fov(&mut map, origin, FOV_RADIUS);
        let second = map.visible.clone();

        assert_eq!(first, second, "FOV result must be identical for same map/origin");
    }

    #[test]
    fn movement_updates_visibility_and_expands_discovery() {
        let mut game = Game::new(123, &ContentPack {}, GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let mut map = Map::new(30, 10);
        for y in 1..9 {
            for x in 1..29 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 1..26 {
            map.set_tile(Pos { y: 4, x }, TileKind::Floor);
        }
        map.discovered.fill(false);
        game.state.map = map;

        let start = Pos { y: 4, x: 5 };
        game.state.actors[game.state.player_id].pos = start;
        compute_fov(&mut game.state.map, start, FOV_RADIUS);
        // Create visible frontier at (4,15) by leaving (4,16) unknown.
        game.state.map.discovered[(4 * game.state.map.internal_width) + 16] = false;
        let discovered_before = game.state.map.discovered.iter().filter(|&&d| d).count();

        let result = game.advance(1);
        assert!(matches!(result.stop_reason, AdvanceStopReason::BudgetExhausted));
        let moved_to = game.state.actors[game.state.player_id].pos;
        assert_eq!(manhattan(start, moved_to), 1, "player should move exactly one tile");
        let discovered_after = game.state.map.discovered.iter().filter(|&&d| d).count();
        assert!(
            discovered_after > discovered_before,
            "moving with FOV recompute should discover at least one new tile"
        );
    }

    #[test]
    fn fov_wall_occlusion_blocks_tiles_behind_wall_in_corridor() {
        let (mut map, origin) = wall_occlusion_fixture();
        compute_fov(&mut map, origin, 10);

        assert!(map.is_visible(Pos { y: 5, x: 5 }));
        assert!(map.is_visible(Pos { y: 5, x: 6 }));
        assert!(
            !map.is_visible(Pos { y: 5, x: 7 }),
            "tile directly behind corridor wall should be occluded"
        );
    }

    #[test]
    fn planner_avoids_hazard_route_when_safe_frontier_exists() {
        let (mut map, start) = hazard_lane_fixture();
        map.set_hazard(Pos { y: 4, x: 3 }, true);

        // Safe alternative route to a different frontier.
        for y in 2..=4 {
            map.set_tile(Pos { y, x: 2 }, TileKind::Floor);
        }
        for x in 2..=4 {
            map.set_tile(Pos { y: 2, x }, TileKind::Floor);
        }

        map.discovered.fill(true);
        map.visible.fill(true);
        // Frontier near hazard lane.
        map.discovered[(4 * map.internal_width) + 6] = false;
        // Safe frontier candidate.
        map.discovered[(2 * map.internal_width) + 5] = false;

        let intent = choose_frontier_intent(&map, start).expect("expected frontier intent");
        assert_eq!(intent.target, Pos { y: 2, x: 4 });
    }

    #[test]
    fn planner_reports_threat_avoidance_when_only_hazard_frontier_exists() {
        let (mut map, start) = hazard_lane_fixture();
        map.set_hazard(Pos { y: 4, x: 5 }, true);
        map.discovered[(4 * map.internal_width) + 6] = false;

        let intent = choose_frontier_intent(&map, start).expect("hazard fallback intent");
        assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
    }

    #[test]
    fn safe_frontier_reachable_only_through_hazards_uses_threat_avoidance() {
        let mut map = Map::new(11, 9);
        for y in 1..8 {
            for x in 1..10 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 2..=8 {
            map.set_tile(Pos { y: 4, x }, TileKind::Floor);
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        map.set_hazard(Pos { y: 4, x: 4 }, true);
        map.set_hazard(Pos { y: 4, x: 6 }, true);
        map.discovered[(4 * map.internal_width) + 1] = false;
        map.discovered[(4 * map.internal_width) + 9] = false;

        let start = Pos { y: 4, x: 5 };
        let intent = choose_frontier_intent(&map, start).expect("fallback on safe frontier");
        assert_eq!(intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(intent.target, Pos { y: 4, x: 2 });
    }

    #[test]
    fn threat_avoidance_intent_is_reused_without_retarget_replan() {
        let mut game = Game::new(123, &ContentPack {}, GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let mut map = Map::new(11, 9);
        for y in 1..8 {
            for x in 1..10 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 2..=8 {
            map.set_tile(Pos { y: 4, x }, TileKind::Floor);
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        map.set_hazard(Pos { y: 4, x: 4 }, true);
        map.set_hazard(Pos { y: 4, x: 6 }, true);
        map.discovered[(4 * map.internal_width) + 1] = false;
        map.discovered[(4 * map.internal_width) + 9] = false;
        game.state.map = map;

        let p1 = Pos { y: 4, x: 5 };
        game.state.actors[game.state.player_id].pos = p1;
        game.plan_auto_intent(p1);
        let first_intent = game.state.auto_intent.expect("first intent");
        assert_eq!(first_intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(first_intent.target, Pos { y: 4, x: 2 });
        let first_log_count =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(first_log_count, 1);

        // Move opposite the current target; replan would switch to x=8, reuse should not.
        let p2 = Pos { y: 4, x: 6 };
        game.state.actors[game.state.player_id].pos = p2;
        game.plan_auto_intent(p2);
        let second_intent = game.state.auto_intent.expect("second intent");
        assert_eq!(second_intent.reason, AutoReason::ThreatAvoidance);
        assert_eq!(second_intent.target, Pos { y: 4, x: 2 });
        let second_log_count =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(second_log_count, 1);
    }

    #[test]
    fn advance_uses_hazard_path_for_threat_avoidance_intent() {
        let mut game = Game::new(123, &ContentPack {}, GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);

        let mut map = Map::new(11, 9);
        for y in 1..8 {
            for x in 1..10 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 2..=8 {
            map.set_tile(Pos { y: 4, x }, TileKind::Floor);
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        map.set_hazard(Pos { y: 4, x: 4 }, true);
        map.set_hazard(Pos { y: 4, x: 6 }, true);
        map.discovered[(4 * map.internal_width) + 1] = false;
        map.discovered[(4 * map.internal_width) + 9] = false;
        game.state.map = map;

        let start = Pos { y: 4, x: 5 };
        game.state.actors[game.state.player_id].pos = start;

        let result = game.advance(1);
        assert!(matches!(result.stop_reason, AdvanceStopReason::BudgetExhausted));
        assert_eq!(game.state.actors[game.state.player_id].pos, Pos { y: 4, x: 4 });
        assert_eq!(game.state.auto_intent.map(|i| i.reason), Some(AutoReason::ThreatAvoidance));
    }

    #[test]
    fn frontier_selection_ignores_non_visible_frontiers() {
        let mut map = Map::new(10, 10);
        for y in 1..9 {
            for x in 1..9 {
                map.set_tile(Pos { y, x }, TileKind::Floor);
            }
        }
        map.discovered.fill(true);
        map.visible.fill(false);
        for x in 2..=5 {
            map.set_visible(Pos { y: 4, x }, true);
        }
        map.discovered[(4 * map.internal_width) + 6] = false; // visible frontier
        map.discovered[(6 * map.internal_width) + 8] = false; // not visible frontier

        let start = Pos { y: 4, x: 3 };
        let intent = choose_frontier_intent(&map, start).expect("visible frontier");
        assert_eq!(intent.target, Pos { y: 4, x: 5 });
    }

    #[test]
    fn closed_door_is_not_used_as_path_transit_tile() {
        let mut map = Map::new(12, 6);
        for y in 1..5 {
            for x in 1..11 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 1..=10 {
            map.set_tile(Pos { y: 3, x }, TileKind::Floor);
        }
        map.set_tile(Pos { y: 3, x: 5 }, TileKind::ClosedDoor);
        map.discovered.fill(true);

        let start = Pos { y: 3, x: 2 };
        let beyond_door = Pos { y: 3, x: 8 };
        assert!(
            astar_path(&map, start, beyond_door).is_none(),
            "closed door should block traversal to tiles behind it"
        );
        assert!(
            astar_path(&map, start, Pos { y: 3, x: 5 }).is_some(),
            "closed door may still be targeted directly for interrupt handling"
        );
    }

    #[test]
    fn door_interrupt_and_open_flow() {
        let mut game = Game::new(12345, &ContentPack {}, GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        let (map, pp, dp) = closed_door_choke_fixture();
        game.state.map = map;
        game.state.actors[game.state.player_id].pos = pp;
        compute_fov(&mut game.state.map, pp, FOV_RADIUS);

        // Manually set intent to target the door (which is a frontier candidate)
        game.state.auto_intent =
            Some(AutoExploreIntent { target: dp, reason: AutoReason::Frontier, path_len: 1 });

        let res = game.advance(1);
        if let AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, pos }) =
            res.stop_reason
        {
            assert_eq!(pos, dp);
            game.apply_choice(prompt_id, Choice::OpenDoor).unwrap();
            assert_eq!(game.state.map.tile_at(dp), TileKind::Floor);
        } else {
            panic!(
                "Expected DoorBlocked at {:?}, got {:?}. Map:\n{}",
                dp,
                res.stop_reason,
                draw_map_diag(&game.state.map, pp)
            );
        }
    }

    #[test]
    fn door_interrupt_open_then_resume_moves_forward() {
        let mut game = Game::new(12345, &ContentPack {}, GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        let (map, start, door) = closed_door_choke_fixture();
        game.state.map = map;
        game.state.actors[game.state.player_id].pos = start;
        compute_fov(&mut game.state.map, start, FOV_RADIUS);
        game.state.auto_intent =
            Some(AutoExploreIntent { target: door, reason: AutoReason::Door, path_len: 1 });

        let first = game.advance(1);
        let prompt_id = match first.stop_reason {
            AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { prompt_id, .. }) => prompt_id,
            other => panic!("expected door interrupt, got {other:?}"),
        };
        game.apply_choice(prompt_id, Choice::OpenDoor).expect("open door");

        let second = game.advance(1);
        assert!(
            !matches!(
                second.stop_reason,
                AdvanceStopReason::Interrupted(Interrupt::DoorBlocked { .. })
            ),
            "door should not immediately re-interrupt after opening"
        );
        assert_eq!(game.state.map.tile_at(door), TileKind::Floor);
    }

    #[test]
    fn unchanged_intent_does_not_duplicate_reason_change_log() {
        let mut game = Game::new(123, &ContentPack {}, GameMode::Ironman);
        if let AdvanceStopReason::Interrupted(Interrupt::LootFound { prompt_id, .. }) =
            game.advance(1).stop_reason
        {
            game.apply_choice(prompt_id, Choice::KeepLoot).unwrap();
        }
        let pos = game.state.actors[game.state.player_id].pos;
        compute_fov(&mut game.state.map, pos, FOV_RADIUS);
        game.plan_auto_intent(pos);
        let cnt1 =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(cnt1, 1);
        game.plan_auto_intent(pos);
        let cnt2 =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(cnt2, 1);
    }

    #[test]
    fn path_len_only_change_does_not_emit_auto_reason_changed() {
        let mut game = Game::new(12345, &ContentPack {}, GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        let mut map = Map::new(12, 7);
        for y in 1..(map.internal_height - 1) {
            for x in 1..(map.internal_width - 1) {
                map.set_tile(Pos { y: y as i32, x: x as i32 }, TileKind::Wall);
            }
        }
        for x in 1..=9 {
            map.set_tile(Pos { y: 3, x }, TileKind::Floor);
        }
        map.discovered.fill(true);
        map.visible.fill(false);
        for x in 1..=8 {
            map.set_visible(Pos { y: 3, x }, true);
        }
        map.discovered[(3 * map.internal_width) + 9] = false;

        game.state.map = map;
        let p1 = Pos { y: 3, x: 3 };
        game.state.actors[game.state.player_id].pos = p1;
        game.plan_auto_intent(p1);
        let prev_intent = game.state.auto_intent.unwrap_or_else(|| {
            panic!("No first intent! Map:\n{}", draw_map_diag(&game.state.map, p1));
        });
        assert_eq!(prev_intent.target, Pos { y: 3, x: 8 });

        let p2 = Pos { y: 3, x: 4 };
        game.state.actors[game.state.player_id].pos = p2;
        game.plan_auto_intent(p2);
        let next_intent = game.state.auto_intent.unwrap();
        assert_eq!(prev_intent.target, next_intent.target);
        assert_ne!(prev_intent.path_len, next_intent.path_len);
        let cnt =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(cnt, 1);
    }

    #[test]
    fn auto_reason_changed_emits_only_on_reason_or_target_changes() {
        let mut game = Game::new(12345, &ContentPack {}, GameMode::Ironman);
        game.state.items.clear();
        game.state.actors.retain(|id, _| id == game.state.player_id);
        let mut map = Map::new(10, 7);
        for y in 1..6 {
            for x in 1..9 {
                map.set_tile(Pos { y, x }, TileKind::Wall);
            }
        }
        for x in 1..=7 {
            map.set_tile(Pos { y: 3, x }, TileKind::Floor);
        }
        map.discovered.fill(true);
        map.visible.fill(true);
        map.discovered[(3 * map.internal_width) + 8] = false;
        game.state.map = map;

        let pos = Pos { y: 3, x: 2 };
        game.state.actors[game.state.player_id].pos = pos;
        game.plan_auto_intent(pos);
        let count_after_first =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(count_after_first, 1);

        // Same target, different reason due to hazard fallback.
        game.state.map.set_hazard(Pos { y: 3, x: 7 }, true);
        game.plan_auto_intent(pos);
        let count_after_reason_change =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(count_after_reason_change, 2);

        // No further reason/target change => no extra log.
        game.plan_auto_intent(pos);
        let count_after_repeat =
            game.log().iter().filter(|e| matches!(e, LogEvent::AutoReasonChanged { .. })).count();
        assert_eq!(count_after_repeat, 2);
    }

    #[test]
    fn fov_does_not_leak_through_corners() {
        let mut map = Map::new(20, 20);
        // Create a 5x5 interior room from (5,5) to (9,9)
        // Walls at (4,4) to (10,10)
        let r_start = 4;
        let r_end = 10;
        for y in r_start..=r_end {
            for x in r_start..=r_end {
                if y == r_start || y == r_end || x == r_start || x == r_end {
                    map.set_tile(Pos { y, x }, TileKind::Wall);
                } else {
                    map.set_tile(Pos { y, x }, TileKind::Floor);
                }
            }
        }

        // Test with player at various positions inside the room
        for py in (r_start + 1)..r_end {
            for px in (r_start + 1)..r_end {
                let origin = Pos { y: py, x: px };
                compute_fov(&mut map, origin, 15);
                for y in 0..map.internal_height {
                    for x in 0..map.internal_width {
                        let p = Pos { y: y as i32, x: x as i32 };
                        if (p.y < r_start || p.y > r_end || p.x < r_start || p.x > r_end)
                            && map.is_visible(p)
                        {
                            panic!(
                                "Light leaked to {p:?} from origin {origin:?}\n{}",
                                draw_map_diag(&map, origin)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fov_fixture_does_not_show_corner_handles() {
        let (mut map, origin) = corner_handle_fixture();
        compute_fov(&mut map, origin, FOV_RADIUS);

        // Cells that can be incorrectly discovered as "corner handles" should stay unknown.
        for p in [Pos { y: 1, x: 0 }, Pos { y: 9, x: 0 }, Pos { y: 1, x: 1 }, Pos { y: 9, x: 1 }] {
            assert!(
                !map.is_visible(p),
                "unexpected corner handle at {p:?}\n{}",
                draw_map_diag(&map, origin)
            );
            assert!(
                !map.is_discovered(p),
                "unexpected discovered corner handle at {p:?}\n{}",
                draw_map_diag(&map, origin)
            );
        }
    }
}
