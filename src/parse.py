import json

class TokenReader:
	def __init__(self):
		self.tokens = []

	def get_token(self):
		if len(self.tokens) == 0:
			self.tokens = input().split()
		ret = self.tokens[0]
		self.tokens = self.tokens[1:]
		return ret

	def get_int(self):
		return int(self.get_token())


reader = TokenReader()


class Factory:
	def __init__(self, player, x, y):
		self.player = player
		self.x = x
		self.y = y

class Dropoff:
	def __init__(self, player, x, y):
		self.player = player
		self.x = x
		self.y = y

class Ship:
	def __init__(self, player, ship_id, x, y, halite):
		self.player = player
		self.ship_id = ship_id
		self.x = x
		self.y = y
		self.halite = halite

class Game:
	def __init__(self):
		self.map = []
		self.factories = []
		self.ships = []
		self.dropoffs = []
		self.energy = dict()	# player ID --> energy (stored halite)

	def pre_parse(self):

		self.constants = json.loads(reader.get_token())

		self.players = reader.get_int()
		self.pid = reader.get_int()		# Our own ID

		for n in range(self.players):
			player = reader.get_int()
			x = reader.get_int()
			y = reader.get_int()
			self.factories.append(Factory(player, x, y))

		self.width = reader.get_int()
		self.height = reader.get_int()

		# Create our map array and zero it...

		self.map = [[0 for y in range(self.height)] for x in range(self.width)]

		# Fill it up with actual data from stdin...

		for y in range(self.height):
			for x in range(self.width):
				self.map[x][y] = reader.get_int()

	def parse(self):

		# We will get sent all ships/dropoffs, so clear those arrays.
		# Note: any AI that has references to individual ships
		# will need to update its references to the new ones.

		self.ships = []
		self.dropoffs = []

		self.turn = reader.get_int() - 1	# Engine is out by 1, imo

		for n in range(self.players):

			player = reader.get_int()
			ships = reader.get_int()
			dropoffs = reader.get_int()

			self.energy[player] = reader.get_int()

			for i in range(ships):
				ship_id = reader.get_int()
				x = reader.get_int()
				y = reader.get_int()
				halite = reader.get_int()
				self.ships.append(Ship(player, ship_id, x, y, halite))

			for i in range(dropoffs):
				__ = reader.get_int()		# Dropoff ID (useless info)
				x = reader.get_int()
				y = reader.get_int()
				self.dropoffs.append(Dropoff(player, x, y))

		map_updates = reader.get_int()

		for n in range(map_updates):
			x = reader.get_int()
			y = reader.get_int()
			value = reader.get_int()

			self.map[x][y] = value


def main():

	game = Game()
	game.pre_parse()

	print("DoNothingBot")

	while 1:
		game.parse()
		print()


if __name__ == "__main__":
	main()