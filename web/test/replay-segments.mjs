#!/usr/bin/env node
// Unit test for replay playback (sources.js ReplaySource): each sample
// frame's streaks and trades are handed out once, not once per rendered
// frame (the trail layer keeps what it is given, so repeats stacked and, while
// paused, piled up without end); a seek back onto a frame shows its volley
// again; a seek at or past the recording's length lands at its end instead of
// looping to tick 0; and a step across the natural loop never counts negative
// ticks (the TICKS/S readout).
//
//   node web/test/replay-segments.mjs

import { ReplaySource } from "../src/sources.js";

function check(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

// A synthetic recording in the `anabios-headless record` layout: 100 ticks,
// a sample every 24, two agents that walk east, one streak and one trade row
// ([x1,y1,x2,y2]) per sample.
const SAMPLE = 24, TICKS = 100;
const frames = [];
for (let t = 0; t < TICKS; t += SAMPLE) {
  const x = 10 + t;
  frames.push({
    t, id: [1, 2], x: [x, x + 5], y: [20, 30], sp: [0, 1], d: [0, 128],
    st: [x, 20, x + 5, 30], tr: [x, 30, x + 5, 20],
  });
}
const R = { meta: { scenario: "synthetic", seed: 1, ticks: TICKS, world_size: 256, sample: SAMPLE, stride: 1 }, frames, events: [] };
const last = frames[frames.length - 1].t;

const src = new ReplaySource(R);
// One rendered frame of the page's loop: agents() first (it picks the frame).
const frame = () => { src.agents(); return { st: src.streaks().count, tr: src.trades().count }; };

src.seek(SAMPLE + 2);
let f = frame();
check(f.st === 1 && f.tr === 1, `the first frame on a sample hands out its volley (got ${f.st} streaks, ${f.tr} trades)`);
f = frame();
check(f.st === 0 && f.tr === 0, `the same tick again (paused) hands out nothing (got ${f.st}, ${f.tr})`);
src.advance(3);
f = frame();
check(f.st === 0 && f.tr === 0, `later ticks of the same sample hand out nothing (got ${f.st}, ${f.tr})`);
src.advance(SAMPLE - 5);   // tick 2·SAMPLE: the next sample
f = frame();
check(f.st === 1 && f.tr === 1, `the next sample hands out its own volley once (got ${f.st}, ${f.tr})`);
check(frame().st === 0, "…and only once");

src.seek(SAMPLE);
f = frame();
check(f.st === 1 && f.tr === 1, `a seek back onto a sample shows its volley again (got ${f.st}, ${f.tr})`);

for (const past of [TICKS, TICKS + 900]) {
  src.seek(past);
  check(src.tick >= last && src.tick < TICKS, `seek(${past}) lands at the end of the recording (got ${src.tick})`);
  src.advance(0.5);
  check(src.tick >= last, `seek(${past}) then advance(0.5) stays at the end, not tick 0 (got ${src.tick})`);
}

src.seek(TICKS - 3);
const n = src.advance(5);   // across the loop
check(n >= 0, `a step across the loop counts no negative ticks (got ${n})`);
check(src.tick < SAMPLE, `the replay still loops back to its start (got tick ${src.tick})`);
f = frame();
check(f.st === 1, `after the loop the first sample's volley shows again (got ${f.st})`);

console.log("replay-segments: ok");
