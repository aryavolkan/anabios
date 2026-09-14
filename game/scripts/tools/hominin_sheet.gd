extends SceneTree
const A = preload("res://scripts/ape_sprites.gd")
const HM = preload("res://scripts/hominin_rigs.gd")


func _init() -> void:
	var scale := 4
	var cols := A.POSE_COUNT
	var cw := (HM.PX + 2) * scale
	var sheet := Image.create(cols * cw, A.SPECIES_COUNT * cw, false, Image.FORMAT_RGBA8)
	sheet.fill(Color(0.35, 0.55, 0.3))
	for sp in A.SPECIES_COUNT:
		var cells: Array = HM.build_cells(sp)
		for c in cols:
			var img: Image = cells[c]
			img.resize(HM.PX * scale, HM.PX * scale, Image.INTERPOLATE_NEAREST)
			sheet.blend_rect(
				img, Rect2i(0, 0, img.get_width(), img.get_height()), Vector2i(c * cw, sp * cw)
			)
	sheet.save_png(OS.get_environment("SHEET_OUT"))
	quit(0)
