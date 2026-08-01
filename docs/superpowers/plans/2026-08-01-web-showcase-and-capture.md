# Web Showcase Hosting, Decks & One-Command Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the web replay player as a hosted, shareable Out-of-Africa experience; add curated Godot cinematic decks; and make every showcase asset (web `replay.js`, cinematic MP4, screenshots) regenerable with one command.

**Architecture:** Two independent showcase systems (do not conflate): (1) the **web player** — static `showcase/index.html` + a recorded `showcase/replay.js` data dump from `anabios-headless record`, eras auto-derived from the event stream; (2) the **Godot cinematic** — `showcase_director.gd` plays a `game/showcase/*.json` beat-list over the live sim, captured to MP4 via `emergence.sh record`. This plan hosts (1), adds decks to (2), and adds wrapper subcommands so both regenerate from source deterministically.

**Tech Stack:** Static HTML/JS (`showcase/`), Rust (`anabios-headless/src/record.rs`), GDScript (`game/scripts/showcase_director.gd`), `bash` (`scripts/emergence.sh`), GitHub Actions, Godot Movie Maker + ffmpeg.

## Global Constraints

- The web player must stay **fully static and `file://`-openable** — `replay.js` is loaded via `<script>` (no fetch). Do not introduce a build step or a server dependency.
- `record` is a **determinism receipt**: it only reads columns + drains events, so the final `state_hash` equals a plain `run` (`record.rs:1-14`; test `capture_is_side_effect_free` `record.rs:472-489`). Keep it read-only.
- Deck event-trigger names are **CamelCase** matched against `codex_panel.gd:4-58` (Godot side); the web player's era regexes use **snake_case** names from `score.rs::event_name`. Never cross the two naming schemes.
- Publishing the web player sends content to a public host — the hosting step (Task 2) must be gated on explicit maintainer action (a tagged release or manual workflow dispatch), not run on every push.

## Background (grounded)

- Web player: `showcase/index.html:228` `<script src="replay.js">`; reads `window.ANABIOS_REPLAY` at `:232`; eras derived `:274-305`; `?era=N` deep-link `:853-862`; `prefers-reduced-motion` freeze `:237`.
- Recorder: `anabios-headless record --scenario --seed [--sample 24 --max-agents 260 --max-events 1000 --biome-res 96 --biome-frames 6] --out showcase/replay.js` (`main.rs:110-141`); `Replay` schema `record.rs:116-126`; `.js` vs `.json` wrap `record.rs:231-233`.
- Cinematic: `ANABIOS_SHOWCASE=<deck path>` adds `showcase_director.gd` (`main.gd:204-207`); beat schema `showcase_director.gd:7-33`; actions `:189-299`; decks `game/showcase/*.json` matched by basename.
- Wrapper: `emergence.sh record` (`:122-170`) = Godot MP4 pipeline (needs window + ffmpeg). **No wrapper generates `replay.js`** (gap). Screenshots via `ANABIOS_SHOT` env on a windowed boot (`debug_capture.gd`).
- CI (`.github/workflows/ci.yml`): no showcase record/deploy step exists.

## File Structure

- `scripts/emergence.sh` — **Modify**: add `record-web` (generates `replay.js`) and `capture` (screenshot) wrapper subcommands.
- `game/showcase/*.json` — **Create**: 2–3 new decks (e.g. `predator-prey.json`, `dialects.json`, `inventions.json`).
- `.github/workflows/showcase.yml` — **Create**: manual/tag-triggered job that regenerates `replay.js` and deploys `showcase/` to GitHub Pages.
- `showcase/README.md` — **Modify**: hosting + regeneration instructions.
- `crates/anabios-headless/tests/record_schema.rs` — **Create**: guard the `replay.js` schema the player depends on.

---

## Task 1: `record-web` + `capture` wrapper subcommands

**Files:** Modify `scripts/emergence.sh`.

**Interfaces:** Produces `emergence.sh record-web <scenario> [--seed N --out PATH]` → writes `showcase/replay.js`; `emergence.sh capture <scenario> [--out PNG --ticks N]` → windowed screenshot via `ANABIOS_SHOT`.

- [ ] **Step 1: Add `record-web`** near the existing `record` case (`emergence.sh:122`):

```bash
record-web)
  scn="$(resolve "${1:-}")"; shift || true; build
  out="${ANABIOS_WEB_OUT:-$ROOT/showcase/replay.js}"
  "$BIN" record --scenario "$scn" --out "$out" "$@"
  echo "web replay: $out  (open showcase/index.html)"
  ;;
```

- [ ] **Step 2: Add `capture`** (windowed Godot screenshot) — resolve scenario, `build` the godot crate, launch Godot on `main.tscn` with `ANABIOS_SCENARIO`/`ANABIOS_SEED`/`ANABIOS_SHOT="$out"` env (mirror the `view` case at `:84`, but set `ANABIOS_SHOT`). Note in the help that it needs a real window (not `--rendering-driver dummy`).
- [ ] **Step 3: Update the usage/help block** with both new subcommands.
- [ ] **Step 4: Smoke.** `scripts/emergence.sh record-web out-of-africa-saga --seed 318` → regenerates `showcase/replay.js`; open `showcase/index.html` and confirm it plays.
- [ ] **Step 5: Commit.** `git commit -m "feat(scripts): emergence.sh record-web + capture wrappers"`

---

## Task 2: Host the web player (GitHub Pages, gated)

**Files:** Create `.github/workflows/showcase.yml`; Modify `showcase/README.md`.

**Interfaces:** Produces a Pages deployment of `showcase/` on `workflow_dispatch` and on `v*` tags — never on ordinary pushes.

- [ ] **Step 1: Write the workflow.** `on: { workflow_dispatch: {}, push: { tags: ['v*'] } }`. Steps: checkout; build release `anabios-headless`; `./target/release/anabios-headless record --scenario scenarios/out-of-africa-saga.toml --seed 318 --out showcase/replay.js`; upload `showcase/` as a Pages artifact; deploy via `actions/deploy-pages`. This regenerates the data at publish time so the hosted deck is reproducible from source (not the committed 1.9 MB `replay.js`).
- [ ] **Step 2: Verify determinism receipt.** The workflow logs the recorded `state_hash` (from `Meta`); document that it must match a local `record` of the same scenario+seed.
- [ ] **Step 3: Update `showcase/README.md`** — the hosted URL, the "publish" trigger (tag or manual dispatch), and how to preview locally (`emergence.sh record-web … && open showcase/index.html`).
- [ ] **Step 4: Manual test** — trigger `workflow_dispatch`, confirm the Pages URL plays the saga on desktop and mobile widths (the player is responsive; `prefers-reduced-motion` freezes to a still).
- [ ] **Step 5: Commit.** `git commit -m "ci(showcase): gated GitHub Pages deploy of the web replay player"`

---

## Task 3: Guard the `replay.js` schema

The player silently mis-renders if the recorder's schema drifts. Lock the fields the player reads.

**Files:** Create `crates/anabios-headless/tests/record_schema.rs`.

**Interfaces:** Consumes the `record` binary; produces a `.json` recording parsed and asserted.

- [ ] **Step 1: Write the test** — run `record --scenario scenarios/predator-prey.toml --ticks 800 --out <tmp>.json`, parse the JSON, assert the top-level keys the player uses exist: `meta.{world_size,state_hash,frame_count}`, `species`, `biome.{res,grids}`, `frames[].{t,id,x,y,sp,d}`, `sites[].{t,sid,x,y,n}`, `events[].{t,type,x,y}`; and that `events[].type` values are snake_case (match `^[a-z_]+$`).
- [ ] **Step 2: Run — expect PASS** (characterization/regression test).
- [ ] **Step 3: Commit.** `git commit -m "test(headless): guard the replay.js schema the web player depends on"`

---

## Task 4: Curated cinematic decks

**Files:** Create `game/showcase/predator-prey.json`, `game/showcase/dialects.json`, `game/showcase/inventions.json`.

**Interfaces:** Each deck follows the beat schema (`showcase_director.gd:7-33`), matched to a same-named scenario in `scenarios/` so `emergence.sh record <name>` finds it by basename.

- [ ] **Step 1: Author `predator-prey.json`** — a collapse-and-recovery arc: open (letterbox, hud off, speed 16, wide camera, title); `at_event: "Predation"` → cut + highlight + lower-third; `at_event: "PopulationCrash"` → punch zoom + caption; `after` beats for recovery; `{"end": true}`. Use only trigger names from `codex_panel.gd:4-58`.
- [ ] **Step 2: Author `dialects.json`** — a meme-sweep arc keying on `DialectFormed`/`MemeSweep`, switching `{"body":"dialect"}` to show dialect hues, camera following a sweeping cluster.
- [ ] **Step 3: Author `inventions.json`** — the innovators-vs-traditionalists race keying on `InventionDiscovered`/`InventionAdopted`, toggling the tech panel (`{"panel":{"name":"tech","visible":true}}`).
- [ ] **Step 4: Reproducibility check** — each deck must render from a pinned scenario+seed. Document the seed per deck in a leading comment field (the schema tolerates extra keys) or in `showcase/README.md`.
- [ ] **Step 5: Smoke each** (needs a window + ffmpeg): `scripts/emergence.sh record predator-prey --seed <s> --max-seconds 60` produces an MP4 with no stalls (every event beat has a `timeout` so it can't hang).
- [ ] **Step 6: Commit.** `git commit -m "feat(showcase): predator-prey / dialects / inventions cinematic decks"`

---

## Task 5: Fold the tier-2/3 effects into the flagship deck

The saga deck predates the embers/firelight/climate-grade/footstep effects; use them deliberately.

**Files:** Modify `game/showcase/out-of-africa-saga.json`.

- [ ] **Step 1: Add ground/body/overlay beats** that showcase the new effects at the right chapters — e.g. `{"ground":"markets"}` during "The Word", camera `punch` on `Market`/`War` events (which now throw embers/trauma), a close `follow` during "The Herd" so footstep trails read. Keep every event trigger with a `timeout`.
- [ ] **Step 2: Re-record and eyeball.** `scripts/emergence.sh record out-of-africa-saga --seed 318 --max-seconds 120` — confirm the cinematic uses the effects without manual camera nudging.
- [ ] **Step 3: Commit.** `git commit -m "feat(showcase): use tier-2/3 effects in the out-of-africa saga deck"`

---

## Testing Plan (summary)

| Level | What | Where |
|---|---|---|
| Integration | `replay.js` schema matches player expectations | `tests/record_schema.rs` (Task 3) |
| Determinism | `record` is side-effect free (state_hash == run) | existing `record.rs:472-489` |
| Smoke | `record-web` regenerates a playable `replay.js` | manual (Task 1) |
| Smoke | each deck records without stalling | manual, `emergence.sh record` (Task 4/5) |
| CI | headless scene load still green | existing `godot` job |
| Manual | hosted URL plays on desktop + mobile, reduced-motion still | Task 2 |

**Done when:** a stranger can open a hosted URL and watch the Out-of-Africa saga; `emergence.sh record-web <scn>` and `emergence.sh record <scn>` regenerate the web and MP4 assets from source; 2–3 new decks exist and record cleanly; and the `replay.js` schema is guarded.

## Risks / open questions

- **Committed `replay.js` (1.9 MB):** decide whether to keep committing it or generate at publish time (Task 2 does the latter). If kept, add a CI check that it matches a fresh `record` of the pinned scenario+seed, or it will silently rot.
- **Godot Movie Maker needs a real window** — the deck-record smoke steps can't run in headless CI; they're manual/local. CI only guards scene-load + the web schema.
- **Q4 fork:** whether the web player grows toward live in-browser simulation (WASM core) — out of scope here; flagged in `ROADMAP.md`.
