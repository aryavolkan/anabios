// Shared colours + classification for the three.js frontend. The palette is
// the showcase player's (basalt / bone / ochre / glacial / ember / chloro) so
// the live atlas and the recorded deck read as one product.

export const UI = Object.freeze({
  basalt: 0x16110d, basalt2: 0x1f1813, bone: 0xe9e1d2, boneDim: 0xb7ac98, muted: 0x8a7e6c,
  ochre: 0xce7c36, glacial: 0x6e9bb5, ember: 0xc0432c, chloro: 0x7a9a54,
});

/** Narrative kind of a codex event type (same regexes as showcase/index.html). */
export function kindOf(type) {
  const t = type;
  if (/war|raid|predation|crash|extinction|collapse|maladapt|panic|fright|grief|rage|epidemic|dehydration/.test(t)) return "war";
  if (/invention|practice|material|module|tool|behavior|specializ|knowledge|medicine/.test(t)) return "fire";
  if (/trade|market|resource|corridor|migration|dialect|signal|alliance|kin|radiation|range/.test(t)) return "trade";
  if (/settle|territory|herd|domestic|livestock|carrying|cooperat|niche|cohesion|succession/.test(t)) return "grow";
  return "mind";
}
export const KIND_COLOR = Object.freeze({
  fire: 0xce7c36, trade: 0x6e9bb5, grow: 0x7a9a54, war: 0xc0432c, mind: 0xe9e1d2,
});
export const KIND_CSS = Object.freeze({
  fire: "#CE7C36", trade: "#6E9BB5", grow: "#7A9A54", war: "#C0432C", mind: "#E9E1D2",
});

/** Terrain legend (TerrainType id → label, css colour from biome::cell_color base). */
export const TERRAIN = [
  ["water", "#17306f"], ["grassland", "#367030"], ["forest", "#12421c"], ["desert", "#ad9454"],
  ["rock", "#6b6673"], ["savanna", "#b8a85c"], ["rainforest", "#0f5729"], ["taiga", "#295742"],
  ["tundra", "#9ea89e"],
];

/** Mood discriminant → colour (mood.rs order: content, seek food, seek water, sleep, flee, fight, seek mate, mate). */
export const MOOD_COLORS = [0xb7ac98, 0x7a9a54, 0x6e9bb5, 0x4a4560, 0xe8b04a, 0xc0432c, 0xd98fb5, 0xf0c8e0];

/** HSV (all in [0,1]) → packed 0xRRGGBB. */
export function hsv(h, s, v) {
  const h6 = ((h % 1) + 1) % 1 * 6;
  const i = Math.floor(h6), f = h6 - i;
  const p = v * (1 - s), q = v * (1 - s * f), t = v * (1 - s * (1 - f));
  let r, g, b;
  switch (i % 6) {
    case 0: [r, g, b] = [v, t, p]; break;
    case 1: [r, g, b] = [q, v, p]; break;
    case 2: [r, g, b] = [p, v, t]; break;
    case 3: [r, g, b] = [p, q, v]; break;
    case 4: [r, g, b] = [t, p, v]; break;
    default: [r, g, b] = [v, p, q];
  }
  return ((r * 255) << 16) | ((g * 255) << 8) | (b * 255);
}

/** Stable, well-separated hue for a species id (golden-angle spacing). */
export function speciesHue(sid) {
  return ((sid * 0.618033988749895) + 0.11) % 1;
}

/** Lerp two packed colours. */
export function mix(a, b, t) {
  const ar = (a >> 16) & 255, ag = (a >> 8) & 255, ab = a & 255;
  const br = (b >> 16) & 255, bg = (b >> 8) & 255, bb = b & 255;
  return ((ar + (br - ar) * t) << 16) | ((ag + (bg - ag) * t) << 8) | (ab + (bb - ab) * t);
}

export function cssHex(c) {
  return "#" + (c >>> 0).toString(16).padStart(6, "0");
}
