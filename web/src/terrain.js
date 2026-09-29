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

/** Drifting cloud shade over world (x, z) in units: two octaves of slow noise thresholded to soft patches. */
const GLSL_CLOUD = /* glsl */ `
  float atlasCloud(vec2 w, float t) {
    vec2 p = w / 110.0;
    float n = atlasNoise(p + vec2(t * 0.011, t * 0.005)) * 0.6 + atlasNoise(p * 2.1 - vec2(t * 0.008, t * 0.004)) * 0.4;
    return smoothstep(0.58, 0.80, n);
  }
`;

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

    // The rendered heights again, (res+1)² vertices in world units, for the
    // water's depth shading. Half float, not 8-bit elevation: one 8-bit step is
    // ~0.3 units, a third of the foam band, and a res² cell texture disagrees
    // with the vertex-averaged mesh by more than that on rough ground — foam
    // then followed the cell grid in blocks instead of the drawn shoreline.
    // R16F filters linearly in WebGL2 (R32F needs an extension).
    this.heightTexels = new Uint16Array(n * n);
    this.heightTexture = new THREE.DataTexture(this.heightTexels, n, n, THREE.RedFormat, THREE.HalfFloatType);
    this.heightTexture.magFilter = THREE.LinearFilter;
    this.heightTexture.minFilter = THREE.LinearFilter;
    this.heightTexture.generateMipmaps = false;
    this.heightTexture.flipY = false;

    this.uniforms = {
      uRes: { value: res },
      uTime: { value: 0 },
      uRelief: { value: this.reliefOn ? 1 : 0 },
      uSnow: { value: (0.9 - seaLevel) * this.heightScale },
      uBeach: { value: this.heightScale * 0.02 },
      uWorld: { value: worldSize },
      uCloud: { value: 1 },
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
        .replace("#include <common>", `#include <common>\nuniform float uRes; uniform float uTime; uniform float uRelief; uniform float uSnow; uniform float uBeach; uniform float uWorld; uniform float uCloud;\nvarying float vSlope; varying float vHeight;\n${GLSL_NOISE}\n${GLSL_CLOUD}`)
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
          // Wet sand: the strip the last wave reached is darker.
          float wetsand = (1.0 - smoothstep(0.0, uBeach * 0.9, vHeight)) * step(0.0, vHeight) * uRelief;
          col *= 1.0 - 0.22 * wetsand;
          float bed = clamp(-vHeight / (uBeach * 8.0), 0.0, 1.0) * uRelief;
          col = mix(col, col * vec3(0.40, 0.55, 0.62), bed);
          // Wet cells (rivers, and water on flat worlds) glitter with a slow drifting sparkle;
          // the seabed under the water plane ripples with a soft caustic.
          vec3 sc = sampledDiffuseColor.rgb;
          float wet = smoothstep(0.05, 0.25, sc.b - max(sc.r, sc.g) * 1.05);
          float glit = pow(atlasNoise(cuv * 9.0 + vec2(uTime * 0.35, -uTime * 0.22)), 7.0) * pow(atlasNoise(cuv * 6.5 - vec2(uTime * 0.18, uTime * 0.27)), 2.0);
          col += vec3(0.55, 0.62, 0.7) * glit * 3.0 * wet;
          float caus = atlasNoise(cuv * 5.0 + vec2(uTime * 0.4, uTime * 0.1)) * atlasNoise(cuv * 4.3 - vec2(uTime * 0.25, uTime * 0.35));
          col += vec3(0.35, 0.5, 0.55) * pow(caus, 2.5) * 1.6 * bed;
          // Cloud shadows drifting over the plate.
          col *= 1.0 - 0.22 * atlasCloud(vMapUv * uWorld, uTime) * uCloud;
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

    this.water = new Water(worldSize, res, this.heightTexture);
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
        this.heightTexels[j * n + i] = THREE.DataUtils.toHalfFloat(h);
      }
    }
    this.heightTexture.needsUpdate = true;
    this.geometry.attributes.position.needsUpdate = true;
    this.geometry.computeVertexNormals();
    this.geometry.computeBoundingSphere();
    this.uniforms.uRelief.value = scale > 0 ? 1 : 0;
  }

  /** Advance the ground shader's clock (sparkle and caustics). */
  setTime(t) { this.uniforms.uTime.value = t; }

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

  /** True when world (x, y) falls on a water cell (torus-wrapped); false before the terrain ids are known. */
  isWater(x, y) {
    const ids = this.terrainIds;
    if (!ids) return false;
    const { res, cell } = this;
    const i = ((Math.floor(x / cell) % res) + res) % res, j = ((Math.floor(y / cell) % res) + res) % res;
    return ids[j * res + i] === T.WATER;
  }

  /** Push new cell colours (RGBA8, res²; the elevation alpha is forced opaque).
   *  `snap`: after a time jump, bare trees take their new size at once. */
  updateColors(rgba, snap = false) {
    if (!rgba || rgba.length !== this.texels.length) return;
    const t = this.texels;
    t.set(rgba);
    for (let i = 3; i < t.length; i += 4) t[i] = 255;
    this.texture.needsUpdate = true;
    if (!this.terrainIds) { this.terrainIds = classifyTerrain(rgba, this.res); this.forest.rebuild(); }
    this.forest.refresh(rgba, snap);
  }

  dispose() {
    this.geometry.dispose();
    this.material.dispose();
    this.texture.dispose();
    this.heightTexture.dispose();
    this.slab.geometry.dispose();
    this.slab.material.dispose();
    this.water.dispose();
    this.forest.dispose();
  }
}

// ---------------------------------------------------------------------------
/** Water plane at y = 0: depth-shaded from the terrain's vertex heights
 *  (turquoise shallows, navy deeps), animated ripple normals with a sun glint,
 *  a foam fringe along the shore and a fresnel rim. */
export class Water {
  /** @param {THREE.Texture} heightTexture  (res+1)² vertex heights, world units above the sea */
  constructor(worldSize, res, heightTexture) {
    this.uniforms = {
      uTime: { value: 0 },
      // The tuned palette, authored for a raw (unencoded) write: converted a
      // second time so it survives the output transfer the same.
      uShallow: { value: new THREE.Color(0x2a7a86).convertSRGBToLinear() },
      uColor: { value: new THREE.Color(0x1c4a86).convertSRGBToLinear() },
      uDeep: { value: new THREE.Color(0x0c2148).convertSRGBToLinear() },
      uSize: { value: worldSize },
      uRes: { value: res },
      uHeight: { value: heightTexture },
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
        uniform float uRes; uniform sampler2D uHeight; uniform vec3 uSun;
        varying vec3 vWorld;
        ${GLSL_NOISE}
        float ripple(vec2 p, float t) {
          return 0.6 * sin(p.x * 0.08 + t * 0.9) * sin(p.y * 0.11 - t * 0.7) + 0.4 * sin((p.x + p.y) * 0.05 + t * 0.5)
               + 0.5 * (atlasNoise(p * 0.06 + vec2(t * 0.05, -t * 0.03)) - 0.5);
        }
        // Ground height under p exactly as drawn: the mesh's two triangles per
        // cell (diagonal from (i+1, j) to (i, j+1)) over its own vertex heights,
        // so depth 0 is precisely where the ground meets the plane.
        float groundHeight(vec2 p) {
          vec2 f = clamp(p / uSize, 0.0, 1.0) * uRes;
          vec2 c = min(floor(f), uRes - 1.0), t = f - c;
          ivec2 i = ivec2(c);
          float a = texelFetch(uHeight, i, 0).r, b = texelFetch(uHeight, i + ivec2(1, 0), 0).r;
          float cc = texelFetch(uHeight, i + ivec2(0, 1), 0).r, d = texelFetch(uHeight, i + ivec2(1, 1), 0).r;
          return t.x + t.y < 1.0 ? a + (b - a) * t.x + (cc - a) * t.y : d + (cc - d) * (1.0 - t.x) + (b - d) * (1.0 - t.y);
        }
        // Filtered height (texel centres are the vertices): smooth enough to difference.
        float smoothHeight(vec2 p) { return texture2D(uHeight, (p / uSize * uRes + 0.5) / (uRes + 1.0)).r; }
        void main() {
          vec2 p = vWorld.xz;
          float depth = max(0.0, -groundHeight(p));
          // Distance to the shore ≈ depth / slope; the slope from half-cell central
          // differences of the filtered heights, which vary smoothly (the per-
          // triangle slope would print the mesh facets into the foam).
          float k = 0.5 * uSize / uRes;
          vec2 g = vec2(smoothHeight(p + vec2(k, 0.0)) - smoothHeight(p - vec2(k, 0.0)),
                        smoothHeight(p + vec2(0.0, k)) - smoothHeight(p - vec2(0.0, k))) / (2.0 * k);
          float shoreDist = depth / max(length(g), 1e-3);
          float r = ripple(p, uTime);
          float rx = ripple(p + vec2(1.5, 0.0), uTime) - r, rz = ripple(p + vec2(0.0, 1.5), uTime) - r;
          vec3 n = normalize(vec3(-rx * 0.35, 1.0, -rz * 0.35));
          vec3 viewDir = normalize(cameraPosition - vWorld);
          float fres = pow(1.0 - max(dot(viewDir, n), 0.0), 2.5);
          vec3 col = mix(uShallow, uColor, clamp(depth / 2.5, 0.0, 1.0));
          col = mix(col, uDeep, clamp(depth / 12.0, 0.0, 1.0));
          col *= 0.9 + 0.2 * r;
          // Sun glint: a tight lobe so it breaks into sparkle on the ripples. A
          // broad one (exponent 48) turned a whole lake into one white sheet
          // under bloom whenever the day cycle lined the sun up with the
          // camera; it also dims as the sun sinks instead of flaring at dusk.
          float spec = pow(max(dot(reflect(-uSun, n), viewDir), 0.0), 220.0);
          col += vec3(1.0, 0.93, 0.82) * spec * 0.5 * smoothstep(0.08, 0.45, uSun.y);
          col += vec3(0.22, 0.30, 0.34) * fres * 0.7;
          // Foam hugs the shore: shallow AND near it, so a gentle beach keeps a
          // narrow fringe and a shallow, flat pond does not whiten all over.
          float foamBand = (1.0 - smoothstep(0.0, 0.9, depth)) * (1.0 - smoothstep(1.5, 4.5, shoreDist));
          // Lace: two octaves of value noise on rotated axes — one thresholded
          // octave on the world axes reads as square blobs. Denser at the water line.
          float lace = 0.6 * atlasNoise(mat2(0.8, -0.6, 0.6, 0.8) * p * 0.5 + vec2(uTime * 0.6, uTime * 0.2))
                     + 0.4 * atlasNoise(mat2(0.28, 0.96, -0.96, 0.28) * p * 1.3 - vec2(uTime * 0.3, uTime * 0.5));
          float foam = foamBand * smoothstep(0.45, 0.8, lace + 0.25 * r + 0.1 * foamBand);
          col = mix(col, vec3(0.92, 0.94, 0.96), foam * 0.75);
          float alpha = mix(0.45, 0.88, clamp(depth / 3.0, 0.0, 1.0)) + fres * 0.1 + foam * 0.3;
          gl_FragColor = vec4(col, clamp(alpha, 0.0, 0.96));
          #include <tonemapping_fragment>
          #include <colorspace_fragment>
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
/** Broadleaf tree: trunk + a cluster of canopy lobes; vertex colours tint the
 *  trunk brown and leave the canopy white for the instance colour. */
function broadleafGeometry() {
  const trunk = new THREE.CylinderGeometry(0.06, 0.12, 0.6, 5);
  trunk.translate(0, 0.3, 0);
  paint(trunk, 0.42, 0.30, 0.20);
  // (Indexed like the trunk — a polyhedron would be non-indexed and refuse to merge.)
  const lobes = [];
  for (const [x, y, z, r] of [[0, 0.84, 0, 0.42], [0.24, 0.66, 0.12, 0.3], [-0.2, 0.7, -0.16, 0.28], [0.02, 0.62, 0.27, 0.24]]) {
    const lobe = new THREE.SphereGeometry(r, 6, 5);
    lobe.scale(1, 0.85, 1);
    lobe.translate(x, y, z);
    paint(lobe, 1, 1, 1);
    lobes.push(lobe);
  }
  return mergeGeometries([trunk, ...lobes], false);
}
/** Conifer: trunk + three stacked cones. */
function coniferGeometry() {
  const trunk = new THREE.CylinderGeometry(0.05, 0.09, 0.45, 5);
  trunk.translate(0, 0.22, 0);
  paint(trunk, 0.38, 0.26, 0.18);
  const tiers = [];
  for (const [r, h, y] of [[0.40, 0.75, 0.62], [0.30, 0.6, 1.0], [0.19, 0.45, 1.33]]) {
    const cone = new THREE.ConeGeometry(r, h, 6);
    cone.translate(0, y, 0);
    paint(cone, 1, 1, 1);
    tiers.push(cone);
  }
  return mergeGeometries([trunk, ...tiers], false);
}
/** Grass tuft: three crossed, tapered blades; darker at the root, lighter at the tips. */
function tuftGeometry() {
  const parts = [];
  for (let i = 0; i < 3; i++) {
    const g = new THREE.PlaneGeometry(0.55, 0.62, 1, 2);
    g.translate(0, 0.31, 0);
    g.rotateY((i * Math.PI) / 3);
    const pos = g.attributes.position.array;
    for (let v = 0; v < pos.length; v += 3) { const t = pos[v + 1] / 0.62; pos[v] *= 1.0 - 0.55 * t; pos[v + 2] *= 1.0 - 0.55 * t; }
    parts.push(g);
  }
  const g = mergeGeometries(parts, false);
  const n = g.attributes.position.count, pos = g.attributes.position.array, col = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) { const v = 0.55 + 0.55 * (pos[i * 3 + 1] / 0.62); col[i * 3] = v; col[i * 3 + 1] = v; col[i * 3 + 2] = v; }
  g.setAttribute("color", new THREE.BufferAttribute(col, 3));
  return g;
}
/** A squat boulder: a flattened, slightly jittered icosahedron (non-indexed, so it stays unmerged). */
function rockGeometry() {
  const g = new THREE.IcosahedronGeometry(0.5, 0);
  g.scale(1, 0.62, 0.85);
  const pos = g.attributes.position.array;
  for (let i = 0; i < pos.length; i += 3) { const j = 0.85 + 0.3 * hash2(i, 91); pos[i] *= j; pos[i + 2] *= j; }
  g.translate(0, 0.2, 0);
  g.computeVertexNormals();
  paint(g, 1, 1, 1);
  return g;
}
function paint(geo, r, g, b) {
  const n = geo.attributes.position.count, col = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) { col[i * 3] = r; col[i * 3 + 1] = g; col[i * 3 + 2] = b; }
  geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
}

/** Per-terrain planting: [trees per cell (fractional = probability), kind, canopy colour, height factor]. */
const PLANTING = {
  [T.ROCK]: [0.9, "rock", 0x6e6a70, 0.55],
  [T.FOREST]: [1.7, "leaf", 0x2f7a32, 0.82],
  [T.RAINFOREST]: [2.6, "leaf", 0x1f6b3a, 1.0],
  [T.TAIGA]: [1.7, "cone", 0x2b5b45, 0.95],
  [T.GRASS]: [0.08, "leaf", 0x4c9a3c, 0.7],
  [T.SAVANNA]: [0.16, "leaf", 0x8a8a3c, 0.65],
  [T.TUNDRA]: [0.06, "cone", 0x5c7060, 0.6],
};
/** Grass tufts per cell (fractional = probability), with their blade colour. */
const TUFTS = { [T.GRASS]: 2.4, [T.SAVANNA]: 1.8, [T.FOREST]: 0.6, [T.TUNDRA]: 0.7, [T.RAINFOREST]: 0.4 };
const TUFT_COLOR = { [T.GRASS]: 0x78b544, [T.SAVANNA]: 0xb9a94e, [T.FOREST]: 0x5a9a44, [T.TUNDRA]: 0x8c9c72, [T.RAINFOREST]: 0x3f8f4a };
/** Extra rock scatter on cells whose main planting is something else. */
const ROCKS = { [T.TUNDRA]: 0.22, [T.DESERT]: 0.05, [T.SAVANNA]: 0.03, [T.GRASS]: 0.015 };
const hash2 = (a, b) => { let h = (a * 374761393 + b * 668265263) | 0; h = Math.imul(h ^ (h >>> 13), 1274126177); return ((h ^ (h >>> 16)) >>> 0) / 4294967296; };

const _fm = new THREE.Matrix4(), _fp = new THREE.Vector3(), _fq = new THREE.Quaternion(), _fs = new THREE.Vector3(), _fe = new THREE.Euler();

/** Scale of a tree on a cell scarred bare, as a fraction of its planted size. */
const BARE = 0.12;
/**
 * Pace of a tree shrinking into a clearing or growing back (and of the
 * scarred-bare shrink): `TRANSITION_TICKS` of sim time — `Villages.GROW`, so
 * the forest makes room in step with the huts' own grow-in ease — held
 * between `TRANSITION_MIN_S` and `TRANSITION_MAX_S` of wall time. Sim time
 * alone would freeze a layer toggle while paused (no ticks pass) and pop in
 * one frame at 64× (40 ticks is under a frame); wall time alone would trail a
 * village founded at 64× by thousands of ticks. At 1× (one tick per 60 Hz
 * frame) the band does not bind and trees and huts ease together.
 */
export const TRANSITION_TICKS = 40, TRANSITION_MIN_S = 0.3, TRANSITION_MAX_S = 0.8;

/** Instanced forests over the terrain's cells. */
export class Forest {
  constructor(terrain, maxPerKind = 60000) {
    this.terrain = terrain;
    this.max = maxPerKind;
    this.uniforms = { uTime: { value: 0 }, uCloud: terrain.uniforms.uCloud };
    // Wind: canopies sway with a slow wave keyed on the instance's world
    // position, so a forest ripples instead of nodding in unison. The same
    // cloud shade as the ground passes over the canopies.
    const mat = (side = THREE.FrontSide) => {
      const m = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.9, flatShading: true, side });
      m.customProgramCacheKey = () => `atlas-tree-${side}`;
      m.onBeforeCompile = (shader) => {
        shader.uniforms.uTime = this.uniforms.uTime;
        shader.uniforms.uCloud = this.uniforms.uCloud;
        shader.vertexShader = shader.vertexShader
          .replace("#include <common>", "#include <common>\nuniform float uTime; varying vec2 vWxz;")
          .replace("#include <begin_vertex>", `#include <begin_vertex>
            {
              #ifdef USE_INSTANCING
                vec2 wp = instanceMatrix[3].xz;
              #else
                vec2 wp = vec2(0.0);
              #endif
              vWxz = wp;
              float lift = smoothstep(0.35, 1.3, position.y);
              float w = sin(uTime * 1.1 + wp.x * 0.045 + wp.y * 0.07) + 0.5 * sin(uTime * 2.3 + wp.x * 0.11);
              transformed.x += w * 0.045 * lift;
              transformed.z += w * 0.025 * lift;
            }`);
        shader.fragmentShader = shader.fragmentShader
          .replace("#include <common>", `#include <common>\nuniform float uTime; uniform float uCloud; varying vec2 vWxz;\n${GLSL_NOISE}\n${GLSL_CLOUD}`)
          .replace("#include <color_fragment>", "#include <color_fragment>\ndiffuseColor.rgb *= 1.0 - 0.22 * atlasCloud(vWxz, uTime) * uCloud;");
      };
      return m;
    };
    this.leaf = new THREE.InstancedMesh(broadleafGeometry(), mat(), maxPerKind);
    this.cone = new THREE.InstancedMesh(coniferGeometry(), mat(), maxPerKind);
    this.tuft = new THREE.InstancedMesh(tuftGeometry(), mat(THREE.DoubleSide), maxPerKind);
    this.rock = new THREE.InstancedMesh(rockGeometry(), new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.95, flatShading: true }), maxPerKind);
    for (const m of [this.leaf, this.cone, this.tuft, this.rock]) {
      m.count = 0; m.frustumCulled = false; m.castShadow = true; m.receiveShadow = true; m.name = "forest";
    }
    this.tuft.castShadow = false;   // thousands of blades: their shadow is noise, not grounding
    this.group = new THREE.Group();
    this.group.add(this.leaf, this.cone, this.tuft, this.rock);
    // {mesh, index, kind, cell, x, z, rot, tx, tz, base, bare, cleared,
    //  sc (drawn scale), to (target scale), from, p (0..1 progress from → to)}
    this.items = [];
    this.scar = null;    // per-cell 0/1 "bare" flags from the last colour refresh
    this.byCell = new Map();   // cell → items planted there (clearing lookups)
    this.clearings = [];       // [{x, y, r}] village footprints the trees stand back from
    this.clearedCells = new Set();
    // Only items between two scales are re-seated each frame; a settled
    // forest costs nothing. Matrix uploads are narrowed to the index span
    // written since the last flush, unless a whole-buffer upload is pending
    // (rebuild), which a narrower range would otherwise cancel.
    this.active = new Set();
    this.dirty = new Map();    // mesh → [lo, hi] instance indices written
    this.fullUpload = new Set();
    for (const m of [this.leaf, this.cone, this.tuft, this.rock]) m.instanceMatrix.onUpload(() => this.fullUpload.delete(m));
    this.rebuild();
  }

  /** Place trees from the terrain ids (deterministic per cell). */
  rebuild() {
    const t = this.terrain, ids = t.terrainIds, res = t.res, cell = t.cell;
    this.items.length = 0;
    this.byCell.clear();
    this.clearedCells.clear();
    this.active.clear();       // the old items' indices are about to be reused
    this.dirty.clear();
    // Scars carry over (same grid, same cells): a relief toggle keeps bare
    // cells bare instead of flashing them full-size until the next refresh.
    const scar = this.scar;
    const counts = { leaf: 0, cone: 0, tuft: 0, rock: 0 };
    if (ids) {
      // Budget: keep every world under the instance cap by thinning uniformly
      // (trees and rocks share one budget, grass has its own).
      let want = 0, wantTuft = 0;
      for (let k = 0; k < res * res; k++) { const p = PLANTING[ids[k]]; if (p) want += p[0]; want += ROCKS[ids[k]] || 0; wantTuft += TUFTS[ids[k]] || 0; }
      const keep = Math.min(1, this.max / Math.max(1, want));
      const keepTuft = Math.min(1, this.max / Math.max(1, wantTuft));
      const m = new THREE.Matrix4(), p = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3(), c = new THREE.Color();
      const e = new THREE.Euler();
      const meshes = { leaf: this.leaf, cone: this.cone, tuft: this.tuft, rock: this.rock };
      const plant = (k, cx, cy, density, kind, colour, hf, salt, keepK = keep) => {
        const n = Math.floor(density) + (hash2(k, salt) < density % 1 ? 1 : 0);
        for (let i = 0; i < n; i++) {
          if (hash2(k, salt + 100 + i) > keepK) continue;
          const mesh = meshes[kind];
          const index = counts[kind]++;
          if (index >= this.max) continue;
          const x = (cx + hash2(k, salt + 200 + i)) * cell, z = (cy + hash2(k, salt + 300 + i)) * cell;
          const base = cell * hf * (0.55 + 0.5 * hash2(k, salt + 400 + i));
          const rot = hash2(k, salt + 500 + i) * Math.PI * 2;
          // Trees lean a little; grass and rocks sit square.
          const lean = kind === "leaf" || kind === "cone" ? 0.16 : 0;
          const tx = (hash2(k, salt + 700 + i) - 0.5) * lean, tz = (hash2(k, salt + 800 + i) - 0.5) * lean;
          const bare = scar && kind !== "rock" ? scar[k] : 0;
          const sc = bare ? base * BARE : base;
          const item = { mesh, index, kind, cell: k, x, z, rot, tx, tz, base, bare, cleared: false, sc, to: sc, from: sc, p: 1 };
          this.items.push(item);
          const list = this.byCell.get(k);
          if (list) list.push(item); else this.byCell.set(k, [item]);
          p.set(x, t.heightAt(x, z), z);
          q.setFromEuler(e.set(tx, rot, tz));
          s.set(sc, sc, sc);
          mesh.setMatrixAt(index, m.compose(p, q, s));
          c.setHex(colour).offsetHSL((hash2(k, salt + 900 + i) - 0.5) * 0.05, (hash2(k, salt + 1000 + i) - 0.5) * 0.18, (hash2(k, salt + 600 + i) - 0.5) * 0.14);
          mesh.setColorAt(index, c);
        }
      };
      for (let k = 0; k < res * res; k++) {
        const cx = k % res, cy = (k / res) | 0;
        const plan = PLANTING[ids[k]];
        if (plan) plant(k, cx, cy, plan[0], plan[1], plan[2], plan[3], 7);
        const rocks = ROCKS[ids[k]];
        if (rocks) plant(k, cx, cy, rocks, "rock", 0x6e6a70, 0.4, 9);
        const tufts = TUFTS[ids[k]];
        if (tufts) plant(k, cx, cy, tufts, "tuft", TUFT_COLOR[ids[k]], 0.42, 11, keepTuft);
      }
    }
    this.leaf.count = Math.min(counts.leaf, this.max);
    this.cone.count = Math.min(counts.cone, this.max);
    this.tuft.count = Math.min(counts.tuft, this.max);
    this.rock.count = Math.min(counts.rock, this.max);
    for (const mesh of [this.leaf, this.cone, this.tuft, this.rock]) {
      mesh.instanceMatrix.clearUpdateRanges();
      mesh.instanceMatrix.needsUpdate = true;
      this.fullUpload.add(mesh);
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    }
    if (this.clearings.length) this.setClearings(this.clearings, true);
  }

  /** Scale an item should settle at: cleared ? 0 : bare ? 0.12·base : base. */
  _target(it) { return it.cleared ? 0 : it.bare && it.kind !== "rock" ? it.base * BARE : it.base; }

  /** Write one item's matrix at its drawn scale `it.sc` and widen its mesh's upload span. */
  _place(it) {
    const sc = it.sc;
    _fp.set(it.x, this.terrain.heightAt(it.x, it.z), it.z);
    _fq.setFromEuler(_fe.set(it.tx, it.rot, it.tz));
    _fs.set(sc, sc, sc);
    it.mesh.setMatrixAt(it.index, _fm.compose(_fp, _fq, _fs));
    const span = this.dirty.get(it.mesh);
    if (!span) this.dirty.set(it.mesh, [it.index, it.index]);
    else { if (it.index < span[0]) span[0] = it.index; if (it.index > span[1]) span[1] = it.index; }
  }

  /** Mark the written spans for upload (one range per mesh). */
  _flush() {
    for (const [mesh, [lo, hi]] of this.dirty) {
      const a = mesh.instanceMatrix;
      // A hidden forest never uploads: stop stacking ranges and send it whole.
      if (a.updateRanges.length > 64) { a.clearUpdateRanges(); this.fullUpload.add(mesh); }
      if (!this.fullUpload.has(mesh)) a.addUpdateRange(lo * 16, (hi - lo + 1) * 16);
      a.needsUpdate = true;
    }
    this.dirty.clear();
  }

  /**
   * An item's bare/cleared state changed: `snap` seats it at the new target
   * at once (rebuilds, time jumps, first refresh); otherwise it eases there
   * from wherever it stands — mid-transition included — as `update` runs.
   */
  _retarget(it, snap) {
    const to = this._target(it);
    if (snap || to === it.sc) {
      this.active.delete(it);
      it.to = it.sc = to; it.p = 1;
      if (snap) this._place(it);
      return;
    }
    if (to === it.to) return;
    it.from = it.sc; it.to = to; it.p = 0;
    this.active.add(it);
  }

  /**
   * Advance the transitions by `dt` wall seconds and `dTicks` sim ticks (see
   * TRANSITION_TICKS for the pace). Pass `dt = Infinity` to finish them now
   * (reduced motion). Eases out like the huts' grow-in.
   */
  update(dt = 0, dTicks = 0) {
    if (!this.active.size) return;
    const k = Math.min(dt / TRANSITION_MIN_S, Math.max(dt / TRANSITION_MAX_S, dTicks / TRANSITION_TICKS));
    if (!(k > 0)) return;
    for (const it of this.active) {
      it.p = Math.min(1, it.p + k);
      it.sc = it.p >= 1 ? it.to : it.from + (it.to - it.from) * (1 - (1 - it.p) ** 3);
      this._place(it);
      if (it.p >= 1) this.active.delete(it);
    }
    this._flush();
  }

  /** Finish every transition now. */
  settle() { this.update(Infinity); }

  /**
   * Keep trees, grass and rocks off village footprints: `circles` is
   * `[{x, y, r}]` in world units. Only cells under the old or new clearings
   * are revisited, so calling this whenever a village appears or moves is cheap.
   * Trees ease out of (and back into) a clearing; `snap` re-seats every item
   * under the old and new clearings at once (rebuilds and time jumps).
   */
  setClearings(circles, snap = false) {
    this.clearings = circles;
    if (!this.items.length) return;
    const { res, cell } = this.terrain;
    const cells = new Set();
    for (const c of circles) {
      const r = c.r + cell;
      const i0 = Math.floor((c.x - r) / cell), i1 = Math.floor((c.x + r) / cell);
      const j0 = Math.floor((c.y - r) / cell), j1 = Math.floor((c.y + r) / cell);
      for (let j = Math.max(0, j0); j <= Math.min(res - 1, j1); j++) {
        for (let i = Math.max(0, i0); i <= Math.min(res - 1, i1); i++) cells.add(j * res + i);
      }
    }
    const touched = new Set(cells);
    for (const k of this.clearedCells) touched.add(k);
    for (const k of touched) {
      const list = this.byCell.get(k);
      if (!list) continue;
      for (const it of list) {
        let cleared = false;
        if (cells.has(k)) {
          for (const c of circles) {
            const dx = it.x - c.x, dz = it.z - c.y;
            if (dx * dx + dz * dz < c.r * c.r) { cleared = true; break; }
          }
        }
        if (cleared === it.cleared && !snap) continue;
        it.cleared = cleared;
        this._retarget(it, snap);
      }
    }
    this._flush();
    this.clearedCells = cells;
  }

  /** Shrink trees on cells the biome has scarred bare (succession/pollution),
   *  restore them as the cell greens again. Cheap: only cells that flipped,
   *  eased like a clearing; the first refresh after a (re)build and `snap`
   *  (a time jump) seat them at once. */
  refresh(rgba, snap = false) {
    if (!this.items.length) return;
    const res = this.terrain.res, n = res * res;
    const scar = new Uint8Array(n);
    for (let k = 0; k < n; k++) {
      const r = rgba[k * 4], g = rgba[k * 4 + 1];
      scar[k] = r > g * 1.05 ? 1 : 0;   // browner than green: bare earth / pioneer brown / pollution smudge
    }
    const prev = this.scar;
    for (const it of this.items) {
      if (it.kind === "rock") continue;
      const bare = scar[it.cell];
      if (prev && prev[it.cell] === bare) continue;
      it.bare = bare;
      this._retarget(it, snap || !prev);
    }
    this._flush();
    this.scar = scar;
  }

  /** Advance the wind clock (seconds). */
  setTime(t) { this.uniforms.uTime.value = t; }

  dispose() {
    for (const m of [this.leaf, this.cone, this.tuft, this.rock]) { m.geometry.dispose(); m.material.dispose(); m.dispose(); }
  }
}
