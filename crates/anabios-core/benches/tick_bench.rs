//! Per-tick benchmarks at 1k and 10k agents, plus stage-level microbenches.

use anabios_core::biome::WORLD_SIZE;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::tick::step;
use anabios_core::world::World;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

/// Deterministic pseudo-random world position for index `i` (no RNG needed;
/// the salts decorrelate x/y and let each caller pick a different spread).
fn scatter_pos(i: usize, x_salt: u32, y_salt: u32) -> Vec2 {
    let x = ((i.wrapping_mul(x_salt as usize)) as u32 as f32) / u32::MAX as f32 * WORLD_SIZE;
    let y = ((i.wrapping_mul(y_salt as usize)) as u32 as f32) / u32::MAX as f32 * WORLD_SIZE;
    Vec2::new(x, y)
}

fn build_population(count: usize, seed: u64) -> World {
    let mut w = World::new(seed);
    for i in 0..count {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Size, 0.4);
        w.spawn_agent(scatter_pos(i, 2_654_435_761, 40_503), g);
    }
    w
}

/// Warm a bench world a few ticks so scratch buffers and the spatial hash are
/// sized and detector windows hold realistic data.
fn warm(w: &mut World, ticks: usize) {
    for _ in 0..ticks {
        step(w);
    }
    let cap = w.agents.capacity();
    w.sensors.resize(cap, anabios_core::sense::SensorRegister::default());
    w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32));
}

fn bench_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick");
    group.sample_size(20);
    for &count in &[1_000_usize, 10_000_usize] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            // Build once outside the timed loop.
            let world_template = build_population(count, 1);
            b.iter_batched(
                || world_template.clone(),
                |mut w| {
                    step(&mut w);
                    w
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Stage-level microbenches, so regressions can be attributed to a specific
/// pipeline stage instead of the whole tick.
fn bench_stages(c: &mut Criterion) {
    let mut group = c.benchmark_group("stages");
    group.sample_size(20);
    let mut w = build_population(10_000, 1);
    warm(&mut w, 5);

    group.bench_function("spatial_rebuild/10000", |b| {
        b.iter(|| w.spatial.rebuild(&w.agents.position, |i| w.agents.is_alive(i as u32)))
    });
    let mut w1k = build_population(1_000, 1);
    warm(&mut w1k, 5);
    for (name, sw) in [(1_000, &mut w1k), (10_000, &mut w)] {
        group.bench_function(BenchmarkId::new("sense", name), |b| {
            let mut sensors = std::mem::take(&mut sw.sensors);
            b.iter(|| {
                anabios_core::sense::sense_all(
                    &sw.agents,
                    &sw.biome,
                    &sw.pheromones,
                    &sw.spatial,
                    &mut sensors,
                    sw.world_size,
                )
            });
            sw.sensors = sensors;
        });
    }
    group.bench_function("codex/10000", |b| {
        b.iter(|| anabios_core::codex::observe_all(&mut w));
    });
    group.finish();
}

/// Scavenge under a mass-death carcass load: the worst case the carcass
/// spatial index fixes (the default tick bench has ~0 carcasses, so the
/// scavenge path is invisible there). Each iteration starts from a fresh
/// clone so carcass counts don't drift across samples.
fn bench_scavenge(c: &mut Criterion) {
    let mut group = c.benchmark_group("scavenge");
    group.sample_size(20);
    let mut w = World::new(1);
    // 2k armed carnivores (predator_kit: Locomotor + Vision + carnivore Mouth
    // + Weapon).
    for i in 0..2_000_usize {
        let id = w.spawn_agent(scatter_pos(i, 2_654_435_761, 40_503), Genome::neutral());
        w.agents.modules[id as usize] = anabios_core::module::predator_kit();
    }
    // 1k carcasses scattered on a second deterministic grid.
    for i in 0..1_000_usize {
        w.carcasses.push(anabios_core::carcass::Carcass {
            pos: scatter_pos(i, 1_103_515_245, 19_379),
            flesh: 10.0,
            age: 0,
            species_id: 0,
        });
    }
    // One warm tick to size the tick scratch buffers interact_all reads.
    step(&mut w);
    group.bench_function("interact/2000a_1000c", |b| {
        b.iter_batched(
            || w.clone(),
            |mut w| {
                anabios_core::interact::interact_all(&mut w);
                w
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group!(benches, bench_tick, bench_stages, bench_scavenge);
criterion_main!(benches);
