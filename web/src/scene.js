// Renderer, camera, lights and controls — the "stage" everything else sits on.

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { UI } from "./palette.js";

export function createStage(canvas) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, powerPreference: "high-performance" });
  renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(UI.basalt);

  const camera = new THREE.PerspectiveCamera(42, 1, 0.5, 20000);
  const controls = new OrbitControls(camera, canvas);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;
  controls.screenSpacePanning = false;
  controls.maxPolarAngle = 1.42;
  controls.minPolarAngle = 0.15;
  controls.zoomSpeed = 0.9;
  controls.panSpeed = 0.9;

  const hemi = new THREE.HemisphereLight(0xdcd2be, 0x2a2118, 0.9);
  const sun = new THREE.DirectionalLight(0xffe2b8, 2.0);
  sun.position.set(0.55, 1.0, 0.35);
  const fill = new THREE.DirectionalLight(0x6e9bb5, 0.35);
  fill.position.set(-0.6, 0.5, -0.7);
  scene.add(hemi, sun, fill);

  const stage = {
    renderer, scene, camera, controls, sun,
    worldSize: 1024,
    /** Fit the camera limits, fog and lights to a world of side `ws`. */
    fit(ws) {
      this.worldSize = ws;
      controls.minDistance = ws * 0.01;
      controls.maxDistance = ws * 2.2;
      camera.far = ws * 8;
      camera.updateProjectionMatrix();
      scene.fog = new THREE.FogExp2(UI.basalt, 0.55 / ws);
      sun.position.set(ws * 0.55, ws * 1.0, ws * 0.35);
      sun.target.position.set(ws / 2, 0, ws / 2);
      sun.target.updateMatrixWorld();
    },
    /** Frame the whole world from a three-quarter view. */
    frame() {
      const ws = this.worldSize;
      controls.target.set(ws / 2, 0, ws / 2);
      camera.position.set(ws / 2 + ws * 0.5, ws * 0.95, ws / 2 + ws * 1.05);
      controls.update();
    },
    /** Smoothly move the orbit target to (x, h, z) keeping the camera offset. */
    flyTo(x, h, z, distance) {
      this._fly = { to: new THREE.Vector3(x, h, z), distance, t: 0 };
    },
    tick(dt) {
      if (this._fly) {
        const f = this._fly;
        f.t = Math.min(1, f.t + dt * 1.8);
        const k = 1 - Math.pow(1 - f.t, 3);
        const offset = camera.position.clone().sub(controls.target);
        if (f.distance) offset.setLength(offset.length() + (f.distance - offset.length()) * k * 0.25);
        controls.target.lerp(f.to, k * 0.35 + 0.05);
        camera.position.copy(controls.target).add(offset);
        if (f.t >= 1 && controls.target.distanceTo(f.to) < 0.5) this._fly = null;
      }
      controls.update();
    },
    resize() {
      const w = canvas.clientWidth || window.innerWidth, h = canvas.clientHeight || window.innerHeight;
      renderer.setSize(w, h, false);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
    },
  };
  stage.resize();
  window.addEventListener("resize", () => stage.resize(), { passive: true });
  return stage;
}
