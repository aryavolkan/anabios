use std::process::Command;

// Runs a tiny sweep and asserts the CSV header/rows carry the novel_types column.
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
        header.ends_with(",emergence_score,novel_events,coverage,novel_types"),
        "header was: {header}"
    );
    // every data row must have exactly 62 fields
    for row in lines {
        assert_eq!(row.split(',').count(), 62, "row: {row}");
    }
}
