use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::BinaryHeap;
use std::iter::FromIterator;
use std::isize;
use data::*;
use serde_json::Value;
use std::cmp::min;
use std::cmp::max;
use std::cmp::Ordering;

pub struct Game {
	pub turn_number: usize,
    pub max_turns: usize,
    pub constants: Constants,
    pub num_players: usize,
    pub my_pid: usize,
    pub factories: Vec<Factory>,
    pub width: usize,
    pub height: usize,
    pub halite_map: GMap<usize>,

    pub ships: HashMap<usize, Ship>,
    pub ship_id_by_player: Vec<Vec<usize>>,
    pub ship_map: HashMap<Point, usize>, // map of locations to ship IDs for lookup in ships?
    pub dropoffs: Vec<Dropoff>,
    pub energy: Vec<usize>,
    pub my_drop_pts: Vec<Point>,
    pub nearest_drop_pt_idx: GMap<usize>,
    pub nearest_drop_pt_dist: GMap<usize>,
    pub enemy_drop_pts: HashMap<Point, usize>,
}

pub struct Constants {
    pub max_turns: usize,
    pub ship_cost: usize,
    pub dropoff_cost: usize,
    pub max_halite: usize,
}

pub fn parse_line_of_nums<I: Iterator<Item = String>>(lines_iter: &mut I) -> Vec<usize> {
    lines_iter.next().unwrap()
        .trim()
        .split_whitespace()
        .map(str::parse::<usize>)
        .map(Result::unwrap)
        .collect()
}

impl Game {
	pub fn init<I: Iterator<Item = String>>(lines_iter: &mut I,
        constant_json: Value, num_players: usize, my_pid: usize) -> Game { // pre-parse

        let mut factories: Vec<Factory> = Vec::new();
        let mut my_drop_pts: Vec<Point> = Vec::new();
        let mut enemy_drop_pts: HashMap<Point, usize> = HashMap::new();
        for _ in 0..num_players {
            let this_player = parse_line_of_nums(lines_iter);
            let f_x = this_player[1] as isize;
            let f_y = this_player[2] as isize;
            factories.push(Factory {
                player: this_player[0],
                pos: Point{
                    x: f_x,
                    y: f_y,
                },
            });
            if this_player[0] == my_pid {
                my_drop_pts.push(Point{x: f_x, y: f_y});
            } else {
                enemy_drop_pts.insert(Point{x: f_x, y: f_y}, this_player[0]);
            }
        }

        let dims = parse_line_of_nums(lines_iter);
        let width = dims[0];
        let height = dims[1];

        let mut h_map: Vec<Vec<usize>> = Vec::new();
        for _y in 0..height {
            h_map.push(parse_line_of_nums(lines_iter));
        }
        info!("num_players: {}, my player id: {}\n factories: {:?}",
            num_players, my_pid, factories);
        // info!("map: ");
        // for y in 0..height {
        //     info!("{:?}", map[y]);
        // }

        let max_turns = constant_json["MAX_TURNS"].as_u64().unwrap() as usize;
        info!("MAX_TURNS: {}", max_turns);

        Game {
			turn_number: 0,
            max_turns,
            constants: Constants {
                max_turns,
                ship_cost: constant_json["NEW_ENTITY_ENERGY_COST"].as_u64().unwrap() as usize,
                dropoff_cost: constant_json["DROPOFF_COST"].as_u64().unwrap() as usize,
                max_halite: constant_json["MAX_ENERGY"].as_u64().unwrap() as usize,
            },
            num_players,
            my_pid,
            factories,
            width, height,
            halite_map: GMap { gmap: h_map },
            ships: HashMap::new(),
            dropoffs: Vec::new(),
            ship_id_by_player: Vec::new(),
            ship_map: HashMap::new(),
            energy: vec![0; num_players],
            my_drop_pts,
            nearest_drop_pt_idx: GMap{ gmap: vec![vec![0; width]; height] },
            nearest_drop_pt_dist: GMap{ gmap: vec![vec![width+height+1; width]; height] },
            enemy_drop_pts,
		}
	}

	pub fn ready(&mut self, name: &str) {
        self.update_dropoff_maps();
        println!("{}", name);
    }

    pub fn update_frame<I: Iterator<Item = String>>(&mut self, lines_iter: &mut I) {
        self.turn_number = str::parse::<usize>(&lines_iter.next().unwrap()).unwrap() - 1;

        info!("====== TURN {} ======", self.turn_number);

        self.ship_id_by_player.clear();
        self.ships.clear();
        self.dropoffs.clear();
        self.ship_map.clear();
        for _ in 0..self.num_players {
            let player_info = parse_line_of_nums(lines_iter);
            let player_id = player_info[0];
            let num_ships = player_info[1];
            let num_dropoffs = player_info[2];
            self.energy[player_id] = player_info[3];

            // info!("player_info: {:?}", player_info);
            self.ship_id_by_player.push(Vec::new());
            info!("player {} info: {:?}", player_id, player_info);
            for _ in 0..num_ships {
                let ship_info = parse_line_of_nums(lines_iter);
                let ship_id = ship_info[0];
                let x = ship_info[1] as isize;
                let y = ship_info[2] as isize;
                self.ship_id_by_player[player_id].push(ship_id);
                let s = Ship {
                    player: player_id,
                    ship_id,
                    pos: Point{x, y,},
                    halite: ship_info[3] as isize,
                };
                self.ships.insert(ship_id, s);
                self.ship_map.insert(Point{x, y}, ship_id);
            }

            for _ in 0..num_dropoffs {
                let dropoff_info = parse_line_of_nums(lines_iter);
                let d_pos = Point {
                        x: dropoff_info[1] as isize,
                        y: dropoff_info[2] as isize,
                    };
                self.dropoffs.push(Dropoff {
                    player: player_id,
                    pos: d_pos,
                });
                if self.my_pid == player_id {
                    if !self.my_drop_pts.contains(&d_pos) {
                        self.my_drop_pts.push(d_pos);
                        self.update_dropoff_maps();
                    }
                } else {
                    if !self.enemy_drop_pts.contains_key(&d_pos) {
                        self.enemy_drop_pts.insert(d_pos, player_id);
                    }
                }
            }
        }

        let num_map_updates = str::parse::<usize>(&lines_iter.next().unwrap()).unwrap();
        for _ in 0..num_map_updates {
            let map_update = parse_line_of_nums(lines_iter);
            let x = map_update[0];
            let y = map_update[1];
            self.halite_map.gmap[y][x] = map_update[2];
        }

        // info!("ships: \n{:?}\nship_id_by_player: \n{:?}\ndropoffs: {:?}\nnum map updates: {}\n",
        //     self.ships, self.ship_id_by_player, self.dropoffs, num_map_updates);
    }

    pub fn end_turn((spawn, ship_commands): (bool, HashMap<usize, ShipCommand>)) {
        if spawn {
            print!("g ");
        }
        for (ship_id, command) in ship_commands.iter() {
            match command {
                ShipCommand::MakeDropoff() => {
                    print!("c {}", ship_id);
                },
                ShipCommand::MoveShip(dir) => {
                    print!("m {} {}", ship_id, dir.get_char_encoding());
                },
            }
        }
    	println!();
    }

    pub fn update_dropoff_maps(&mut self) {
        let vmaps = self.make_vmaps(&self.my_drop_pts);
        self.nearest_drop_pt_idx = vmaps.0;
        self.nearest_drop_pt_dist = vmaps.1;

        // f-log contour map of distance
        // let color_str = vec!["#000010", "#000030", "#000050", "#000070", "#000090", "#0000B0", "#0000D0"];
        // for x in 0..self.width {
        //     for y in 0..self.height {
        //         let d = self.nearest_drop_pt_dist.gmap[y][x];
        //         warn!("{{\"t\": {}, \"x\": {}, \"y\": {}, \"msg\": \"dist {} dropoff {}\", \"color\": \"{}\"}},",
        //         self.turn_number, x, y, d, self.nearest_drop_pt_idx.gmap[y][x],
        //         color_str[d%color_str.len()]);
        //     }
        // }
    }

    // returns (index, distance)
    pub fn make_vmaps(&self, pts: &Vec<Point>) -> (GMap<usize>, GMap<usize>) {
        // assume width == height
        let nd = pts.len();
        assert!(nd > 0);
        if nd == 1 {
            let nearest_pt_idx = GMap{ gmap: vec![vec![0; self.width]; self.height] };
            let mut nearest_pt_dist = GMap{ gmap: vec![vec![0; self.width]; self.height] };
            for d in 0..(self.width+1) {
                for p in self.tiles_at_dist(pts[0], d) {
                    *nearest_pt_dist.get_mut(p) = d;
                }
            }
            (nearest_pt_idx, nearest_pt_dist)
        } else {
            let mut open = vec![true; nd];
            let mut nearest_pt_idx = GMap{ gmap: vec![vec![self.width*self.height+1; self.width]; self.height] };
            let mut nearest_pt_dist = GMap{ gmap: vec![vec![self.width+self.height+1; self.width]; self.height] };
            for d in 0..(self.width+1) {
                for d_idx in 0..nd {
                    if open[d_idx] {
                        let mut update = false;
                        for p in self.tiles_at_dist(pts[d_idx], d) {
                            if *nearest_pt_idx.get(p) >= self.width*self.height+1 {
                                *nearest_pt_idx.get_mut(p) = d_idx;
                                *nearest_pt_dist.get_mut(p) = d;
                                update = true;
                            }
                        }
                        open[d_idx] = update;
                    }
                }
            }
            (nearest_pt_idx, nearest_pt_dist)
        }   
    }

    pub fn nearest_drop_pos(&self, pos: Point) -> Point{
        let idx = *self.nearest_drop_pt_idx.get(pos);
        assert!(idx < self.my_drop_pts.len());
        self.my_drop_pts[idx]
    }

    pub fn normalize(&self, pos: Point) -> Point {
        let width: isize = self.width as isize;
        let height: isize = self.height as isize;
        let x = ((pos.x % width) + width) % width;
        let y = ((pos.y % height) + height) % height;
        Point {x, y}
    }

    pub fn is_occupied(&self, pos: Point) -> bool {
        self.ship_map.contains_key(&pos)
    }

    pub fn halite_at(&self, pos: Point) -> usize {
        *self.halite_map.get(pos)
    }

    pub fn dist(&self, pos1: Point, pos2: Point) -> usize {
        let dx = (pos1.x - pos2.x).abs() as usize;
        let dy = (pos1.y - pos2.y).abs() as usize;
        let toroidal_dx = min(dx, self.width-dx);
        let toroidal_dy = min(dy, self.height-dy);
        toroidal_dx + toroidal_dy
    }

    pub fn tiles_at_dist(&self, pos: Point, dist: usize) -> Vec<Point> {
        // assumes width == height and both are even
        if dist == 0 {
            vec![pos]
        } else if dist < self.width/2 {
            (0..(dist as isize)).map(|d|
                self.normalize(Point{
                    x: pos.x + (dist as isize) - d, 
                    y: pos.y + d}))
            .chain((0..(dist as isize)).map(|d|
                self.normalize(Point{
                    x: pos.x - d, 
                    y: pos.y + (dist as isize) - d})))
            .chain((0..(dist as isize)).map(|d|
                self.normalize(Point{
                    x: pos.x - (dist as isize) + d, 
                    y: pos.y - d})))
            .chain((0..(dist as isize)).map(|d|
                self.normalize(Point{
                    x: pos.x + d, 
                    y: pos.y - (dist as isize) + d})))
            .collect()
        } else if dist == self.width/2 {
            (0..(dist as isize)).map(|d|
                self.normalize(Point{
                    x: pos.x + (dist as isize) - d, 
                    y: pos.y + d}))
            .chain((0..(dist as isize)).map(|d|
                self.normalize(Point{
                    x: pos.x - d, 
                    y: pos.y + (dist as isize) - d})))
            .chain((1..(dist as isize)).map(|d|
                self.normalize(Point{
                    x: pos.x - (dist as isize) + d, 
                    y: pos.y - d})))
            .chain((1..(dist as isize)).map(|d|
                self.normalize(Point{
                    x: pos.x + d, 
                    y: pos.y - (dist as isize) + d})))
            .collect()
        } else if dist <= self.width {
            let antipode = self.normalize(Point {
                x: pos.x + (self.width as isize)/2,
                y: pos.y + (self.height as isize)/2,
            });
            self.tiles_at_dist(antipode, self.width - dist)
        } else {
            vec![]
        }
    }

    pub fn tiles_within_dist(&self, pos: Point, dist: usize) -> Vec<Point> {
        let mut tiles: Vec<Point> = Vec::new();
        for d in 0..(dist+1) {
            tiles.extend(self.tiles_at_dist(pos, d));
        }
        tiles
    }

    pub fn tiles_between_dist(&self, pos: Point, dist1: usize, dist2: usize) -> Vec<Point> {
        let mut tiles: Vec<Point> = Vec::new();
        for d in dist1..(dist2+1) {
            tiles.extend(self.tiles_at_dist(pos, d));
        }
        tiles
    }

    pub fn num_tiles_within_dist(&self, dist: usize) -> usize {
        2*dist*(dist+1)+1
    }

    pub fn step_toward(&self, pos: Point, d: Direction) -> Point {
        let (dx, dy) = match d {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
            Direction::Still => (0, 0),
        };

        self.normalize(Point { x: pos.x + dx, y: pos.y + dy })
    }

    pub fn neighbors(&self, pos: Point) -> Vec<Point> {
        Direction::adjacent_directions().iter().map(|&d| self.step_toward(pos, d)).collect()
    }

    pub fn neighborhood(&self, pos: Point) -> Vec<Point> {
        Direction::all_directions().iter().map(|&d| self.step_toward(pos, d)).collect()
    }

    pub fn navigate_naive(&self, start: Point, target: Point) -> (Point, Direction) {
        let mut best_score = 100000;
        let mut best_direction = Direction::Still;
        let mut best_pos = start;
        for d in Direction::all_directions() {
            let new_pos = self.step_toward(start, d);
            let mut score = self.dist(new_pos, target);
            if score < best_score {
                best_score = score;
                best_direction = d;
                best_pos = new_pos;
            }
        }
        (best_pos, best_direction)
    }

    pub fn navigate_no_collide(&self, start: Point, target: Point, forbidden: &mut HashSet<Point>) -> (Point, Direction) {
        let mut best_score = 100000;
        let mut best_direction = Direction::Still;
        let mut best_pos = start;
        for d in Direction::all_directions() {
            let new_pos = self.step_toward(start, d);
            let mut score = self.dist(new_pos, target);
            if forbidden.contains(&new_pos) {
                score += 1000;
            }
            if score < best_score {
                best_score = score;
                best_direction = d;
                best_pos = new_pos;
            }
        }
        forbidden.insert(best_pos);
        (best_pos, best_direction)
    }

    pub fn halite_between2(&self, start: Point, goal: Point, h_fn: &impl Fn(&usize) -> usize) -> usize {
        let width: isize = self.width as isize;
        let height: isize = self.height as isize;
        // let x = ((pos.x % width) + width) % width;
        // let y = ((pos.y % height) + height) % height;

        if start == goal {
            return 0;
        }

        let (x_wrap, y_wrap, rightwards, downwards);

        let x1 = min(start.x, goal.x) as usize;
        let x2 = max(start.x, goal.x) as usize;
        let y1 = min(start.y, goal.y) as usize;
        let y2 = max(start.y, goal.y) as usize;

        let x_len = min(x2-x1, self.width-(x2-x1))+1;
        let y_len = min(y2-y1, self.height-(y2-y1))+1;

        let no_dx = start.x == goal.x;
        let no_dy = start.y == goal.y;
        if start.x > goal.x {
            rightwards = start.x-goal.x > width/2;
            x_wrap = rightwards;
        } else {
            rightwards = goal.x-start.x < width/2;
            x_wrap = !rightwards;
        }
        if start.y > goal.y {
            downwards = start.y-goal.y > height/2;
            y_wrap = downwards;
        } else {
            downwards = goal.y-start.y < height/2;
            y_wrap = !downwards;
        }

        let sum: usize;

        if no_dy {
            sum = if x_wrap {
                self.halite_map.gmap[y1][x2..self.width]
                    .iter()
                    .map(h_fn)
                    .sum::<usize>() as usize +
                self.halite_map.gmap[y1][0..(x1+1)]
                    .iter()
                    .map(h_fn)
                    .sum::<usize>() as usize
            } else {
                self.halite_map.gmap[y1][x1..(x2+1)]
                    .iter()
                    .sum::<usize>() as usize
            };
        } else if no_dx {
            sum = if y_wrap {
                self.halite_map.gmap[y2..self.height]
                    .iter()
                    .map(|s| h_fn(&s[x1]))
                    .sum::<usize>() as usize +
                self.halite_map.gmap[0..(y1+1)]
                    .iter()
                    .map(|s| h_fn(&s[x1]))
                    .sum::<usize>() as usize
            } else {
                self.halite_map.gmap[y1..(y2+1)]
                    .iter()
                    .map(|s| h_fn(&s[x1]))
                    .sum::<usize>() as usize
            };   
        } else {
            let x_idx = get_wrap_idx(x1, x2, x_wrap, !rightwards, self.width);
            let y_idx = get_wrap_idx(y1, y2, y_wrap, !downwards, self.height);
            assert_eq!(x_idx.len(), x_len);
            assert_eq!(y_idx.len(), y_len);

            let mut sum_so_far = vec![vec![0; x_len]; y_len];
            sum_so_far[0][0] = h_fn(&self.halite_map.gmap[y_idx[0]][x_idx[0]]);
            for xi in 1..x_len {
                sum_so_far[0][xi] = sum_so_far[0][xi-1] + h_fn(&self.halite_map.gmap[y_idx[0]][x_idx[xi]]);
            }
            for yi in 1..y_len {
                sum_so_far[yi][0] = sum_so_far[yi-1][0] + h_fn(&self.halite_map.gmap[y_idx[yi]][x_idx[0]]);
            }
            for xi in 1..x_len {
                for yi in 1..y_len {
                    sum_so_far[yi][xi] = h_fn(&self.halite_map.gmap[y_idx[yi]][x_idx[xi]]) +
                        min(sum_so_far[yi][xi-1], sum_so_far[yi-1][xi]);
                }
            }
            sum = sum_so_far[y_len-1][x_len-1];
        }
        sum - h_fn(&self.halite_at(start)) - h_fn(&self.halite_at(goal))
    }

    pub fn halite_between(&self, start: Point, goal: Point) -> usize {
        if start == goal {
            return 0;
        }
        let k: isize = 100000;
        let mut frontier = BinaryHeap::new(); // open set
        let mut explored = HashSet::new(); // closed set
        frontier.push(CellPriority{ pos: start, w: 0 }); // start nav from target square

        let mut came_from: HashMap<Point, Option<Point>> = HashMap::new();
        let mut cost_so_far: HashMap<Point, isize> = HashMap::new();
        came_from.insert(start, None);
        cost_so_far.insert(start, 0);

        while !frontier.is_empty() {
            let current = frontier.pop().unwrap().pos;

            // terminate when have assigned priorities to every square in the neighborhood
            if current == goal {
                break;
            }

            if !explored.contains(&current) {
                for next in self.neighbors(current) {
                    let new_cost = cost_so_far.get(&current).unwrap() + k + self.halite_at(next) as isize;
                    if !cost_so_far.contains_key(&next) || new_cost < *cost_so_far.get(&next).unwrap() {
                        cost_so_far.insert(next, new_cost);
                        let cost_to_goal = k*self.dist(start, next) as isize;
                        let total_cost = new_cost + cost_to_goal;
                        frontier.push(CellPriority{ pos: next, w: total_cost });
                            // RIP - problem: what if already exists CellPriority { pos: next, w: different_cost } ?
                        came_from.insert(next, Some(current));
                    }
                }

                explored.insert(current);
            } else {
                // info!("nav from {:?} to {:?}, {:?} already in closed set", start, target, current); // this happens quite frequently
            }
        }

        let mut sum = 0;
        let mut node = goal;
        loop {
            node = came_from.get(&node).unwrap().unwrap();
            if node == start {
                break;
            }
            sum += self.halite_at(node);
        }
        sum
    }

        // figure which of the five squares in the neighborhood of start to go to
    pub fn backwards_a_star_scores(&self, start: Point, target: Point, k: isize) -> Vec<(Point, Direction, isize)> {
        let mut frontier = BinaryHeap::new(); // open set
        let mut explored = HashSet::new(); // closed set
        frontier.push(CellPriority{ pos: target, w: 0 }); // start nav from target square

        let mut came_from: HashMap<Point, Option<Point>> = HashMap::new();
        let mut cost_so_far: HashMap<Point, isize> = HashMap::new();
        came_from.insert(target, None);
        cost_so_far.insert(target, 0);

        let start_neighborhood0 = Vec::from_iter(self.neighborhood(start));
        let mut start_neighborhood: HashSet<Point> = HashSet::from_iter(self.neighborhood(start));

        while !frontier.is_empty() && !start_neighborhood.is_empty() {
            let current = frontier.pop().unwrap().pos;

            // terminate when have assigned priorities to every square in the neighborhood

            if !explored.contains(&current) {
                for next in self.neighbors(current) {
                    let new_cost = cost_so_far.get(&current).unwrap() + k + self.halite_at(next) as isize;
                    if !cost_so_far.contains_key(&next) || new_cost < *cost_so_far.get(&next).unwrap() {
                        cost_so_far.insert(next, new_cost);
                        let cost_to_goal = k*self.dist(start, next) as isize;
                        let total_cost = new_cost + cost_to_goal;
                        frontier.push(CellPriority{ pos: next, w: total_cost });
                            // RIP - problem: what if already exists CellPriority { pos: next, w: different_cost } ?
                        came_from.insert(next, Some(current));
                    }
                }

                explored.insert(current);
                start_neighborhood.remove(&current);
            } else {
                // info!("nav from {:?} to {:?}, {:?} already in closed set", start, target, current); // this happens quite frequently
            }
        }

        let mut scores = Vec::new();
        for m in 0..5 {
            scores.push((start_neighborhood0[m], Direction::all_directions()[m],
                *cost_so_far.get(&start_neighborhood0[m]).unwrap()));
        }

        return scores;
    }

    // old
    // figure which of the five squares in the neighborhood of start to go to
    pub fn backwards_a_star(&self, start: Point, target: Point, forbidden: &HashSet<Point>) -> (Point, Direction) {
        let k: isize = 100000;
        let mut frontier = BinaryHeap::new(); // open set
        let mut explored = HashSet::new(); // closed set
        frontier.push(CellPriority{ pos: target, w: 0 }); // start nav from target square

        let mut came_from: HashMap<Point, Option<Point>> = HashMap::new();
        let mut cost_so_far: HashMap<Point, isize> = HashMap::new();
        came_from.insert(target, None);
        cost_so_far.insert(target, 0);

        let mut start_neighborhood: HashSet<Point> = HashSet::from_iter(self.neighborhood(start));

        while !frontier.is_empty() && !start_neighborhood.is_empty() {
            let current = frontier.pop().unwrap().pos;

            // terminate when have assigned priorities to every square in the neighborhood

            if !explored.contains(&current) {
                for next in self.neighbors(current) {
                    let new_cost = cost_so_far.get(&current).unwrap() + k + self.halite_at(next) as isize;
                    if !cost_so_far.contains_key(&next) || new_cost < *cost_so_far.get(&next).unwrap() {
                        cost_so_far.insert(next, new_cost);
                        let cost_to_goal = k*self.dist(start, next) as isize;
                        let total_cost = new_cost + cost_to_goal;
                        frontier.push(CellPriority{ pos: next, w: total_cost });
                            // RIP - problem: what if already exists CellPriority { pos: next, w: different_cost } ?
                        came_from.insert(next, Some(current));
                    }
                }

                explored.insert(current);
                start_neighborhood.remove(&current);
            } else {
                // info!("nav from {:?} to {:?}, {:?} already in closed set", start, target, current); // this happens quite frequently
            }
        }

        let mut best_score = isize::MAX;
        let mut best_direction = Direction::Still;
        let mut best_pos = start;
        for d in Direction::all_directions() {
            let new_pos = self.step_toward(start, d);
            let mut score = *cost_so_far.get(&new_pos).unwrap();
            if forbidden.contains(&new_pos) {
                score += k*1000;
            }
            if start == target {
                score -= 4*self.halite_at(new_pos) as isize;
            }
            if score < best_score {
                best_score = score;
                best_direction = d;
                best_pos = new_pos;
            }
        }
        (best_pos, best_direction)
    }
}

pub fn get_wrap_idx(c1: usize, c2: usize, wrap: bool, reverse: bool, max_dim: usize) -> Vec<usize> {
    let mut idx: Vec<usize>;
    if wrap {
        idx = (c2..max_dim).collect();
        idx.extend(0..(c1+1));
    } else {
        idx = (c1..(c2+1)).collect();
    }
    if reverse {
        idx.reverse();
    }
    idx
}

#[derive(Debug)]
pub struct GMap<T> {
    pub gmap: Vec<Vec<T>>,
}

impl <T> GMap<T> {
    pub fn get(&self, pos: Point) -> &T {
        &self.gmap[pos.y as usize][pos.x as usize]
    }

    pub fn get_idx(&self, x: usize, y: usize) -> &T {
        &self.gmap[y][x]
    }

    pub fn get_mut(&mut self, pos: Point) -> &mut T {
        &mut self.gmap[pos.y as usize][pos.x as usize]
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPriority {
    pub pos: Point,
    pub w: isize,
}

impl Ord for CellPriority {
    fn cmp(&self, other: &CellPriority) -> Ordering {
        other.w.cmp(&self.w) // so that smaller numbers go first
    }
}

impl PartialOrd for CellPriority {
    fn partial_cmp(&self, other: &CellPriority) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPriorityMax {
    pub pos: Point,
    pub w: isize,
}

impl Ord for CellPriorityMax {
    fn cmp(&self, other: &CellPriorityMax) -> Ordering {
        self.w.cmp(&other.w) // so that smaller numbers go first
    }
}

impl PartialOrd for CellPriorityMax {
    fn partial_cmp(&self, other: &CellPriorityMax) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}