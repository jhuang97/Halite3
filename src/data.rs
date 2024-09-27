use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: isize,
    pub y: isize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub pos: Point,
    pub w: isize,
}

impl Ord for Target {
    fn cmp(&self, other: &Target) -> Ordering {
        self.w.cmp(&other.w)
    }
}

impl PartialOrd for Target {
    fn partial_cmp(&self, other: &Target) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
pub struct Factory {
    pub player: usize,
    pub pos: Point,
}

#[derive(Debug, Clone)]
pub struct Dropoff {
    pub player: usize,
    pub pos: Point,
}

#[derive(Debug, Clone)]
pub struct Ship {
    pub player: usize,
    pub ship_id: usize,
    pub pos: Point,
    pub halite: isize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Direction {
    North, East, South, West, Still,
}

impl Direction {
    pub fn get_char_encoding(&self) -> char {
        match self {
            Direction::North => 'n',
            Direction::East => 'e',
            Direction::South => 's',
            Direction::West => 'w',
            Direction::Still => 'o',
        }
    }

    pub fn all_directions() -> Vec<Direction> {
        vec![Direction::North, Direction::East, 
        Direction::South, Direction::West, Direction::Still]
    }

    pub fn adjacent_directions() -> Vec<Direction> {
        vec![Direction::North, Direction::East, 
        Direction::South, Direction::West]
    }
}

#[derive(Debug)]
pub enum ShipCommand {
    MakeDropoff(),
    MoveShip(Direction),
}