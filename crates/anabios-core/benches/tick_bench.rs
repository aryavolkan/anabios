//! Per-tick benchmarks at 1k and 10k agents.

use anabios_core::biome::WORLD_SIZE;
use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::prelude_test::Vec2;
use anabios_core::tick::step;
use anabios_core::world::World;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn build_population(count: usize, seed: u64) -> World {
    let mut w = World::new(seed);
    for i in 0..count {
        let x = ((i.wrapping_mul(2_654_435_761)) as u32 as f32) / u32::MAX as f32 * WORLD_SIZE;
        let y = ((i.wrapping_mul(40_503)) as u32 as f32) / u32::MAX as f32 * WORLD_SIZE;
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Size, 0.4);
        w.spawn_agent(Vec2::new(x, y), g);
    }
    w
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

criterion_group!(benches, bench_tick);
criterion_main!(benches);
