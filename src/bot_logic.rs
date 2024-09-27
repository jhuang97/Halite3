use game::{Game, CellPriorityMax, GMap};
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::BinaryHeap;
use std::iter::FromIterator;
use data::{ShipCommand, Direction, Point, Ship};
use std::cmp::min;
use std::cmp::Ordering;
use std::f32;
// use pathfinding::matrix::Matrix;
// use pathfinding::kuhn_munkres::kuhn_munkres;
use disjoint_set::DisjointSet;

pub struct Logic {
	ship_goal_types: HashMap<usize, GoalType>,
	ship_turns_stuck: HashMap<usize, usize>,
	ship_prev_pos: HashMap<usize, Point>,
	endgame: bool,
	saving_for_dropoff: bool,
	dropoff_candidates: Vec<DropoffCandidate>,
	// temp_vmap: TempVMap,
	// temp_vmap_valid: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GoalType {
    TowardsMine, Mine, Deposit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Goal {
	pos: Point,
}

pub struct DropoffCandidate {
	center: Point,
	dist: usize,
}

// pub struct TempVMap {
// 	drop_pts: Vec<Point>,
// 	pt_idx: GMap<usize>,
// 	pt_dist: GMap<usize>,
// }

// fn tile_cost(tile_halite: f32, d_tile_ship: f32, d_tile_fac: f32) -> f32 {
// 	let is_factory = d_tile_fac == 0.0;
// 	if tile_halite < 5.0 || is_factory {
// 		return 10000.0;
// 	} else {
// 		return 1.0/(tile_halite.min(1100.0) / (d_tile_ship + d_tile_fac));
// 	}
// }

fn miner_goal_cost(game: &Game, miner: &Ship, goal_pos: Point, goal_type: GoalType, drop_pos_f: impl Fn(Point) -> Point) -> f32 {
	let goal_halite = game.halite_at(goal_pos);
	let movement_cost = game.halite_at(miner.pos) as isize/10isize;
	let cost_of_stopping_mining = if goal_pos != miner.pos && goal_type == GoalType::Mine { 
			if game.halite_at(miner.pos) > game.constants.max_halite / 10 {
				10000.0*movement_cost as f32
			} else { game.halite_at(miner.pos) as f32*0.23 }
		}
		else {0.0};
	let drop_pos = drop_pos_f(goal_pos);
	let net_halite = goal_halite as f32*0.8
		// + game.tiles_at_dist(goal_pos, 1).iter().map(|&pt| game.halite_at(pt)).sum::<usize>() as f32*0.2
		// + game.tiles_at_dist(goal_pos, 2).iter().map(|&pt| game.halite_at(pt)).sum::<usize>() as f32*0.05
		- cost_of_stopping_mining
		- 0.10*game.halite_between2(miner.pos, goal_pos, &|&x| x) as f32
		- game.halite_between2(goal_pos, drop_pos, &|&x| x/10) as f32*0.2;
	let net_turns: f32 = 1.2*game.dist(miner.pos, goal_pos) as f32 + 0.9*game.dist(goal_pos, drop_pos) as f32 + 1.0;
	net_halite/(net_turns as f32).powf(1.4)
}

fn pick_goals(game: &Game, my_miners: &Vec<usize>, min_num_goals: usize, dist_map: &GMap<usize>) -> Vec<Goal> {
	let mut out: Vec<Goal> = Vec::new();

	let mut goal_order = BinaryHeap::new();

	for x in 0..game.width {
		for y in 0..game.height {
			let cell_pos = Point{x: x as isize, y: y as isize,};
			if !game.my_drop_pts.contains(&cell_pos) {
				let efficiency = game.halite_at(cell_pos) as f32/(1.0 + *dist_map.get(cell_pos) as f32);
				let rounded_efficiency = (1000000.0*efficiency) as isize;
				goal_order.push(CellPriorityMax {
					pos: cell_pos,
					w: rounded_efficiency,
				});
			}
		}
	}

	for id in my_miners {
		let s = game.ships.get(&id).unwrap();
		out.push(Goal{pos: s.pos}); // maybe add neighborhood? but too many goals...
	}
	// add the first min_num_goals from goal_order into out
	while let Some(g) = goal_order.pop() {
		if out.len() >= min_num_goals {
			break;
		}
		let goal = Goal{pos: g.pos};
		if !out.contains(&goal) {
			out.push(goal);
			if game.turn_number % 2 == 0 {
				warn!("{{\"t\": {}, \"x\": {}, \"y\": {}, \"msg\": \"goal\", \"color\": \"{}\"}},",
						game.turn_number, g.pos.x, g.pos.y, 
						"#990099");
			}
		}
	}

	// if there are not enough in goal_order, add copies of the latest dropoff
	while out.len() < min_num_goals {
		out.push(Goal{pos: game.my_drop_pts[game.my_drop_pts.len()-1]});
	}
	out
}

fn enemy_ship_position_prediction(game: &Game, e_ship: &Ship, e_id: &usize) -> Vec<(Point, f64)> {
	// order of all_directions should be N, E, S, W, Still
	let mut prbs = vec![0.0; 5]; // in same order
	let points = game.neighborhood(e_ship.pos);
	let mut neighbor_ships_id: Vec<Option<&Ship>> = vec![None; 4];
	for i in 0..4 {
		if let Some(id) = game.ship_map.get(&points[i]) {
			neighbor_ships_id[i] = game.ships.get(&id);
		}
	}

	if game.halite_at(e_ship.pos) as isize / 10 > e_ship.halite { // ship literally can't move
		prbs[4] = 1.0;
	} else {

		if e_ship.halite > 950 { // deposit going home
			prbs = vec![2.0,2.0,2.0,2.0,1.0];
			let (_, best_dir) = game.navigate_naive(e_ship.pos, game.factories[*e_id].pos);
			prbs[Direction::all_directions().iter().position(|&x| x == best_dir).unwrap()] *= 3.0;
		} else if game.halite_at(e_ship.pos) > 200 { // probably will stay and mine?
			prbs = vec![1.0,1.0,1.0,1.0, 6.0];
		} else { // travel mine?
			for k in 0..5 {
				prbs[k] = sigmoid1(game.halite_at(points[k]) as f64);
			}
			prbs[4] += 0.1;
		}
		for i in 0..4 {
			match neighbor_ships_id[i] {
				Some(_) => prbs[i]*=0.03*(e_ship.halite as f64/1000.0 + 0.2),
				None => (),
			}
		}
	}
	let n_prbs: Vec<f64> = prbs.iter()
					.map(|&x| x/prbs.iter().sum::<f64>())
					.collect();
	return points.iter().cloned()
		.zip(n_prbs.iter().cloned())
		.collect();
}

fn enemy_position_prediction(game: &Game) -> HashMap<Point, usize> {
	let mut prb_map: HashMap<Point, f64> = HashMap::new();
	for pid in 0..game.num_players {
		if pid != game.my_pid {
			for ship_id in &game.ship_id_by_player[pid] {
				let ship = game.ships.get(&ship_id).unwrap();
				let ship_predictions = enemy_ship_position_prediction(&game, &ship, &pid);
				for (pos, prb) in ship_predictions {
					if prb > 0.0 {
						if prb_map.contains_key(&pos) {
							let prb2 = *prb_map.get(&pos).unwrap();
							prb_map.insert(pos, 1.0 - (1.0-prb)*(1.0-prb2));
						} else {
							prb_map.insert(pos, prb);
						}
					}
				}
			}
		}
	}

	let prob_uint: HashMap<Point, usize> = prb_map.iter()
		.map(|(k, v)| (*k, (100.0*v) as usize)).collect();
	return prob_uint;
}

impl Logic {
	pub fn new() -> Logic {
		Logic {
			ship_goal_types: HashMap::new(),
			endgame: false,
			saving_for_dropoff: false,
			dropoff_candidates: Vec::new(),
			ship_turns_stuck: HashMap::new(),
			ship_prev_pos: HashMap::new(),
			// temp_vmap: TempVMap {
			// 	drop_pts: vec![],
			// 	pt_idx: GMap{ gmap: vec![vec![]]},
			// 	pt_dist: GMap{ gmap: vec![vec![]]},
			// },
			// temp_vmap_valid: false,
		}
	}

	fn add_dropoff_candidates(&mut self, game: &Game, v_goals: Vec<Point>, dropoff_spacing: usize) {
		// get unique list of goal points
		let goals_set: HashSet<Point> = HashSet::from_iter(v_goals.iter().cloned());
		let goal_pos: Vec<Point> = Vec::from_iter(goals_set.iter().cloned());

		let dc_pos: Vec<Point> = self.dropoff_candidates.iter().map(|dc| dc.center).collect();

		let num_pos = goal_pos.len();
		if num_pos >= 3 {
			// find clusters
			let mut ds = DisjointSet::make_singletons(num_pos);
			for i in 0..(num_pos-1) {
				for j in (i+1)..num_pos {
					if game.dist(goal_pos[i], goal_pos[j]) <= 2 {
						ds.unite(i, j);
					}
				}
			}

			let mut groups: HashMap<usize, Vec<Point>> = HashMap::new();
			for i in 0..ds.size {
				let group_id = ds.parent[i];
				if !groups.contains_key(&group_id) {
					groups.insert(group_id, vec![goal_pos[i]]);
				} else {
					(*groups.get_mut(&group_id).unwrap())
						.push(goal_pos[i]);
				}
			}

			// find center/extent of group
			for group in groups.values() {
				if group.len() >= 3 {
					let mut center = group[0];
					let mut min_dist_total: usize = 1000;

					for i in 0..group.len() {
						let mut dist_total = 0;
						for j in 0..group.len() {
							dist_total += game.dist(group[i], group[j]);
						}
						if dist_total < min_dist_total {
							center = group[i];
							min_dist_total = dist_total;
						}
					}

					if !dc_pos.contains(&center) {
						let dist = min(min_dist_total/group.len() + 2, 5);
						let nearby_halite = game.tiles_within_dist(center, 5).iter()
								.map(|&p| game.halite_at(p))
								.sum::<usize>();
						let nearby_halite_density = nearby_halite as f32/game.num_tiles_within_dist(5) as f32;
						let drop_dist = *game.nearest_drop_pt_dist.get(center);

						warn!("{{\"t\": {}, \"x\": {}, \"y\": {}, \"msg\": \"dc[r {}, dist {}, n_h_density {}]\", \"color\": \"{}\"}},",
							game.turn_number, center.x, center.y, dist, drop_dist,
							format!("{:.1}", nearby_halite_density),
							"#D0D000");

						if nearby_halite_density + 3.0*drop_dist as f32 > 310.0 && drop_dist >= dropoff_spacing {
							self.dropoff_candidates.push(DropoffCandidate {
								center,	dist,
							});
						}
					}
				}
			}
		}
	}

	pub fn make_moves(&mut self, game: &Game) -> (bool, HashMap<usize, ShipCommand>) {
		let me = game.my_pid;
		let mut my_halite = game.energy[me];
		let my_ships_ids = &game.ship_id_by_player[me];
		let my_factory = &game.factories[me];
		let turns_left = game.constants.max_turns - game.turn_number;
		let dropoff_spacing = if game.num_players == 4 && game.width <= 32 {11} else {15};
		let mut endgame_margin =
			if game.num_players == 4 {
				if game.width >= 40 {
					6
				} else {
					5
				}
			} else {
				3
			};
		if game.turn_number == game.constants.max_turns - game.width*2 {
			if my_ships_ids.len() > 50 {
				endgame_margin += 3;
			} else if my_ships_ids.len() > 30 {
				endgame_margin += 2;
			}
		}
		let stop_spawn_margin =
			if game.num_players == 4 {
				if game.width >= 64 {
					250
				} else if game.width >= 48 {
					225
				} else if game.width >= 40 {
					225
				} else {
					225
				}
			} else {
				200
			};

		let enemy_forecast = enemy_position_prediction(&game);
		// info!("enemy forecast: {:?}", enemy_forecast);

		let mut commands: HashMap<usize, ShipCommand> = HashMap::new();

		self.ship_turns_stuck.retain(|&id, _| my_ships_ids.contains(&id));
		self.ship_prev_pos.retain(|&id, _| my_ships_ids.contains(&id));
		for &id in my_ships_ids {
			if !self.ship_prev_pos.contains_key(&id) {
				self.ship_turns_stuck.insert(id, 0);
			} else {
				let ship = game.ships.get(&id).unwrap();
				let mut e_ship_near = false;
				for p in game.tiles_between_dist(ship.pos, 1, 2) {
					if let Some(&o_id) = game.ship_map.get(&p) {
						if game.ships.get(&o_id).unwrap().player != me {
							e_ship_near = true;
							break;
						}
					}
				}
				if ship.pos == *self.ship_prev_pos.get(&id).unwrap() && e_ship_near {
					if !self.ship_turns_stuck.contains_key(&id) {
						self.ship_turns_stuck.insert(id, 1);
					} else {
						(*self.ship_turns_stuck.get_mut(&id).unwrap()) += 1;
					}
				} else {
					self.ship_turns_stuck.insert(id, 0);
				}
			}
		}

		for (&id, &turns) in &self.ship_turns_stuck {
			if turns >= 1 {
				info!("ship {} stuck for {} turns", id, turns);
			}
		}

		// info!("is there ship on spawn? {}", game.is_occupied(my_factory.pos));

		// handle dropoffs
		// ===============
		let mut halite_densities: HashMap<Point, f32> = HashMap::new();
		self.dropoff_candidates.retain(|dc| {
			let nearby_halite = game.tiles_within_dist(dc.center, 5).iter()
							.map(|&p| game.halite_at(p))
							.sum::<usize>();
			let nearby_halite_density = nearby_halite as f32/game.num_tiles_within_dist(5) as f32;
			halite_densities.insert(dc.center, nearby_halite_density);
			nearby_halite_density > 150.0 && *game.nearest_drop_pt_dist.get(dc.center) >= dropoff_spacing
		}); // also that enemy is not outcrowding the dropoff site?

		let mut dropoff_str = String::new();

		for dc in &self.dropoff_candidates {
			dropoff_str.push_str(&format!("{{\"t\": {}, \"x\": {}, \"y\": {}, \"msg\": \"dc[r {}, dist {}, n_h_density {}]\", \"color\": \"{}\"}},",
						game.turn_number, dc.center.x, dc.center.y, dc.dist, *game.nearest_drop_pt_dist.get(dc.center),
						format!("{:.1}", halite_densities.get(&dc.center).unwrap()),
						"#D000D0"));
		}

		self.saving_for_dropoff = !self.dropoff_candidates.is_empty() && my_ships_ids.len() > 15
			&& game.turn_number < game.constants.max_turns - (game.width*3/2);

		let mut dropoff_ship_id = 10000;
		if my_halite >= game.constants.dropoff_cost && self.saving_for_dropoff {
			let mut best_score = -1.0;

			for &id in my_ships_ids {
				let ship = game.ships.get(&id).unwrap();
				if *game.nearest_drop_pt_dist.get(ship.pos) >= dropoff_spacing // ship has to be far enough
					&& !game.enemy_drop_pts.contains_key(&ship.pos) {
					for dc in &self.dropoff_candidates {
						if game.dist(ship.pos, dc.center) <= dc.dist { // ship has to be close to the dropoff candidate
							let score = halite_densities.get(&dc.center).unwrap()
								+ 3.0 * *game.nearest_drop_pt_dist.get(ship.pos) as f32; 
							if score > best_score {
								best_score = score;
								dropoff_ship_id = id;
							}
						}
					}
				}
			}

			// actually assign the ship to build the dropoff
			if dropoff_ship_id != 10000 {
				commands.insert(dropoff_ship_id, ShipCommand::MakeDropoff());
				my_halite -= game.constants.dropoff_cost;
				self.saving_for_dropoff = false;
			}
		}

		if self.saving_for_dropoff {
			info!("saving for dropoff on t {}", game.turn_number);
		// 	if !self.temp_vmap_valid || self.temp_vmap.drop_pts[self.temp_vmap.drop_pts.len()-1] != self.dropoff_candidates[0].center {
		// 		let mut drop_pts = game.my_drop_pts.to_vec();
		// 		drop_pts.push(self.dropoff_candidates[0].center);
		// 		let vmap = game.make_vmaps(&drop_pts);
		// 		self.temp_vmap = TempVMap {
		// 			drop_pts,
		// 			pt_idx: vmap.0,
		// 			pt_dist: vmap.1,
		// 		};
		// 	}
		// } else {
		// 	self.temp_vmap_valid = false;
		}

		// update ship lists
		// =================
		let mut my_immovable: Vec<usize> = Vec::new();
		let mut my_movable: Vec<usize> = Vec::new();

		self.ship_goal_types.retain(|&id, _| my_ships_ids.contains(&id));

		for id in my_ships_ids {
			if *id != dropoff_ship_id {
				if !self.ship_goal_types.contains_key(id) { // new ship?
					self.ship_goal_types.insert(*id, GoalType::Mine);
				}

				let ship = game.ships.get(id).unwrap();
				if game.halite_at(ship.pos) as isize / 10 > ship.halite {
					commands.insert(*id, ShipCommand::MoveShip(Direction::Still));
					my_immovable.push(*id);
				} else {
					my_movable.push(*id);
				}
			}
		}

		// update GoalType for each ship
		let mut targets: HashMap<usize, Point> = HashMap::new();
		for id in &my_movable {
			let ship = game.ships.get(&id).unwrap();
			// let dist_to_fac = game.dist(my_factory.pos, ship.pos);
			let dist_to_drop = game.nearest_drop_pt_dist.get(ship.pos);
			let endgame_collect = dist_to_drop+endgame_margin >= turns_left;
			if endgame_collect && !self.endgame {
				self.endgame = true;
				info!("endgame: begin cashing out all ships");
			}

			match *self.ship_goal_types.get(id).unwrap() {
				GoalType::Deposit => {
					if game.my_drop_pts.contains(&ship.pos) && !self.endgame {
						self.ship_goal_types.insert(*id, GoalType::TowardsMine);
					}
				},
				GoalType::TowardsMine => {
					if ship.halite >= 950 || endgame_collect {
						self.ship_goal_types.insert(*id, GoalType::Deposit);	
					} else if game.halite_at(ship.pos) > game.constants.max_halite / 10 {
						self.ship_goal_types.insert(*id, GoalType::Mine);
					}
				},
				GoalType::Mine => {
					if ship.halite >= 950 || endgame_collect {
						self.ship_goal_types.insert(*id, GoalType::Deposit);	
					} else if game.halite_at(ship.pos) <= game.constants.max_halite / 10 {
						self.ship_goal_types.insert(*id, GoalType::TowardsMine);	
					}
				},
			}
		}

		let my_miners: Vec<usize> = my_movable.iter()
			.filter(|id| *self.ship_goal_types.get(id).unwrap() != GoalType::Deposit)
			.cloned()
			.collect();

		// list the most efficient squares for mining
		let picked_goals = pick_goals(game, &my_miners, 4*my_ships_ids.len()+20, &game.nearest_drop_pt_dist);
		// let mut goal_str = "Goals: ".to_owned();
		// for g in &picked_goals {
		// 	goal_str.push_str(&format!("({},{}) ", g.pos.x, g.pos.y));
		// }
		// info!("{}", goal_str);

		// assign miners to objectives
		info!("Miners: {:?}", my_miners);

		// if !my_movable.is_empty() {
		// 	let id = my_movable.get(0).unwrap();
		// 	let ship = game.ships.get(&id).unwrap();
		// 	info!("ship {} at ({},{}):", id, ship.pos.x, ship.pos.y);
		// 	for k in 0..min(5, picked_goals.len()) {
		// 		let g = picked_goals.get(k).unwrap();
		// 		let h1 = game.halite_between(ship.pos, g.pos);
		// 		let h2 = game.halite_between2(ship.pos, g.pos);
		// 		info!("...halite between ({},{}): {} ?= {}", g.pos.x, g.pos.y, h1, h2);
		// 		if h1 != h2 {
		// 			info!("!!!!!!!!!!!!!!!!!! halite between mismatch")
		// 		}
		// 	}
		// }

		if !my_miners.is_empty() {
			// make table, with my_miners as the rows and picked_goals as the columns
			let mut weights = vec![vec![-1.0; my_miners.len()]; picked_goals.len()];
			// let mut fweights = vec![-1.0; picked_goals.len()*my_miners.len()];
			// info!("weights: ");
			for i in 0..my_miners.len() {
				let id = my_miners[i];
				let ship = game.ships.get(&id).unwrap();
				for j in 0..picked_goals.len() {
					let cell_pos = picked_goals[j].pos;
					weights[j][i] = miner_goal_cost(game, ship,
						cell_pos, *self.ship_goal_types.get(&id).unwrap(),
							|p| game.nearest_drop_pos(p));
					// fweights[i*picked_goals.len() + j] = weights[i*picked_goals.len() + j] as f32 / 1000000.0;
				}
				// info!("weights: {:?}", &fweights[i*picked_goals.len()..(i+1)*picked_goals.len()]);
			}

			let mut miner_idx_wo_actions: HashSet<usize> = HashSet::from_iter(0..my_miners.len());
			let mut rem_goals_idx: HashSet<usize> = HashSet::from_iter(0..picked_goals.len());

			// let mut assignments: Vec<usize> = vec![0; my_miners.len()];
			let mut picked_goals_pos: Vec<Point> = Vec::new();

			while miner_idx_wo_actions.len() > 0 {
				let mut best_ship_idx = 10000;
				let mut best_goal_idx = 100000;
				let mut best_weight = -10000.0;

				for &goal_idx in rem_goals_idx.iter() {
					// let goal_pos = picked_goals[goal_idx].pos;
					for &miner_idx in miner_idx_wo_actions.iter() {
						// let id = my_miners[miner_idx];
						// let ship = game.ships.get(&id).unwrap();

						let a_weight = weights[goal_idx][miner_idx];

						if a_weight > best_weight {
							best_weight = a_weight;
							best_ship_idx = miner_idx;
							best_goal_idx = goal_idx;
						}
					}
				}

				let goal_pos = picked_goals[best_goal_idx].pos;
				let id = my_miners[best_ship_idx];
				let ship = game.ships.get(&id).unwrap();

				miner_idx_wo_actions.remove(&best_ship_idx);
				rem_goals_idx.remove(&best_goal_idx);

				info!("ship {}({}, {}) -> ({}, {})", 
					ship.ship_id, ship.pos.x, ship.pos.y, goal_pos.x, goal_pos.y);
				warn!("{{\"t\": {}, \"x\": {}, \"y\": {}, \"msg\": \"ship {}\", \"color\": \"{}\"}},",
					game.turn_number, goal_pos.x, goal_pos.y, id,
					if ship.pos == goal_pos { "#0000DD" } else { "#00DD00" });
				targets.insert(id, goal_pos);
				self.ship_goal_types.insert(id,
					if ship.pos == goal_pos { GoalType::Mine } else { GoalType::TowardsMine });
				if ship.pos != goal_pos && !game.my_drop_pts.contains(&goal_pos) {
					picked_goals_pos.push(goal_pos);
				}
			}

			self.add_dropoff_candidates(game, picked_goals_pos, dropoff_spacing);
		}

		warn!("{}", dropoff_str);

		let mut move_order = BinaryHeap::new();
		for id in &my_movable{
			let ship = game.ships.get(&id).unwrap();
			let drop_pos = game.nearest_drop_pos(ship.pos);
			if *self.ship_goal_types.get(id).unwrap() == GoalType::Deposit {
				targets.insert(*id, drop_pos); // go home
				info!("ship {} at ({},{}) going back to base", id, ship.pos.x, ship.pos.y);
			}
			let mut priority = 3*game.dist(ship.pos, drop_pos) as isize;
			if ship.pos == drop_pos {
				priority -= 300;
			}
			if *self.ship_goal_types.get(id).unwrap() == GoalType::Deposit {
				priority -= 300;
				priority -= ship.halite/10;
			} else if *self.ship_goal_types.get(id).unwrap() == GoalType::Mine {
				priority -= 150;
				priority -= game.halite_at(ship.pos) as isize/20;
			} else if *self.ship_goal_types.get(id).unwrap() == GoalType::TowardsMine {
				priority -= game.halite_at(ship.pos) as isize/20;
				priority += game.dist(ship.pos, *targets.get(&id).unwrap()) as isize;
			}
			move_order.push(ShipPriority{
					id: *id,
					w: priority,
				});
			// info!("ship at: {:?}, target: {:?}\n", ship.pos, targets.get(id).unwrap());
		}

		let immovable_pos: HashSet<Point> = my_immovable.iter()
											.map(|&id| game.ships.get(&id).unwrap().pos)
											.collect();
		let mut forbidden: HashSet<Point> = HashSet::from_iter(immovable_pos.iter().cloned());
		if self.endgame {
			forbidden.retain(|&p| !game.my_drop_pts.contains(&p));
		}

		let mut movable_next: HashMap<Point, usize> = HashMap::new();
		let mut colliding_ships: HashSet<usize> = HashSet::new();

		let mut move_scores: HashMap<usize, Vec<(Point, Direction, f32)>> = HashMap::new();
		let mut o_directions: HashMap<usize, Direction> = HashMap::new();  // where does the ship want to go, if self-collisions with others of my movable ships were not a problem?
		let k: isize = 100000;
		while let Some(ShipPriority{ id, w: _ }) = move_order.pop() {
			let ship = game.ships.get(&id).unwrap();
			// info!("ship {} goes here, forbidden: {:?}\n", id, &forbidden);
			let target = *targets.get(&id).unwrap();
			let nav_scores = game.backwards_a_star_scores(ship.pos, *targets.get(&id).unwrap(), k);

			let mut best_score = 1000000000000.0;
	        let mut direction = Direction::Still;
	        let mut best_pos = ship.pos;

	        let mut o_best_score = best_score;
	        let mut o_direction = Direction::Still;

	        let mut forbidden_count = 0;
	        let mut ship_move_scores: Vec<(Point, Direction, f32)> = Vec::new();
	        for (pos, d, nscore) in nav_scores {
	        	let mut score = nscore as f32;
	        	let mut o_score = 0.0;
	            if forbidden.contains(&pos) {
	                score += k as f32*1000.0; // + ship.halite;
	                // if movable_next.contains_key(&pos) { // doesn't make that much of a difference
	                // 	score += -1001 + game.ships.get(movable_next.get(&pos).unwrap()).unwrap().halite;
	                // }
	                if !immovable_pos.contains(&pos) {
	                	o_score -= k as f32*1000.0; // don't care about self-collisions with others of my movable ships
	                }
	                forbidden_count += 1;
	            } else if game.num_players == 4 && enemy_forecast.contains_key(&pos) {
	            	let dist_to_dropoff = *game.nearest_drop_pt_dist.get(ship.pos);
	            	let mut factor: f32 = *enemy_forecast.get(&pos).unwrap() as f32 * dropoff_proximity(dist_to_dropoff) *
	            		ship_val(game.turn_number as f32/game.constants.max_turns as f32, ship.halite as f32/1000.0);
	            	if let Some(&turns) = self.ship_turns_stuck.get(&id) {
	            		factor *= 0.87_f32.powf(turns as f32);
	            	}
	            	score += k as f32 * factor;
	            }
	            if ship.pos == target {
	                score -= 4.0*game.halite_at(pos) as f32;
	            }
	            o_score += score;
	            ship_move_scores.push((pos, d, o_score));

	            if score < best_score {
	                best_score = score;
	                direction = d;
	                best_pos = pos;
	            }
	            if o_score < o_best_score {
	            	o_best_score = o_score;
	            	o_direction = d;
	            }
	        }
	        move_scores.insert(id, ship_move_scores);
	        o_directions.insert(id, o_direction);

	        let colliding = forbidden_count == 5;
	        if colliding {
				colliding_ships.insert(id);
	        } else {
				if !game.my_drop_pts.contains(&best_pos) || !self.endgame {
					forbidden.insert(best_pos);
					movable_next.insert(best_pos, id);
				}
				commands.insert(id, ShipCommand::MoveShip(direction));
			}
		}

		// fix collisions
		for id in colliding_ships {
			info!("ship {} self-colliding: {:?}", id, o_directions.get(&id).unwrap());

			// update commands, movable_next
			let mut best_dir_list: Vec<(usize, Point, Direction)> = Vec::new();
			let mut best_dir = Direction::Still;
			let mut best_num_ships_rerouted = 10000;
			let mut best_diff_oscore = 1.0e20;
			let mut best_new_pos = Point{ x:0, y:0 };
			for &(pos, dir, score) in move_scores.get(&id).unwrap() {
				// info!("({},{}), {:?}, {}", pos.x, pos.y, dir, score);
				let rmc = resolve_movable_chain(game, game.ships.get(&id).unwrap(), pos, &movable_next, &immovable_pos, &move_scores,
					&commands);
				info!("{:?}", rmc);
				match rmc {
					Some((mut d_oscore, dir_list)) => {
						let mut num_ships = dir_list.len();
						if dir != *o_directions.get(&id).unwrap() {
							num_ships += 1;
						}
						d_oscore += score;
						if num_ships < best_num_ships_rerouted ||
							(num_ships == best_num_ships_rerouted && d_oscore < best_diff_oscore) {
							best_dir_list = dir_list;
							best_dir = dir;
							best_num_ships_rerouted = num_ships;
							best_diff_oscore = d_oscore;
							best_new_pos = pos;
						}
					},
					None => (),
				}
			}

			if best_num_ships_rerouted < 10000 {
				info!("rerouting: {:?}, {:?}", best_dir, best_dir_list);
				for &(_, prev_pos, _) in &best_dir_list {
					forbidden.remove(&prev_pos);
					movable_next.remove(&prev_pos);
				}
				commands.insert(id, ShipCommand::MoveShip(best_dir));
				if !game.my_drop_pts.contains(&best_new_pos) || !self.endgame {
					forbidden.insert(best_new_pos);
					movable_next.insert(best_new_pos, id);
				}

				for (oid, _, dir) in best_dir_list {
					let new_pos = game.step_toward((*game.ships.get(&oid).unwrap()).pos, dir);
					commands.insert(oid, ShipCommand::MoveShip(dir));
					if !game.my_drop_pts.contains(&new_pos) || !self.endgame {
						forbidden.insert(new_pos);
						movable_next.insert(new_pos, oid);
					}
				}
			} else {
				info!("failed to re-route ship {}", id);
			}
		}


		for &id in my_ships_ids {
			let ship = game.ships.get(&id).unwrap();
			self.ship_prev_pos.insert(id, ship.pos);
		}

		let spawn = !forbidden.contains(&my_factory.pos) &&
			game.turn_number <= game.constants.max_turns-stop_spawn_margin &&
			my_halite >= game.constants.ship_cost
				+ if self.saving_for_dropoff {game.constants.dropoff_cost} else {0};
		(spawn, commands)
	}
}

pub fn resolve_movable_chain(game: &Game, c_ship: &Ship, pos: Point, o_movable_next: &HashMap<Point, usize>,
	immovable_pos: &HashSet<Point>, move_scores: &HashMap<usize, Vec<(Point, Direction, f32)>>,
	commands: &HashMap<usize, ShipCommand>) -> Option<(f32, Vec<(usize, Point, Direction)>)> {

	if immovable_pos.contains(&pos) {
		return None;
	} else {
		let mut movable_next = o_movable_next.clone();

		// let mut prev_ship = c_ship;
		let mut prev_id = c_ship.ship_id;
		let mut prev_pos = pos;

		let mut new_commands: Vec<(usize, Point, Direction)> = Vec::new();
		let mut d_oscore: f32 = 0.0;

		loop {
			// info!("prev_pos ({},{}), prev_id {}, new_commands {:?}", prev_pos.x, prev_pos.y, prev_id, new_commands);
			if immovable_pos.contains(&prev_pos) {
				info!("uh-oh - immovable_pos contains ({},{}), {:?}", prev_pos.x, prev_pos.y, new_commands);
				return None;
			}
			match movable_next.get(&prev_pos) {
				Some(&id) => {
					let curr_id = id;
					let curr_ship = game.ships.get(&id).unwrap();
					let curr_pos = curr_ship.pos;
					let curr_dir = match commands.get(&id) {
						Some(&ShipCommand::MoveShip(dir)) => dir,
						_ => {info!("invalid command found for ship {}", id); return None},
					};

					movable_next.insert(prev_pos, prev_id); // overwrites what was prev_pos => curr_id

					// need to handle endgame when it is okay to collide on drop point

					// see if curr_ship could go somewhere free
					let mut best_oscore = f32::MAX;
					// let mut best_new_pos = curr_pos;
					let mut prev_oscore = f32::MAX;
					let mut still_oscore = f32::MAX;
					let mut best_dir = curr_dir;
					for &(new_pos, dir, o_score) in move_scores.get(&id).unwrap() {
						if !immovable_pos.contains(&new_pos) && !movable_next.contains_key(&new_pos)
							&& o_score < best_oscore {
								best_oscore = o_score;
								// best_new_pos = new_pos;
								best_dir = dir;
							}
						if dir == curr_dir {
							prev_oscore = o_score;
						}
						if dir == Direction::Still {
							still_oscore = o_score;
						}
					}

					if best_oscore != f32::MAX {
						new_commands.push((curr_id, prev_pos, best_dir));
						d_oscore += best_oscore - prev_oscore;
						// info!("diff {}", best_oscore - prev_oscore);
						return Some((d_oscore, new_commands));
					} else {
					// if curr_ship cannot go anywhere else free...
					// if it was planning to be still, give up
					// if it was planning to move, plan to be still and bump the ship moving in (next iteration of loop)
						if curr_dir == Direction::Still {
							return None;
						} else {
							new_commands.push((curr_id, prev_pos, Direction::Still));
							d_oscore += still_oscore - prev_oscore;
							// info!("diff {}", still_oscore - prev_oscore);
						}
						prev_pos = curr_pos;
						prev_id = curr_id;
					}
				},
				None => return Some((d_oscore, new_commands)),
			}
		}
	}
}

pub fn sigmoid1(h: f64) -> f64 {
	(-0.148047 + 1.0/(1.0 + (0.0025*(700.0-h)).exp())).max(0.0)
}

pub fn ship_val(game_progress: f32, ship_fullness: f32) -> f32 {
	1.0 - game_progress.powf(2.0)*(1.0-ship_fullness)
}

pub fn dropoff_proximity(dist: usize) -> f32 {
	if dist >= 6 {
		1.0
	} else if dist <= 3 {
		0.0
	} else {
		((dist as f32)-3.0)/3.0
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipPriority {
    pub id: usize,
    pub w: isize,
}

impl Ord for ShipPriority {
    fn cmp(&self, other: &ShipPriority) -> Ordering {
        other.w.cmp(&self.w) // so that smaller numbers go first
    }
}

impl PartialOrd for ShipPriority {
    fn partial_cmp(&self, other: &ShipPriority) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}