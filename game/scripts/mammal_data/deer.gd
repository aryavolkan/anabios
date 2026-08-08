extends RefCounted
# Deer — prey/quadruped rig (mammal_sprites RigKind.PREY, large-bodied). 16x16,
# facing right. Tall thin legs, a long neck angled up toward a small head with
# upright ears. Same 12-slot layout as Wolf: 0 stand, 1/2/3 trot (contact-L /
# passing / contact-R), 4/5 graze (neck all the way to the ground), 6/7 charge,
# 8/9 alert, 10/11 gallop (long bounding leap). Blocks are [x, y, w, h, zone];
# painted back-to-front, auto-outlined.

const POSES: Array = [
	# 0 stand
	[
		[1, 5, 2, 2, "c"],  # tail
		[3, 5, 7, 4, "c"],  # slim torso
		[4, 8, 4, 1, "u"],  # underbelly
		[9, 2, 4, 5, "c"],  # long neck + head
		[13, 4, 2, 2, "n"],  # muzzle
		[12, 2, 1, 1, "e"],  # eye
		[9, 1, 1, 2, "c"],
		[11, 1, 1, 2, "c"],  # upright ears
		[3, 9, 1, 7, "c"],
		[5, 9, 1, 7, "c"],  # hind legs, thin
		[8, 9, 1, 7, "c"],
		[10, 9, 1, 7, "c"],  # fore legs, thin
	],
	# 1 contact-L — near-hind + far-fore reach forward, other diagonal lifts
	[
		[1, 5, 2, 2, "c"],
		[3, 5, 7, 4, "c"],
		[4, 8, 4, 1, "u"],
		[9, 2, 4, 5, "c"],
		[13, 4, 2, 2, "n"],
		[12, 2, 1, 1, "e"],
		[9, 1, 1, 2, "c"],
		[11, 1, 1, 2, "c"],
		[2, 9, 1, 7, "c"],  # hind: forward, planted
		[6, 10, 1, 5, "c"],  # hind: lifted
		[7, 10, 1, 5, "c"],  # fore: lifted
		[11, 9, 1, 7, "c"],  # fore: forward, planted
	],
	# 2 passing — legs gathered, whole figure lifted 1px (the trot bob)
	[
		[1, 4, 2, 2, "c"],
		[3, 4, 7, 4, "c"],
		[4, 7, 4, 1, "u"],
		[9, 1, 4, 5, "c"],
		[13, 3, 2, 2, "n"],
		[12, 1, 1, 1, "e"],
		[9, 0, 1, 2, "c"],
		[11, 0, 1, 2, "c"],
		[4, 8, 1, 5, "c"],
		[6, 8, 1, 5, "c"],
		[8, 8, 1, 5, "c"],
		[10, 8, 1, 5, "c"],
	],
	# 3 contact-R — opposite diagonal pair extended
	[
		[1, 5, 2, 2, "c"],
		[3, 5, 7, 4, "c"],
		[4, 8, 4, 1, "u"],
		[9, 2, 4, 5, "c"],
		[13, 4, 2, 2, "n"],
		[12, 2, 1, 1, "e"],
		[9, 1, 1, 2, "c"],
		[11, 1, 1, 2, "c"],
		[4, 10, 1, 5, "c"],  # hind: lifted
		[6, 9, 1, 7, "c"],  # hind: forward, planted
		[8, 10, 1, 5, "c"],  # fore: lifted
		[11, 9, 1, 7, "c"],  # fore: forward, planted
	],
	# 4 graze A — the whole neck drops toward the ground
	[
		[1, 5, 2, 2, "c"],
		[3, 5, 7, 4, "c"],
		[4, 8, 4, 1, "u"],
		[10, 5, 2, 7, "c"],  # neck, arcing down
		[11, 10, 3, 3, "c"],  # head, near ground
		[12, 10, 1, 1, "e"],
		[14, 11, 1, 1, "n"],
		[10, 9, 1, 1, "c"],
		[12, 9, 1, 1, "c"],  # ears, folded near head
		[3, 9, 1, 7, "c"],
		[5, 9, 1, 7, "c"],
		[8, 9, 1, 7, "c"],
		[10, 9, 1, 7, "c"],
	],
	# 5 graze B — nose to the dirt, a small dip
	[
		[1, 5, 2, 2, "c"],
		[3, 5, 7, 4, "c"],
		[4, 8, 4, 1, "u"],
		[10, 5, 2, 8, "c"],
		[11, 11, 3, 3, "c"],
		[12, 11, 1, 1, "e"],
		[14, 13, 1, 1, "n"],
		[10, 10, 1, 1, "c"],
		[12, 10, 1, 1, "c"],
		[3, 9, 1, 7, "c"],
		[5, 9, 1, 7, "c"],
		[8, 9, 1, 7, "c"],
		[10, 9, 1, 7, "c"],
	],
	# 6 charge A — head drawn back, haunches braced (wind-up)
	[
		[1, 6, 2, 2, "c"],
		[3, 5, 7, 4, "c"],
		[4, 8, 4, 1, "u"],
		[8, 1, 4, 5, "c"],
		[11, 3, 2, 2, "n"],
		[10, 1, 1, 1, "e"],
		[8, 0, 1, 2, "c"],
		[10, 0, 1, 2, "c"],
		[3, 9, 1, 7, "c"],
		[5, 9, 1, 7, "c"],
		[8, 9, 1, 7, "c"],
		[10, 9, 1, 7, "c"],
	],
	# 7 charge B — head thrust forward, out past the frame edge
	[
		[1, 6, 2, 2, "c"],
		[3, 5, 7, 4, "c"],
		[4, 8, 4, 1, "u"],
		[10, 3, 5, 4, "c"],
		[15, 4, 1, 2, "n"],
		[14, 3, 1, 1, "e"],
		[10, 1, 1, 2, "c"],
		[12, 1, 1, 2, "c"],
		[3, 10, 1, 6, "c"],
		[5, 10, 1, 6, "c"],
		[9, 9, 1, 7, "c"],
		[11, 9, 1, 7, "c"],
	],
	# 8 alert A — head up, ears pricked
	[
		[1, 4, 2, 2, "c"],
		[3, 5, 7, 4, "c"],
		[4, 8, 4, 1, "u"],
		[9, 1, 4, 5, "c"],
		[13, 3, 2, 2, "n"],
		[12, 1, 1, 1, "e"],
		[9, 0, 1, 2, "c"],
		[11, 0, 1, 2, "c"],
		[3, 9, 1, 7, "c"],
		[5, 9, 1, 7, "c"],
		[8, 9, 1, 7, "c"],
		[10, 9, 1, 7, "c"],
	],
	# 9 alert B — a small weight shift
	[
		[1, 5, 2, 2, "c"],
		[3, 5, 7, 4, "c"],
		[4, 8, 4, 1, "u"],
		[9, 2, 4, 5, "c"],
		[13, 4, 2, 2, "n"],
		[12, 2, 1, 1, "e"],
		[8, 1, 1, 2, "c"],
		[10, 1, 1, 2, "c"],
		[3, 9, 1, 7, "c"],
		[5, 9, 1, 7, "c"],
		[8, 9, 1, 7, "c"],
		[10, 9, 1, 7, "c"],
	],
	# 10 gallop A — body stretched, the long bounding leap
	[
		[1, 6, 2, 2, "c"],
		[3, 6, 8, 4, "c"],
		[4, 9, 5, 1, "u"],
		[10, 3, 4, 5, "c"],
		[14, 5, 2, 2, "n"],
		[13, 3, 1, 1, "e"],
		[10, 2, 1, 2, "c"],
		[12, 2, 1, 2, "c"],
		[1, 10, 1, 5, "c"],  # hind flung back
		[3, 11, 1, 4, "c"],
		[12, 10, 1, 5, "c"],  # fore reaching
		[14, 11, 1, 4, "c"],
	],
	# 11 gallop B — gathered: legs sweep under, body compressed
	[
		[1, 6, 2, 2, "c"],
		[4, 6, 7, 4, "c"],
		[5, 9, 4, 1, "u"],
		[10, 3, 4, 5, "c"],
		[14, 5, 2, 2, "n"],
		[13, 3, 1, 1, "e"],
		[10, 2, 1, 2, "c"],
		[12, 2, 1, 2, "c"],
		[6, 10, 1, 5, "c"],
		[8, 10, 1, 5, "c"],
		[10, 10, 1, 5, "c"],
		[12, 10, 1, 5, "c"],
	],
]
