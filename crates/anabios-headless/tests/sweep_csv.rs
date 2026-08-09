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
    // every data row must have exactly 69 fields (5 prefix + EVENT_TYPE_COUNT=59
    // per-event columns + emergence_score,novel_events,coverage,total_trades,novel_types).
    for row in lines {
        assert_eq!(row.split(',').count(), 69, "row: {row}");
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
    let status = Command::new(env!("CARGO_BIN_EXE_anabios-headless"))
        .args([
            "sweep",
            "--scenario",
            scenario,
            "--seeds",
            "4",
            "--ticks",
            "1500",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let csv = std::fs::read_to_string(out.join("summary.csv")).unwrap();
    let any_novel = csv.lines().skip(1).any(|r| {
        // novel_events is field index 65: 5 prefix + 59 event counts + emergence_score.
        r.split(',').nth(65).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) > 0
    });
    if any_novel {
        let novel = out.join("novel");
        assert!(novel.is_dir(), "novel/ dir missing though a run had novel_events>0");
        assert!(std::fs::read_dir(&novel).unwrap().count() > 0);
    }
}
