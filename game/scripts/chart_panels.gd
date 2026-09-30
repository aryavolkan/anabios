extends RefCounted

# The three full-size chart panels, [T] evolution, [Y] co-evolution and
# [X] helix, open in the same free column right of the codex, under the top
# bar. Any two open at once printed one chart over the other (the later
# sibling on top), so they share one slot: showing one closes the others.
# Each panel calls close_others() from its visibility_changed hook, which
# covers the keys, the capture switches and the showcase director alike.

const NAMES: PackedStringArray = ["EvolutionPanel", "CoevolutionPanel", "HelixPanel"]


# Hide every other chart panel beside `shown` and clear its `_shown` redraw
# gate, so its own key reopens it on the next press.
static func close_others(shown: Control) -> void:
	var parent := shown.get_parent()
	if parent == null:
		return
	for n in NAMES:
		var other := parent.get_node_or_null(NodePath(n)) as Control
		if other != null and other != shown and other.visible:
			other.set("_shown", false)
			other.visible = false
