extends SceneTree
# Headless unit test for agent_layer.gd's pure static helpers: the
# view-rect arithmetic behind the Phase 3 visible-set culling
# (docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md, §6
# Phase 3), the torus-aware death-ghost rect check, the two-pointer id
# merge that the smoothing pass is built on, the idle-spear-ready act
# pick, and the drawn-centre click resolution behind pick() — the pieces
# most likely to regress. AgentLayer itself references no
# autoload (see camera_controller.gd
# and main.gd, which cannot compile under -s for that reason — noted in
# test_event_fx.gd), so the whole script preloads cleanly here. Run with:
#   godot --headless --rendering-driver dummy --path game \
#     -s res://scripts/test_agent_layer.gd
# Exits 0 on success, 1 on the first failed assertion.

const AgentLayer = preload("res://scripts/agent_layer.gd")
const MammalSprites = preload("res://scripts/mammal_sprites.gd")

var _failed := false


func _check(cond: bool, msg: String) -> void:
	if not cond:
		push_error("FAIL: " + msg)
		_failed = true


func _check_view_rect() -> void:
	# zoom 2x halves the world-space viewport extent; margin grows it back out
	# on every side.
	var r: Rect2 = AgentLayer.view_rect(Vector2(100, 50), 2.0, Vector2(800, 600), 64.0)
	_check(
		r.position.is_equal_approx(Vector2(-164, -164)),
		"view_rect position: expected (-164,-164), got %s" % r.position
	)
	_check(
		r.size.is_equal_approx(Vector2(528, 428)),
		"view_rect size: expected (528,428), got %s" % r.size
	)

	# zoom 1x, no margin: the rect is exactly the viewport in world units,
	# centred on the camera.
	var r2: Rect2 = AgentLayer.view_rect(Vector2.ZERO, 1.0, Vector2(200, 100), 0.0)
	_check(
		r2.position.is_equal_approx(Vector2(-100, -50)),
		"view_rect (no margin) position: expected (-100,-50), got %s" % r2.position
	)
	_check(
		r2.size.is_equal_approx(Vector2(200, 100)),
		"view_rect (no margin) size: expected (200,100), got %s" % r2.size
	)

	# A zero zoom must not divide by zero (defensive floor); the rect should
	# still come out finite and centred on cam_pos.
	var r3: Rect2 = AgentLayer.view_rect(Vector2(5, 5), 0.0, Vector2(100, 100), 10.0)
	_check(is_finite(r3.position.x) and is_finite(r3.size.x), "view_rect stays finite at zoom 0")
	_check(
		r3.get_center().is_equal_approx(Vector2(5, 5)),
		"view_rect stays centred on cam_pos at zoom 0, got center %s" % r3.get_center()
	)


func _check_pos_in_rect() -> void:
	# A plain non-wrapping rect.
	_check(
		AgentLayer.pos_in_rect(Vector2(5, 5), 0.0, 0.0, 10.0, 10.0, 100.0),
		"a point inside a normal rect is inside"
	)
	_check(
		not AgentLayer.pos_in_rect(Vector2(50, 5), 0.0, 0.0, 10.0, 10.0, 100.0),
		"a point outside a normal rect is outside"
	)

	# A rect that straddles the torus seam: [90, 110) on a world of size 100
	# wraps to cover [90,100) and [0,10).
	_check(
		AgentLayer.pos_in_rect(Vector2(95, 5), 90.0, 0.0, 110.0, 10.0, 100.0),
		"a point on the near side of a seam-straddling rect is inside"
	)
	_check(
		AgentLayer.pos_in_rect(Vector2(5, 5), 90.0, 0.0, 110.0, 10.0, 100.0),
		"a point wrapped onto the far side of a seam-straddling rect is inside"
	)
	_check(
		not AgentLayer.pos_in_rect(Vector2(50, 5), 90.0, 0.0, 110.0, 10.0, 100.0),
		"a point on neither side of a seam-straddling rect is outside"
	)


func _check_merge_prev() -> void:
	var prev_ids := PackedInt32Array([1, 3, 5])
	var prev_vals := PackedVector2Array([Vector2(1, 1), Vector2(3, 3), Vector2(5, 5)])
	var ids := PackedInt32Array([3, 4, 5, 6])
	var fallback := PackedVector2Array(
		[Vector2(30, 30), Vector2(40, 40), Vector2(50, 50), Vector2(60, 60)]
	)
	var out: PackedVector2Array = AgentLayer.merge_prev(prev_ids, prev_vals, ids, fallback)
	_check(out.size() == 4, "merge_prev returns one entry per current id")
	_check(out[0].is_equal_approx(Vector2(3, 3)), "id 3 matches prev id 3's value, got %s" % out[0])
	_check(out[1].is_equal_approx(Vector2(40, 40)), "id 4 (new) falls back, got %s" % out[1])
	_check(out[2].is_equal_approx(Vector2(5, 5)), "id 5 matches prev id 5's value, got %s" % out[2])
	_check(out[3].is_equal_approx(Vector2(60, 60)), "id 6 (new) falls back, got %s" % out[3])

	# No previous frame at all: every id falls back.
	var out_empty: PackedVector2Array = AgentLayer.merge_prev(
		PackedInt32Array(), PackedVector2Array(), ids, fallback
	)
	for i in out_empty.size():
		_check(
			out_empty[i].is_equal_approx(fallback[i]),
			"empty prev_ids falls back for every id (index %d)" % i
		)

	# Identical id sets: every entry matches, fallback unused entirely.
	var same_ids := PackedInt32Array([1, 3, 5])
	var out_same: PackedVector2Array = AgentLayer.merge_prev(
		prev_ids,
		prev_vals,
		same_ids,
		PackedVector2Array([Vector2(-1, -1), Vector2(-1, -1), Vector2(-1, -1)])
	)
	for i in out_same.size():
		_check(
			out_same[i].is_equal_approx(prev_vals[i]),
			"identical id sets match every entry, got %s at %d" % [out_same[i], i]
		)

	# A death (prev id 3 has no counterpart in ids) does not corrupt the
	# matches around it.
	var ids_no4 := PackedInt32Array([1, 5])
	var out_gap: PackedVector2Array = AgentLayer.merge_prev(
		prev_ids, prev_vals, ids_no4, PackedVector2Array([Vector2(-1, -1), Vector2(-1, -1)])
	)
	_check(out_gap[0].is_equal_approx(Vector2(1, 1)), "id 1 still matches across a gap")
	_check(out_gap[1].is_equal_approx(Vector2(5, 5)), "id 5 still matches across a gap")


func _check_crowd_cells() -> void:
	# The crowd cell is CROWD_CELL at CROWD_ZOOM and grows with sqrt(zoom).
	_check(
		is_equal_approx(AgentLayer.crowd_cell_for(AgentLayer.CROWD_ZOOM), AgentLayer.CROWD_CELL),
		"crowd cell is CROWD_CELL at CROWD_ZOOM"
	)
	var c4: float = AgentLayer.crowd_cell_for(4.0)
	var c8: float = AgentLayer.crowd_cell_for(8.0)
	_check(c4 > AgentLayer.CROWD_CELL and c8 > c4, "crowd cell grows with zoom")
	_check(is_equal_approx(c8, c4 * sqrt(2.0)), "crowd cell grows with sqrt(zoom)")
	_check(
		is_equal_approx(AgentLayer.crowd_cell_for(1.0), AgentLayer.CROWD_CELL),
		"below CROWD_ZOOM the cell does not shrink"
	)
	# Odd rows are staggered by half a cell: two figures the same distance
	# apart share a cell on an even row and split on the odd row above.
	var k_even_a := AgentLayer.crowd_key(Vector2(1.0, 1.0), 10.0)
	var k_even_b := AgentLayer.crowd_key(Vector2(9.0, 1.0), 10.0)
	_check(k_even_a == k_even_b, "same cell on an even row")
	var k_odd_a := AgentLayer.crowd_key(Vector2(1.0, 11.0), 10.0)
	var k_odd_b := AgentLayer.crowd_key(Vector2(9.0, 11.0), 10.0)
	_check(k_odd_a != k_odd_b, "odd rows are staggered by half a cell")
	_check(k_odd_a.y == 1 and k_even_a.y == 0, "row index is the plain cell row")


func _check_idle_weapon_act() -> void:
	# Genuinely idle (act still 0, not walking) with Hafted Spears researched:
	# carry the spear at rest instead of standing bare-handed.
	var spear_mask := 1 << 10  # FxMath.INV_HAFTED_SPEARS
	_check(
		is_equal_approx(
			AgentLayer.idle_weapon_act(0.0, false, spear_mask), AgentLayer.ACT_SPEAR_READY
		),
		"idle + spear tech -> ACT_SPEAR_READY"
	)
	# No spear tech: stays bare-handed (act unchanged at 0).
	_check(
		is_equal_approx(AgentLayer.idle_weapon_act(0.0, false, 0), 0.0),
		"idle + no spear tech -> unchanged 0.0"
	)
	# Walking, even with spear tech: not idle, so the pick does not apply.
	_check(
		is_equal_approx(AgentLayer.idle_weapon_act(0.0, true, spear_mask), 0.0),
		"walking + spear tech -> unchanged since not idle"
	)
	# A higher-priority act already picked (e.g. fighting): never overridden.
	_check(
		is_equal_approx(
			AgentLayer.idle_weapon_act(AgentLayer.ACT_FIGHT, false, spear_mask),
			AgentLayer.ACT_FIGHT
		),
		"a non-zero act (e.g. fight) is left alone even with spear tech"
	)


func _check_raw_walking() -> void:
	# Under turning inertia a resting body keeps a non-zero heading: with the
	# bridge's moving flags the heading must not count as walking, or no
	# sleeper, drinker or grazer ever reaches its idle pose.
	_check(not AgentLayer.raw_walking(true, 0, 1.2), "a still body with a heading stands")
	_check(AgentLayer.raw_walking(true, 1, 0.0), "a moving body walks whatever its heading")
	# Without the flags (an older bridge) the heading-is-0-at-rest contract.
	_check(AgentLayer.raw_walking(false, 0, 0.7), "no flags: a non-zero heading walks")
	_check(not AgentLayer.raw_walking(false, 0, 0.0), "no flags: heading 0 stands")


func _check_archetype_sizes() -> void:
	# A deer fawn at a third of its adult size must still be picked as a deer:
	# the archetype reads the adult size whenever the bridge supplies one.
	var grown := PackedFloat32Array([0.6, 2.0])
	var adult := PackedFloat32Array([1.8, 2.0])
	var picked: PackedFloat32Array = AgentLayer.archetype_sizes(adult, grown)
	_check(picked == adult, "archetype sizes are the adult sizes")
	_check(
		MammalSprites.archetype_for(0.1, picked[0], false) == MammalSprites.DEER,
		"a herbivore fawn keeps the deer silhouette"
	)
	_check(
		AgentLayer.archetype_sizes(PackedFloat32Array(), grown) == grown,
		"no adult sizes: fall back to the grown sizes"
	)


func _check_air_lift() -> void:
	_check(AgentLayer.air_lift(10.0, false) == Vector2.ZERO, "ground figures are not lifted")
	var lift: Vector2 = AgentLayer.air_lift(10.0, true)
	_check(lift.x == 0.0 and lift.y < 0.0, "flyers ride above their ground point")
	_check(is_equal_approx(lift.y, -10.0 * AgentLayer.AIR_LIFT), "lift scales with body size")


func _check_nearest_drawn() -> void:
	var centres := PackedVector2Array([Vector2(0, 0), Vector2(10, 0), Vector2(3, 3)])
	var ids := PackedInt32Array([11, 22, 33])
	_check(
		AgentLayer.nearest_drawn(centres, ids, Vector2(2.5, 2.5), 4.0) == 33,
		"the closest drawn centre in range wins, not the first"
	)
	_check(
		AgentLayer.nearest_drawn(centres, ids, Vector2(20, 20), 4.0) == -1,
		"nothing within the radius picks nothing"
	)
	# The radius is strict, as in the bridge's agent_near.
	_check(
		AgentLayer.nearest_drawn(centres, ids, Vector2(14, 0), 4.0) == -1,
		"a centre exactly radius away is out of range"
	)
	_check(
		AgentLayer.nearest_drawn(centres, ids, Vector2(13.9, 0), 4.0) == 22,
		"a centre just inside the radius is picked"
	)
	# Equidistant centres: the first in draw order wins, deterministically.
	var pair := PackedVector2Array([Vector2(0, 0), Vector2(4, 0)])
	_check(
		AgentLayer.nearest_drawn(pair, PackedInt32Array([5, 6]), Vector2(2, 0), 4.0) == 5,
		"an equidistant tie goes to the first drawn body"
	)
	_check(
		AgentLayer.nearest_drawn(PackedVector2Array(), PackedInt32Array(), Vector2.ZERO, 4.0) == -1,
		"no drawn bodies picks nothing"
	)
	# A lifted flyer is picked at its drawn centre, not at its ground point:
	# a click on the body (air_lift above the ground) hits it, and a click at
	# the ground point, outside the 4-unit disc for any body size, misses.
	var ground := Vector2(50, 50)
	var flyer := PackedVector2Array([ground + AgentLayer.air_lift(AgentLayer.BODY_MIN, true)])
	var flyer_id := PackedInt32Array([7])
	_check(
		AgentLayer.nearest_drawn(flyer, flyer_id, flyer[0], 4.0) == 7,
		"a click on the lifted body hits the flyer"
	)
	_check(
		AgentLayer.nearest_drawn(flyer, flyer_id, ground, 4.0) == -1,
		"a click at the flyer's ground point misses the lifted body"
	)


func _check_body_diameter() -> void:
	# Size export 0.5 is genome Size 0: base radius only, doubled to a
	# diameter (2 * 0.4).
	_check(
		is_equal_approx(AgentLayer.body_diameter(0.5), 0.8),
		"body_diameter(0.5) expected 0.8, got %s" % AgentLayer.body_diameter(0.5)
	)
	# Size export 3.0 is genome Size 1 (clamped): base + size term, doubled
	# (2 * 0.75).
	_check(
		is_equal_approx(AgentLayer.body_diameter(3.0), 1.5),
		"body_diameter(3.0) expected 1.5, got %s" % AgentLayer.body_diameter(3.0)
	)


func _check_sprite_size() -> void:
	# Zoom 1x: legible (14) is above BODY_CAP, so the readable size passes
	# through unchanged, exactly like today's clampf(sizes[i] * BODY_SCALE, ...).
	_check(
		is_equal_approx(AgentLayer.sprite_size(1.0, 1.0, 6.0), 8.5),
		"sprite_size at zoom 1 expected 8.5, got %s" % AgentLayer.sprite_size(1.0, 1.0, 6.0)
	)

	# Zoom 4x: legible = 14 / 4 = 3.5, below the readable size (8.5) and
	# above the physical diameter for size 1.0, so legible wins.
	_check(
		is_equal_approx(AgentLayer.sprite_size(1.0, 4.0, 6.0), 3.5),
		"sprite_size at zoom 4 expected 3.5, got %s" % AgentLayer.sprite_size(1.0, 4.0, 6.0)
	)

	# A large zoom (16x): legible = 14 / 16 = 0.875, below the physical
	# diameter (0.94 for size 1.0), so the floor wins and the sprite is
	# drawn at its true body size.
	_check(
		is_equal_approx(AgentLayer.sprite_size(1.0, 16.0, 6.0), AgentLayer.body_diameter(1.0)),
		(
			"sprite_size at zoom 16 expected the physical diameter, got %s"
			% AgentLayer.sprite_size(1.0, 16.0, 6.0)
		)
	)

	# Across a zoom sweep, the result never drops below the physical
	# diameter and never exceeds the readable (BODY_SCALE-clamped) size.
	var size_export := 2.0
	var readable: float = clampf(size_export * AgentLayer.BODY_SCALE, 6.0, AgentLayer.BODY_CAP)
	var diameter: float = AgentLayer.body_diameter(size_export)
	for zoom in [0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0]:
		var sz: float = AgentLayer.sprite_size(size_export, zoom, 6.0)
		_check(
			sz >= diameter - 0.0001,
			"sprite_size(%s) %s never drops below the physical diameter %s" % [zoom, sz, diameter]
		)
		_check(
			sz <= readable + 0.0001,
			"sprite_size(%s) %s never exceeds the readable size %s" % [zoom, sz, readable]
		)


func _check_energy_scale() -> void:
	# The energy overlay shares the unit card's HP scale, and a typical fed
	# body (predator-prey's median energy, 77) sits inside the ramp instead
	# of clipping to its top swatch as it did when normalised by 50.
	_check(AgentLayer.ENERGY_FULL == AgentLayer.UnitCard.HP_FULL, "energy ramp = the HP scale")
	var top: Color = AgentLayer.Palette.ramp(AgentLayer.Palette.RAMP_ENERGY, 1.0)
	var median: Color = AgentLayer.Palette.ramp(
		AgentLayer.Palette.RAMP_ENERGY, 77.0 / AgentLayer.ENERGY_FULL
	)
	_check(not median.is_equal_approx(top), "a median-energy body is not drawn at the ramp top")


func _init() -> void:
	_check_view_rect()
	_check_pos_in_rect()
	_check_merge_prev()
	_check_crowd_cells()
	_check_idle_weapon_act()
	_check_raw_walking()
	_check_archetype_sizes()
	_check_air_lift()
	_check_nearest_drawn()
	_check_body_diameter()
	_check_sprite_size()
	_check_energy_scale()

	if _failed:
		quit(1)
		return
	print("test_agent_layer: all passed")
	quit(0)
