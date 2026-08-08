# O2b findings: payoff-biased social learning — measured negative (energy proxy)

**Date:** 2026-08-07
**Milestone:** O2b (detailed roadmap §4.1 item 2.3). Design:
[`2026-08-03-o2-payoff-biased-learning-design.md`](2026-08-03-o2-payoff-biased-learning-design.md)
(its "O2a mechanism" phase). Measurement protocol and baseline numbers:
[`2026-08-07-o2a-corrected-decomposition.md`](2026-08-07-o2a-corrected-decomposition.md).
**Pre-registered bar (from the design, corrected per O2a):** with the flag on
and practices still in the world, the cultural **founder lineage** moves
measurably up from the payoff-blind baseline toward the re-derived
practices-off reference — at minimum, materially fewer lineage extinctions —
measured with `--tag founder` + `invasion_fitness_share`, n=10, with an
"invention/skill adoption not suppressed" control.

## Result: negative at the measured horizon

`scenarios/experiments/o2-payoff-biased-learning.toml` = the O1 cul→aso
invasion scenario + `payoff_biased_learning = true` (one variable apart).
n=10 seeds, 2500 ticks, window 500, founder tag. Data:
`docs/superpowers/data/o2/o2a/o2b-payoff-biased-{1..10}.{csv,log}`.

| condition | invade-fraction | mean retention f2000/f500 | mean share-r | extinct by t2000 |
|---|---|---|---|---|
| baseline (payoff-blind; O2a) | 0/10 | 0.079 | -0.778 | 3/10 |
| **payoff-biased (this milestone)** | **0/10** | **0.094** | **-0.770** | **3/10** |
| practices OFF reference (O2a) | 0/10 | 0.244 | -0.583 | 2/10 |

Payoff-biased transmission is statistically indistinguishable from the
payoff-blind baseline on every readout, and clearly short of even the
practices-off reference. The bar is **not met**: no seed gains share, no
extinction is prevented, mean share-fitness moves +0.008.

**Control (skill adoption):** suppressed, not preserved. Surviving cultural
founders under the flag reach mean_skill 0.08–0.28 at t2000 where the
baseline's survivors reach 0.13–0.78 (seeds 2/6/8/9). Model bias copies from
the highest-energy neighbour, whose skill is typically *not* the highest —
the receiver trades skill acquisition for model fitness. So the mechanism as
built is not even neutral on good-trait transmission; it is mildly costly.

## Mechanism of the negative (why it failed)

The design's own risk §9 called this: **"energy is a weak fitness proxy" —
confirmed, and it is the whole story.** The maladaptive practices exact
their cost *reproductively* — Inbreeding stillbirths and Child-Sacrifice
culls at birth (`practice.rs`: `INBREEDING_STILLBIRTH`, `CHILD_SACRIFICE_CULL`)
— not through the holder's current energy. A practice holder's energy is
unharmed at the moment a learner's neighbourhood scan reads it, so:

1. **Content bias has no signal:** holder-vs-non-holder *energy* means do not
   separate practice-holders from non-holders, because the harm lands on the
   holder's *offspring count*, not its energy pool.
2. **Model bias aims at the wrong target:** the highest-energy neighbour is
   not the highest-*fitness* neighbour; it is often just a lucky forager, and
   copying it dilutes skill (the control regression above) without avoiding
   practices.

There is a second, structural limitation: the scan only sees *Communicator*
neighbours, so in the invasion setting (20 cultural mutants among ~1000
asocial residents) the evidence pool for the content-bias test is a handful
of agents even when an energy signal would exist.

## What ships

- **Flag:** `payoff_biased_learning` (Scenario + World, default false;
  `FORMAT_VERSION` 29→30, layout-growth-only golden rehash across
  determinism/cognition/inventions/affect pins — flag off ⇒ byte-identical).
- **Mechanism** (`culture.rs`, gated on the flag): model bias (copy source =
  highest-energy Communicator neighbour, retargeting skill/invention/
  practice channels + variant descent) and content bias (decline a practice
  channel when local holders' mean energy < non-holders', both groups
  present). Content bias is scoped to practice channels — the only channels
  carrying coded harm; invention channels keep model-bias retargeting only,
  avoiding the over-filtering path the design's risk table flags.
- **Tests** (`tests/culture.rs`): content bias declines a locally-harmful
  practice even when the fittest model carries it (with flag-off control);
  negative control — a trait whose holders are locally *fitter* still
  transmits; model bias copies from the highest-energy model, not the
  highest-skill one.
- **Deferred:** the `SelectiveLearning` codex detector (design §5). With the
  mechanism negative, a detector for payoff-biased rejection "actually
  happening" is premature; it should ship with the *working* variant.

## Handoff — the honest next move

The design's flagged fallback is now the main line: a **reproductive-success
fitness proxy** (lifetime offspring / lineage growth per neighbour, or a
parent-vs-child survival signal readable in the neighbourhood scan). That is
the only proxy that can see a stillbirth/cull cost. Per the roadmap's
evidence discipline, that variant should be pre-registered against the same
n=10 founder-tag protocol before another measurement run. Until then, the
honest O-track statement is: **payoff-blind transmission is confirmed as the
antagonist (O1/O2a), but payoff bias keyed on energy does not pay (O2b
negative); the bias must key on reproductive success, which the current
per-tick neighbourhood scan does not expose.**
