extends RefCounted
# Wader — prey-family/quadruped rig (mammal_sprites RigKind.PREY, small
# aquatic-adjacent herbivore). 16x16, facing right. A wading bird: an oval
# body on two long thin legs, a folded wing doubling as the tail read, a long
# neck up to a small head with a pointed beak. Same 14-slot layout as Wolf
# (two legs instead of four — the shared shader/slot contract doesn't require
# a quadruped stance): 0 stand, 1/2/3 stride (contact-L / passing /
# contact-R), 4/5 peck (neck dips to the ground/water), 6/7 jab (the beak
# thrust — the "attack" slot), 8/9 alert (neck stretched up), 10/11 flee
# (wings spread, stretched stride), 12/13 sleep (neck tucked). Blocks are
# [x, y, w, h, zone]; painted back-to-front, auto-outlined.

const POSES: Array = [
	# 0 stand
	[
		[5, 7, 6, 3, "c"],  # oval torso
		[3, 7, 2, 2, "c"],  # folded wing / tail
		[10, 3, 2, 5, "c"],  # long neck
		[11, 1, 3, 2, "c"],  # head
		[14, 2, 2, 1, "n"],  # beak
		[12, 1, 1, 1, "e"],  # eye
		[6, 10, 1, 6, "c"],
		[9, 10, 1, 6, "c"],  # long thin legs
	],
	# 1 contact-L — near leg forward-planted, far leg lifted
	[
		[5, 7, 6, 3, "c"],
		[3, 7, 2, 2, "c"],
		[10, 3, 2, 5, "c"],
		[11, 1, 3, 2, "c"],
		[14, 2, 2, 1, "n"],
		[12, 1, 1, 1, "e"],
		[5, 10, 1, 6, "c"],
		[10, 9, 1, 5, "c"],
	],
	# 2 passing — legs gathered under, whole figure lifted 1px (the stride bob)
	[
		[5, 6, 6, 3, "c"],
		[3, 6, 2, 2, "c"],
		[10, 2, 2, 5, "c"],
		[11, 0, 3, 2, "c"],
		[14, 1, 2, 1, "n"],
		[12, 0, 1, 1, "e"],
		[7, 9, 1, 5, "c"],
		[8, 9, 1, 5, "c"],
	],
	# 3 contact-R — opposite leg pairing
	[
		[5, 7, 6, 3, "c"],
		[3, 7, 2, 2, "c"],
		[10, 3, 2, 5, "c"],
		[11, 1, 3, 2, "c"],
		[14, 2, 2, 1, "n"],
		[12, 1, 1, 1, "e"],
		[6, 9, 1, 5, "c"],
		[9, 10, 1, 6, "c"],
	],
	# 4 peck A — neck dips the head toward the ground/water
	[
		[5, 7, 6, 3, "c"],
		[3, 7, 2, 2, "c"],
		[10, 6, 2, 7, "c"],
		[11, 11, 3, 2, "c"],
		[14, 12, 2, 1, "n"],
		[12, 11, 1, 1, "e"],
		[6, 10, 1, 6, "c"],
		[9, 10, 1, 6, "c"],
	],
	# 5 peck B — beak touches the water
	[
		[5, 7, 6, 3, "c"],
		[3, 7, 2, 2, "c"],
		[10, 6, 2, 8, "c"],
		[11, 12, 3, 2, "c"],
		[14, 13, 2, 1, "n"],
		[12, 12, 1, 1, "e"],
		[6, 10, 1, 6, "c"],
		[9, 10, 1, 6, "c"],
	],
	# 6 jab A — neck draws back (wind-up)
	[
		[5, 7, 6, 3, "c"],
		[3, 7, 2, 2, "c"],
		[8, 2, 2, 5, "c"],
		[9, 0, 3, 2, "c"],
		[12, 1, 2, 1, "n"],
		[10, 0, 1, 1, "e"],
		[6, 10, 1, 6, "c"],
		[9, 10, 1, 6, "c"],
	],
	# 7 jab B — beak thrusts forward, out past the frame edge
	[
		[5, 7, 6, 3, "c"],
		[3, 7, 2, 2, "c"],
		[10, 3, 3, 4, "c"],
		[13, 2, 3, 2, "c"],
		[15, 3, 1, 1, "n"],
		[14, 2, 1, 1, "e"],
		[6, 10, 1, 6, "c"],
		[9, 10, 1, 6, "c"],
	],
	# 8 alert A — neck stretched fully upright
	[
		[5, 7, 6, 3, "c"],
		[3, 7, 2, 2, "c"],
		[10, 2, 2, 6, "c"],
		[11, 0, 3, 2, "c"],
		[14, 1, 2, 1, "n"],
		[12, 0, 1, 1, "e"],
		[6, 10, 1, 6, "c"],
		[9, 10, 1, 6, "c"],
	],
	# 9 alert B — a small weight shift
	[
		[5, 7, 6, 3, "c"],
		[3, 7, 2, 2, "c"],
		[9, 2, 2, 6, "c"],
		[10, 0, 3, 2, "c"],
		[13, 1, 2, 1, "n"],
		[11, 0, 1, 1, "e"],
		[6, 10, 1, 6, "c"],
		[9, 10, 1, 6, "c"],
	],
	# 10 flee A — wings spread, body stretched into a run
	[
		[4, 7, 7, 3, "c"],
		[2, 5, 3, 3, "c"],  # wing spread wide
		[11, 3, 2, 5, "c"],
		[12, 1, 3, 2, "c"],
		[15, 2, 1, 1, "n"],
		[13, 1, 1, 1, "e"],
		[2, 10, 1, 6, "c"],  # hind flung back
		[11, 10, 1, 6, "c"],  # fore reaching
	],
	# 11 flee B — gathered mid-stride
	[
		[5, 7, 7, 3, "c"],
		[3, 5, 3, 3, "c"],
		[11, 3, 2, 5, "c"],
		[12, 1, 3, 2, "c"],
		[15, 2, 1, 1, "n"],
		[13, 1, 1, 1, "e"],
		[7, 10, 1, 6, "c"],
		[9, 10, 1, 6, "c"],
	],
	# 12 sleep — settled low, neck tucked back over the body, one leg drawn
	# up, eye closed
	[
		[5, 9, 6, 4, "c"],  # body settled
		[3, 9, 2, 2, "c"],  # wing
		[9, 8, 3, 3, "c"],  # neck tucked
		[10, 7, 3, 2, "c"],  # head resting
		[13, 8, 1, 1, "n"],  # beak
		[7, 13, 1, 2, "c"],  # single standing leg, tucked
	],
	# 13 sleep B — the inhale: back rises a pixel
	[
		[5, 8, 6, 4, "c"],
		[3, 8, 2, 2, "c"],
		[9, 7, 3, 3, "c"],
		[10, 6, 3, 2, "c"],
		[13, 7, 1, 1, "n"],
		[7, 13, 1, 2, "c"],
	],
]
