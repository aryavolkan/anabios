extends RefCounted
# Fox — predator/quadruped rig (mammal_sprites RigKind.PREDATOR, small-bodied).
# 16x16, facing right. Slim low body, a pointed head with large triangular
# ears, a big bushy tail (its signature), slender legs with light "socks" at
# the paws. Same 12-slot layout as Wolf: 0 stand, 1/2/3 trot (contact-L /
# passing / contact-R), 4/5 eat, 6/7 pounce (crouch + airborne lunge), 8/9
# alert, 10/11 gallop. Blocks are [x, y, w, h, zone]; painted back-to-front,
# auto-outlined.

const POSES: Array = [
	# 0 stand
	[
		[4, 6, 6, 3, "c"],  # slim low body
		[5, 8, 4, 1, "u"],  # underbelly
		[9, 4, 4, 3, "c"],  # pointed head
		[13, 5, 2, 1, "n"],  # muzzle tip
		[11, 4, 1, 1, "e"],  # eye
		[9, 1, 2, 3, "c"],
		[11, 1, 2, 3, "c"],  # large triangular ears
		[0, 5, 3, 3, "c"],  # bushy tail
		[0, 4, 2, 2, "u"],  # tail tip
		[4, 9, 1, 6, "c"],
		[6, 9, 1, 6, "c"],  # hind legs, slender
		[8, 9, 1, 6, "c"],
		[10, 9, 1, 6, "c"],  # fore legs, slender
		[4, 13, 1, 1, "u"],
		[10, 13, 1, 1, "u"],  # sock accents
	],
	# 1 contact-L — near-hind + far-fore reach forward
	[
		[4, 6, 6, 3, "c"],
		[5, 8, 4, 1, "u"],
		[9, 4, 4, 3, "c"],
		[13, 5, 2, 1, "n"],
		[11, 4, 1, 1, "e"],
		[9, 1, 2, 3, "c"],
		[11, 1, 2, 3, "c"],
		[0, 5, 3, 3, "c"],
		[0, 4, 2, 2, "u"],
		[3, 9, 1, 6, "c"],  # hind: forward, planted
		[7, 10, 1, 4, "c"],  # hind: lifted
		[9, 10, 1, 4, "c"],  # fore: lifted
		[11, 9, 1, 6, "c"],  # fore: forward, planted
		[3, 13, 1, 1, "u"],
		[11, 13, 1, 1, "u"],
	],
	# 2 passing — legs gathered, whole figure lifted 1px (the trot bob)
	[
		[4, 5, 6, 3, "c"],
		[5, 7, 4, 1, "u"],
		[9, 3, 4, 3, "c"],
		[13, 4, 2, 1, "n"],
		[11, 3, 1, 1, "e"],
		[9, 0, 2, 3, "c"],
		[11, 0, 2, 3, "c"],
		[0, 4, 3, 3, "c"],
		[0, 3, 2, 2, "u"],
		[5, 8, 1, 4, "c"],
		[7, 8, 1, 4, "c"],
		[9, 8, 1, 4, "c"],
		[11, 8, 1, 4, "c"],
	],
	# 3 contact-R — opposite diagonal pair extended
	[
		[4, 6, 6, 3, "c"],
		[5, 8, 4, 1, "u"],
		[9, 4, 4, 3, "c"],
		[13, 5, 2, 1, "n"],
		[11, 4, 1, 1, "e"],
		[9, 1, 2, 3, "c"],
		[11, 1, 2, 3, "c"],
		[0, 5, 3, 3, "c"],
		[0, 4, 2, 2, "u"],
		[5, 10, 1, 4, "c"],  # hind: lifted
		[6, 9, 1, 6, "c"],  # hind: forward, planted
		[8, 9, 1, 6, "c"],  # fore: forward, planted
		[10, 10, 1, 4, "c"],  # fore: lifted
		[6, 13, 1, 1, "u"],
		[8, 13, 1, 1, "u"],
	],
	# 4 eat A — head lowered to the kill
	[
		[4, 6, 6, 3, "c"],
		[5, 8, 4, 1, "u"],
		[9, 8, 4, 4, "c"],
		[13, 10, 2, 1, "n"],
		[11, 8, 1, 1, "e"],
		[9, 6, 2, 2, "c"],
		[11, 6, 2, 2, "c"],
		[0, 5, 3, 3, "c"],
		[0, 4, 2, 2, "u"],
		[4, 9, 1, 6, "c"],
		[6, 9, 1, 6, "c"],
		[8, 9, 1, 6, "c"],
		[10, 9, 1, 6, "c"],
	],
	# 5 eat B — a deeper dip
	[
		[4, 6, 6, 3, "c"],
		[5, 8, 4, 1, "u"],
		[9, 9, 4, 4, "c"],
		[13, 11, 2, 1, "n"],
		[11, 9, 1, 1, "e"],
		[9, 7, 2, 2, "c"],
		[11, 7, 2, 2, "c"],
		[0, 5, 3, 3, "c"],
		[0, 4, 2, 2, "u"],
		[4, 9, 1, 6, "c"],
		[6, 9, 1, 6, "c"],
		[8, 9, 1, 6, "c"],
		[10, 9, 1, 6, "c"],
	],
	# 6 pounce A — crouched low, coiled for the leap
	[
		[4, 7, 6, 3, "c"],
		[5, 9, 4, 1, "u"],
		[8, 5, 4, 3, "c"],
		[12, 6, 2, 1, "n"],
		[10, 5, 1, 1, "e"],
		[8, 2, 2, 3, "c"],
		[10, 2, 2, 3, "c"],
		[0, 6, 3, 3, "c"],
		[0, 5, 2, 2, "u"],
		[4, 11, 1, 3, "c"],
		[6, 11, 1, 3, "c"],
		[8, 11, 1, 3, "c"],
		[10, 11, 1, 3, "c"],
	],
	# 7 pounce B — airborne lunge, out past the frame edge
	[
		[4, 5, 7, 3, "c"],
		[5, 7, 5, 1, "u"],
		[11, 3, 4, 3, "c"],
		[15, 4, 1, 1, "n"],
		[13, 3, 1, 1, "e"],
		[11, 0, 2, 3, "c"],
		[13, 0, 2, 3, "c"],
		[0, 4, 3, 3, "c"],
		[0, 3, 2, 2, "u"],
		[1, 9, 1, 3, "c"],
		[3, 9, 1, 3, "c"],
		[12, 9, 1, 3, "c"],
		[14, 9, 1, 3, "c"],
	],
	# 8 alert A — head up, ears pricked
	[
		[4, 5, 6, 3, "c"],
		[5, 7, 4, 1, "u"],
		[9, 3, 4, 3, "c"],
		[13, 4, 2, 1, "n"],
		[11, 3, 1, 1, "e"],
		[9, 0, 2, 3, "c"],
		[11, 0, 2, 3, "c"],
		[0, 4, 3, 3, "c"],
		[0, 3, 2, 2, "u"],
		[4, 9, 1, 6, "c"],
		[6, 9, 1, 6, "c"],
		[8, 9, 1, 6, "c"],
		[10, 9, 1, 6, "c"],
	],
	# 9 alert B — a small weight shift
	[
		[4, 6, 6, 3, "c"],
		[5, 8, 4, 1, "u"],
		[9, 4, 4, 3, "c"],
		[13, 5, 2, 1, "n"],
		[11, 4, 1, 1, "e"],
		[8, 1, 2, 3, "c"],
		[10, 1, 2, 3, "c"],
		[0, 5, 3, 3, "c"],
		[0, 4, 2, 2, "u"],
		[4, 9, 1, 6, "c"],
		[6, 9, 1, 6, "c"],
		[8, 9, 1, 6, "c"],
		[10, 9, 1, 6, "c"],
	],
	# 10 gallop A — body stretched, the chase
	[
		[3, 6, 8, 3, "c"],
		[4, 8, 5, 1, "u"],
		[10, 4, 4, 3, "c"],
		[14, 5, 2, 1, "n"],
		[12, 4, 1, 1, "e"],
		[10, 1, 2, 3, "c"],
		[12, 1, 2, 3, "c"],
		[0, 5, 3, 3, "c"],
		[0, 4, 2, 2, "u"],
		[1, 10, 1, 4, "c"],  # hind flung back
		[3, 11, 1, 3, "c"],
		[12, 10, 1, 4, "c"],  # fore reaching
		[14, 11, 1, 3, "c"],
	],
	# 11 gallop B — gathered: legs sweep under, body compressed
	[
		[5, 6, 7, 3, "c"],
		[6, 8, 5, 1, "u"],
		[11, 4, 4, 3, "c"],
		[15, 5, 1, 1, "n"],
		[13, 4, 1, 1, "e"],
		[11, 1, 2, 3, "c"],
		[13, 1, 2, 3, "c"],
		[2, 5, 3, 3, "c"],
		[2, 4, 2, 2, "u"],
		[7, 10, 1, 4, "c"],
		[9, 10, 1, 4, "c"],
		[11, 10, 1, 4, "c"],
		[13, 10, 1, 4, "c"],
	],
]
