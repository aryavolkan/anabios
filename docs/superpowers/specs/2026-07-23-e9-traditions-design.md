# E9 — Traditions & Institutions — Design Spec

**Date:** 2026-07-23
**Status:** Approved
**Milestone:** E9 of the emergence roadmap (`2026-07-22-emergence-roadmap-design.md`)
**Crate:** `anabios-core` (meme lineage + detectors). `FORMAT_VERSION` 14→15; goldens regenerated once. Meme lineage tracking is observability + a fidelity effect inside settlements; baseline scenarios unchanged. Meme-lineage tree viewer deferred to E10's codex screen (noted).

## 1. Goal & success criteria

Culture that *outlives* its carriers (design §4.4) — the ratchet beyond individual memory. Three new event types: `TraditionPreserved` (45), `CulturalRadiation` (46), `InstitutionalRatchet` (47).

Success criteria:

1. Every meme-channel value an agent carries can be traced to a variant with a parent — descent across generations is queryable.
2. Each detector has positive + negative handcrafted tests; `traditions.toml` (gene-culture + settlement + inventions, long run) fires ≥2 of 3 across a 16-seed sweep.
3. Institutional memory is real: transmission fidelity is measurably higher inside settled species.

## 2. Meme variants (lineage tracking)

- A **variant** = `(id, channel, band, parent: Option<id>, born_tick, born_species)`; `band = floor(value × 10)` (11 bands over [0,1]). Registry on `CodexState` (`meme_variants: BTreeMap<u32, MemeVariant>`, `next_variant_id`).
- `AgentBuffers.meme_lineage: Vec<[u32; MEME_CHANNELS]>` — the variant id each agent carries per channel (0 = untracked). Serialized.
- **Birth:** at reproduction (communicator children), per channel: child's band == parent's band → inherit parent's variant; differs → NEW variant with `parent = parent's variant`. Untracked parents spawn root variants when a channel first reaches band ≥ 1.
- **Transmission:** in `culture_step`, when an agent's channel value moves into a new band via social learning, the agent adopts the best-neighbor's variant if its band matches the new band, else a new variant parented by the neighbor's variant. (Approximation of descent through social spread — documented.)

## 3. Institutional memory (fidelity)

In `culture_step`, when BOTH parties' species are settlement-latched (`CodexState.settlement_active` from E8), the transmission drift is scaled ×`SETTLED_FIDELITY = 0.25` — culture anchored to place mutates more slowly.

## 4. Detectors (`codex/traditions.rs`)

- **`TraditionPreserved`** — a variant held by ≥30% of a species' members continuously for `TRADITION_WINDOW = 2000` ticks, and the variant is at least that old (the carriers turned over while the custom persisted). `value` = variant id; loc = species centroid.
- **`CulturalRadiation`** — a variant whose descendant tree (parent links) reaches ≥ `RADIATION_MIN_DESCENDANTS = 4` distinct variants across ≥ 2 species. One-shot per ancestor. `value` = descendant count.
- **`InstitutionalRatchet`** — an inventions-enabled species holds era ≥ 2 (highest adopted tier) continuously for `RATCHET_WINDOW = 2000` ticks — the institution never regresses despite holder turnover. `value` = era.

## 5. Wiring & scenario

- EventTypes 45–47; `score.rs` (48 names, +3 bonus); `sweep.rs` CSV +3; `codex_panel.gd` +3 (Tradition / Radiation / Ratchet).
- `scenarios/traditions.toml`: communicators + gene-culture skill channel + `inventions_enabled` + `settlement_enabled`, designed for a 20k-tick run. Menu entry.

## 6. Testing & evidence

- Unit: variant birth on band jump, inheritance preserves same-band lineage, radiation counts descendants across species, tradition needs both age and adoption streak, ratchet needs sustained era, settled-fidelity scaling.
- Integration: traditions.toml long run fires ≥1 type; replay one.
- Sweep: 16 seeds × 20000 ticks (or 12000 if wall-clock bound); counts in completion notes.
- Gallery: codex tally with Tradition/Ratchet live on the long-run world.

## 7. Completion notes (2026-07-23)

**Status: complete.** Re-homed onto `e9-traditions` off `main` (the WIP was
authored against the pre-merge `e8` tip); reconciled `FORMAT_VERSION` 15→16 and
regenerated the three golden tables. Merged constant tuning during
implementation: `RADIATION_MIN_DESCENDANTS = 50`, `TRADITION_ADOPTION_SHARE =
0.5`, `TRADITION_MIN_AGE = 4000` (the tests track the code, not the earlier
draft numbers in §2–4).

**Success criteria.**
1. ✅ Descent is queryable — `CodexState.meme_variants` (id → parent/root) +
   `AgentBuffers.meme_lineage`, both serialized.
2. ✅ Each detector has positive + negative unit tests (10 in
   `codex::traditions`, incl. tradition age/adoption/minority and the
   settled-fidelity scaling). Sweep fires **≥2 of 3** — see below.
3. ✅ Institutional memory is real — `settled_fidelity_shrinks_inheritance_jitter`
   proves settled-species transmission jitter is exactly `SETTLED_FIDELITY` (¼)
   of baseline off the same RNG draw.

**Sweep (release, `traditions.toml`).**
- 16 seeds × 12 000 ticks: `cultural_radiation` 359 (16/16 runs),
  `institutional_ratchet` 32 (16/16), `tradition_preserved` 0.
- 4 seeds × 20 000 ticks: radiation 99 (4/4), ratchet 8 (4/4), tradition 0.

`TraditionPreserved` does not fire in the showcase even at 20 k ticks. Cause is
a real design tension, not a detector bug (unit-tested correct): a *successful*
culture radiates **across** species (that is the `CulturalRadiation` event),
but `detect_tradition` requires a lineage root held by ≥50 % of **one**
species for 2 000 continuous ticks — speciation fragments the within-species
share below the bar before the streak matures. Firing it in-sim would need
either a slower-speciation scenario or crediting the tradition at the
lineage-**faction** level (kin root) rather than per species; deferred as a
tuning follow-up. Criterion 2 (≥2 of 3) is met regardless.
