// Terrain + water for the atlas. The biome grid (res² cells, row = y) becomes a
// (res+1)² vertex heightmap in the XZ plane: world x → three x, world y → three
// z, elevation → three y, with the sea level at y = 0 so the translucent water
// plane sits exactly on it. Cell colours are the sim's own `cell_color` view
// bytes (river-tinted, biomass-lushed) uploaded as a nearest-filtered data
// texture, so the ground keeps the crisp per-cell pixel look of the showcase
// player while the relief and lighting come from the mesh. The torus wraps:
// the last vertex row/column samples the first cell.

import * as THREE from "three";

export class Terrain {
  /**
   * @param {number} res       biome cells per axis
   * @param {number} worldSize world units per axis
   * @param {number} seaLevel  normalized elevation of the water line
   * @param {Float32Array|null} elevation res² normalized heights (null → flat)
   */
  constructor(res, worldSize, seaLevel, elevation) {
    this.res = res;
    this.worldSize = worldSize;
    this.cell = worldSize / res;
    this.seaLevel = seaLevel;
    this.elevation = elevation;
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
    // Colour lives in a res² RGBA8 texture (row 0 = world y 0, so no flip);
    // nearest magnification keeps cells crisp, linear minification keeps the
    // overview from shimmering.
    this.texels = new Uint8Array(res * res * 4).fill(255);
    this.texture = new THREE.DataTexture(this.texels, res, res, THREE.RGBAFormat, THREE.UnsignedByteType);
    this.texture.colorSpace = THREE.SRGBColorSpace;
    this.texture.magFilter = THREE.NearestFilter;
    this.texture.minFilter = THREE.LinearFilter;
    this.texture.generateMipmaps = false;
    this.texture.flipY = false;
    this.texture.needsUpdate = true;
    this.material = new THREE.MeshStandardMaterial({
      map: this.texture, roughness: 0.92, metalness: 0.0, flatShading: false,
    });
    this.mesh = new THREE.Mesh(geo, this.material);
    this.mesh.name = "terrain";
    this.mesh.receiveShadow = false;
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

    this.water = new Water(worldSize);
    this.group = new THREE.Group();
    this.group.add(this.mesh, this.slab, this.water.mesh);
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
  }

  setRelief(on) {
    this.reliefOn = on && !!this.elevation;
    this.rebuildHeights();
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
  }

  dispose() {
    this.geometry.dispose();
    this.material.dispose();
    this.texture.dispose();
    this.slab.geometry.dispose();
    this.slab.material.dispose();
    this.water.dispose();
  }
}

/** Translucent water plane at y = 0 with a cheap animated ripple + fresnel. */
export class Water {
  constructor(worldSize) {
    this.uniforms = {
      uTime: { value: 0 },
      uColor: { value: new THREE.Color(0x173a6e) },
      uDeep: { value: new THREE.Color(0x0a1a3a) },
      uSize: { value: worldSize },
    };
    this.material = new THREE.ShaderMaterial({
      uniforms: this.uniforms,
      transparent: true,
      depthWrite: false,
      vertexShader: /* glsl */ `
        varying vec3 vWorld;
        varying vec3 vNormalW;
        void main() {
          vec4 w = modelMatrix * vec4(position, 1.0);
          vWorld = w.xyz;
          vNormalW = normalize(mat3(modelMatrix) * normal);
          gl_Position = projectionMatrix * viewMatrix * w;
        }`,
      fragmentShader: /* glsl */ `
        uniform float uTime; uniform vec3 uColor; uniform vec3 uDeep; uniform float uSize;
        varying vec3 vWorld; varying vec3 vNormalW;
        void main() {
          // two travelling sine sets → a soft moving shimmer
          float s = sin(vWorld.x * 0.08 + uTime * 0.9) * sin(vWorld.z * 0.11 - uTime * 0.7);
          float t = sin((vWorld.x + vWorld.z) * 0.05 + uTime * 0.5);
          float ripple = 0.5 + 0.5 * (0.6 * s + 0.4 * t);
          vec3 viewDir = normalize(cameraPosition - vWorld);
          float fres = pow(1.0 - max(dot(viewDir, vNormalW), 0.0), 2.0);
          vec3 col = mix(uDeep, uColor, 0.35 + 0.65 * ripple);
          col += vec3(0.25, 0.32, 0.36) * fres * 0.6;
          float alpha = 0.62 + 0.25 * fres + 0.05 * ripple;
          gl_FragColor = vec4(col, alpha);
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
