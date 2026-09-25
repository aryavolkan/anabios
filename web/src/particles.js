// A small CPU particle pool drawn as two `Points` clouds: an additive one for
// embers, sparks and motes, and an alpha-blended one for smoke. Everything is
// spawned in world units and sized relative to the world so a 1024² and a
// 4096² map get the same look. Kinds:
//
//   ember  – rises and drifts, orange → dark red, for fire/invention events
//   spark  – radial burst under gravity, gold, for war/combat hits
//   mote   – slow rising glow in the event's colour, for the other kinds
//   smoke  – grey puffs that expand and fade, from village hearths

import * as THREE from "three";

const MAX = 6000;

const VERT = /* glsl */ `
  attribute float aSize; attribute vec3 aColor; attribute float aAlpha;
  uniform float uScale;
  varying vec4 vCol;
  void main() {
    vec4 mv = modelViewMatrix * vec4(position, 1.0);
    gl_PointSize = clamp(aSize * uScale / max(1.0, -mv.z), 1.0, 96.0);
    gl_Position = projectionMatrix * mv;
    vCol = vec4(aColor, aAlpha);
  }`;
const FRAG = /* glsl */ `
  varying vec4 vCol;
  void main() {
    float d = length(gl_PointCoord - 0.5) * 2.0;
    float a = smoothstep(1.0, 0.25, d);
    gl_FragColor = vec4(vCol.rgb, vCol.a * a);
    #include <tonemapping_fragment>
    #include <colorspace_fragment>
  }`;

class Cloud {
  constructor(additive) {
    this.pos = new Float32Array(MAX * 3);
    this.vel = new Float32Array(MAX * 3);
    this.col = new Float32Array(MAX * 3);
    this.size = new Float32Array(MAX);
    this.alpha = new Float32Array(MAX);
    this.life = new Float32Array(MAX);
    this.max = new Float32Array(MAX);
    this.kind = new Uint8Array(MAX);
    this.count = 0;
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(this.pos, 3).setUsage(THREE.DynamicDrawUsage));
    geo.setAttribute("aColor", new THREE.BufferAttribute(this.col, 3).setUsage(THREE.DynamicDrawUsage));
    geo.setAttribute("aSize", new THREE.BufferAttribute(this.size, 1).setUsage(THREE.DynamicDrawUsage));
    geo.setAttribute("aAlpha", new THREE.BufferAttribute(this.alpha, 1).setUsage(THREE.DynamicDrawUsage));
    geo.setDrawRange(0, 0);
    this.uniforms = { uScale: { value: 400 } };
    this.points = new THREE.Points(geo, new THREE.ShaderMaterial({
      uniforms: this.uniforms, vertexShader: VERT, fragmentShader: FRAG, transparent: true, depthWrite: false,
      blending: additive ? THREE.AdditiveBlending : THREE.NormalBlending,
    }));
    this.points.frustumCulled = false;
    this.points.renderOrder = 3;
  }
  kill(i) {
    const j = --this.count;
    if (i === j) return;
    for (const a of [this.pos, this.vel, this.col]) { a[i * 3] = a[j * 3]; a[i * 3 + 1] = a[j * 3 + 1]; a[i * 3 + 2] = a[j * 3 + 2]; }
    this.size[i] = this.size[j]; this.alpha[i] = this.alpha[j]; this.life[i] = this.life[j]; this.max[i] = this.max[j]; this.kind[i] = this.kind[j];
  }
  flush() {
    const g = this.points.geometry;
    g.setDrawRange(0, this.count);
    for (const k of ["position", "aColor", "aSize", "aAlpha"]) g.attributes[k].needsUpdate = true;
  }
}

export const KIND = Object.freeze({ EMBER: 0, SPARK: 1, MOTE: 2, SMOKE: 3 });

export class Particles {
  constructor() {
    this.glow = new Cloud(true);
    this.smoke = new Cloud(false);
    this.group = new THREE.Group();
    this.group.add(this.glow.points, this.smoke.points);
    this.enabled = true;
    this.unit = 1;   // world units per unit of a standard 8-unit biome cell
    this._c = new THREE.Color();
  }
  setWorldSize(ws, cell = ws / 128) { this.unit = cell / 8; }

  /**
   * Spawn `n` particles of `kind` at world (x, h, z). `color` is a packed hex
   * for motes/sparks/embers; smoke ignores it.
   */
  spawn(kind, x, h, z, n, color = 0xffffff) {
    if (!this.enabled) return;
    const cloud = kind === KIND.SMOKE ? this.smoke : this.glow;
    const u = this.unit;
    this._c.setHex(color);
    for (let k = 0; k < n; k++) {
      if (cloud.count >= MAX) break;
      const i = cloud.count++;
      const a = Math.random() * Math.PI * 2, r = Math.random();
      let vx = 0, vy = 0, vz = 0, life = 1, size = 1, cr = this._c.r, cg = this._c.g, cb = this._c.b;
      switch (kind) {
        case KIND.EMBER:
          vx = Math.cos(a) * (1 + r * 2.5) * u; vz = Math.sin(a) * (1 + r * 2.5) * u; vy = (2.5 + Math.random() * 3.5) * u;
          life = 1.4 + Math.random() * 1.6; size = (0.45 + Math.random() * 0.5) * u;
          cr = 1.0; cg = 0.55 + Math.random() * 0.3; cb = 0.15;
          break;
        case KIND.SPARK:
          vx = Math.cos(a) * (3 + r * 7) * u; vz = Math.sin(a) * (3 + r * 7) * u; vy = (2 + Math.random() * 6) * u;
          life = 0.45 + Math.random() * 0.5; size = (0.35 + Math.random() * 0.35) * u;
          cr = 1.0; cg = 0.8 + Math.random() * 0.2; cb = 0.45;
          break;
        case KIND.MOTE:
          vx = Math.cos(a) * (0.4 + r * 1.2) * u; vz = Math.sin(a) * (0.4 + r * 1.2) * u; vy = (0.8 + Math.random() * 1.4) * u;
          life = 1.8 + Math.random() * 1.6; size = (0.4 + Math.random() * 0.5) * u;
          break;
        case KIND.SMOKE:
          vx = (0.4 + Math.random() * 0.4) * u; vz = (Math.random() - 0.5) * 0.3 * u; vy = (1.2 + Math.random() * 0.8) * u;
          life = 2.8 + Math.random() * 1.6; size = (0.8 + Math.random() * 0.5) * u;
          cr = cg = cb = 0.14 + Math.random() * 0.06;
          break;
      }
      const spread = kind === KIND.SPARK ? 0.6 : kind === KIND.SMOKE ? 1.0 : 3.0;
      cloud.pos[i * 3] = x + (Math.random() - 0.5) * spread * u;
      cloud.pos[i * 3 + 1] = h;
      cloud.pos[i * 3 + 2] = z + (Math.random() - 0.5) * spread * u;
      cloud.vel[i * 3] = vx; cloud.vel[i * 3 + 1] = vy; cloud.vel[i * 3 + 2] = vz;
      cloud.col[i * 3] = cr; cloud.col[i * 3 + 1] = cg; cloud.col[i * 3 + 2] = cb;
      cloud.size[i] = size; cloud.alpha[i] = 1; cloud.life[i] = 0; cloud.max[i] = life; cloud.kind[i] = kind;
    }
  }

  /** Step every particle by `dt` seconds; `pxScale` = viewport height / (2·tan(fov/2)). */
  update(dt, pxScale) {
    const u = this.unit;
    for (const cloud of [this.glow, this.smoke]) {
      cloud.uniforms.uScale.value = pxScale;
      for (let i = cloud.count - 1; i >= 0; i--) {
        const life = (cloud.life[i] += dt), t = life / cloud.max[i];
        if (t >= 1) { cloud.kill(i); continue; }
        const k = cloud.kind[i];
        // motion
        if (k === KIND.SPARK) cloud.vel[i * 3 + 1] -= 14 * u * dt;
        else if (k === KIND.EMBER) { cloud.vel[i * 3 + 1] -= 0.6 * u * dt; cloud.vel[i * 3] *= 0.985; cloud.vel[i * 3 + 2] *= 0.985; }
        else if (k === KIND.SMOKE) cloud.vel[i * 3] += 0.35 * u * dt;
        cloud.pos[i * 3] += cloud.vel[i * 3] * dt;
        cloud.pos[i * 3 + 1] += cloud.vel[i * 3 + 1] * dt;
        cloud.pos[i * 3 + 2] += cloud.vel[i * 3 + 2] * dt;
        // look
        if (k === KIND.SMOKE) { cloud.size[i] += 1.4 * u * dt; cloud.alpha[i] = 0.26 * (1 - t) * Math.min(1, t * 6); }
        else if (k === KIND.EMBER) { cloud.alpha[i] = (1 - t) * (1 - t); cloud.col[i * 3 + 1] = 0.75 * (1 - t) + 0.1; }
        else if (k === KIND.SPARK) cloud.alpha[i] = 1 - t * t;
        else cloud.alpha[i] = Math.sin(t * Math.PI);
      }
      cloud.flush();
    }
  }

  clear() { this.glow.count = 0; this.smoke.count = 0; this.glow.flush(); this.smoke.flush(); }
}
