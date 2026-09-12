# Anabios 2D Visual-System Pass

## Purpose

Complete the presentation layer with a coherent, readable pixel-art system for field actors, settlements and inventions, biome decoration, and event feedback. The simulation remains read-only presentation input; all additions live in Godot presentation code.

## Visual Contract

- Retain the existing 16×16 authored pixel cell and 128×128 square (8×8) atlas layout.
- Keep nearest-neighbour sampling for field and settlement pixel art.
- Do not introduce extreme-aspect atlases or per-agent nodes; the Metal/MultiMesh2D path requires square atlas grids and compact per-instance data.
- Reuse the project's restrained dark outlines, earth tones, amber light, cyan water, and invention-specific accent colours.
- Preserve the existing separation: `main.gd` is orchestration only; visual registries, layers, and effects stay in focused scripts.

## Components

### Field actors

Expand the existing ape and mammal atlas families only where a readable state is missing. Every animated family uses a neutral idle, four-frame locomotion cycle, and paired contextual action poses. All frames must remain legible at field scale and share the existing shader pose addressing.

### Settlements and inventions

Keep landmarks as 16×16 static authored silhouettes on their dedicated plain MultiMesh layers. Add only subtle presentation animation that can be applied at the layer or effect level—chimney smoke, fire flicker, construction pop, spark/mote bursts—without adding node-per-building runtime cost or simulation state.

### Biome props

Add a low-density decorative prop layer with a small set of biome-resolved pixel silhouettes (for example reeds, scrub, conifer, rock, and fallen log). Props are deterministic from existing terrain data, use batched MultiMesh layers, and do not imply gameplay collision, resources, or authority.

### Event and combat feedback

Extend the existing pooled effect vocabulary with pixel-art-shaped bursts where the current procedural rings/motes are not distinctive enough. Reuse the existing per-event cooldown/budget model; effects are local, short-lived, palette-coded, and never change simulation state.

## Asset Workflow

Image generation may be used for compact pixel-art style and silhouette exploration. Production assets are transcribed into the deterministic 16×16 block-art registries so they are reproducible, atlas-safe, source-controlled, and suitable for the existing renderer. Generated reference images do not ship as runtime textures unless a later approved change explicitly adds an import pipeline.

## Implementation Order

1. Capture the current scene and current animation states for a visual baseline.
2. Add missing/readability-improving field and effect frames without changing existing simulation mapping.
3. Add batched biome-prop data and layer integration.
4. Add or refine settlement/invention and event presentation animations using existing pools where possible.
5. Run diagnostics, scene validation, and rendered image/animation-sheet review after each affected subsystem.

## Verification

- Affected GDScript and shader files pass diagnostics and formatting/lint checks.
- Main scene structural validation remains clean.
- Rendered captures confirm every new asset is crisp, visible, and correctly layered at gameplay zoom.
- Animation sheets show full cycles in chronological order and do not reveal atlas bleeding, flipped art, or torn Metal rendering.
- The working tree contains only scoped presentation changes; no simulation source or deterministic replay behavior changes.

## Non-goals

- No changes to simulation rules, species data, or emergent outcomes.
- No node-per-agent rendering, arbitrary imported sprite sheets, or changes to the existing MultiMesh shader contract.
- No unbounded particles, camera shake without a per-event cooldown, or décor that suggests unavailable gameplay interaction.
