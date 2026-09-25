#!/usr/bin/env node
// Smoke + determinism test for the wasm bridge, runnable with nothing but node.
//
//   node web/test/wasm-smoke.mjs [--wasm PATH] [--scenario PATH] [--ticks N]
//                                [--seed N] [--expect-fingerprint 0x…]
//
// Loads the module, runs a scenario, checks every buffer export is
// well-formed, prints the tick rate, and — when `--expect-fingerprint` is
// given — asserts the in-wasm portable fingerprint equals the one the native
// `fingerprint` example printed for the same scenario/seed/ticks
// (`cargo run -p anabios-wasm --example fingerprint`). `scripts/web.sh test`
// wires the two together. Exits non-zero on any failure.
//
// Note: `state_hash` (the CLI's determinism receipt) is NOT comparable across
// native and wasm — it hashes bincode output whose BitVec store words are
// pointer-width dependent. The fingerprint is the portable equivalent.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { AnabiosSim, AGENT, EVENT } from "../src/sim.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");

const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, all) => {
    if (a.startsWith("--")) acc.push([a.slice(2), all[i + 1] && !all[i + 1].startsWith("--") ? all[i + 1] : "true"]);
    return acc;
  }, []),
);
const wasmPath = args.wasm ?? resolve(root, "web/wasm/anabios.wasm");
const scenarioPath = args.scenario ?? resolve(root, "scenarios/predator-prey.toml");
const ticks = Number(args.ticks ?? 300);
const seed = args.seed;
const expectFp = args["expect-fingerprint"];

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

const sim = await AnabiosSim.fromBytes(readFileSync(wasmPath));
check(sim.catalog.events.length >= 63, "catalog lists the codex event types");
check(sim.catalog.events[38] === "war", `event 38 is 'war' (got ${sim.catalog.events[38]})`);

// A bad scenario must fail loudly, not trap.
let threw = false;
try { sim.load("name = ", 1); } catch (e) { threw = /parse failed/.test(e.message); }
check(threw, "malformed TOML reports a parse error through the error channel");

sim.load(readFileSync(scenarioPath, "utf8"), seed);
const res = sim.biomeRes(), ws = sim.worldSize();
check(res > 0 && ws > 0, "world has dimensions");
check(sim.elevation().length === res * res, "elevation buffer is res²");
check(sim.biomeRgba().length === res * res * 4, "biome RGBA buffer is res²×4");
check(sim.terrain().length === res * res, "terrain buffer is res²");
const sea = sim.seaLevel();
check(sea >= 0 && sea <= 1, `sea level in [0,1] (got ${sea})`);

const t0 = performance.now();
const batch = 50;
let fired = 0;
for (let done = 0; done < ticks; done += batch) {
  sim.step(Math.min(batch, ticks - done));
  const ev = sim.events();
  for (let k = 0; k < ev.count; k++) {
    const row = ev.data.subarray(k * sim.stride.event, (k + 1) * sim.stride.event);
    check(row[EVENT.TYPE] >= 0 && row[EVENT.TYPE] < sim.catalog.events.length, "event type in range");
    check(row[EVENT.TICK] <= sim.tick(), "event tick not in the future");
  }
  fired += ev.count;
}
const dt = (performance.now() - t0) / 1000;

const { count, data } = sim.agents();
check(count === sim.alive(), "agent buffer count equals alive()");
for (let k = 0; k < count; k++) {
  const r = data.subarray(k * sim.stride.agent, (k + 1) * sim.stride.agent);
  check(r[AGENT.X] >= 0 && r[AGENT.X] <= ws && r[AGENT.Y] >= 0 && r[AGENT.Y] <= ws, `agent ${r[AGENT.ID]} inside the torus`);
  check(r[AGENT.DIET] >= 0 && r[AGENT.DIET] <= 1, "diet in [0,1]");
}
if (count > 0) {
  const detail = sim.agent(data[AGENT.ID]);
  check(detail && detail.genome.length === 50, "agent inspector returns a 50-slot genome");
}
check(sim.agent(0xfffffff0) === null, "dead/out-of-range agent inspector is null");
const species = sim.species();
check(species.species.reduce((s, r) => s + r.count, 0) === count, "species table sums to the alive count");
sim.streaks(); sim.trades(); sim.sites(); sim.hubs();

const meta = sim.meta();
check(meta.tick === ticks, `ran ${ticks} ticks (meta says ${meta.tick})`);
console.log(
  `scenario=${meta.scenario} seed=${meta.seed} ticks=${meta.tick} alive=${meta.alive} ` +
  `species=${species.species.length} events=${fired} fingerprint=${meta.fingerprint} state_hash=${meta.state_hash}`,
);
console.log(`wasm tick rate: ${(ticks / dt).toFixed(0)} ticks/s (${(dt * 1000 / ticks).toFixed(2)} ms/tick, ${count} agents at the end)`);
if (expectFp) {
  check(meta.fingerprint === expectFp, `fingerprint ${meta.fingerprint} != native ${expectFp}`);
  console.log("determinism: wasm fingerprint matches the native run");
}
sim.free();
console.log("wasm smoke test: OK");
