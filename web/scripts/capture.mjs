#!/usr/bin/env node
// Reproducible gallery captures of the atlas, headless.
//
//   scripts/web.sh build && scripts/web.sh serve &        # the page must be served
//   node web/scripts/capture.mjs gallery/atlas/shots.json  [--out gallery/atlas]
//        [--base http://127.0.0.1:8080/] [--only name[,name…]] [--width 1280] [--height 800]
//
// Each entry in the shots file is `{ "name", "params": { scenario | replay, seed,
// tick, cam, color, inspect, hud }, "note" }`. The page's capture harness
// (`?capture=1&tick=N&cam=…&inspect=…`, see `prepareShot` in web/src/main.js)
// fast-forwards the deterministic world to the exact tick, frames the camera
// and pins the agent, then sets `__atlas.state.ready`; this script waits for
// that and writes `<out>/<name>.png`. Every run is bit-identical per seed, so
// a shot reproduces from its params alone.
//
// Needs Playwright: `npm --prefix web i -D playwright && npx playwright install chromium`,
// or point PLAYWRIGHT_MODULE at an installed copy and PLAYWRIGHT_CHROMIUM at a
// Chromium binary. Software GL (swiftshader) is fine — it is what the
// committed gallery was rendered with.

import { readFileSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";

const args = process.argv.slice(2);
const shotsPath = args.find((a) => !a.startsWith("--"));
if (!shotsPath) { console.error("usage: capture.mjs <shots.json> [--out DIR] [--base URL] [--only a,b]"); process.exit(2); }
const opt = (k, d) => { const i = args.indexOf(`--${k}`); return i >= 0 ? args[i + 1] : d; };
const out = resolve(opt("out", "gallery/atlas"));
const base = opt("base", "http://127.0.0.1:8080/");
const only = opt("only", "")?.split(",").filter(Boolean);
const width = Number(opt("width", 1280)), height = Number(opt("height", 800));
mkdirSync(out, { recursive: true });

let chromium;
try {
  ({ chromium } = await import("playwright"));
} catch {
  ({ chromium } = createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE || "playwright"));
}
const launch = {
  args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
};
if (process.env.PLAYWRIGHT_CHROMIUM) launch.executablePath = process.env.PLAYWRIGHT_CHROMIUM;

const shots = JSON.parse(readFileSync(shotsPath, "utf8")).shots.filter((s) => !only?.length || only.includes(s.name));
let browser = await chromium.launch(launch);
let failed = 0;
for (const shot of shots) {
  const q = new URLSearchParams({ capture: "1", ...Object.fromEntries(Object.entries(shot.params).map(([k, v]) => [k, String(v)])) });
  const url = `${base}?${q}`;
  if (!browser.isConnected()) browser = await chromium.launch(launch);   // a crashed renderer must not sink the rest of the run
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
  const t0 = Date.now();
  try {
    await page.goto(url, { waitUntil: "load" });
    // Poll from the driver side: an in-page waitForFunction competes with the
    // render loop for the main thread and can starve under software GL.
    const deadline = Date.now() + 40 * 60 * 1000;
    for (;;) {
      await page.waitForTimeout(500);
      if (await page.evaluate(() => window.__atlas?.state.ready === true)) break;
      if (Date.now() > deadline) throw new Error("timed out waiting for the world to be ready");
    }
    await page.waitForTimeout(600);
    const stats = await page.evaluate(() => {
      const g = (id) => document.getElementById(id).textContent;
      return { tick: g("stat-tick"), alive: g("stat-alive"), species: g("stat-species"), era: g("stat-era") };
    });
    await page.screenshot({ path: resolve(out, `${shot.name}.png`) });
    console.log(`${shot.name}.png  tick=${stats.tick} alive=${stats.alive} species=${stats.species} era=${stats.era}  (${((Date.now() - t0) / 1000).toFixed(0)}s)${errors.length ? `  console: ${errors.join(" | ")}` : ""}`);
  } catch (e) {
    failed++;
    console.error(`${shot.name}: FAILED after ${((Date.now() - t0) / 1000).toFixed(0)}s — ${e.message}${errors.length ? ` | ${errors.join(" | ")}` : ""}`);
  }
  await page.close().catch(() => {});
}
await browser.close();
console.log(`${shots.length - failed}/${shots.length} captured → ${out}`);
process.exit(failed ? 1 : 0);
