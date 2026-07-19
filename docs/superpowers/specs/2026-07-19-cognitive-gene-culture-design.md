# Cognitive Gene–Culture Coevolution — Design Spec

**Date:** 2026-07-19
**Status:** Draft for review
**Depends on:** invention tree (`invention.rs`), Big-Five personality (`personality.rs`), meme substrate (`culture.rs`).

## 1. Goal & success criteria

Give each agent a **cognitive phenotype** — genetically-influenced but developmentally-shaped **IQ** — that gates *which* cultural traits it can acquire, and add **maladaptive practice memes** that spread socially yet damage genetic/reproductive fitness. Together these turn the existing one-directional culture ratchet into a genuine **gene ↔ culture conflict**.

Success (all under a flag; baseline scenarios unchanged):

1. **Nature + nurture.** Realized IQ correlates with *both* the heritable `CognitivePotential` gene *and* the agent's juvenile environment (nutrition + social enrichment). A genetically-bright agent raised starving underperforms a genetically-average one raised rich.
2. **Capability gate.** Only lineages whose realized IQ clears an invention's `iq_req` can acquire it (discover *or* copy). High-era tech (`writing`→`nuclear`) is reachable only by high-IQ lineages; low-IQ lineages are stuck early.
3. **Maladaptive culture.** `inbreeding` and `child_sacrifice` spread via the same payoff-blind social copying as inventions, but reduce carriers' realized reproductive success. They can transiently invade before selection punishes them — visible as rise-and-crash waves on the coevolution chart.
4. **Selection responds.** Because IQ is metabolically costly *and* the only escape from the low-IQ maladaptive trap, `CognitivePotential` (and `Openness`) come under joint, non-trivial selection rather than trending to an extreme.
5. **Determinism preserved.** Flag-off is byte-identical modulo a one-time serialized-layout golden refresh (the meme-vector widening), following the invention-tree precedent.

## 2. Why this should work (mechanism)

Culture in the current sim is a pure ratchet: every invention is net-positive, so it always sweeps. Real gene–culture systems are two-sided — culture can carry *maladaptive* traditions that persist because social transmission is payoff-blind. Adding (a) a *capability ceiling* on acquisition (IQ) and (b) *fitness-negative* memes creates the missing tension: openness to culture is double-edged, and cognition is the lever that decides whether a lineage nets ahead or gets dragged down. The developmental (nurture) component means the environment a lineage *builds* (well-fed, socially-rich colonies) feeds back into the cognition of the next cohort — a culture→phenotype→culture loop.

## 3. Mechanism

### 3.1 Personality (existing — reused, not rebuilt)

The five OCEAN traits already exist as heritable genome slots (`Agreeableness=10`, `Neuroticism=11`, `Openness=12`, `Extraversion=13`, `Conscientiousness=21`), on a separate RNG substream and **excluded from speciation distance**. `Openness` already scales invention-discovery rate. No change to the personality system; this spec only *reads* `Openness` alongside the new IQ gate.

### 3.2 IQ: a gene × environment phenotype

**Genetic component — `CognitivePotential` gene.** Rename reserved slot `16` (`_DriveReserved16`) → `CognitivePotential`, value in `[0,1]`. Mutates like any adaptive gene. **Unlike personality, it counts toward speciation distance** (it is adaptive, so lineages that diverge in cognition should be able to speciate).

**Realized IQ — a per-agent phenotype (runtime state, serialized).** New SoA fields on `AgentBuffers`:
- `iq: Vec<f32>` — realized IQ in `[0,1]`, the value all gates read.
- `iq_enrich_acc: Vec<f32>` and `iq_enrich_ticks: Vec<u32>` — juvenile enrichment accumulator + sample count.

**Development (nature + nurture), deterministic, no RNG:**
- On birth (cognition on): `iq = CognitivePotential` (nature baseline), accumulators zeroed.
- Each tick while `age < IQ_MATURATION_AGE`, sample enrichment in `[0,1]`:
  - `nutrition = clamp(energy_gained_this_tick / IQ_NUTRITION_REF, 0, 1)`
  - `social = clamp(communicator_neighbors / IQ_SOCIAL_REF, 0, 1)`
  - `iq_enrich_acc += 0.5*nutrition + 0.5*social`; `iq_enrich_ticks += 1`
  - `enrich_mean = iq_enrich_acc / iq_enrich_ticks`
  - `iq = lerp(CognitivePotential, enrich_mean, IQ_PLASTICITY)` (continuously refined)
- At `age == IQ_MATURATION_AGE`, IQ **crystallizes** (no further updates).

Defaults (tunable): `IQ_MATURATION_AGE = 100`, `IQ_PLASTICITY = 0.5` (half nature / half nurture), `IQ_NUTRITION_REF` and `IQ_SOCIAL_REF` set so a well-fed, socially-embedded juvenile saturates each signal.

**Metabolic cost.** Brains are expensive: basal metabolism `*= 1 + IQ_METABOLIC_COST * iq` (`IQ_METABOLIC_COST = 0.25`), applied in `integrate` exactly like the invention `metabolism_multiplier`. This is what prevents IQ from freely maxing out and makes it an evolvable tradeoff.

### 3.3 IQ-gated meme acquisition

Each catalog entry gains an `iq_req: f32`. With cognition on, an agent may **discover** or **copy** a trait `k` only if `iq >= iq_req[k]` (in addition to the existing prereq/holder rules):
- Discovery: filter `candidates()` by `iq_req <= agent.iq`.
- Spread (`culture_step`): copy channel `k` only if `iq_req <= receiver.iq`.

`iq_req` by era (tunable): era 1 → `0.15`, era 2 → `0.35`, era 3 → `0.55`, era 4 → `0.75`. Maladaptive practices → `0.10` (anyone can catch a bad habit). `Openness` continues to set discovery *rate*; IQ is the hard *ceiling*.

### 3.4 Maladaptive practice memes

Two new **practices**, held in their own meme-channel block *above* the inventions so the invention scenarios' RNG stream is untouched:
`PRACTICE_CHANNEL_BASE = INVENTION_CHANNEL_BASE + INVENTION_COUNT` (= 18); `MEME_CHANNELS` widens 18 → 20.
- `INBREEDING` (channel 18), `CHILD_SACRIFICE` (channel 19). No prereqs, `iq_req = 0.10`, no buff.

They are discovered and spread by the **same** copy-toward-best-neighbour mechanism as inventions, gated on `cognition_enabled` (not `inventions_enabled`), so an inventions-only scenario never sees them.

**Reproductive/genetic effect sites (all gated on `cognition_enabled`):**
- **`child_sacrifice`** — in `reproduce`, if the primary parent holds it, cull the newborn with probability `CHILD_SACRIFICE_CULL = 0.5` (one RNG draw per such birth). Direct fecundity cut.
- **`inbreeding`** — three coupled effects (the mate bias raises the *frequency* of close pairings; the depression supplies the *cost*):
  1. *Mate-choice bias:* a holder seeks the genetically-nearest eligible partner (min genome distance, tie-break lowest id) in `find_mate` instead of the default lowest-id pairing.
  2. *Inbreeding depression — frailty:* the child's starting energy is scaled by `1 - INBREEDING_DEPRESSION * closeness` (`INBREEDING_DEPRESSION = 0.5`), where `closeness` rises 0→1 as parent genome distance falls from `INBREEDING_DIST` (= 0.15) to 0.
  3. *Inbreeding depression — viability:* the child is stillborn with probability `INBREEDING_STILLBIRTH * closeness` (`INBREEDING_STILLBIRTH = 0.45`, one RNG draw per inbred birth). **This lethal cost is what makes inbreeding a real population-level selector** — the energy-only frailty penalty proved too weak (a frailed newborn just re-feeds in a rich biome; validated by the `cognition_evolution` harness). All three effects gated on `cognition_enabled`, so non-cognition scenarios are byte-identical.

### 3.5 The coevolutionary loop

Low-IQ lineages are wide open to `iq_req=0.1` maladaptive practices but cannot reach the high-`iq_req` tech that would offset them → selection *for* IQ, checked by IQ's metabolic cost and its vulnerable juvenile window → an intermediate cognitive optimum, with `Openness` co-adjusting willingness. Maladaptive memes rise where copying outruns selection, then crash with their carriers.

### 3.6 Flag

`World::cognition_enabled: bool`, `#[serde(default)]` (false). Off ⇒ `CognitivePotential` unread, IQ never develops (stays default), no metabolic cost, no IQ gating (inventions behave exactly as today), practices inert and never jittered. On ⇒ full system.

## 4. Determinism

- **New gene slot is a rename only** — reserved slot 16 already held `0.5` in every neutral genome, so naming it changes no values.
- **Meme widening 18 → 20** grows the serialized meme vector ⇒ all goldens move once by *layout* (documented refresh, exactly like the invention PR). Behavior is unchanged: `inherit_meme` jitters practice channels **only when `cognition_enabled`**, so the flag-ON *inventions* scenario keeps its exact RNG draw count and the flag-OFF *minimal* scenario keeps its stream — only the serialized bytes grow. (This relies on the existing `inherit_meme` flag-gated-jitter fix.)
- **IQ development consumes no RNG** (pure function of energy + neighbour count). The new RNG draws are `child_sacrifice`'s cull roll and `inbreeding`'s stillbirth roll, both gated on `cognition_enabled` *and* a holder — zero draws when off, and `&&` short-circuits so a non-inbreeding/non-sacrificing birth draws nothing, keeping unrelated scenarios' streams unchanged.
- **Sensor reads** (social enrichment, mate neighbours) use the per-agent bounds discipline (`i < sensors.len()`) established by the crowding-stress fix.
- **New golden:** pin a flag-ON `cognitive-coevolution.toml` hash at fixed ticks, alongside the existing minimal + inventions goldens.

## 5. Tick integration & effect sites

- **IQ development:** new stage after `sense` (needs this tick's feeding + neighbour data), before `reproduce`. Reads energy delta + Communicator-neighbour count; writes `iq`.
- **Metabolic cost:** `integrate` (with the invention metabolism multiplier).
- **IQ gating:** inside `invention_step` discovery and `culture_step` spread.
- **Practices spread/atrophy:** reuse `culture_step` / `invention_step` paths extended to the practice channel block.
- **Reproductive effects:** `reproduce` (mate choice, child-sacrifice cull, inbreeding-depression starting energy).

## 6. Experiment / scenario

`scenarios/cognitive-coevolution.toml`: a well-mixed population with a spread of `CognitivePotential` and `Openness`, cognition + inventions both on, ample food in some regions and scarcity in others (so juvenile nutrition varies spatially → IQ variance). Watch: (a) realized-IQ distribution vs genetic potential; (b) which eras each lineage reaches; (c) `inbreeding`/`child_sacrifice` adoption waves against population/genetic-diversity traces.

## 7. Testing

- **Unit:** IQ blends nature+nurture (bright+poor < bright+rich, average+rich > average+poor); metabolic cost scales basal; gating filters discovery + spread candidates by `iq_req`; `child_sacrifice` culls at the expected rate; inbreeding depression reduces child starting energy for close pairs; practices excluded from the generic meme lerp.
- **Determinism:** flag-off byte-identical (post layout refresh); flag-ON `cognitive-coevolution` golden pinned + self-consistency; `inherit_meme` still flag-neutral for inventions-only scenarios.
- **Directional (ignored/statistical):** IQ mean rises when maladaptive memes are seeded; high-`iq_req` tech never appears in a capped-IQ population.

## 8. Risks & open questions

- **Tuning the trap:** maladaptive memes may purge too fast to observe, or spread hard enough to collapse the population. Spread rate vs fitness cost needs sweeping.
- **IQ cost balance:** too high → IQ never rises (tech never unlocks); too low → IQ maxes and cost is irrelevant.
- **Juvenile mortality:** the metabolic cost during an unfed juvenile window could over-cull bright juveniles; may need to defer cost until maturity.
- **Speciation fragmentation:** including `CognitivePotential` in distance could over-split; watch species counts.
- **Mate-choice wiring:** `inbreeding`'s kin-bias depends on the existing pairing code; if pairing is purely spatial, the bias is a re-rank of local candidates by genome distance.

## 9. Out of scope

- Open-ended novel-meme genesis (declined — catalog stays fixed).
- Prestige/conformity-biased transmission (future; copying stays payoff-blind).
- IQ influencing program execution or behaviour beyond meme-acquisition gating.
- A separate `Inventiveness` gene — **IQ replaces it** as the genetic gate.

## 10. Phasing (one PR per phase, per the milestone workflow)

1. **`CognitivePotential` gene + realized-IQ phenotype** (nature+nurture development, no RNG) + metabolic cost + flag. No gating yet.
2. **IQ-gated invention acquisition** (discovery + spread filtered by `iq_req`).
3. **Maladaptive practices** (`inbreeding`, `child_sacrifice`) + reproductive effect sites.
4. **Experiment scenario + coevolution-panel/codex wiring** (adoption series, discovery/adoption events, realized-IQ readout).
