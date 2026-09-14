extends RefCounted
# Tortoise — prey-family/quadruped rig (mammal_sprites RigKind.PREY, armoured
# herbivore/omnivore). 16x16, facing right. Low domed shell, short stub legs
# mostly hidden beneath the rim, a small head on a stubby neck that can
# withdraw. Same 14-slot layout as Wolf: 0 stand, 1/2/3 amble (contact-L /
# passing / contact-R), 4/5 graze (neck dips to the ground), 6/7 withdraw
# (head draws back into the shell — the defensive "attack" slot), 8/9 alert
# (neck stretches up), 10/11 flee (stretched scramble), 12/13 sleep (head
# fully withdrawn). Blocks are [x, y, w, h, zone]; painted back-to-front,
# auto-outlined.

const POSES: Array = [
	# 0 stand
	[
		[6, 4, 4, 1, "c"],  # shell top ridge
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],  # domed shell, main bulk
		[4, 10, 7, 1, "u"],  # underside peeking below the shell rim
		[11, 7, 2, 2, "c"],  # neck
		[13, 7, 2, 2, "c"],  # head
		[15, 8, 1, 1, "n"],  # muzzle
		[13, 7, 1, 1, "e"],  # eye
		[1, 8, 1, 1, "c"],  # tail nub
		[3, 10, 2, 2, "c"],  # hind leg stub
		[10, 10, 2, 2, "c"],  # fore leg stub
	],
	# 1 contact-L — hind stub forward, fore stub trails
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[11, 7, 2, 2, "c"],
		[13, 7, 2, 2, "c"],
		[15, 8, 1, 1, "n"],
		[13, 7, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[4, 11, 2, 1, "c"],
		[9, 10, 2, 2, "c"],
	],
	# 2 passing — whole shell lifted 1px (the amble bob), stubs gathered under
	[
		[6, 3, 4, 1, "c"],
		[4, 4, 8, 1, "c"],
		[2, 5, 11, 4, "c"],
		[4, 9, 7, 1, "u"],
		[11, 6, 2, 2, "c"],
		[13, 6, 2, 2, "c"],
		[15, 7, 1, 1, "n"],
		[13, 6, 1, 1, "e"],
		[1, 7, 1, 1, "c"],
		[5, 9, 2, 1, "c"],
		[8, 9, 2, 1, "c"],
	],
	# 3 contact-R — opposite stub pairing
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[11, 7, 2, 2, "c"],
		[13, 7, 2, 2, "c"],
		[15, 8, 1, 1, "n"],
		[13, 7, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[2, 10, 2, 1, "c"],
		[11, 11, 2, 1, "c"],
	],
	# 4 graze A — neck bends the head down toward the ground
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[11, 9, 2, 2, "c"],
		[13, 10, 2, 2, "c"],
		[15, 11, 1, 1, "n"],
		[13, 10, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[3, 10, 2, 2, "c"],
		[10, 10, 2, 2, "c"],
	],
	# 5 graze B — muzzle to the dirt, a deeper dip
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[11, 10, 2, 2, "c"],
		[13, 11, 2, 2, "c"],
		[15, 12, 1, 1, "n"],
		[13, 11, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[3, 10, 2, 2, "c"],
		[10, 10, 2, 2, "c"],
	],
	# 6 withdraw A — neck draws back toward the shell rim (wind-up)
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[10, 7, 2, 2, "c"],
		[11, 7, 2, 2, "c"],
		[12, 7, 1, 1, "n"],
		[11, 7, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[3, 10, 2, 2, "c"],
		[10, 10, 2, 2, "c"],
	],
	# 7 withdraw B — head fully pulled into the shell, just a peeking eye
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[10, 7, 3, 2, "c"],
		[11, 7, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[3, 10, 2, 2, "c"],
		[10, 10, 2, 2, "c"],
	],
	# 8 alert A — neck stretches straight up
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[11, 4, 2, 4, "c"],
		[12, 3, 3, 2, "c"],
		[15, 4, 1, 1, "n"],
		[13, 3, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[3, 10, 2, 2, "c"],
		[10, 10, 2, 2, "c"],
	],
	# 9 alert B — a small weight shift
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[10, 4, 2, 4, "c"],
		[11, 3, 3, 2, "c"],
		[14, 4, 1, 1, "n"],
		[12, 3, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[3, 10, 2, 2, "c"],
		[10, 10, 2, 2, "c"],
	],
	# 10 flee A — shell stretched into a scramble, stubs flung
	[
		[5, 4, 4, 1, "c"],
		[3, 5, 8, 1, "c"],
		[1, 6, 12, 4, "c"],
		[3, 10, 8, 1, "u"],
		[12, 7, 2, 2, "c"],
		[14, 7, 1, 2, "c"],
		[15, 8, 1, 1, "n"],
		[14, 7, 1, 1, "e"],
		[0, 8, 1, 1, "c"],
		[0, 10, 2, 3, "c"],  # hind flung back
		[12, 10, 2, 3, "c"],  # fore reaching
	],
	# 11 flee B — gathered mid-scramble
	[
		[6, 4, 4, 1, "c"],
		[4, 5, 8, 1, "c"],
		[2, 6, 11, 4, "c"],
		[4, 10, 7, 1, "u"],
		[12, 7, 2, 2, "c"],
		[14, 7, 1, 2, "c"],
		[15, 8, 1, 1, "n"],
		[14, 7, 1, 1, "e"],
		[1, 8, 1, 1, "c"],
		[4, 10, 2, 2, "c"],
		[10, 10, 2, 2, "c"],
	],
	# 12 sleep — shell settled low, head fully withdrawn, eye closed
	[
		[6, 7, 4, 1, "c"],  # shell top ridge, settled
		[4, 8, 8, 1, "c"],
		[2, 9, 11, 4, "c"],  # shell bulk, settled
		[4, 13, 7, 1, "u"],  # underside grounded
		[10, 9, 3, 2, "c"],  # head withdrawn
		[1, 10, 1, 1, "c"],  # tail
	],
	# 13 sleep B — the inhale: back rises a pixel
	[
		[6, 6, 4, 1, "c"],
		[4, 7, 8, 1, "c"],
		[2, 8, 11, 4, "c"],
		[4, 13, 7, 1, "u"],
		[10, 8, 3, 2, "c"],
		[1, 9, 1, 1, "c"],
	],
]
