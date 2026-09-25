#!/usr/bin/env node
// Copy the three.js runtime out of node_modules into web/vendor/ so the page
// is self-hosted (no third-party CDN at runtime) and needs no bundler: the
// import map in index.html points `three` and `three/addons/` here.
//
//   npm --prefix web install && npm --prefix web run vendor

import { copyFileSync, mkdirSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const web = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = resolve(web, "node_modules/three");
if (!existsSync(src)) {
  console.error("three is not installed — run `npm --prefix web install` first");
  process.exit(1);
}
const out = resolve(web, "vendor");
mkdirSync(resolve(out, "addons/controls"), { recursive: true });
mkdirSync(resolve(out, "addons/utils"), { recursive: true });
mkdirSync(resolve(out, "addons/postprocessing"), { recursive: true });
mkdirSync(resolve(out, "addons/shaders"), { recursive: true });

// three.module.js re-exports three.core.js; both are needed. The addons are
// plain ES modules importing from 'three', which the import map resolves.
const files = [
  ["build/three.module.js", "three.module.js"],
  ["build/three.core.js", "three.core.js"],
  ["examples/jsm/controls/OrbitControls.js", "addons/controls/OrbitControls.js"],
  ["examples/jsm/utils/BufferGeometryUtils.js", "addons/utils/BufferGeometryUtils.js"],
  // Bloom: EffectComposer → RenderPass → UnrealBloomPass → OutputPass, and their shaders.
  ...["Pass", "EffectComposer", "RenderPass", "ShaderPass", "MaskPass", "UnrealBloomPass", "OutputPass"]
    .map((n) => [`examples/jsm/postprocessing/${n}.js`, `addons/postprocessing/${n}.js`]),
  ...["CopyShader", "LuminosityHighPassShader", "OutputShader"]
    .map((n) => [`examples/jsm/shaders/${n}.js`, `addons/shaders/${n}.js`]),
  ["LICENSE", "LICENSE.three"],
];
for (const [from, to] of files) copyFileSync(resolve(src, from), resolve(out, to));

const version = JSON.parse(readFileSync(resolve(src, "package.json"), "utf8")).version;
writeFileSync(resolve(out, "VERSION"), `three@${version}\n`);
console.log(`vendored three@${version} → web/vendor/ (${files.length} files)`);
