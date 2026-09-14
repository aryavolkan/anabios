extends SceneTree
# Dump every hand-authored master's 16 pose cells (one row per archetype).
const M = preload("res://scripts/mammal_sprites.gd")
const H = preload("res://scripts/hero_rigs.gd")


func _init() -> void:
	var scale := 4
	var cols := H.CELL_COUNT
	var rows := M.QUAD_ORDER.size()
	var cw := (H.PX + 2) * scale
	var sheet := Image.create(cols * cw, rows * cw, false, Image.FORMAT_RGBA8)
	sheet.fill(Color(0.35, 0.55, 0.3))
	for r in rows:
		var cells: Array = H.build_cells(M.NAMES[M.QUAD_ORDER[r]])
		for c in cols:
			var img: Image = cells[c]
			img.resize(H.PX * scale, H.PX * scale, Image.INTERPOLATE_NEAREST)
			sheet.blend_rect(
				img, Rect2i(0, 0, img.get_width(), img.get_height()), Vector2i(c * cw, r * cw)
			)
	sheet.save_png(OS.get_environment("SHEET_OUT"))
	quit(0)
