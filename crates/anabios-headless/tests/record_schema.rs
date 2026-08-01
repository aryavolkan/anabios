//! Characterization test locking the JSON schema `record` produces — the
//! schema the web showcase player (`showcase/index.html` → `replay.js`)
//! reads. If the recorder's field names drift, the player silently
//! mis-renders instead of failing loudly, so this test pins the exact keys
//! the player depends on.
//!
//! `--ticks 800` on `predator-prey.toml` was chosen because it reliably
//! fires codex events (927 events observed across 14 distinct types when
//! this test was written) without the ~3.5-minute runtime of the tool's own
//! `ticks=4000` default.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

/// Assert `value` is a JSON object containing every key in `keys`.
fn assert_has_keys(value: &Value, keys: &[&str], context: &str) {
    let obj = value.as_object().unwrap_or_else(|| panic!("{context} is not a JSON object"));
    for key in keys {
        assert!(obj.contains_key(*key), "{context} is missing key `{key}`");
    }
}

/// `record --scenario … --ticks 800 --out …` (raw JSON since `--out` ends in
/// `.json`) matches the schema the web player reads: `meta.{world_size,
/// state_hash,frame_count}`, top-level `species`/`biome.{res,grids}`, well
/// formed `frames`/`sites` entries, and snake_case `events[].type`s.
#[test]
fn record_output_matches_player_schema() {
    let repo_root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let scenario = repo_root.join("scenarios/predator-prey.toml");
    assert!(scenario.is_file(), "expected scenario at {}", scenario.display());

    let out_path =
        std::env::temp_dir().join(format!("anabios_record_schema_{}.json", std::process::id()));
    let _cleanup = CleanupOnDrop(out_path.clone());

    let bin = env!("CARGO_BIN_EXE_anabios-headless");
    let status = Command::new(bin)
        .arg("record")
        .arg("--scenario")
        .arg(&scenario)
        .arg("--ticks")
        .arg("800")
        .arg("--out")
        .arg(&out_path)
        .current_dir(&repo_root)
        .status()
        .expect("failed to run anabios-headless record");
    assert!(status.success(), "record exited with {status}");

    let text = std::fs::read_to_string(&out_path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", out_path.display()));
    let replay: Value = serde_json::from_str(&text).expect("record output is not raw JSON");

    // meta.{world_size,state_hash,frame_count}
    let meta = replay.get("meta").expect("missing top-level `meta`");
    assert_has_keys(meta, &["world_size", "state_hash", "frame_count"], "meta");

    // top-level species (object), biome.{res,grids}
    assert!(replay.get("species").is_some_and(Value::is_object), "`species` must be an object");
    let biome = replay.get("biome").expect("missing top-level `biome`");
    assert_has_keys(biome, &["res", "grids"], "biome");
    assert!(biome["grids"].is_array(), "`biome.grids` must be an array");

    // frames: non-empty array; frames[0] has t,id,x,y,sp,d (parallel arrays).
    let frames = replay.get("frames").and_then(Value::as_array).expect("`frames` must be an array");
    assert!(!frames.is_empty(), "expected at least one frame");
    assert_has_keys(&frames[0], &["t", "id", "x", "y", "sp", "d"], "frames[0]");
    for key in ["id", "x", "y", "sp", "d"] {
        assert!(frames[0][key].is_array(), "frames[0].{key} must be an array");
    }

    // frames[].st (combat streaks) and frames[].tr (trade routes) are
    // `#[serde(skip_serializing_if = "Vec::is_empty")]` in record.rs, so
    // presence is NOT required on any given frame — only guard shape when
    // the key is present, across every frame, so the test stays robust on
    // runs with no combat/trade while still catching a type/shape drift.
    for (i, frame) in frames.iter().enumerate() {
        for key in ["st", "tr"] {
            let Some(v) = frame.get(key) else { continue };
            let arr = v.as_array().unwrap_or_else(|| panic!("frames[{i}].{key} must be an array"));
            assert!(
                arr.len().is_multiple_of(4),
                "frames[{i}].{key} length {} is not a multiple of 4 (flattened [x1,y1,x2,y2,...])",
                arr.len()
            );
            for (j, elem) in arr.iter().enumerate() {
                assert!(elem.is_i64() || elem.is_u64(), "frames[{i}].{key}[{j}] is not an integer");
            }
        }
    }

    // sites: array; guard the non-empty case (predator-prey may not settle).
    let sites = replay.get("sites").and_then(Value::as_array).expect("`sites` must be an array");
    if let Some(first) = sites.first() {
        assert_has_keys(first, &["t", "sid", "x", "y", "n"], "sites[0]");
    }

    // events: array; assert at least one fired, and every event has
    // t,type,x,y with a snake_case `type`.
    let events = replay.get("events").and_then(Value::as_array).expect("`events` must be an array");
    assert!(!events.is_empty(), "expected at least one codex event by tick 800");
    for (i, event) in events.iter().enumerate() {
        assert_has_keys(event, &["t", "type", "x", "y"], &format!("events[{i}]"));
        let kind =
            event["type"].as_str().unwrap_or_else(|| panic!("events[{i}].type is not a string"));
        assert!(!kind.is_empty(), "events[{i}].type is empty");
        assert!(
            kind.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "events[{i}].type `{kind}` is not snake_case"
        );
    }
}

/// Best-effort temp-file cleanup, even on assertion panic (unwind).
struct CleanupOnDrop(PathBuf);

impl Drop for CleanupOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
