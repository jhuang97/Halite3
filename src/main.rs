#[macro_use] extern crate log;
extern crate simplelog;
extern crate rand;
extern crate serde_json;
extern crate my_bot;

use serde_json::Value;
use simplelog::*;
use std::io::{BufRead, BufReader, stdin};
use std::fs::File;

use my_bot::Game;


fn main() {
	let stdin = stdin();
	let reader = BufReader::new(stdin);
	let mut lines_iter = reader.lines().map(|l| l.unwrap());

    let constants: Value = serde_json::from_str(&lines_iter.next().unwrap()).unwrap();

    let player_info = my_bot::parse_line_of_nums(&mut lines_iter);
    let num_players = player_info[0];
    let my_pid = player_info[1];

    let mut game = Game::init(&mut lines_iter, constants, num_players, my_pid);
    let _ = CombinedLogger::init(
    	vec![
		    WriteLogger::new(
		    	LevelFilter::Info,
		    	Config {time: None, level: None, target: None, location: None, time_format: None},
		    	File::create(format!("Jank-log-{}.log", my_pid)).unwrap()),
		    WriteLogger::new(
		    	LevelFilter::Warn,
		    	Config {time: None, level: None, target: None, location: None, time_format: None},
		    	File::create(format!("f-{}.log", my_pid)).unwrap())
    	]
    ).unwrap();
    warn!("[");

    let mut logic = my_bot::Logic::new();

	game.ready("jank_bot_17");

	loop {
		game.update_frame(&mut lines_iter);
		Game::end_turn(logic.make_moves(&game));
	}
}