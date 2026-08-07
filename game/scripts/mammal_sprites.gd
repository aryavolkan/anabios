extends RefCounted
# Archetype registry for the field figures. An agent's *archetype* (shape +
# animation rig) is chosen from its diet, body size, and livestock status; its
# *coat* is a per-species tint applied as a per-instance modulate over the
# archetype's neutral value-ramp atlas. The Primate archetype is the exception:
# it delegates to ape_sprites.gd, which bakes the five hominins' own colours.

const ApeSprites = preload("res://scripts/ape_sprites.gd")

enum { HARE, DEER, BOAR, PRIMATE, FOX, WOLF, LIVESTOCK }
const ARCHETYPE_COUNT := 7
const NAMES: PackedStringArray = ["Hare", "Deer", "Boar", "Primate", "Fox", "Wolf", "Livestock"]

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
]

# Pose strip is the same 12-slot layout as the apes so one shader serves all.
const POSE_COUNT := ApeSprites.POSE_COUNT

# Selection thresholds (tunable; validated in Task 9's capture pass).
const SIZE_SPLIT := 1.25
const HERB_MAX := 0.34
const CARN_MIN := 0.66


static func rig_kind(archetype: int) -> int:
	return _RIG_KIND[archetype]


# Pure archetype selector. `size` in world units (0.5..3.0), `diet` carnivory
# 0..1. Stable per agent (diet/size fixed at birth) so no per-frame flicker.
static func archetype_for(diet: float, size: float, livestock: bool) -> int:
	if livestock:
		return LIVESTOCK
	var large := size >= SIZE_SPLIT
	if diet < HERB_MAX:
		return DEER if large else HARE
	if diet < CARN_MIN:
		return PRIMATE if large else BOAR
	return WOLF if large else FOX


static func primate_skin_for(species_id: int) -> int:
	return ApeSprites.ape_for_species(species_id)


# --- Render-bucket table ---------------------------------------------------
# Primate is NOT a single mesh: its five hominins keep their own buckets (0..4,
# byte-identical to today), and the six quadruped archetypes take buckets 5..10.
const SKIN_COUNT := ApeSprites.SPECIES_COUNT  # 5 hominins
const QUAD_ORDER: Array = [HARE, DEER, BOAR, FOX, WOLF, LIVESTOCK]
const BUCKET_COUNT := SKIN_COUNT + 6  # 11

# Quadruped pose data, filled in Tasks 6-7. Absent archetypes fall back to ape
# skin-0 art (harmless: those buckets carry 0 instances until their rig lands).
const _QUAD_DATA := {}


# An agent's render bucket: hominin skin (0..4) for Primate, else the quad slot.
static func bucket_of(archetype: int, species_id: int) -> int:
	if archetype == PRIMATE:
		return primate_skin_for(species_id)
	return SKIN_COUNT + QUAD_ORDER.find(archetype)


static func bucket_atlas(b: int) -> ImageTexture:
	if b < SKIN_COUNT:
		return ApeSprites.build_species_atlas(b)
	var arch: int = QUAD_ORDER[b - SKIN_COUNT]
	if _QUAD_DATA.has(arch):
		return build_quad_atlas(_QUAD_DATA[arch].POSES)
	return ApeSprites.build_species_atlas(0)  # fallback until the rig lands


static func bucket_fallen(b: int) -> ImageTexture:
	if b < SKIN_COUNT:
		return ApeSprites.build_fallen_texture(b)
	var arch: int = QUAD_ORDER[b - SKIN_COUNT]
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
	# Quad cadence: hares scurry, deer lope, boar trot, fox/wolf trot, cattle amble.
	return [7.0, 4.6, 5.2, 6.5, 5.6, 4.0][b - SKIN_COUNT]


# --- Quadruped atlas + per-species coat hue --------------------------------
# Neutral value-ramp for quadruped rigs: coat mid, underside light, eye dark,
# nose a touch lighter than coat. A per-instance coat-hue modulate (main.gd
# _body_colors) turns this into a counter-shaded coloured animal.
const QUAD_ZONES := {
	"c": Color(0.60, 0.60, 0.60),
	"u": Color(0.92, 0.92, 0.92),
	"n": Color(0.74, 0.74, 0.74),
	"e": Color(0.10, 0.10, 0.11),
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


# One rig's 12 poses baked into a 16x(12*16) neutral strip (pre-flipped for the
# QuadMesh's flipped V, same as ApeSprites.build_species_atlas).
static func build_quad_atlas(poses: Array) -> ImageTexture:
	var atlas := Image.create(16, POSE_COUNT * 16, false, Image.FORMAT_RGBA8)
	atlas.fill(Color(0, 0, 0, 0))
	for fr in POSE_COUNT:
		var cell: Image = _build_quad_cell(poses[fr])
		cell.flip_y()
		atlas.blit_rect(cell, Rect2i(0, 0, 16, 16), Vector2i(0, fr * 16))
	return ImageTexture.create_from_image(atlas)


# Fallen ghost: neutral pose rotated 90 CW, matching ApeSprites.build_fallen_texture.
static func build_quad_fallen(poses: Array) -> ImageTexture:
	var blocks: Array = []
	for b in poses[0]:
		blocks.append([15 - (b[1] + b[3]), b[0], b[3], b[2], b[4]])
	var cell: Image = _build_quad_cell(blocks)
	cell.flip_y()
	return ImageTexture.create_from_image(cell)


# Resolve a rig's zone-keyed blocks to explicit Colours via QUAD_ZONES, then
# reuse the shared ApeSprites._build_cell (Color-aware after Step below) so the
# 1px auto-outline pass is written once, not duplicated.
static func _build_quad_cell(pose: Array) -> Image:
	var blocks: Array = []
	for b in pose:
		blocks.append([b[0], b[1], b[2], b[3], QUAD_ZONES[b[4]]])
	return ApeSprites._build_cell(blocks)
