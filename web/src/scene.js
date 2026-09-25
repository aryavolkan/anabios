// Renderer, camera, lights and controls — the "stage" everything else sits on.

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { EffectComposer } from "three/addons/postprocessing/EffectComposer.js";
import { RenderPass } from "three/addons/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/addons/postprocessing/UnrealBloomPass.js";
import { OutputPass } from "three/addons/postprocessing/OutputPass.js";
import { UI } from "./palette.js";

/**
 * Sky dome: a camera-centred sphere shaded by view direction — indigo overhead
 * falling to the warm basalt of the fog, with a sun glow that swells and
 * reddens as the day cycle lowers the sun. Drawn at the far plane under
 * everything else.
 */
function skyDome() {
  const mat = new THREE.ShaderMaterial({
    side: THREE.BackSide, depthWrite: false, depthTest: false, fog: false,
    uniforms: {
      uSun: { value: new THREE.Vector3(0.55, 1.0, 0.35).normalize() },
      uUp: { value: 1 },
      uZenith: { value: new THREE.Color(0x171626) },
      uHorizon: { value: new THREE.Color(0x2b2019) },
      uGround: { value: new THREE.Color(0x1c1611) },
    },
    vertexShader: /* glsl */ `varying vec3 vDir;
      void main() { vDir = normalize(position); vec4 p = projectionMatrix * modelViewMatrix * vec4(position, 1.0); gl_Position = p.xyww; }`,
    fragmentShader: /* glsl */ `uniform vec3 uSun; uniform float uUp; uniform vec3 uZenith; uniform vec3 uHorizon; uniform vec3 uGround; varying vec3 vDir;
      void main() {
        vec3 d = normalize(vDir);
        float y = d.y;
        vec3 col = y >= 0.0 ? mix(uHorizon, uZenith, pow(y, 0.55)) : mix(uHorizon, uGround, pow(-y, 0.5));
        float s = max(dot(d, uSun), 0.0);
        float dusk = 1.0 - uUp;
        vec3 warm = mix(vec3(0.95, 0.80, 0.55), vec3(1.0, 0.55, 0.22), dusk);
        col += warm * (0.10 + 0.22 * dusk) * pow(s, 3.0);      // broad haze toward the sun
        col += warm * (0.35 + 0.55 * dusk) * pow(s, 24.0);     // the sun's glow
        col += uHorizon * 0.6 * exp(-abs(y) * 5.0);            // a band along the horizon
        gl_FragColor = vec4(col, 1.0);
        #include <tonemapping_fragment>
        #include <colorspace_fragment>
      }`,
  });
  const mesh = new THREE.Mesh(new THREE.SphereGeometry(1, 32, 16), mat);
  mesh.scale.setScalar(100);
  mesh.frustumCulled = false;
  mesh.renderOrder = -10;
  mesh.name = "sky";
  return mesh;
}

export function createStage(canvas) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, powerPreference: "high-performance" });
  renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.12;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFShadowMap;

  const scene = new THREE.Scene();
  const sky = skyDome();
  scene.add(sky);

  const camera = new THREE.PerspectiveCamera(42, 1, 0.5, 20000);
  const controls = new OrbitControls(camera, canvas);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;
  controls.screenSpacePanning = false;
  controls.maxPolarAngle = 1.42;
  controls.minPolarAngle = 0.15;
  controls.zoomSpeed = 0.9;
  controls.panSpeed = 0.9;

  const hemi = new THREE.HemisphereLight(0xd9d0c0, 0x2a2118, 0.7);
  const sun = new THREE.DirectionalLight(0xffdcae, 2.6);
  sun.position.set(0.55, 1.0, 0.35);
  sun.castShadow = true;
  sun.shadow.mapSize.set(2048, 2048);
  sun.shadow.bias = -0.0004;
  const fill = new THREE.DirectionalLight(0x6e9bb5, 0.3);
  fill.position.set(-0.6, 0.5, -0.7);
  scene.add(hemi, sun, sun.target, fill);

  // Post: a soft bloom lifts the additive layers (event pillars, sparks,
  // sun glint) without washing the ground; the OutputPass applies tone
  // mapping and the sRGB transfer that the direct path gets for free.
  const composer = new EffectComposer(renderer);
  composer.addPass(new RenderPass(scene, camera));
  const bloom = new UnrealBloomPass(new THREE.Vector2(1, 1), 0.3, 0.4, 1.15);
  composer.addPass(bloom);
  composer.addPass(new OutputPass());

  const stage = {
    renderer, scene, camera, controls, sun, sky, composer, bloom,
    /** Bloom on: render through the composer; off: straight to the canvas. */
    post: true,
    render() {
      if (this.post) composer.render(); else renderer.render(scene, camera);
    },
    worldSize: 1024,
    /** Fit the camera limits, fog and lights to a world of side `ws`. */
    fit(ws) {
      this.worldSize = ws;
      controls.minDistance = ws * 0.01;
      controls.maxDistance = ws * 2.2;
      camera.far = ws * 8;
      camera.updateProjectionMatrix();
      scene.fog = new THREE.FogExp2(UI.basalt, 0.5 / ws);
      sun.position.set(ws / 2 + ws * 0.55, ws * 1.0, ws / 2 + ws * 0.35);
      sun.target.position.set(ws / 2, 0, ws / 2);
      sun.target.updateMatrixWorld();
      // One shadow cascade over the whole plate: crisp enough for a 1024² world,
      // soft on the huge tiers where it mostly adds grounding.
      const sc = sun.shadow.camera;
      sc.left = -ws * 0.72; sc.right = ws * 0.72; sc.top = ws * 0.72; sc.bottom = -ws * 0.72;
      sc.near = ws * 0.2; sc.far = ws * 2.6;
      sc.updateProjectionMatrix();
      sun.shadow.normalBias = ws * 0.0015;
      sun.shadow.needsUpdate = true;
    },
    /**
     * Daylight for day-fraction `u` in [0,1): noon at 0, dusk at 0.5. The sun
     * sweeps around the plate, sinks toward the horizon and warms; the fill
     * and exposure follow. Never darker than dusk so the map stays readable.
     */
    setDaylight(u) {
      const ws = this.worldSize, t = u * Math.PI * 2;
      const up = 0.5 + 0.5 * Math.cos(t);            // 1 noon … 0 midnight
      const az = 0.57 + t * 0.85;                    // slow sweep, starting where the fixed sun sat
      const r = ws * (0.55 + 0.3 * (1 - up)), h = ws * (0.5 + 0.5 * up);
      sun.position.set(ws / 2 + Math.cos(az) * r, h, ws / 2 + Math.sin(az) * r);
      sun.intensity = 2.0 + 0.6 * up;
      sun.color.setHex(0xffb070).lerp(new THREE.Color(0xffdcae), Math.pow(up, 0.6));
      hemi.intensity = 0.6 + 0.1 * up;
      fill.intensity = 0.26 + 0.06 * up;
      renderer.toneMappingExposure = 1.02 + 0.1 * up;
      this.sunDir.copy(sun.position).sub(sun.target.position).normalize();
      sky.material.uniforms.uSun.value.copy(this.sunDir);
      sky.material.uniforms.uUp.value = up;
    },
    sunDir: new THREE.Vector3(0.55, 1.0, 0.35).normalize(),
    /** Frame the whole world from a three-quarter view. */
    frame() {
      const ws = this.worldSize;
      controls.target.set(ws / 2, 0, ws / 2);
      camera.position.set(ws / 2 + ws * 0.5, ws * 0.95, ws / 2 + ws * 1.05);
      controls.update();
    },
    /**
     * Snap the camera to look at world (x, h, z) from `distance`, at `polar`
     * radians off vertical and `azimuth` radians east of due south. Used by
     * the capture harness (`?cam=x,y,zoom`) so gallery shots are reproducible.
     */
    lookAt(x, h, z, distance, polar = 0.85, azimuth = 0.4) {
      this._fly = null;
      controls.target.set(x, h, z);
      camera.position.set(
        x + distance * Math.sin(polar) * Math.sin(azimuth),
        h + distance * Math.cos(polar),
        z + distance * Math.sin(polar) * Math.cos(azimuth),
      );
      controls.update();
    },
    /** Camera distance at which `worldWidth` world units span the viewport width. */
    distanceForWidth(worldWidth) {
      const vfov = (camera.fov * Math.PI) / 180;
      return worldWidth / (2 * Math.tan(vfov / 2) * camera.aspect);
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
      sky.position.copy(camera.position);
    },
    resize() {
      const w = canvas.clientWidth || window.innerWidth, h = canvas.clientHeight || window.innerHeight;
      renderer.setSize(w, h, false);
      composer.setPixelRatio(renderer.getPixelRatio());
      composer.setSize(w, h);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
    },
  };
  stage.resize();
  window.addEventListener("resize", () => stage.resize(), { passive: true });
  return stage;
}
