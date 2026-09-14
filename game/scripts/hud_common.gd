extends RefCounted
# Small helpers shared by HUD panels that can live at different depths of the
# UI tree (a panel embedded as a page inside another panel cannot rely on a
# fixed "../../Simulation" path).


# The Simulation node: the nearest ancestor that owns one.
static func find_sim(from: Node) -> Node:
	var n: Node = from.get_parent()
	while n != null:
		var s: Node = n.get_node_or_null("Simulation")
		if s != null:
			return s
		n = n.get_parent()
	return null
