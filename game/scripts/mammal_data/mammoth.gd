extends RefCounted
# Mammoth — prey-family/quadruped rig (mammal_sprites RigKind.PREY, large
# herbivore). 16x16, facing right. Big bulky torso, thick legs, a large ear,
# a pale tusk, and a trunk that hangs from the head and reaches to the ground
# to graze or curls up to trumpet. Same 14-slot layout as Wolf: 0 stand,
# 1/2/3 amble (contact-L / passing / contact-R), 4/5 graze (trunk reaches
# down), 6/7 trumpet (head up, trunk curls — the "attack" slot), 8/9 alert
# (ear and neck up), 10/11 gallop (charge), 12/13 sleep (trunk curled at
# rest). Blocks are [x, y, w, h, zone]; painted back-to-front, auto-outlined.

const POSES: Array = [
	# 0 stand
	[
		[2, 4, 11, 7, "c"],  # bulky torso
		[1, 5, 1, 2, "c"],  # tail
		[10, 2, 3, 3, "c"],  # big flappy ear
		[11, 3, 4, 4, "c"],  # head
		[3, 10, 9, 1, "u"],  # underbelly
		[14, 6, 2, 4, "c"],  # trunk
		[15, 9, 1, 1, "n"],  # trunk tip
		[13, 7, 2, 1, "u"],  # tusk
		[12, 3, 1, 1, "e"],  # eye
		[3, 11, 2, 4, "c"],
		[6, 11, 2, 4, "c"],  # hind legs, thick
		[9, 11, 2, 4, "c"],
		[11, 11, 2, 4, "c"],  # fore legs, thick
	],
	# 1 contact-L — near-hind + far-fore reach forward
	[
		[2, 4, 11, 7, "c"],
		[1, 5, 1, 2, "c"],
		[10, 2, 3, 3, "c"],
		[11, 3, 4, 4, "c"],
		[3, 10, 9, 1, "u"],
		[14, 6, 2, 4, "c"],
		[15, 9, 1, 1, "n"],
		[13, 7, 2, 1, "u"],
		[12, 3, 1, 1, "e"],
		[2, 12, 2, 3, "c"],
		[7, 11, 2, 4, "c"],
		[8, 12, 2, 3, "c"],
		[12, 11, 2, 4, "c"],
	],
	# 2 passing — legs gathered, whole figure lifted 1px (the amble bob)
	[
		[2, 3, 11, 7, "c"],
		[1, 4, 1, 2, "c"],
		[10, 1, 3, 3, "c"],
		[11, 2, 4, 4, "c"],
		[3, 9, 9, 1, "u"],
		[14, 5, 2, 4, "c"],
		[15, 8, 1, 1, "n"],
		[13, 6, 2, 1, "u"],
		[12, 2, 1, 1, "e"],
		[4, 10, 2, 4, "c"],
		[6, 10, 2, 4, "c"],
		[9, 10, 2, 4, "c"],
		[11, 10, 2, 4, "c"],
	],
	# 3 contact-R — mirror diagonal of pose 1
	[
		[2, 4, 11, 7, "c"],
		[1, 5, 1, 2, "c"],
		[10, 2, 3, 3, "c"],
		[11, 3, 4, 4, "c"],
		[3, 10, 9, 1, "u"],
		[14, 6, 2, 4, "c"],
		[15, 9, 1, 1, "n"],
		[13, 7, 2, 1, "u"],
		[12, 3, 1, 1, "e"],
		[3, 12, 2, 3, "c"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"],
		[12, 12, 2, 3, "c"],
	],
	# 4 graze A — trunk reaches down toward the ground
	[
		[2, 4, 11, 7, "c"],
		[1, 5, 1, 2, "c"],
		[10, 2, 3, 3, "c"],
		[11, 5, 4, 4, "c"],
		[3, 10, 9, 1, "u"],
		[14, 9, 2, 5, "c"],
		[15, 13, 1, 1, "n"],
		[13, 9, 2, 1, "u"],
		[12, 5, 1, 1, "e"],
		[3, 11, 2, 4, "c"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"],
		[11, 11, 2, 4, "c"],
	],
	# 5 graze B — trunk tip touches the ground
	[
		[2, 4, 11, 7, "c"],
		[1, 5, 1, 2, "c"],
		[10, 2, 3, 3, "c"],
		[11, 6, 4, 4, "c"],
		[3, 10, 9, 1, "u"],
		[14, 10, 2, 6, "c"],
		[15, 15, 1, 1, "n"],
		[13, 10, 2, 1, "u"],
		[12, 6, 1, 1, "e"],
		[3, 11, 2, 4, "c"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"],
		[11, 11, 2, 4, "c"],
	],
	# 6 trumpet A — ear and head raised, trunk curls up (wind-up)
	[
		[2, 4, 11, 7, "c"],
		[1, 5, 1, 2, "c"],
		[10, 1, 3, 3, "c"],
		[11, 1, 4, 4, "c"],
		[3, 10, 9, 1, "u"],
		[14, 0, 2, 3, "c"],
		[14, 0, 1, 1, "n"],
		[13, 4, 2, 1, "u"],
		[12, 1, 1, 1, "e"],
		[3, 11, 2, 4, "c"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"],
		[11, 11, 2, 4, "c"],
	],
	# 7 trumpet B — trunk flares fully up and out
	[
		[2, 4, 11, 7, "c"],
		[1, 5, 1, 2, "c"],
		[10, 1, 3, 3, "c"],
		[11, 1, 4, 4, "c"],
		[3, 10, 9, 1, "u"],
		[13, 0, 3, 2, "c"],
		[13, 0, 1, 1, "n"],
		[13, 4, 3, 1, "u"],
		[12, 1, 1, 1, "e"],
		[3, 11, 2, 4, "c"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"],
		[11, 11, 2, 4, "c"],
	],
	# 8 alert A — ear raised tall, head up
	[
		[2, 4, 11, 7, "c"],
		[1, 5, 1, 2, "c"],
		[10, 0, 3, 4, "c"],
		[11, 2, 4, 4, "c"],
		[3, 10, 9, 1, "u"],
		[14, 5, 2, 4, "c"],
		[15, 8, 1, 1, "n"],
		[13, 6, 2, 1, "u"],
		[12, 2, 1, 1, "e"],
		[3, 11, 2, 4, "c"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"],
		[11, 11, 2, 4, "c"],
	],
	# 9 alert B — a small weight shift
	[
		[2, 4, 11, 7, "c"],
		[1, 5, 1, 2, "c"],
		[9, 0, 3, 4, "c"],
		[10, 2, 4, 4, "c"],
		[3, 10, 9, 1, "u"],
		[13, 5, 2, 4, "c"],
		[14, 8, 1, 1, "n"],
		[12, 6, 2, 1, "u"],
		[11, 2, 1, 1, "e"],
		[3, 11, 2, 4, "c"],
		[6, 11, 2, 4, "c"],
		[9, 11, 2, 4, "c"],
		[11, 11, 2, 4, "c"],
	],
	# 10 gallop A — body stretched into a charge
	[
		[1, 5, 12, 6, "c"],
		[0, 6, 1, 2, "c"],
		[11, 2, 3, 3, "c"],
		[12, 3, 4, 4, "c"],
		[2, 10, 10, 1, "u"],
		[15, 6, 1, 4, "c"],
		[15, 9, 1, 1, "n"],
		[14, 7, 1, 1, "u"],
		[13, 3, 1, 1, "e"],
		[0, 11, 3, 5, "c"],  # hind flung back
		[13, 11, 2, 4, "c"],  # fore reaching
	],
	# 11 gallop B — gathered mid-stride, compressed
	[
		[3, 5, 10, 6, "c"],
		[2, 6, 1, 2, "c"],
		[12, 2, 3, 3, "c"],
		[13, 3, 3, 4, "c"],
		[4, 10, 8, 1, "u"],
		[15, 6, 1, 4, "c"],
		[15, 9, 1, 1, "n"],
		[14, 7, 1, 1, "u"],
		[14, 3, 1, 1, "e"],
		[5, 11, 2, 4, "c"],
		[8, 11, 2, 4, "c"],
		[10, 11, 2, 4, "c"],
		[12, 11, 2, 4, "c"],
	],
	# 12 sleep — bulk settled low, ear drooped, trunk curled at rest, eye
	# closed
	[
		[2, 7, 11, 6, "c"],  # torso on the ground
		[1, 8, 1, 1, "c"],  # tail
		[10, 6, 3, 2, "c"],  # ear drooped
		[11, 7, 4, 3, "c"],  # head
		[3, 12, 9, 1, "u"],  # underbelly grounded
		[14, 9, 2, 3, "c"],  # trunk curled
		[14, 11, 1, 1, "n"],  # trunk tip
		[13, 10, 2, 1, "u"],  # tusk
	],
	# 13 sleep B — the inhale: back rises a pixel
	[
		[2, 6, 11, 6, "c"],
		[1, 7, 1, 1, "c"],
		[10, 5, 3, 2, "c"],
		[11, 6, 4, 3, "c"],
		[3, 12, 9, 1, "u"],
		[14, 8, 2, 3, "c"],
		[14, 10, 1, 1, "n"],
		[13, 9, 2, 1, "u"],
	],
]
