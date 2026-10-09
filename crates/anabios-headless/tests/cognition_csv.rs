use std::process::Command;

// Runs the cognition instrument briefly over two gate scales and checks the
// four CSVs: headers, one series row per (run, window, tier), one gates row
// per run with the scaled ladder, and tiers keyed by founding species.
#[test]
fn cognition_writes_the_four_csvs_keyed_by_seed_and_scale() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cog");
    let scenario = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/cognition-threshold.toml");
    let status = Command::new(env!("CARGO_BIN_EXE_anabios-headless"))
        .args([
            "cognition",
            "--scenario",
            scenario,
            "--ticks",
            "30",
            "--window",
            "10",
            "--gate-scales",
            "0,1",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let series = std::fs::read_to_string(out.join("series.csv")).unwrap();
    let mut lines = series.lines();
    assert_eq!(
        lines.next().unwrap(),
        "seed,scale,tick,tier,founder_potential,alive,births_cum,deaths_cum,mean_iq,\
         mean_potential,mean_energy,mean_era,share_practice_gate,share_era1,share_era2,\
         share_era3,share_era4,discoveries_cum"
    );
    // 2 runs × windows at ticks 0,10,20,30 × 6 tiers.
    let rows: Vec<&str> = lines.collect();
    assert_eq!(rows.len(), 2 * 4 * 6, "series rows");
    assert!(rows.iter().all(|r| r.split(',').count() == 18), "18 columns per series row");
    let founders_alive_at_0 = rows
        .iter()
        .filter(|r| r.starts_with("1601,0.000,0,"))
        .map(|r| r.split(',').nth(5).unwrap().parse::<u32>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(founders_alive_at_0, vec![40; 6], "40 founders per tier at tick 0");

    let gates = std::fs::read_to_string(out.join("gates.csv")).unwrap();
    let mut g = gates.lines();
    assert!(g
        .next()
        .unwrap()
        .starts_with("seed,scale,era_req_1,era_req_2,era_req_3,era_req_4,practice_req,"));
    let g: Vec<Vec<&str>> = g.map(|r| r.split(',').collect()).collect();
    assert_eq!(g.len(), 2, "one gates row per run");
    assert_eq!(&g[0][1..7], &["0.000", "0.000", "0.000", "0.000", "0.000", "0.000"], "open ladder");
    assert_eq!(
        &g[1][1..7],
        &["1.000", "0.150", "0.350", "0.550", "0.750", "0.100"],
        "default ladder"
    );

    let tiers = std::fs::read_to_string(out.join("tiers.csv")).unwrap();
    let t: Vec<&str> = tiers.lines().skip(1).collect();
    assert_eq!(t.len(), 2 * 6, "six tiers per run");
    assert!(t.iter().all(|r| r.split(',').nth(4) == Some("40")), "40 founders per tier");

    let fitness = std::fs::read_to_string(out.join("fitness.csv")).unwrap();
    let f: Vec<&str> = fitness.lines().skip(1).collect();
    assert_eq!(f.len(), 2 * 20, "twenty realized-IQ bins per run");
    assert!(f[0].starts_with("1601,0.000,0.00,0.05,"));
}
