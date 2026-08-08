extends RefCounted
# Livestock — placid bovine/goat rig (mammal_sprites RigKind.LIVESTOCK_RIG).
# 16x16, facing right. Deep barrel body, short sturdy legs, a blunt head with
# two short down-horns and a broad muzzle, minimal tail. No gallop drama: the
# flee poses are just a mild trot. Idle chewing head-bob is the shader's job
# (rig_kind == 3), not baked into the art. Same 12-slot layout as Wolf: 0
# stand, 1/2/3 trot (contact-L / passing / contact-R), 4/5 graze, 6/7
# headbutt, 8/9 alert, 10/11 mild trot. Blocks are [x, y, w, h, zone];
# painted back-to-front, auto-outlined.

const POSES: Array = [
	# 0 stand
	[
		[3, 4, 8, 6, "c"],  # deep barrel body
		[4, 8, 6, 1, "u"],  # underbelly
		[10, 5, 4, 4, "c"],  # blunt head
		[13, 7, 2, 2, "n"],  # broad muzzle
		[12, 5, 1, 1, "e"],  # eye
		[10, 3, 1, 2, "c"],
		[12, 3, 1, 2, "c"],  # short down-horns
		[2, 5, 1, 2, "c"],  # minimal tail
		[4, 10, 2, 4, "c"],
		[6, 10, 2, 4, "c"],  # hind legs, short and sturdy
		[9, 10, 2, 4, "c"],
		[11, 10, 2, 4, "c"],  # fore legs, short and sturdy
	],
	# 1 contact-L — near-hind + far-fore reach forward
	[
		[3, 4, 8, 6, "c"],
		[4, 8, 6, 1, "u"],
		[10, 5, 4, 4, "c"],
		[13, 7, 2, 2, "n"],
		[12, 5, 1, 1, "e"],
		[10, 3, 1, 2, "c"],
		[12, 3, 1, 2, "c"],
		[2, 5, 1, 2, "c"],
		[3, 10, 2, 4, "c"],  # hind: forward, planted
		[7, 11, 2, 3, "c"],  # hind: lifted
		[9, 11, 2, 3, "c"],  # fore: lifted
		[12, 10, 2, 4, "c"],  # fore: forward, planted
	],
	# 2 passing — legs gathered, whole figure lifted 1px (the trot bob)
	[
		[3, 3, 8, 6, "c"],
		[4, 7, 6, 1, "u"],
		[10, 4, 4, 4, "c"],
		[13, 6, 2, 2, "n"],
		[12, 4, 1, 1, "e"],
		[10, 2, 1, 2, "c"],
		[12, 2, 1, 2, "c"],
		[2, 4, 1, 2, "c"],
		[5, 9, 2, 4, "c"],
		[7, 9, 2, 4, "c"],
		[9, 9, 2, 4, "c"],
		[11, 9, 2, 4, "c"],
	],
	# 3 contact-R — opposite diagonal pair extended
	[
		[3, 4, 8, 6, "c"],
		[4, 8, 6, 1, "u"],
		[10, 5, 4, 4, "c"],
		[13, 7, 2, 2, "n"],
		[12, 5, 1, 1, "e"],
		[10, 3, 1, 2, "c"],
		[12, 3, 1, 2, "c"],
		[2, 5, 1, 2, "c"],
		[4, 11, 2, 3, "c"],  # hind: lifted
		[6, 10, 2, 4, "c"],  # hind: forward, planted
		[8, 10, 2, 4, "c"],  # fore: forward, planted
		[11, 11, 2, 3, "c"],  # fore: lifted
	],
	# 4 graze A — head lowered to the grass
	[
		[3, 4, 8, 6, "c"],
		[4, 8, 6, 1, "u"],
		[10, 8, 4, 4, "c"],
		[13, 10, 2, 2, "n"],
		[12, 8, 1, 1, "e"],
		[10, 7, 1, 2, "c"],
		[12, 7, 1, 2, "c"],
		[2, 5, 1, 2, "c"],
		[4, 10, 2, 4, "c"],
		[6, 10, 2, 4, "c"],
		[9, 10, 2, 4, "c"],
		[11, 10, 2, 4, "c"],
	],
	# 5 graze B — a deeper dip
	[
		[3, 4, 8, 6, "c"],
		[4, 8, 6, 1, "u"],
		[10, 9, 4, 4, "c"],
		[13, 11, 2, 2, "n"],
		[12, 9, 1, 1, "e"],
		[10, 8, 1, 2, "c"],
		[12, 8, 1, 2, "c"],
		[2, 5, 1, 2, "c"],
		[4, 10, 2, 4, "c"],
		[6, 10, 2, 4, "c"],
		[9, 10, 2, 4, "c"],
		[11, 10, 2, 4, "c"],
	],
	# 6 headbutt A — head drawn back and down (wind-up)
	[
		[3, 5, 8, 6, "c"],
		[4, 9, 6, 1, "u"],
		[9, 6, 4, 4, "c"],
		[12, 8, 2, 2, "n"],
		[11, 6, 1, 1, "e"],
		[9, 5, 1, 2, "c"],
		[11, 5, 1, 2, "c"],
		[2, 6, 1, 2, "c"],
		[4, 11, 2, 4, "c"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"],
		[11, 11, 2, 4, "c"],
	],
	# 7 headbutt B — a short, blunt lunge forward
	[
		[3, 5, 8, 5, "c"],
		[4, 8, 6, 1, "u"],
		[10, 6, 5, 4, "c"],
		[14, 8, 2, 2, "n"],
		[12, 6, 1, 1, "e"],
		[10, 5, 1, 2, "c"],
		[13, 5, 1, 2, "c"],
		[2, 6, 1, 2, "c"],
		[3, 11, 2, 4, "c"],
		[6, 10, 2, 4, "c"],
		[9, 10, 2, 4, "c"],
		[12, 11, 2, 4, "c"],
	],
	# 8 alert A — head up
	[
		[3, 3, 8, 6, "c"],
		[4, 7, 6, 1, "u"],
		[10, 2, 4, 4, "c"],
		[13, 4, 2, 2, "n"],
		[12, 2, 1, 1, "e"],
		[10, 0, 1, 2, "c"],
		[12, 0, 1, 2, "c"],
		[2, 3, 1, 2, "c"],
		[4, 10, 2, 4, "c"],
		[6, 10, 2, 4, "c"],
		[9, 10, 2, 4, "c"],
		[11, 10, 2, 4, "c"],
	],
	# 9 alert B — a small weight shift
	[
		[3, 4, 8, 6, "c"],
		[4, 8, 6, 1, "u"],
		[10, 3, 4, 4, "c"],
		[13, 5, 2, 2, "n"],
		[12, 3, 1, 1, "e"],
		[9, 1, 1, 2, "c"],
		[11, 1, 1, 2, "c"],
		[2, 4, 1, 2, "c"],
		[4, 10, 2, 4, "c"],
		[6, 10, 2, 4, "c"],
		[9, 10, 2, 4, "c"],
		[11, 10, 2, 4, "c"],
	],
	# 10 mild trot A — no gallop drama, just a quicker stride
	[
		[3, 4, 8, 6, "c"],
		[4, 8, 6, 1, "u"],
		[10, 5, 4, 4, "c"],
		[13, 7, 2, 2, "n"],
		[12, 5, 1, 1, "e"],
		[10, 3, 1, 2, "c"],
		[12, 3, 1, 2, "c"],
		[2, 5, 1, 2, "c"],
		[3, 10, 2, 4, "c"],
		[7, 11, 2, 3, "c"],
		[9, 11, 2, 3, "c"],
		[12, 10, 2, 4, "c"],
	],
	# 11 mild trot B — opposite pair forward
	[
		[3, 4, 8, 6, "c"],
		[4, 8, 6, 1, "u"],
		[10, 5, 4, 4, "c"],
		[13, 7, 2, 2, "n"],
		[12, 5, 1, 1, "e"],
		[10, 3, 1, 2, "c"],
		[12, 3, 1, 2, "c"],
		[2, 5, 1, 2, "c"],
		[4, 11, 2, 3, "c"],
		[6, 10, 2, 4, "c"],
		[8, 10, 2, 4, "c"],
		[11, 11, 2, 3, "c"],
	],
]
