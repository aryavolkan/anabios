use std::process::Command;

// Runs a tiny sweep and asserts the CSV header/rows carry the total_trades and
// novel_types columns (canonical final-column order: …coverage,total_trades,novel_types).
#[test]
fn summary_csv_has_novel_types_column() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("swp");
    let scenario = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/minimal.toml");
    let status = Command::new(env!("CARGO_BIN_EXE_anabios-headless"))
        .args([
            "sweep",
            "--scenario",
            scenario,
            "--seeds",
            "2",
            "--ticks",
            "50",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let csv = std::fs::read_to_string(out.join("summary.csv")).unwrap();
    let mut lines = csv.lines();
    let header = lines.next().unwrap();
    assert!(
        header.ends_with(",emergence_score,novel_events,coverage,total_trades,novel_types"),
        "header was: {header}"
    );
    // every data row must have exactly 70 fields (5 prefix + EVENT_TYPE_COUNT=60
    // per-event columns + emergence_score,novel_events,coverage,total_trades,novel_types).
    for row in lines {
        assert_eq!(row.split(',').count(), 70, "row: {row}");
    }
}

// Runs a sweep against biome-trade (which fires resource_traded/material_learning,
// treated as rare/novel by the default score table) and asserts any run with
// novel_events > 0 gets its events JSONL copied into <out>/novel/.
#[test]
fn novel_runs_are_copied_to_novel_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("swp");
    let scenario = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/biome-trade.toml");
    // The mechanism under test is the novel/ copy, not the sweep length — and
    // the spawned binary is instrumented under cargo llvm-cov, where every tick
    // is ~10x slower (this test was a 13.5-minute single pole in the coverage
    // job). Shorten under cfg(coverage), mirroring record_schema.rs.
    let (seeds, ticks) = if cfg!(coverage) { ("2", "300") } else { ("4", "1500") };
    let status = Command::new(env!("CARGO_BIN_EXE_anabios-headless"))
        .args([
            "sweep",
            "--scenario",
            scenario,
            "--seeds",
            seeds,
            "--ticks",
            ticks,
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let csv = std::fs::read_to_string(out.join("summary.csv")).unwrap();
    // Resolve the column by name — the per-event block grows as the codex adds
    // event types, so a hardcoded index silently goes stale (it did: it read
    // emergence_score for a while, parse::<u64> fails on the float, and the
    // assertions below were vacuously skipped).
    let header = csv.lines().next().unwrap();
    let novel_idx = header
        .split(',')
        .position(|c| c == "novel_events")
        .expect("summary.csv has a novel_events column");
    let any_novel = csv
        .lines()
        .skip(1)
        .any(|r| r.split(',').nth(novel_idx).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) > 0);
    if any_novel {
        let novel = out.join("novel");
        assert!(novel.is_dir(), "novel/ dir missing though a run had novel_events>0");
        assert!(std::fs::read_dir(&novel).unwrap().count() > 0);
    }
}
