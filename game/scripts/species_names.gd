extends RefCounted
# Deterministic display names for species (Phase 5 of
# docs/superpowers/specs/2026-09-12-pixel-world-at-scale-design.md): the
# reference boards name their creatures ("Velocra"), the sim only numbers
# them. A species id maps to two or three syllables by base-20 digits, so the
# mapping is injective below 8000 ids (no two live species share a name) and
# stable across runs and reloads. Presentation only: every panel still shows
# the numeric id beside the name.

const _HEAD: PackedStringArray = [
	"Vel",
	"Kor",
	"Ash",
	"Bra",
	"Dun",
	"Fen",
	"Gor",
	"Hal",
	"Ith",
	"Jor",
	"Kal",
	"Lum",
	"Mor",
	"Nar",
	"Ost",
	"Pyr",
	"Rho",
	"Syl",
	"Tar",
	"Ulm",
]
const _MID: PackedStringArray = [
	"o",
	"a",
	"e",
	"i",
	"u",
	"ar",
	"en",
	"ol",
	"is",
	"ur",
	"an",
	"or",
	"el",
	"ia",
	"ux",
	"ath",
	"em",
	"ir",
	"os",
	"ya",
]
const _TAIL: PackedStringArray = [
	"cra",
	"don",
	"theon",
	"vex",
	"mir",
	"tos",
	"rak",
	"nis",
	"gar",
	"lyn",
	"phos",
	"dra",
	"kis",
	"mon",
	"tar",
	"wen",
	"zor",
	"bel",
	"fax",
	"rune",
]
const BASE := 20


static func name_of(species_id: int) -> String:
	var id: int = maxi(species_id, 0)
	var head: String = _HEAD[id % BASE]
	var tail: String = _TAIL[(id / BASE) % BASE]
	if id < BASE * BASE:
		return head + tail
	var mid: String = _MID[(id / (BASE * BASE)) % BASE]
	return head + mid + tail
