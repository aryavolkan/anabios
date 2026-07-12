//! M15 mechanism tests: kin recognition, sharing, and the cooperation detectors.

use anabios_core::genome::{Genome, GenomeSlot};
use anabios_core::kin::kinship;

#[test]
fn kinship_high_for_siblings_low_for_unrelated() {
    // Siblings: share both parent lineages (2 and 3); near-identical genomes.
    let g = Genome::neutral();
    let sib = kinship(10, &[2, 3], &g, 11, &[2, 3], &g);
    // Unrelated: no shared parents; distant genomes.
    let mut far = Genome::neutral();
    far.set(GenomeSlot::Size, 1.0);
    far.set(GenomeSlot::DietCarnivory, 1.0);
    far.set(GenomeSlot::SpeedMax, 1.0);
    let unrel = kinship(10, &[2, 3], &g, 99, &[50, 51], &far);
    assert!(sib > 0.7, "siblings with identical genome are highly related ({sib})");
    assert!(unrel < sib, "unrelated distant-genome pair is less related ({unrel} < {sib})");
}

#[test]
fn kinship_parent_child_is_related() {
    let g = Genome::neutral();
    // Agent 5 is a parent of agent 12 (12's parents include lineage 5).
    let r = kinship(5, &[1, 2], &g, 12, &[5, 7], &g);
    assert!(r > 0.5, "parent-child relatedness ({r})");
}

use anabios_core::codex::EventType;
use anabios_core::prelude_test::Vec2;
use anabios_core::program::{Node, Program};
use anabios_core::tick::step;
use anabios_core::world::World;

#[test]
fn share_transfers_energy_scaled_by_altruism() {
    let mut w = World::new(4);
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Altruism, 1.0);
    // High reproduction threshold so the adjacent pair does NOT mate this tick —
    // reproduction's energy cost would otherwise mask the share transfer.
    g.set(GenomeSlot::ReproductionThreshold, 1.0);
    let mut rg = Genome::neutral();
    rg.set(GenomeSlot::ReproductionThreshold, 1.0);
    let donor = w.spawn_agent(Vec2::new(500.0, 500.0), g);
    let recipient = w.spawn_agent(Vec2::new(501.0, 500.0), rg); // within SHARE_RANGE
                                                                // Donor always shares (share_intent = 1.0 via Const + Share).
    w.agents.program[donor as usize] = Program::from_slice(&[Node::Const(1.0), Node::Share]);
    w.agents.program[recipient as usize] = Program::from_slice(&[Node::Idle]);
    let d0 = w.agents.energy[donor as usize];
    let r0 = w.agents.energy[recipient as usize];
    step(&mut w);
    assert!(w.agents.energy[donor as usize] < d0, "donor lost energy");
    assert!(w.agents.energy[recipient as usize] > r0, "recipient gained energy");
}

#[test]
fn zero_altruism_means_no_sharing() {
    let mut w = World::new(4);
    let mut g = Genome::neutral();
    g.set(GenomeSlot::Altruism, 0.0);
    let donor = w.spawn_agent(Vec2::new(500.0, 500.0), g);
    let recipient = w.spawn_agent(Vec2::new(501.0, 500.0), Genome::neutral());
    w.agents.program[donor as usize] = Program::from_slice(&[Node::Const(1.0), Node::Share]);
    // Idle recipient so it neither grazes nor moves — isolates "no share in".
    w.agents.program[recipient as usize] = Program::from_slice(&[Node::Idle]);
    let r0 = w.agents.energy[recipient as usize];
    step(&mut w);
    // Recipient's only energy change is its own metabolism/grazing — no share in.
    // Assert it did not gain the share amount (energy did not increase from sharing).
    assert!(w.agents.energy[recipient as usize] <= r0 + 1e-3, "no altruism → no share");
}

#[test]
fn scratch_stays_sized_across_reproduction() {
    // With reproduction growing capacity mid-tick, world.actions must stay sized
    // to capacity every tick so the AlarmCall detector never early-returns on a
    // birth tick (M14 whole-branch-review follow-up).
    let mut w = World::new(7);
    for k in 0..8 {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::ReproductionThreshold, 0.0); // reproduce readily
        let _ = w.spawn_agent(Vec2::new(500.0 + k as f32, 500.0), g);
    }
    for _ in 0..30 {
        step(&mut w);
        assert!(
            w.actions.len() >= w.agents.capacity(),
            "world.actions must stay sized to capacity (alarm scratch invariant)"
        );
    }
}

#[test]
fn evolved_cooperation_fires_on_sustained_sharing() {
    let mut w = World::new(5);
    // A tight cluster of altruists that always share with their neighbor.
    let mut ids = Vec::new();
    for k in 0..8 {
        let mut g = Genome::neutral();
        g.set(GenomeSlot::Altruism, 1.0);
        let id = w.spawn_agent(Vec2::new(500.0 + (k % 3) as f32, 500.0 + (k / 3) as f32), g);
        w.agents.program[id as usize] = Program::from_slice(&[Node::Const(1.0), Node::Share]);
        ids.push(id);
    }
    let mut fired = false;
    for _ in 0..200 {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::EvolvedCooperation) {
            fired = true;
            break;
        }
    }
    assert!(fired, "sustained kin sharing → EvolvedCooperation");
}

use anabios_core::module::Module;

#[test]
fn pack_hunting_fires_when_three_attackers_hit_one_target() {
    let mut w = World::new(6);
    let prey = w.spawn_agent(Vec2::new(500.0, 500.0), Genome::neutral());
    // Move prey to its own species so the attackers are "other species" to it.
    let psid = w.species_centroids.len() as u32;
    w.species_centroids.push(Genome::neutral());
    w.species_parents.push(Some(0));
    w.species_member_counts.push(0);
    w.next_species_id = psid + 1;
    w.remove_from_species(w.agents.species_id[prey as usize]);
    w.agents.species_id[prey as usize] = psid;
    w.add_to_species(psid);
    // Three armed same-species predators adjacent to the prey, all firing.
    for k in 0..3 {
        let pred = w.spawn_agent(Vec2::new(501.0, 500.0 + k as f32 * 0.3), Genome::neutral());
        let mut kit = anabios_core::module::ModuleList::new();
        kit.push(Module::Locomotor { max_speed: 0.6, terrain_affinity: 0.5 });
        kit.push(Module::Sensor {
            sensor_type: anabios_core::module::SensorType::Vision,
            radius: 0.6,
            acuity: 0.6,
        });
        kit.push(Module::Weapon { damage: 1.0, energy_cost: 0.1 });
        w.agents.modules[pred as usize] = kit;
        w.agents.program[pred as usize] =
            Program::from_slice(&[Node::Const(1.0), Node::FireWeapon]);
    }
    let mut fired = false;
    for _ in 0..12 {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::PackHunting) {
            fired = true;
            break;
        }
    }
    assert!(fired, "3 same-species attackers on one target → PackHunting");
}

#[test]
fn herd_cohesion_fires_for_a_tight_persistent_herd() {
    use anabios_core::codex::HERD_WINDOW;
    let mut w = World::new(8);
    // A tight cluster of same-species herders (default species 0).
    let mut ids = Vec::new();
    for k in 0..10 {
        let id = w.spawn_agent(
            Vec2::new(500.0 + (k % 5) as f32 * 0.5, 500.0 + (k / 5) as f32 * 0.5),
            Genome::neutral(),
        );
        // Herd behavior: cohere toward same-species neighbor.
        w.agents.program[id as usize] = Program::from_slice(&[
            Node::SenseSameDirX,
            Node::MoveTowardX,
            Node::SenseSameDirY,
            Node::MoveTowardY,
        ]);
        ids.push(id);
    }
    let mut fired = false;
    for _ in 0..(HERD_WINDOW + 20) {
        step(&mut w);
        if w.codex.events.iter().any(|e| e.event_type == EventType::HerdCohesion) {
            fired = true;
            break;
        }
    }
    assert!(fired, "a tight persistent herd → HerdCohesion");
}
