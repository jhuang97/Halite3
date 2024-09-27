
// https://en.wikipedia.org/wiki/Disjoint-set_data_structure
#[derive(Debug)]
pub struct DisjointSet {
	pub size: usize,
	pub parent: Vec<usize>,
	pub rank: Vec<usize>,
}

impl DisjointSet {
	pub fn make_singletons(size: usize) -> DisjointSet {
		DisjointSet {
			size,
			parent: (0..size).collect(),
			rank: vec![0; size],
		}
	}

	pub fn find(&mut self, x: usize) -> usize {
		let parent = self.parent[x];
		if parent != x {
			self.parent[x] = self.find(parent);
		}
		self.parent[x]
	}

	pub fn unite(&mut self, x: usize, y: usize) {
		let x_root = self.find(x);
		let y_root = self.find(y);
		if x_root != y_root {
			if self.rank[x_root] < self.rank[y_root] {
				self.parent[x_root] = y_root;
			} else if self.rank[x_root] > self.rank[y_root] {
				self.parent[y_root] = x_root;
			} else {
				self.parent[y_root] = x_root;
				self.rank[x_root]+= 1;
			}
		}
	}
}