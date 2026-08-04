# Union partitioned lcov tracefiles and gate line coverage.
#
# Reads one or more lcov `.info` files (the coverage shards) and computes line
# coverage as the UNION of their per-line hits: a source line counts as covered
# if ANY shard executed it. Operates purely on lcov `SF:`/`DA:` records, so the
# result is independent of the `lcov` CLI version (nothing to install, nothing
# to merge on disk).
#
# Emits two floors, mirroring the pre-sharding single-job gates:
#   - workspace line coverage >= wmin   (whole tree; dragged down by the Godot
#     bindings + CLI binaries that can't run under headless unit tests)
#   - anabios-core line coverage >= cmin (simulation logic; the Godot/CLI crates
#     are excluded so their low-coverage glue can't mask a core regression)
#
# Usage: awk -v wmin=82 -v cmin=95 -f lcov-line-gate.awk shard1.info shard2.info ...
# Exit status is non-zero if either floor is breached.

/^SF:/ { sf = substr($0, 4); next }

/^DA:/ {
    n = split(substr($0, 4), a, ",")
    line = a[1]
    hit = a[2] + 0
    key = sf SUBSEP line
    if (!(key in seen)) { seen[key] = 1; file[key] = sf }   # union of instrumented lines
    if (hit > 0) covered[key] = 1                            # covered by ANY shard
    next
}

END {
    for (k in seen) {
        sf = file[k]
        wf++
        if (k in covered) wh++
        # Core floor excludes the Godot bindings + headless CLI crates, matching
        # the old `--ignore-filename-regex 'anabios-(godot|headless)/'`.
        if (sf !~ /anabios-(godot|headless)\//) {
            cf++
            if (k in covered) ch++
        }
    }

    wp = (wf > 0) ? 100 * wh / wf : 0
    cp = (cf > 0) ? 100 * ch / cf : 0
    printf "workspace   line coverage: %6.2f%%  (%d/%d)\n", wp, wh, wf
    printf "anabios-core line coverage: %6.2f%%  (%d/%d)\n", cp, ch, cf

    fail = 0
    if (wf == 0 || cf == 0) {
        print "::error::no DA line records found in shard tracefiles — coverage collection produced nothing"
        exit 1
    }
    if (wp + 1e-9 < wmin) { printf "::error::workspace line coverage %.2f%% is below the %d%% floor\n", wp, wmin; fail = 1 }
    if (cp + 1e-9 < cmin) { printf "::error::anabios-core line coverage %.2f%% is below the %d%% floor\n", cp, cmin; fail = 1 }
    exit fail
}
