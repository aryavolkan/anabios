extends RefCounted
# Archetype registry for the field figures. An agent's *archetype* (shape +
# animation rig) is chosen from its diet, body size, and livestock status; its
# *coat* is a per-species tint applied as a per-instance modulate over the
# archetype's neutral value-ramp atlas. The Primate archetype is the exception:
# it delegates to ape_sprites.gd, which bakes the five hominins' own colours.

const ApeSprites = preload("res://scripts/ape_sprites.gd")
const HeroRigs = preload("res://scripts/hero_rigs.gd")
const HomininRigs = preload("res://scripts/hominin_rigs.gd")

enum {
	HARE,
	DEER,
	BOAR,
	PRIMATE,
	FOX,
	WOLF,
	LIVESTOCK,
	TORTOISE,
	PORCUPINE,
	MAMMOTH,
	WADER,
}
const ARCHETYPE_COUNT := 11
const NAMES: PackedStringArray = [
	"Hare",
	"Deer",
	"Boar",
	"Primate",
	"Fox",
	"Wolf",
	"Livestock",
	"Tortoise",
	"Porcupine",
	"Mammoth",
	"Wader",
]

# Signature-move family, passed to the shader as the `rig_kind` uniform.
enum RigKind { PREY, PREDATOR, PRIMATE_RIG, LIVESTOCK_RIG }
const _RIG_KIND: PackedInt32Array = [
	RigKind.PREY,  # HARE
	RigKind.PREY,  # DEER
	RigKind.PREY,  # BOAR (omnivore, but prey-family gait/flourish)
	RigKind.PRIMATE_RIG,  # PRIMATE
	RigKind.PREDATOR,  # FOX
	RigKind.PREDATOR,  # WOLF
	RigKind.LIVESTOCK_RIG,  # LIVESTOCK
	RigKind.PREY,  # TORTOISE
	RigKind.PREY,  # PORCUPINE
	RigKind.PREY,  # MAMMOTH
	RigKind.PREY,  # WADER
]

# Pose grid is the same 16-slot layout as the apes so one shader serves all.
const POSE_COUNT := ApeSprites.POSE_COUNT

# Selection thresholds (tunable; validated in Task 9's capture pass).
const SIZE_SPLIT := 1.25
const HERB_MAX := 0.34
const CARN_MIN := 0.66

# Module-keyed body tags (crates/anabios-godot `alive_body_tags` bit layout;
# also the 8th column of `alive_render_state`). Bit per module family the
# archetype selector cares about, independent of diet/size.
const TAG_ARMOR := 1 << 0
const TAG_SPINES := 1 << 1
const TAG_JAWS := 1 << 2
const TAG_STORAGE := 1 << 3
const TAG_LOCOMOTOR2 := 1 << 4


static func rig_kind(archetype: int) -> int:
	return _RIG_KIND[archetype]


# Pure archetype selector. `size` in world units (0.5..3.0), `diet` carnivory
# 0..1. Stable per agent (diet/size fixed at birth) so no per-frame flicker.
# `tags` (default 0, see the TAG_* bits above) layers module-keyed families
# on top of the diet/size table: livestock still wins outright, then armour
# on a non-carnivore reads as a tortoise, spines as a porcupine at any diet,
# a large Storage-bearing herbivore as a mammoth, and a small herbivore with
# two-or-more Locomotor modules as a wading bird. Anything else falls through
# to the original hare/deer/boar/primate/fox/wolf table unchanged.
static func archetype_for(diet: float, size: float, livestock: bool, tags: int = 0) -> int:
	if livestock:
		return LIVESTOCK
	var large := size >= SIZE_SPLIT
	var herbivore := diet < HERB_MAX
	if (tags & TAG_ARMOR) != 0 and diet < CARN_MIN:
		return TORTOISE
	if (tags & TAG_SPINES) != 0:
		return PORCUPINE
	if large and herbivore and (tags & TAG_STORAGE) != 0:
		return MAMMOTH
	if not large and herbivore and (tags & TAG_LOCOMOTOR2) != 0:
		return WADER
	if herbivore:
		return DEER if large else HARE
	if diet < CARN_MIN:
		return PRIMATE if large else BOAR
	return WOLF if large else FOX


static func primate_skin_for(species_id: int) -> int:
	return ApeSprites.ape_for_species(species_id)


# --- Render-bucket table ---------------------------------------------------
# Primate is NOT a single mesh: its five hominins keep their own buckets (0..4,
# byte-identical to today), and the ten quadruped archetypes take buckets 5..14.
const SKIN_COUNT := ApeSprites.SPECIES_COUNT  # 5 hominins
const QUAD_ORDER: Array = [
	HARE, DEER, BOAR, FOX, WOLF, LIVESTOCK, TORTOISE, PORCUPINE, MAMMOTH, WADER
]
const BUCKET_COUNT := SKIN_COUNT + 10  # 15

# Quadruped pose data, filled in Tasks 6-7. Absent archetypes fall back to ape
# skin-0 art (harmless: those buckets carry 0 instances until their rig lands).
const _QUAD_DATA := {
	HARE: preload("res://scripts/mammal_data/hare.gd"),
	DEER: preload("res://scripts/mammal_data/deer.gd"),
	BOAR: preload("res://scripts/mammal_data/boar.gd"),
	FOX: preload("res://scripts/mammal_data/fox.gd"),
	WOLF: preload("res://scripts/mammal_data/wolf.gd"),
	LIVESTOCK: preload("res://scripts/mammal_data/livestock.gd"),
	TORTOISE: preload("res://scripts/mammal_data/tortoise.gd"),
	PORCUPINE: preload("res://scripts/mammal_data/porcupine.gd"),
	MAMMOTH: preload("res://scripts/mammal_data/mammoth.gd"),
	WADER: preload("res://scripts/mammal_data/wader.gd"),
}


# An agent's render bucket: hominin skin (0..4) for Primate, else the quad slot.
static func bucket_of(archetype: int, species_id: int) -> int:
	if archetype == PRIMATE:
		return primate_skin_for(species_id)
	return SKIN_COUNT + QUAD_ORDER.find(archetype)


# Every bucket's hero atlas packed into one square grid (ATLAS_GRID x
# ATLAS_GRID cells of HERO_ATLAS_PX), bucket b at cell (b % ATLAS_GRID,
# b / ATLAS_GRID), so all figures can share ONE MultiMesh and y-sort across
# species; the field shader picks the bucket cell from the instance colour's
# alpha. Square, as the Metal MultiMesh path requires.
const ATLAS_GRID := 4
const COMBINED_ATLAS_PX := ATLAS_GRID * ApeSprites.HERO_ATLAS_PX


static func combined_atlas() -> ImageTexture:
	var side: int = COMBINED_ATLAS_PX
	var img := Image.create(side, side, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for b in BUCKET_COUNT:
		var cell: Image = bucket_atlas(b).get_image()
		var px: int = ApeSprites.HERO_ATLAS_PX
		img.blit_rect(
			cell,
			Rect2i(0, 0, px, px),
			Vector2i((b % ATLAS_GRID) * px, int(b / float(ATLAS_GRID)) * px)
		)
	return ImageTexture.create_from_image(img)


# The instance-colour alpha that names bucket `b` to the field shader (the
# live bodies never use alpha for fading, so the channel is free).
static func bucket_alpha(b: int) -> float:
	return (float(b) + 0.5) / float(ATLAS_GRID * ATLAS_GRID)


static func bucket_atlas(b: int) -> ImageTexture:
	if b < SKIN_COUNT:
		if HomininRigs.has(b):
			return ApeSprites.pack_cells(HomininRigs.build_cells(b))
		return ApeSprites.build_species_atlas(b)
	var arch: int = QUAD_ORDER[b - SKIN_COUNT]
	# Hand-authored 24 px silhouettes first (hero_rigs.gd); the scaled rect
	# rig is the fallback for an archetype without a master.
	if HeroRigs.has(NAMES[arch]):
		return ApeSprites.pack_cells(HeroRigs.build_cells(NAMES[arch]))
	if _QUAD_DATA.has(arch):
		return build_quad_atlas(_with_celebration_poses(_QUAD_DATA[arch].POSES), arch)
	return ApeSprites.build_species_atlas(0)  # fallback until the rig lands


# Single-pose portrait for the inspector's avatar tile: the archetype's neutral
# (frame 0) pose, upright. The field atlases are pre-flipped for the MultiMesh
# quad's V axis, so they cannot be shown directly in a TextureRect — and the
# inspector used to fall back to an ape portrait for every quadruped, labelling
# a chimp silhouette "Deer". Pair with coat_hue() as the TextureRect modulate.
static func portrait(archetype: int) -> ImageTexture:
	if archetype >= 0 and archetype < NAMES.size() and HeroRigs.has(NAMES[archetype]):
		return ImageTexture.create_from_image(
			HeroRigs.build_cell(HeroRigs.MASTERS[NAMES[archetype].to_upper()], 0)
		)
	if not _QUAD_DATA.has(archetype):
		return ApeSprites.build(0)
	return ImageTexture.create_from_image(
		_build_quad_cell(_QUAD_DATA[archetype].POSES[0], archetype)
	)


# A hominin's standing master as a HUD portrait (unit card, codex).
static func hominin_portrait(species: int) -> ImageTexture:
	if HomininRigs.has(species):
		return ImageTexture.create_from_image(HomininRigs.build_cell(species, 0))
	return ApeSprites.build(species)


static func bucket_fallen(b: int) -> ImageTexture:
	if b < SKIN_COUNT:
		if HomininRigs.has(b):
			var hcell: Image = HomininRigs.build_cell(b, 0)
			hcell.rotate_90(CLOCKWISE)
			hcell.flip_y()
			return ImageTexture.create_from_image(hcell)
		return ApeSprites.build_fallen_texture(b)
	var arch: int = QUAD_ORDER[b - SKIN_COUNT]
	if HeroRigs.has(NAMES[arch]):
		# The standing master on its side, pre-flipped like the atlas cells.
		var cell: Image = HeroRigs.build_cell(HeroRigs.MASTERS[NAMES[arch].to_upper()], 0)
		cell.rotate_90(CLOCKWISE)
		cell.flip_y()
		return ImageTexture.create_from_image(cell)
	if _QUAD_DATA.has(arch):
		return build_quad_fallen(_QUAD_DATA[arch].POSES)
	return ApeSprites.build_fallen_texture(0)


static func bucket_rig_kind(b: int) -> int:
	if b < SKIN_COUNT:
		return RigKind.PRIMATE_RIG
	return rig_kind(QUAD_ORDER[b - SKIN_COUNT])


static func bucket_gait_fps(b: int) -> float:
	if b < SKIN_COUNT:
		return ApeSprites.WALK_FPS[b]
	# Quad cadence: hares scurry, deer lope, boar trot, fox/wolf trot, cattle
	# amble, tortoises plod, porcupines scuttle, mammoths lumber, waders stride.
	return [7.0, 4.6, 5.2, 6.5, 5.6, 4.0, 1.8, 4.2, 2.2, 5.5][b - SKIN_COUNT]


# --- Quadruped atlas + per-species coat hue --------------------------------
# Neutral value-ramp for quadruped rigs: coat mid, underside light, eye dark,
# nose a touch lighter than coat. A per-instance coat-hue modulate (main.gd
# _body_colors) turns this into a counter-shaded coloured animal.
const QUAD_ZONES := {
	"c": Color(0.60, 0.60, 0.60),
	"u": Color(0.92, 0.92, 0.92),
	"n": Color(0.74, 0.74, 0.74),
	"e": Color(0.10, 0.10, 0.11),
	"i": Color(0.96, 0.94, 0.86),  # ivory (tusks)
	"h": Color(0.30, 0.28, 0.25),  # horn (antlers, horns)
}

# Hero-atlas accents per archetype: [dx, dy, w, h, zone] in 24 px cell
# pixels, placed from the head block's top-right corner (see
# ApeSprites._build_pose). They follow the head through every pose, so a
# grazing deer dips its antlers and a sleeping one lays them down.
const ACCENTS := {
	DEER: [[-4, -5, 1, 5, "h"], [-2, -6, 1, 6, "h"], [-5, -3, 1, 1, "h"], [-1, -4, 1, 1, "h"]],
	MAMMOTH: [[0, 3, 3, 1, "i"], [2, 4, 1, 2, "i"]],
	LIVESTOCK: [[-4, -2, 1, 2, "h"], [-1, -2, 1, 2, "h"]],
	HARE: [[-4, -5, 1, 5, "c"], [-2, -6, 1, 6, "c"]],
	FOX: [[-4, -2, 1, 2, "c"], [-1, -2, 1, 2, "c"]],
	WOLF: [[-4, -2, 1, 2, "c"], [-1, -2, 1, 2, "c"]],
	BOAR: [[0, 2, 1, 1, "i"]],
	WADER: [[-5, -2, 1, 2, "c"]],
}

# Per-archetype coat palette band: [base_hue, hue_jitter, saturation, value].
# Species id jitters the hue within the band so a herd varies without leaving
# the archetype's look (foxes rusty, wolves grey-brown, deer tan, hares sandy).
const _COAT_BAND := {
	HARE: [0.09, 0.05, 0.35, 0.82],
	DEER: [0.07, 0.04, 0.45, 0.72],
	BOAR: [0.05, 0.03, 0.30, 0.55],
	FOX: [0.045, 0.02, 0.75, 0.90],
	WOLF: [0.08, 0.06, 0.18, 0.62],
	LIVESTOCK: [0.08, 0.10, 0.25, 0.80],
	TORTOISE: [0.12, 0.04, 0.35, 0.55],
	PORCUPINE: [0.08, 0.03, 0.30, 0.42],
	MAMMOTH: [0.07, 0.03, 0.15, 0.45],
	WADER: [0.53, 0.05, 0.10, 0.88],
}


# Per-agent coat modulate. PRIMATE returns white (ape atlases carry their own
# colours); quad archetypes return a stable per-species hue within their band.
static func coat_hue(archetype: int, species_id: int) -> Color:
	if not _COAT_BAND.has(archetype):
		return Color(1, 1, 1)
	var band: Array = _COAT_BAND[archetype]
	var j := sin(float(species_id) * 12.9898)  # deterministic jitter in [-1, 1]
	var hue: float = fposmod(band[0] + j * band[1], 1.0)
	return Color.from_hsv(hue, band[2], band[3])


# One rig's poses baked into the shared 64x64 grid atlas (pre-flipped for
# the QuadMesh's flipped V). QUAD_ZONES already maps each zone key to an
# explicit neutral Colour, so ApeSprites._pack_grid resolves it the same way
# the ape atlas resolves its PAL keys. Square-grid layout (not a 16x192 strip)
# avoids the Metal MultiMesh2D texture corruption — see ApeSprites.ATLAS_PX.
static func build_quad_atlas(poses: Array, archetype: int = -1) -> ImageTexture:
	return ApeSprites._pack_grid(poses, QUAD_ZONES, true, ACCENTS.get(archetype, []))


# Quadruped rigs share the 14 authored cells, but the celebration action needs
# two additional cells to match the primate atlas. Generate those cells from
# each rig's alert pair and add a tiny raised-tail flag; this keeps the animal's
# authored silhouette and palette while giving the action a concrete 2D asset.
# A quad atlas is complete at POSE_SPEAR (16) cells: the weapon pairs beyond
# it are ape-only art that quads deliberately never carry.
static func _with_celebration_poses(poses: Array) -> Array:
	if poses.size() >= ApeSprites.POSE_SPEAR:
		return poses
	var out: Array = poses.duplicate(true)
	for idx in [8, 9]:
		var cheer: Array = poses[idx].duplicate(true)
		cheer.append([0, 2 if idx == 8 else 1, 1, 2, "c"])
		out.append(cheer)
	return out


# Fallen ghost: neutral pose rotated 90 CW, matching ApeSprites.build_fallen_texture.
static func build_quad_fallen(poses: Array) -> ImageTexture:
	var blocks: Array = []
	for b in poses[0]:
		blocks.append([15 - (b[1] + b[3]), b[0], b[3], b[2], b[4]])
	var cell: Image = ApeSprites._build_pose(blocks, QUAD_ZONES, true)
	cell.flip_y()
	return ImageTexture.create_from_image(cell)


# Resolve a rig's zone-keyed blocks to explicit Colours via QUAD_ZONES, then
# reuse the shared ApeSprites._build_cell (Color-aware after Step below) so the
# 1px auto-outline pass is written once, not duplicated.
static func _build_quad_cell(pose: Array, archetype: int = -1) -> Image:
	return ApeSprites._build_pose(pose, QUAD_ZONES, true, ACCENTS.get(archetype, []))
