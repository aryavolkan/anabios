#!/usr/bin/env node
// Stage the scenario files for the web page and write web/scenarios/index.json.
//
// The page is served from web/ alone (GitHub Pages uploads that directory), so
// the curated scenario TOMLs are copied in, together with a manifest carrying
// each scenario's name, seed, dimensions, population, opt-in flags and the
// one-line phenomenon from docs/scenarios.md. If showcase/replay.js exists it
// is staged too, as the recorded-deck entry.
//
//   node web/scripts/manifest.mjs        (also run by scripts/web.sh build)

import { readdirSync, readFileSync, writeFileSync, mkdirSync, copyFileSync, existsSync, rmSync } from "node:fs";
import { dirname, resolve, basename } from "node:path";
import { fileURLToPath } from "node:url";

const web = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(web, "..");
const srcDir = resolve(root, "scenarios");
const outDir = resolve(web, "scenarios");
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

// Phenomenon ("Shows", the third) column of docs/scenarios.md, keyed by file
// name (a row may list several files separated by " / ").
const descriptions = new Map();
try {
  const doc = readFileSync(resolve(root, "docs/scenarios.md"), "utf8");
  for (const line of doc.split("\n")) {
    const m = line.match(/^\|\s*(.+?)\s*\|\s*(.+?)\s*\|\s*(.+?)\s*\|\s*$/);
    if (!m) continue;
    for (const f of m[1].matchAll(/`([^`]+\.toml)`/g)) descriptions.set(f[1], m[3].split("|")[0].trim().replace(/`/g, ""));
  }
} catch { /* docs are optional */ }

const scenarios = [];
for (const file of readdirSync(srcDir).filter((f) => f.endsWith(".toml")).sort()) {
  const text = readFileSync(resolve(srcDir, file), "utf8");
  const top = text.split(/^\[/m)[0]; // top-level keys only
  const get = (k) => top.match(new RegExp(`^${k}\\s*=\\s*([^#\\n]+)`, "m"))?.[1].trim();
  const name = get("name")?.replace(/^"|"$/g, "") ?? basename(file, ".toml");
  // The schema turns every feature on; what a world OPTS OUT of is the
  // informative list. `= false` for a feature knob, or a lever the world
  // deliberately turns on (env_period / climate_drift_rate > 0).
  const off = [...top.matchAll(/^([a-z_]+_enabled|living_biome|terrain_habitat|biome_adaptation|nutrient_variation|soil_fertility|gene_tech_coupling|gene_requirements|conserve_goods_on_death|repro_biased_learning)\s*=\s*false/gm)]
    .map((m) => m[1].replace(/_enabled$/, ""));
  if (get("season_period") === "0") off.push("season");
  const levers = [];
  for (const k of ["env_period"]) if (Number(get(k)) > 0) levers.push("env");
  if (Number(get("climate_drift_rate")) > 0) levers.push("drift");
  for (const k of ["payoff_biased_learning", "unilateral_trade"]) if (get(k) === "true") levers.push(k);
  const agents = [...text.matchAll(/^count\s*=\s*(\d+)/gm)].reduce((s, m) => s + Number(m[1]), 0);
  scenarios.push({
    file: `scenarios/${file}`, id: basename(file, ".toml"), name,
    seed: Number(get("seed") ?? 0),
    world_size: Number(get("world_size") ?? 1024),
    biome_res: Number(get("biome_res") ?? 128),
    agents, off, levers,
    description: descriptions.get(file) ?? "",
  });
  copyFileSync(resolve(srcDir, file), resolve(outDir, file));
}

const replays = [];
const replaySrc = resolve(root, "showcase/replay.js");
if (existsSync(replaySrc)) {
  mkdirSync(resolve(web, "replays"), { recursive: true });
  copyFileSync(replaySrc, resolve(web, "replays/out-of-africa-saga.js"));
  replays.push({ file: "replays/out-of-africa-saga.js", id: "out-of-africa-saga", name: "out-of-africa-saga · seed 318 (recorded)" });
}

writeFileSync(resolve(outDir, "index.json"), JSON.stringify({ generated: new Date().toISOString(), scenarios, replays }, null, 1));
console.log(`staged ${scenarios.length} scenarios + ${replays.length} replay(s) → web/scenarios/index.json`);
