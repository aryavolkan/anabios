# O2 — Payoff-Biased Social Learning — Design Spec

**Date:** 2026-08-03
**Status:** Approved (framing + mechanism + cross-cutting approved in brainstorming)
**Arc:** Phase II of the Open-Ended Complexity Arc (O1–O8),
`docs/superpowers/specs/2026-08-02-open-ended-complexity-arc-design.md`.
**Depends on:** O1 (merged, PR #106) — the `autopsy` instrument, the
`practices_enabled` flag, and the finding this milestone is built on.

---

## 1. Goal & thesis

O1 established, with a powered n=10 decomposition, **why culture loses**: its
transmission is **payoff-blind**. Communicator (culture-capable) agents copy
maladaptive practices (Inbreeding, Child Sacrifice) by the same
copy-toward-best-trait-level rule they use for good tech, so they import a
reproductive-fitness burden asocial foragers can't incur. Disabling practice
discovery flips the cultural mutant from excluded (invade-fraction 40%) to
invading (90%) — but that *deletes* the antagonist.

**O2's thesis:** make transmission **payoff-biased** so culture keeps the good
and declines the bad *while practices still exist in the world* — culture learns
to avoid them rather than the world removing them. This is the honest, general
form of the O1 result, it is a genuine lifetime-learning mechanism (Phase II's
remit), and it is uniquely well-instrumented: O1 handed us the exact **floor**
(payoff-blind baseline ~40%) and **ceiling** (practices-off ~90%) that O2 must
close.

The intuitive alternative — cutting culture's *cost* (the IQ tax) — was falsified
in O1's probes (removing the IQ metabolic cost lifts invade-fraction only
40%→50%). O2 does **not** pursue it.

## 2. Staging

Two sub-milestones, per the approved plan:

- **O2a — mechanism.** An opt-in flag makes transmission payoff-biased; prove it
  lets culture invade (invade-fraction rises toward the 90% ceiling).
- **O2b — evolution.** A heritable gene for the *degree* of payoff bias; test
  whether it climbs from standing variation **and** rescues culture.

Preceded by **Step 0** (two small instrument fixes O1 flagged; §3), which gate
O2b's honesty and are cheap, so they land first.

## 3. Step 0 — instrument prerequisites (from O1's handoff)

Both were surfaced and scoped in the O1 findings doc (§7 there). They land
before the mechanism because O2b cannot be measured honestly without them.

1. **Share-relative `invasion_fitness`.** The committed metric is absolute-count
   log-growth, confounded by non-stationary population. O2b's rescue runs are
   non-stationary by construction (culture rescued → population grows), so the
   evolution claim is unmeasurable without a frequency/share-relative variant.
   Add it as a first-class output of `autopsy` alongside (not replacing) the
   absolute metric; ship the share-retention computation the O1 milestone did by
   hand (`invasion-share-analysis.csv`) as code.
2. **Lineage-locked strategy tag.** The current cultural/asocial tag is a
   per-tick readout of Communicator-module presence, which recombines and
   mutates every birth — O1 showed this confounds the "who is cultural"
   direction. O2b asks whether a *gene* sweeps *within the cultural lineage*,
   which needs a stable founder-population id assigned at scenario start and
   inherited unchanged (separate from the mutable module phenotype).

Both are headless-side / read-only where possible; neither changes sim behavior
by itself (no golden movement for Step 0).

## 4. The mechanism (shared by O2a and O2b)

**Where it hooks.** The neighbor-meme scan in `crates/anabios-core/src/culture.rs`
(`scan_neighbor_memes` and the `best_skill_neighbor` / `best_tech` /
`max_neighbour_inv` / `max_neighbour_practice` copy-target logic). Today that
selects the copy source by **trait level** and copies payoff-blind. O2 brings
each Communicator neighbour's **energy** (the fitness proxy) into that scan and
adds two filters, both emergent from energy, both distinct:

1. **Model bias — copy the successful.** Change the transmission source from
   "highest-trait-level neighbour" to the **highest-energy Communicator
   neighbour**. Practice-holders carry a reproductive cost, so the fittest models
   tend to carry fewer practices — this alone biases copying toward good tech.
2. **Content bias — decline the locally-harmful.** A per-channel acceptance test,
   independent of the chosen source: decline a candidate trait if, across the
   neighbourhood, its **holders have lower mean energy than non-holders** (the
   trait is empirically maladaptive here). This catches a practice even when the
   fittest model happens to hold one — Boyd–Richerson *indirect bias*, and it is
   emergent (no oracle read of the coded debuff).

**Fitness proxy = current energy.** Smallest viable, available per-tick, cheap to
read in the existing scan. If O2a's bar isn't met because energy is too noisy a
signal, the fallback (flagged, not built first) is lifetime reproductive success.

**Read-only over sim state.** The filters read energy and reweight which meme a
receiver moves toward; they do not write agent state beyond the meme update the
transmission stage already performs. The tick-pipeline insertion point is a
read-only extension of the existing fused meme scan — named explicitly in the
plan, with a confirmation that RNG draw order is unchanged when the flag is off.

## 5. O2a — the mechanism, as an opt-in flag

- **Flag:** `payoff_biased_learning` on Scenario + World, default **false** (a new
  opt-in feature — off = today's payoff-blind transmission, byte-identical).
  Serialized field → `FORMAT_VERSION` bump. **Check main's current
  `FORMAT_VERSION` immediately before bumping** (it is 25 as of this writing; the
  O1/#106 merge collided because main moved). Rehash determinism/cognition/
  inventions goldens (pure schema growth — behavior unchanged with the flag off).
- **Detector:** a new codex event (working name `SelectiveLearning`) that fires
  when payoff-biased rejection is *actually happening* — e.g. a species sustains
  a rate of content-bias declines of maladaptive traits above a threshold. Ships
  with the E-arc evidence trio (one handcrafted world firing it once, ≥1 sweep
  seed, a gallery/CSV capture) and a **negative** handcrafted test (a
  payoff-blind world that must *not* fire it). Extends `EventType` +
  `EVENT_TYPE_COUNT` + the viewer's parallel name/color arrays per the boot-assert
  convention.
- **Done-when:** with the flag on and practices still in the world, the cultural
  mutant's **invade-fraction rises materially toward the practices-off ceiling
  (~90%)**, at n≥10, measured on `autopsy` against the two O1 reference points
  (payoff-blind baseline ~40%, practices-off ~90%) using the share-relative
  metric from Step 0. A control assertion confirms **invention adoption is not
  suppressed** (payoff bias must reject practices without rejecting good tech).

## 6. O2b — heritable payoff bias, and whether it evolves

- **Gene:** a heritable "payoff-bias degree" in `[0,1]` in a reserved genome slot
  in the culture/drive cluster (candidate: the reserved Communicator-effectiveness
  slot or a `_DriveReserved` slot — the plan pins the least-conflicting index).
  `0` = payoff-blind (today's rule); `1` = both filters fully applied; intermediate
  values scale the filters' strength (e.g. the probability the source is chosen by
  energy vs trait level, and the strength of the content-bias decline). Inert at
  its neutral default and with `payoff_biased_learning` off, so baseline scenarios
  stay byte-identical.
- **Speciation & RNG:** like the personality genes, the payoff-bias slot is
  **excluded from the speciation-distance metric** (else it perturbs clustering),
  and any RNG it consumes draws from a **dedicated substream**.
- **Done-when (honest either-way).** The gene-culture history is explicit that
  first-principles genes often do *not* sweep from standing variation. Acceptable
  outcomes:
  - **(a) Positive:** the payoff-bias degree climbs from standing variation
    **and** rescues culture, reported with **bidirectional invasion analysis** —
    a payoff-biased cultural mutant invades an asocial world, and a payoff-blind
    cultural variant cannot re-invade the resulting payoff-biased world — using
    the lineage-locked tag from Step 0.
  - **(b) Negative:** a documented "the gene does not sweep from standing
    variation" with the measured margin and the confound controls, as a valid
    deliverable.
  A hand-tuned "the gene wins" is **not** acceptable; the invasion test is the
  guard, exactly as in O1/O3.

## 7. Cross-cutting invariants

1. **Opt-in / baseline-stable.** O2a ships off-by-default; O2b's gene is inert at
   neutral default and with the flag off. Existing scenarios and goldens unchanged
   except the deliberate `FORMAT_VERSION` schema rehash.
2. **Determinism.** Deterministic by default (the filters are read-only over
   energy, seeded where they draw). Behavior-altering work regenerates goldens
   with `UPDATE_HASHES` in the same PR; **check main's `FORMAT_VERSION` before
   bumping** and re-version past it (the #106 lesson). New persistent state (the
   flag; the gene rides the existing genome) gets a save→load→step round-trip test
   — the `#[serde(skip)]`-feeds-hashed-state footgun applies.
3. **Perf budget.** ≤10% tick-time regression at 10k agents; the new reads join
   the existing per-neighbour meme scan (no new per-agent loop).
4. **Reconcile with `critical_learner`.** The DIT `critical_learner` archetype
   already exists but is a **genome-propensity label only** — it maps to
   `(communicator_kit(), starter_asocial_forager())` with an env-mode genome
   default (`scenario.rs`), not a transmission-rule change. O2 supplies the actual
   payoff-biased transmission *rule* it was always meant to name: **repurpose the
   archetype for O2's scenarios, do not duplicate it**; align naming.
5. **Tagged & documented.** `o2a`/`o2b` tags; each phase gets its own spec+plan
   pair; this doc is the milestone design.

## 8. Evidence discipline (carried from O1/gene-culture)

- **The reference ceiling is built in.** O2a either closes the 40%→90% gap or it
  doesn't — the floor and ceiling are already measured and committed.
- **Invasion analysis, both directions**, for any "culture now pays / the gene
  swept" claim (§6). Growth curves are not proof.
- **Seeded vs first-principles stated plainly**, with the margin.
- **Negative controls:** the `SelectiveLearning` detector ships a scripted
  near-miss that must not fire; the O2a bar includes the "invention adoption not
  suppressed" control.
- **Scorecard/share-metric driven**, n≥10, pre-registered before looking.

## 9. Risks & mitigations

- **Over-filtering — payoff bias rejects good tech too.** If energy is noisy,
  content bias could decline beneficial inventions whose holders are momentarily
  low-energy (e.g. just paid a learning cost). Mitigation: content bias tests
  *relative* holder-vs-non-holder energy over the neighbourhood, not absolute;
  the O2a bar includes an explicit "invention adoption not suppressed" control;
  the O2b gene lets selection tune bias strength rather than hard-coding it.
- **Energy is a weak fitness proxy.** Mitigation: ship energy first (smallest
  viable); flag reproductive-success as the fallback if the O2a bar isn't met.
- **The gene doesn't sweep (O2b).** Expected-possible per history; §6(b) makes a
  documented negative a valid deliverable, not a failure.
- **Determinism erosion from new scan reads.** Mitigation: read-only over energy
  inside the existing fused scan; plan names the exact insertion point and
  confirms no RNG-order change with the flag off.
- **FORMAT_VERSION collision (the #106 failure mode).** Mitigation: re-check
  main's `FORMAT_VERSION` immediately before bumping and re-version past it;
  regenerate goldens on the merged tree.

## 10. Out of scope

- Neural cognition — O2 stays a hand-engineered transmission rule.
- Making the OoA grand run climb — that is the end-to-end test, O3's remit.
- New agent-level objects; a reproductive-success fitness proxy (fallback only).

## 11. Open questions (resolve as phases close)

1. Is energy a strong enough fitness proxy for the O2a bar, or is
   reproductive-success needed?
2. Does model bias alone nearly close the gap, making content bias marginal — or
   are both genuinely required? (Decompose within O2a, per the O1 method.)
3. Does the payoff-bias gene sweep from standing variation (O2b positive), or is
   it another first-principles gene that needs seeding (O2b negative)?
4. Which reserved genome slot for the O2b gene (least serde/speciation conflict)?

---

_Living document — revise as O2a/O2b close. This is the milestone design; each
phase gets its own spec+plan pair._
