extends RefCounted
# Hare — prey/quadruped rig (mammal_sprites RigKind.PREY, small-bodied). 16x16,
# facing right. Compact rounded body, very short fore legs, one big folded
# hind haunch (the hop engine), two long ears laid back along the spine. Same
# 12-slot layout as Wolf: 0 stand, 1/2/3 amble (contact-L / passing /
# contact-R), 4/5 graze, 6/7 kick, 8/9 alert (ears prick up), 10/11 gallop
# (the big prey hop). Blocks are [x, y, w, h, zone]; painted back-to-front,
# auto-outlined.

const POSES: Array = [
	# 0 stand
	[
		[4, 6, 7, 5, "c"],  # compact rounded body
		[5, 9, 5, 1, "u"],  # underbelly
		[11, 5, 3, 3, "c"],  # small head
		[14, 6, 1, 1, "n"],  # muzzle
		[13, 5, 1, 1, "e"],  # eye
		[6, 2, 6, 1, "c"],
		[6, 3, 6, 1, "c"],  # long ears laid along the back
		[3, 8, 1, 1, "c"],  # tiny tail
		[3, 8, 3, 5, "c"],  # big folded hind haunch
		[10, 11, 1, 2, "c"],
		[9, 11, 1, 2, "c"],  # short fore legs
	],
	# 1 contact-L — fore paws step, hind haunch settles
	[
		[4, 6, 7, 5, "c"],
		[5, 9, 5, 1, "u"],
		[11, 5, 3, 3, "c"],
		[14, 6, 1, 1, "n"],
		[13, 5, 1, 1, "e"],
		[6, 2, 6, 1, "c"],
		[6, 3, 6, 1, "c"],
		[3, 8, 1, 1, "c"],
		[3, 8, 3, 5, "c"],
		[11, 11, 1, 2, "c"],
		[9, 10, 1, 2, "c"],
	],
	# 2 passing — figure lifted 1px, haunch tucked (the amble bob)
	[
		[4, 5, 7, 5, "c"],
		[5, 8, 5, 1, "u"],
		[11, 4, 3, 3, "c"],
		[14, 5, 1, 1, "n"],
		[13, 4, 1, 1, "e"],
		[6, 1, 6, 1, "c"],
		[6, 2, 6, 1, "c"],
		[3, 7, 1, 1, "c"],
		[4, 8, 3, 4, "c"],
		[9, 10, 1, 2, "c"],
		[11, 10, 1, 2, "c"],
	],
	# 3 contact-R — opposite paw forward
	[
		[4, 6, 7, 5, "c"],
		[5, 9, 5, 1, "u"],
		[11, 5, 3, 3, "c"],
		[14, 6, 1, 1, "n"],
		[13, 5, 1, 1, "e"],
		[6, 2, 6, 1, "c"],
		[6, 3, 6, 1, "c"],
		[3, 8, 1, 1, "c"],
		[3, 8, 3, 5, "c"],
		[9, 11, 1, 2, "c"],
		[11, 10, 1, 2, "c"],
	],
	# 4 graze A — head dips toward the ground
	[
		[4, 6, 7, 5, "c"],
		[5, 9, 5, 1, "u"],
		[11, 8, 3, 3, "c"],
		[14, 10, 1, 1, "n"],
		[13, 8, 1, 1, "e"],
		[7, 5, 5, 1, "c"],
		[7, 6, 5, 1, "c"],
		[3, 8, 1, 1, "c"],
		[3, 8, 3, 5, "c"],
		[10, 11, 1, 2, "c"],
		[9, 11, 1, 2, "c"],
	],
	# 5 graze B — nibbling low, a small dip
	[
		[4, 6, 7, 5, "c"],
		[5, 9, 5, 1, "u"],
		[11, 9, 3, 3, "c"],
		[14, 11, 1, 1, "n"],
		[13, 9, 1, 1, "e"],
		[7, 6, 5, 1, "c"],
		[7, 7, 5, 1, "c"],
		[3, 8, 1, 1, "c"],
		[3, 8, 3, 5, "c"],
		[10, 11, 1, 2, "c"],
		[9, 11, 1, 2, "c"],
	],
	# 6 kick A — reared up, ears flattened back (wind-up)
	[
		[4, 5, 7, 5, "c"],
		[5, 8, 5, 1, "u"],
		[11, 3, 3, 3, "c"],
		[14, 4, 1, 1, "n"],
		[13, 3, 1, 1, "e"],
		[6, 0, 6, 1, "c"],
		[6, 1, 6, 1, "c"],
		[3, 7, 1, 1, "c"],
		[3, 7, 3, 6, "c"],
		[10, 9, 1, 2, "c"],
		[12, 8, 1, 2, "c"],
	],
	# 7 kick B — fore paws snap out
	[
		[4, 5, 7, 5, "c"],
		[5, 8, 5, 1, "u"],
		[11, 3, 3, 3, "c"],
		[14, 4, 1, 1, "n"],
		[13, 3, 1, 1, "e"],
		[6, 0, 6, 1, "c"],
		[6, 1, 6, 1, "c"],
		[3, 7, 1, 1, "c"],
		[3, 7, 3, 6, "c"],
		[13, 7, 1, 3, "c"],
		[9, 10, 1, 2, "c"],
	],
	# 8 alert A — ears prick straight up
	[
		[4, 6, 7, 5, "c"],
		[5, 9, 5, 1, "u"],
		[11, 5, 3, 3, "c"],
		[14, 6, 1, 1, "n"],
		[13, 5, 1, 1, "e"],
		[10, 0, 1, 4, "c"],
		[12, 0, 1, 4, "c"],
		[3, 8, 1, 1, "c"],
		[3, 8, 3, 5, "c"],
		[10, 11, 1, 2, "c"],
		[9, 11, 1, 2, "c"],
	],
	# 9 alert B — a small weight shift
	[
		[4, 6, 7, 5, "c"],
		[5, 9, 5, 1, "u"],
		[11, 4, 3, 3, "c"],
		[14, 5, 1, 1, "n"],
		[13, 4, 1, 1, "e"],
		[9, 0, 1, 4, "c"],
		[11, 0, 1, 4, "c"],
		[3, 8, 1, 1, "c"],
		[3, 8, 3, 5, "c"],
		[10, 11, 1, 2, "c"],
		[9, 11, 1, 2, "c"],
	],
	# 10 gallop A — body stretched into the big prey hop
	[
		[3, 6, 8, 4, "c"],
		[4, 9, 6, 1, "u"],
		[11, 4, 3, 3, "c"],
		[14, 5, 1, 1, "n"],
		[13, 4, 1, 1, "e"],
		[7, 1, 6, 1, "c"],
		[7, 2, 6, 1, "c"],
		[2, 7, 1, 1, "c"],
		[0, 9, 3, 5, "c"],  # hind flung back
		[12, 9, 2, 4, "c"],  # fore reaching
	],
	# 11 gallop B — gathered mid-bound, compressed
	[
		[5, 6, 7, 4, "c"],
		[6, 9, 5, 1, "u"],
		[12, 4, 3, 3, "c"],
		[15, 5, 1, 1, "n"],
		[14, 4, 1, 1, "e"],
		[8, 1, 6, 1, "c"],
		[8, 2, 6, 1, "c"],
		[4, 7, 1, 1, "c"],
		[5, 9, 3, 5, "c"],
		[10, 10, 2, 3, "c"],
	],
]
