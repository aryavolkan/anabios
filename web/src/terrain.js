// Terrain, water and forests for the atlas. The biome grid (res² cells, row =
// y) becomes a (res+1)² vertex heightmap in the XZ plane: world x → three x,
// world y → three z, elevation → three y, with the sea level at y = 0 so the
// water plane sits exactly on it. Cell colours are the sim's own `cell_color`
// view bytes (river-tinted, biomass-lushed, succession-scarred) in a res²
// texture; a small shader patch on the standard material turns the hard cell
// grid into painterly ground — noise-jittered borders, broad patchiness and
// fine grain, rock on steep slopes, snow on the peaks, a beach band at the
// water line and a darkened seabed — while lighting, shadows and relief come
// from the mesh. Forests are instanced trees placed from the terrain ids, one
// deterministic jitter per cell, scaled down where a cell is scarred bare.
// The torus wraps: the last vertex row/column samples the first cell.

import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

/** TerrainType ids (anabios_core::biome::TerrainType). */
export const T = Object.freeze({ WATER: 0, GRASS: 1, FOREST: 2, DESERT: 3, ROCK: 4, SAVANNA: 5, RAINFOREST: 6, TAIGA: 7, TUNDRA: 8 });

/** Base colours of `cell_color` (linear RGB 0..1), used to classify a colour
 *  back to a terrain id when a source has no id grid (recorded replays). */
const BASE = [
  [0.09, 0.19, 0.44], [0.21, 0.44, 0.19], [0.07, 0.26, 0.11], [0.68, 0.58, 0.33], [0.42, 0.40, 0.45],
  [0.72, 0.66, 0.36], [0.06, 0.34, 0.16], [0.16, 0.34, 0.26], [0.62, 0.66, 0.62],
];

/** Nearest base-colour terrain id for an sRGB8 cell (lushness moves grass /
 *  forest / desert within their own hue family, so nearest-base still lands). */
export function classifyTerrain(rgba, res) {
  const ids = new Uint8Array(res * res);
  for (let k = 0; k < res * res; k++) {
    const r = rgba[k * 4] / 255, g = rgba[k * 4 + 1] / 255, b = rgba[k * 4 + 2] / 255;
    let best = 0, bd = Infinity;
    for (let t = 0; t < BASE.length; t++) {
      const d = (r - BASE[t][0]) ** 2 + (g - BASE[t][1]) ** 2 + (b - BASE[t][2]) ** 2;
      if (d < bd) { bd = d; best = t; }
    }
    ids[k] = best;
  }
  return ids;
}

const GLSL_NOISE = /* glsl */ `
  float atlasHash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
  float atlasNoise(vec2 p) {
    vec2 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
    float a = atlasHash(i), b = atlasHash(i + vec2(1.0, 0.0)), c = atlasHash(i + vec2(0.0, 1.0)), d = atlasHash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
  }
`;

export class Terrain {
  /**
   * @param {number} res        biome cells per axis
   * @param {number} worldSize  world units per axis
   * @param {number} seaLevel   normalized elevation of the water line
   * @param {Float32Array|null} elevation  res² normalized heights (null → flat)
   * @param {Uint8Array|null} terrainIds   res² TerrainType ids (null → classified from colour)
   */
  constructor(res, worldSize, seaLevel, elevation, terrainIds = null) {
    this.res = res;
    this.worldSize = worldSize;
    this.cell = worldSize / res;
    this.seaLevel = seaLevel;
    this.elevation = elevation;
    this.terrainIds = terrainIds;
    /** Vertical exaggeration: 1 elevation unit → this many world units. */
    this.heightScale = worldSize * 0.075;
    this.reliefOn = !!elevation;
    const n = res + 1;
    this.heights = new Float32Array(n * n);

    const geo = new THREE.BufferGeometry();
    const pos = new Float32Array(n * n * 3);
    const uv = new Float32Array(n * n * 2);
    for (let j = 0; j < n; j++) {
      for (let i = 0; i < n; i++) {
        const v = (j * n + i) * 3;
        pos[v] = i * this.cell;
        pos[v + 1] = 0;
        pos[v + 2] = j * this.cell;
        uv[(j * n + i) * 2] = i / res;
        uv[(j * n + i) * 2 + 1] = j / res;
      }
    }
    const idx = new Uint32Array(res * res * 6);
    let k = 0;
    for (let j = 0; j < res; j++) {
      for (let i = 0; i < res; i++) {
        const a = j * n + i, b = a + 1, c = a + n, d = c + 1;
        idx[k++] = a; idx[k++] = c; idx[k++] = b;
        idx[k++] = b; idx[k++] = c; idx[k++] = d;
      }
    }
    geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    geo.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
    geo.setIndex(new THREE.BufferAttribute(idx, 1));
    this.geometry = geo;

    // Colour lives in a res² RGBA8 texture (row 0 = world y 0, so no flip).
    // Linear filtering + the shader's border jitter give organic edges.
    this.texels = new Uint8Array(res * res * 4).fill(255);
    this.texture = new THREE.DataTexture(this.texels, res, res, THREE.RGBAFormat, THREE.UnsignedByteType);
    this.texture.colorSpace = THREE.SRGBColorSpace;
    this.texture.magFilter = THREE.LinearFilter;
    this.texture.minFilter = THREE.LinearFilter;
    this.texture.generateMipmaps = false;
    this.texture.flipY = false;
    this.texture.needsUpdate = true;

    // Elevation as an 8-bit texture for the water's depth shading.
    this.elevTexels = new Uint8Array(res * res);
    if (elevation) for (let i = 0; i < res * res; i++) this.elevTexels[i] = Math.round(Math.min(1, Math.max(0, elevation[i])) * 255);
    this.elevTexture = new THREE.DataTexture(this.elevTexels, res, res, THREE.RedFormat, THREE.UnsignedByteType);
    this.elevTexture.magFilter = THREE.LinearFilter;
    this.elevTexture.minFilter = THREE.LinearFilter;
    this.elevTexture.generateMipmaps = false;
    this.elevTexture.flipY = false;
    this.elevTexture.needsUpdate = true;

    this.uniforms = {
      uRes: { value: res },
      uRelief: { value: this.reliefOn ? 1 : 0 },
      uSnow: { value: (0.9 - seaLevel) * this.heightScale },
      uBeach: { value: this.heightScale * 0.02 },
    };
    this.material = new THREE.MeshStandardMaterial({ map: this.texture, roughness: 0.94, metalness: 0.0 });
    this.material.customProgramCacheKey = () => "atlas-terrain";
    this.material.onBeforeCompile = (shader) => {
      Object.assign(shader.uniforms, this.uniforms);
      shader.vertexShader = shader.vertexShader
        .replace("#include <common>", "#include <common>\nvarying float vSlope; varying float vHeight;")
        .replace("#include <beginnormal_vertex>", "#include <beginnormal_vertex>\nvSlope = objectNormal.y;")
        .replace("#include <begin_vertex>", "#include <begin_vertex>\nvHeight = transformed.y;");
      shader.fragmentShader = shader.fragmentShader
        .replace("#include <common>", `#include <common>\nuniform float uRes; uniform float uRelief; uniform float uSnow; uniform float uBeach;\nvarying float vSlope; varying float vHeight;\n${GLSL_NOISE}`)
        .replace("#include <map_fragment>", /* glsl */ `
          vec2 cuv = vMapUv * uRes;
          // Organic cell borders: jitter the sample point by low-frequency noise.
          vec2 jit = vec2(atlasNoise(cuv * 2.3 + 13.1), atlasNoise(cuv * 2.3 + 71.7)) - 0.5;
          vec4 sampledDiffuseColor = texture2D(map, (cuv + jit * 0.75) / uRes);
          float macro = atlasNoise(cuv * 0.37) - 0.5;
          float fine = atlasNoise(cuv * 7.0) - 0.5;
          vec3 col = sampledDiffuseColor.rgb * (1.0 + 0.18 * macro + 0.12 * fine);
          // Steep ground reads as rock, high ground as snow (relief only).
          float steep = smoothstep(0.30, 0.75, 1.0 - vSlope) * uRelief;
          vec3 rock = vec3(0.30, 0.27, 0.26) * (0.85 + 0.5 * fine);
          col = mix(col, rock, steep);
          float snow = smoothstep(uSnow, uSnow * 1.25, vHeight) * (1.0 - steep * 0.6) * uRelief;
          col = mix(col, vec3(0.90, 0.92, 0.95) * (0.9 + 0.2 * fine), snow);
          // Shore: a sandy band just above the water line, a darker blue-green bed below it.
          float beach = (1.0 - smoothstep(0.0, uBeach * 3.0, vHeight)) * step(0.0, vHeight) * uRelief;
          col = mix(col, vec3(0.78, 0.70, 0.48) * (0.85 + 0.4 * fine), beach * 0.7);
          float bed = clamp(-vHeight / (uBeach * 8.0), 0.0, 1.0) * uRelief;
          col = mix(col, col * vec3(0.40, 0.55, 0.62), bed);
          diffuseColor.rgb *= col;
        `);
    };
    this.mesh = new THREE.Mesh(geo, this.material);
    this.mesh.name = "terrain";
    this.mesh.receiveShadow = true;
    this.rebuildHeights();

    // The plate: a dark slab under the map so the world reads as an object on
    // a table rather than a sheet floating in fog.
    const depth = Math.max(6, this.heightScale * seaLevel + 6);
    const slab = new THREE.Mesh(
      new THREE.BoxGeometry(worldSize * 1.02, depth, worldSize * 1.02),
      new THREE.MeshStandardMaterial({ color: 0x1a140f, roughness: 1 }),
    );
    slab.position.set(worldSize / 2, -depth / 2 - this.heightScale * seaLevel + 0.5, worldSize / 2);
    this.slab = slab;

    this.water = new Water(worldSize, this.elevTexture, seaLevel, this.heightScale);
    this.water.mesh.visible = this.reliefOn;   // flat worlds paint water cells in the ground texture instead
    this.forest = new Forest(this);
    this.group = new THREE.Group();
    this.group.add(this.mesh, this.slab, this.water.mesh, this.forest.group);
  }

  /** Recompute vertex heights from the elevation grid (or flatten). */
  rebuildHeights() {
    const { res, heights } = this;
    const n = res + 1, pos = this.geometry.attributes.position.array;
    const scale = this.reliefOn && this.elevation ? this.heightScale : 0;
    for (let j = 0; j < n; j++) {
      for (let i = 0; i < n; i++) {
        let h = 0;
        if (scale > 0) {
          // average the (up to) four cells meeting at this vertex, torus-wrapped
          const i0 = (i - 1 + res) % res, i1 = i % res, j0 = (j - 1 + res) % res, j1 = j % res;
          const e = this.elevation;
          const avg = (e[j0 * res + i0] + e[j0 * res + i1] + e[j1 * res + i0] + e[j1 * res + i1]) / 4;
          h = (avg - this.seaLevel) * scale;
        }
        heights[j * n + i] = h;
        pos[(j * n + i) * 3 + 1] = h;
      }
    }
    this.geometry.attributes.position.needsUpdate = true;
    this.geometry.computeVertexNormals();
    this.geometry.computeBoundingSphere();
    this.uniforms.uRelief.value = scale > 0 ? 1 : 0;
  }

  setRelief(on) {
    this.reliefOn = on && !!this.elevation;
    this.rebuildHeights();
    this.water.mesh.visible = this.reliefOn;
    this.forest?.rebuild();
  }

  /** Bilinear terrain height at world (x, y), torus-wrapped. */
  heightAt(x, y) {
    const { res, cell, heights } = this;
    const n = res + 1;
    const fx = ((x / cell) % res + res) % res, fy = ((y / cell) % res + res) % res;
    const i = Math.floor(fx), j = Math.floor(fy), u = fx - i, v = fy - j;
    const h00 = heights[j * n + i], h10 = heights[j * n + i + 1];
    const h01 = heights[(j + 1) * n + i], h11 = heights[(j + 1) * n + i + 1];
    return (h00 * (1 - u) + h10 * u) * (1 - v) + (h01 * (1 - u) + h11 * u) * v;
  }

  /** Push new cell colours (RGBA8, res²; the elevation alpha is forced opaque). */
  updateColors(rgba) {
    if (!rgba || rgba.length !== this.texels.length) return;
    const t = this.texels;
    t.set(rgba);
    for (let i = 3; i < t.length; i += 4) t[i] = 255;
    this.texture.needsUpdate = true;
    if (!this.terrainIds) { this.terrainIds = classifyTerrain(rgba, this.res); this.forest.rebuild(); }
    this.forest.refresh(rgba);
  }

  dispose() {
    this.geometry.dispose();
    this.material.dispose();
    this.texture.dispose();
    this.elevTexture.dispose();
    this.slab.geometry.dispose();
    this.slab.material.dispose();
    this.water.dispose();
    this.forest.dispose();
  }
}

// ---------------------------------------------------------------------------
/** Water plane at y = 0: depth-shaded from the elevation texture (turquoise
 *  shallows, navy deeps), animated ripple normals with a sun glint, a foam
 *  fringe along the shore and a fresnel rim. */
export class Water {
  constructor(worldSize, elevTexture, seaLevel, heightScale) {
    this.uniforms = {
      uTime: { value: 0 },
      uShallow: { value: new THREE.Color(0x2a7a86) },
      uColor: { value: new THREE.Color(0x1c4a86) },
      uDeep: { value: new THREE.Color(0x0c2148) },
      uSize: { value: worldSize },
      uElev: { value: elevTexture },
      uSea: { value: seaLevel },
      uHs: { value: heightScale },
      uSun: { value: new THREE.Vector3(0.55, 1.0, 0.35).normalize() },
    };
    this.material = new THREE.ShaderMaterial({
      uniforms: this.uniforms,
      transparent: true,
      depthWrite: false,
      vertexShader: /* glsl */ `
        varying vec3 vWorld;
        void main() {
          vec4 w = modelMatrix * vec4(position, 1.0);
          vWorld = w.xyz;
          gl_Position = projectionMatrix * viewMatrix * w;
        }`,
      fragmentShader: /* glsl */ `
        uniform float uTime; uniform vec3 uShallow; uniform vec3 uColor; uniform vec3 uDeep; uniform float uSize;
        uniform sampler2D uElev; uniform float uSea; uniform float uHs; uniform vec3 uSun;
        varying vec3 vWorld;
        ${GLSL_NOISE}
        float ripple(vec2 p, float t) {
          return 0.6 * sin(p.x * 0.08 + t * 0.9) * sin(p.y * 0.11 - t * 0.7) + 0.4 * sin((p.x + p.y) * 0.05 + t * 0.5)
               + 0.5 * (atlasNoise(p * 0.06 + vec2(t * 0.05, -t * 0.03)) - 0.5);
        }
        void main() {
          vec2 uv = vWorld.xz / uSize;
          float e = texture2D(uElev, uv).r;
          float depth = max(0.0, (uSea - e) * uHs);
          vec2 p = vWorld.xz;
          float r = ripple(p, uTime);
          float rx = ripple(p + vec2(1.5, 0.0), uTime) - r, rz = ripple(p + vec2(0.0, 1.5), uTime) - r;
          vec3 n = normalize(vec3(-rx * 0.35, 1.0, -rz * 0.35));
          vec3 viewDir = normalize(cameraPosition - vWorld);
          float fres = pow(1.0 - max(dot(viewDir, n), 0.0), 2.5);
          vec3 col = mix(uShallow, uColor, clamp(depth / 2.5, 0.0, 1.0));
          col = mix(col, uDeep, clamp(depth / 12.0, 0.0, 1.0));
          col *= 0.9 + 0.2 * r;
          float spec = pow(max(dot(reflect(-uSun, n), viewDir), 0.0), 48.0);
          col += vec3(1.0, 0.95, 0.85) * spec * 0.55;
          col += vec3(0.22, 0.30, 0.34) * fres * 0.7;
          float foamBand = 1.0 - smoothstep(0.0, 0.9, depth);
          float foam = foamBand * smoothstep(0.45, 0.75, atlasNoise(p * 0.5 + vec2(uTime * 0.6, uTime * 0.2)) + 0.25 * r);
          col = mix(col, vec3(0.92, 0.94, 0.96), foam * 0.75);
          float alpha = mix(0.45, 0.88, clamp(depth / 3.0, 0.0, 1.0)) + fres * 0.1 + foam * 0.3;
          gl_FragColor = vec4(col, clamp(alpha, 0.0, 0.96));
        }`,
    });
    const geo = new THREE.PlaneGeometry(worldSize, worldSize, 1, 1);
    geo.rotateX(-Math.PI / 2);
    this.mesh = new THREE.Mesh(geo, this.material);
    this.mesh.position.set(worldSize / 2, 0, worldSize / 2);
    this.mesh.name = "water";
    this.mesh.renderOrder = 1;
  }
  update(seconds) { this.uniforms.uTime.value = seconds; }
  dispose() { this.mesh.geometry.dispose(); this.material.dispose(); }
}

// ---------------------------------------------------------------------------
/** Broadleaf tree: trunk + squashed icosahedron canopy; vertex colours tint
 *  the trunk brown and leave the canopy white for the instance colour. */
function broadleafGeometry() {
  const trunk = new THREE.CylinderGeometry(0.07, 0.11, 0.55, 5);
  trunk.translate(0, 0.27, 0);
  paint(trunk, 0.42, 0.30, 0.20);
  // (Indexed like the trunk — a polyhedron would be non-indexed and refuse to merge.)
  const canopy = new THREE.SphereGeometry(0.46, 6, 5);
  canopy.scale(1, 0.85, 1);
  canopy.translate(0, 0.78, 0);
  paint(canopy, 1, 1, 1);
  return mergeGeometries([trunk, canopy], false);
}
/** Conifer: trunk + two stacked cones. */
function coniferGeometry() {
  const trunk = new THREE.CylinderGeometry(0.06, 0.09, 0.4, 5);
  trunk.translate(0, 0.2, 0);
  paint(trunk, 0.38, 0.26, 0.18);
  const lower = new THREE.ConeGeometry(0.38, 0.8, 6);
  lower.translate(0, 0.7, 0);
  paint(lower, 1, 1, 1);
  const upper = new THREE.ConeGeometry(0.26, 0.6, 6);
  upper.translate(0, 1.15, 0);
  paint(upper, 1, 1, 1);
  return mergeGeometries([trunk, lower, upper], false);
}
function paint(geo, r, g, b) {
  const n = geo.attributes.position.count, col = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) { col[i * 3] = r; col[i * 3 + 1] = g; col[i * 3 + 2] = b; }
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
}

/** Per-terrain planting: [trees per cell (fractional = probability), kind, canopy colour, height factor]. */
const PLANTING = {
  [T.FOREST]: [1.7, "leaf", 0x2f7a32, 0.82],
  [T.RAINFOREST]: [2.6, "leaf", 0x1f6b3a, 1.0],
  [T.TAIGA]: [1.7, "cone", 0x2b5b45, 0.95],
  [T.GRASS]: [0.08, "leaf", 0x4c9a3c, 0.7],
  [T.SAVANNA]: [0.16, "leaf", 0x8a8a3c, 0.65],
  [T.TUNDRA]: [0.06, "cone", 0x5c7060, 0.6],
};
const hash2 = (a, b) => { let h = (a * 374761393 + b * 668265263) | 0; h = Math.imul(h ^ (h >>> 13), 1274126177); return ((h ^ (h >>> 16)) >>> 0) / 4294967296; };

/** Instanced forests over the terrain's cells. */
export class Forest {
  constructor(terrain, maxPerKind = 60000) {
    this.terrain = terrain;
    this.max = maxPerKind;
    const mat = () => new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.9, flatShading: true });
    this.leaf = new THREE.InstancedMesh(broadleafGeometry(), mat(), maxPerKind);
    this.cone = new THREE.InstancedMesh(coniferGeometry(), mat(), maxPerKind);
    for (const m of [this.leaf, this.cone]) {
      m.count = 0; m.frustumCulled = false; m.castShadow = true; m.receiveShadow = true; m.name = "forest";
    }
    this.group = new THREE.Group();
    this.group.add(this.leaf, this.cone);
    this.items = [];     // {mesh, index, cell, x, z, rot, base, dead}
    this.scar = null;    // per-cell 0/1 "bare" flags from the last colour refresh
    this.rebuild();
  }

  /** Place trees from the terrain ids (deterministic per cell). */
  rebuild() {
    const t = this.terrain, ids = t.terrainIds, res = t.res, cell = t.cell;
    this.items.length = 0;
    const counts = { leaf: 0, cone: 0 };
    if (ids) {
      // Budget: keep every world under the instance cap by thinning uniformly.
      let want = 0;
      for (let k = 0; k < res * res; k++) { const p = PLANTING[ids[k]]; if (p) want += p[0]; }
      const keep = Math.min(1, this.max / Math.max(1, want));
      const m = new THREE.Matrix4(), p = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3(), c = new THREE.Color();
      for (let k = 0; k < res * res; k++) {
        const plan = PLANTING[ids[k]];
        if (!plan) continue;
        const [density, kind, colour, hf] = plan;
        const cx = k % res, cy = (k / res) | 0;
        const n = Math.floor(density) + (hash2(k, 7) < density % 1 ? 1 : 0);
        for (let i = 0; i < n; i++) {
          if (hash2(k, 100 + i) > keep) continue;
          const mesh = kind === "leaf" ? this.leaf : this.cone;
          const index = counts[kind]++;
          if (index >= this.max) continue;
          const x = (cx + hash2(k, 200 + i)) * cell, z = (cy + hash2(k, 300 + i)) * cell;
          const base = cell * hf * (0.55 + 0.5 * hash2(k, 400 + i));
          const rot = hash2(k, 500 + i) * Math.PI * 2;
          const item = { mesh, index, cell: k, x, z, rot, base, scale: 1 };
          this.items.push(item);
          p.set(x, t.heightAt(x, z), z);
          q.setFromAxisAngle(new THREE.Vector3(0, 1, 0), rot);
          s.set(base, base, base);
          mesh.setMatrixAt(index, m.compose(p, q, s));
          c.setHex(colour).offsetHSL(0, 0, (hash2(k, 600 + i) - 0.5) * 0.12);
          mesh.setColorAt(index, c);
        }
      }
    }
    this.leaf.count = Math.min(counts.leaf, this.max);
    this.cone.count = Math.min(counts.cone, this.max);
    for (const mesh of [this.leaf, this.cone]) {
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    }
    this.scar = null;
  }

  /** Shrink trees on cells the biome has scarred bare (succession/pollution),
   *  restore them as the cell greens again. Cheap: only cells that flipped. */
  refresh(rgba) {
    if (!this.items.length) return;
    const res = this.terrain.res, n = res * res;
    const scar = new Uint8Array(n);
    for (let k = 0; k < n; k++) {
      const r = rgba[k * 4], g = rgba[k * 4 + 1];
      scar[k] = r > g * 1.05 ? 1 : 0;   // browner than green: bare earth / pioneer brown / pollution smudge
    }
    const prev = this.scar;
    const m = new THREE.Matrix4(), p = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3(), up = new THREE.Vector3(0, 1, 0);
    let dirty = false;
    for (const it of this.items) {
      const bare = scar[it.cell];
      if (prev && prev[it.cell] === bare) continue;
      const sc = bare ? it.base * 0.12 : it.base;
      p.set(it.x, this.terrain.heightAt(it.x, it.z), it.z);
      q.setFromAxisAngle(up, it.rot);
      s.set(sc, sc, sc);
      it.mesh.setMatrixAt(it.index, m.compose(p, q, s));
      dirty = true;
    }
    if (dirty) { this.leaf.instanceMatrix.needsUpdate = true; this.cone.instanceMatrix.needsUpdate = true; }
    this.scar = scar;
  }

  dispose() {
    for (const m of [this.leaf, this.cone]) { m.geometry.dispose(); m.material.dispose(); m.dispose(); }
  }
}
