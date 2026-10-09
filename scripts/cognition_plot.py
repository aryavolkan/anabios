#!/usr/bin/env python3
"""Draw the cognition-threshold readings as SVG.

Reads the four CSVs `anabios-headless cognition --out DIR` writes and renders:

  alive-by-tier.svg     population per founder tier over time (gate scale 1)
  iq-by-tier.svg        mean realized IQ per tier over time, the five gates ruled
  fitness-by-iq.svg     lifetime offspring per matured agent by realized-IQ bin
  fitness-by-gate.svg   lifetime offspring per matured agent per tier, by gate height
  births-by-gate.svg    births per run by gate height (whole population)

Standard library only (no matplotlib), so it runs anywhere the CLI does:

  scripts/cognition_plot.py runs/cognition            # writes SVGs next to the CSVs
  scripts/cognition_plot.py runs/cognition --out gallery --prefix cognition-threshold-

Seeds are pooled: time series take the mean over seeds, lifetime tables are
summed before dividing (every matured lifetime weighs the same). A realized-IQ
bin with fewer than MIN_LIFETIMES pooled lifetimes is drawn hollow and left
unlabelled, so a bar never rests on a handful of agents.
"""

import argparse
import csv
import math
import os
import sys
from collections import defaultdict

MIN_LIFETIMES = 25

# Chart chrome (light surface) and the categorical slots, in fixed order, for
# the six tiers. Tier order follows the founding gene, lowest first.
SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
MUTED = "#898781"
GRID = "#e1e0d9"
AXIS = "#c3c2b7"
SERIES = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6", "#e34948"]
SEQ = "#2a78d6"
SEQ_HOLLOW = "#9ec5f4"
FONT = 'font-family="system-ui, -apple-system, \'Segoe UI\', sans-serif"'

GATE_LABELS = [("practice", 0), ("era 1", 1), ("era 2", 2), ("era 3", 3), ("era 4", 4)]


def read_csv(path):
    with open(path, newline="") as f:
        return list(csv.DictReader(f))


def fnum(s):
    return float(s)


def nice_max(v):
    """Round `v` up to a clean axis top."""
    if v <= 0:
        return 1.0
    mag = 10 ** math.floor(math.log10(v))
    for step in (1, 2, 2.5, 5, 10):
        if v <= step * mag:
            return step * mag
    return 10 * mag


def ticks_for(vmax, n=5):
    step = nice_max(vmax / n)
    out, t = [], 0.0
    while t <= vmax + 1e-9:
        out.append(round(t, 6))
        t += step
    return out


def fmt(v):
    if abs(v - round(v)) < 1e-9:
        return str(int(round(v)))
    return f"{v:.2f}".rstrip("0").rstrip(".")


class Svg:
    """A minimal SVG writer with the chart chrome baked in."""

    def __init__(self, width, height, title, subtitle):
        self.w, self.h = width, height
        self.parts = [
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
            f'viewBox="0 0 {width} {height}" {FONT} font-size="12">',
            f"<title>{esc(title)}</title>",
            f'<rect width="{width}" height="{height}" fill="{SURFACE}"/>',
            f'<text x="24" y="28" font-size="16" font-weight="600" fill="{INK}">{esc(title)}</text>',
            f'<text x="24" y="46" fill="{INK2}">{esc(subtitle)}</text>',
        ]

    def add(self, s):
        self.parts.append(s)

    def text(self, x, y, s, fill=INK2, anchor="start", size=12, weight="normal", extra=""):
        self.add(
            f'<text x="{x:.1f}" y="{y:.1f}" fill="{fill}" text-anchor="{anchor}" '
            f'font-size="{size}" font-weight="{weight}" {extra}>{esc(s)}</text>'
        )

    def line(self, x1, y1, x2, y2, stroke, width=1, extra=""):
        self.add(
            f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" '
            f'stroke="{stroke}" stroke-width="{width}" {extra}/>'
        )

    def write(self, path):
        self.parts.append("</svg>")
        with open(path, "w") as f:
            f.write("\n".join(self.parts) + "\n")


def esc(s):
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


class Frame:
    """Plot area with linear scales and the recessive grid/axes."""

    def __init__(self, svg, x0, y0, x1, y1, xmin, xmax, ymin, ymax):
        self.svg, self.x0, self.y0, self.x1, self.y1 = svg, x0, y0, x1, y1
        self.xmin, self.xmax, self.ymin, self.ymax = xmin, xmax, ymin, ymax

    def sx(self, v):
        return self.x0 + (v - self.xmin) / (self.xmax - self.xmin) * (self.x1 - self.x0)

    def sy(self, v):
        return self.y1 - (v - self.ymin) / (self.ymax - self.ymin) * (self.y1 - self.y0)

    def axes(self, xticks, yticks, xlabel, ylabel, xfmt=fmt, yfmt=fmt):
        for t in yticks:
            y = self.sy(t)
            self.svg.line(self.x0, y, self.x1, y, GRID)
            self.svg.text(self.x0 - 8, y + 4, yfmt(t), MUTED, "end", 11)
        self.svg.line(self.x0, self.y1, self.x1, self.y1, AXIS)
        for t in xticks:
            x = self.sx(t)
            self.svg.text(x, self.y1 + 16, xfmt(t), MUTED, "middle", 11)
        self.svg.text((self.x0 + self.x1) / 2, self.y1 + 36, xlabel, INK2, "middle")
        self.svg.text(
            16, (self.y0 + self.y1) / 2, ylabel, INK2, "middle",
            extra=f'transform="rotate(-90 16 {(self.y0 + self.y1) / 2:.1f})"',
        )

    def polyline(self, pts, color, width=2, title=None):
        d = " ".join(f"{self.sx(x):.1f},{self.sy(y):.1f}" for x, y in pts)
        t = f"<title>{esc(title)}</title>" if title else ""
        self.svg.add(
            f'<polyline points="{d}" fill="none" stroke="{color}" stroke-width="{width}" '
            f'stroke-linejoin="round" stroke-linecap="round">{t}</polyline>'
        )

    def dot(self, x, y, color, r=4):
        self.svg.add(
            f'<circle cx="{self.sx(x):.1f}" cy="{self.sy(y):.1f}" r="{r + 2}" fill="{SURFACE}"/>'
            f'<circle cx="{self.sx(x):.1f}" cy="{self.sy(y):.1f}" r="{r}" fill="{color}"/>'
        )

    def bar(self, xlo, xhi, v, color, gap=2, maxw=24, title=None, hollow=False):
        x0, x1 = self.sx(xlo) + gap / 2, self.sx(xhi) - gap / 2
        w = min(x1 - x0, maxw)
        x0 = (x0 + x1) / 2 - w / 2
        ytop, ybase = self.sy(v), self.sy(self.ymin)
        h = max(ybase - ytop, 0.0)
        r = min(4, h / 2, w / 2)
        if hollow:
            fill, stroke = "none", f'stroke="{color}" stroke-width="1.5"'
        else:
            fill, stroke = color, ""
        t = f"<title>{esc(title)}</title>" if title else ""
        # Rounded cap at the data end, square at the baseline.
        path = (
            f"M{x0:.1f},{ybase:.1f} V{ytop + r:.1f} Q{x0:.1f},{ytop:.1f} {x0 + r:.1f},{ytop:.1f} "
            f"H{x0 + w - r:.1f} Q{x0 + w:.1f},{ytop:.1f} {x0 + w:.1f},{ytop + r:.1f} V{ybase:.1f} Z"
        )
        self.svg.add(f'<path d="{path}" fill="{fill}" {stroke}>{t}</path>')

    def vrule(self, x, label, color=AXIS, row=0):
        px = self.sx(x)
        self.svg.line(px, self.y0, px, self.y1, color, 1)
        self.svg.text(px + 4, self.y0 + 12 + 12 * row, label, MUTED, "start", 10)

    def hrule(self, y, label, color=AXIS):
        py = self.sy(y)
        self.svg.line(self.x0, py, self.x1, py, color, 1)
        self.svg.text(self.x1 - 4, py - 4, label, MUTED, "end", 10)


def legend(svg, x, y, entries):
    """Entries: (label, color). One row, swatch + label in text ink."""
    cx = x
    for label, color in entries:
        svg.add(f'<rect x="{cx}" y="{y - 9}" width="12" height="12" rx="2" fill="{color}"/>')
        svg.text(cx + 17, y + 1, label, INK2, "start", 11)
        cx += 17 + 7 * len(label) + 18


def spread_labels(labels, gap=13):
    """Nudge direct labels apart vertically so none overlap: `labels` are
    (y, text) pairs; returns them with y moved by the least amount that keeps
    neighbours `gap` px apart, in the original order of y."""
    items = sorted(labels, key=lambda p: p[0])
    ys = [y for y, _ in items]
    for i in range(1, len(ys)):
        ys[i] = max(ys[i], ys[i - 1] + gap)
    for i in range(len(ys) - 2, -1, -1):
        ys[i] = min(ys[i], ys[i + 1] - gap)
    return [(y, text) for y, (_, text) in zip(ys, items)]


def tier_label(pot):
    return f"gene {pot:.1f}"


def tiers_in_order(rows):
    """Tier ids sorted by founding potential, with their labels."""
    pot = {}
    for r in rows:
        pot[r["tier"]] = fnum(r["founder_potential"])
    return sorted(pot, key=lambda t: pot[t]), pot


def plot_series(series, out, prefix, column, ylabel, title, subtitle, gates=None, ymax=None):
    rows = [r for r in series if abs(fnum(r["scale"]) - 1.0) < 1e-9]
    if not rows:
        return
    order, pot = tiers_in_order(rows)
    acc = defaultdict(lambda: defaultdict(list))  # tier -> tick -> [values]
    for r in rows:
        acc[r["tier"]][int(r["tick"])].append(fnum(r[column]))
    ticks = sorted({int(r["tick"]) for r in rows})
    seeds = len({r["seed"] for r in rows})
    curves = {}
    top = 0.0
    for t in order:
        pts = []
        for tk in ticks:
            vals = acc[t].get(tk)
            if vals:
                m = sum(vals) / len(vals)
                pts.append((tk, m))
                top = max(top, m)
        curves[t] = pts
    ymax = ymax if ymax is not None else nice_max(top * 1.08)
    svg = Svg(880, 460, title, f"{subtitle} Mean over {seeds} seed{'s' if seeds != 1 else ''}.")
    fr = Frame(svg, 64, 70, 780, 380, 0, ticks[-1], 0, ymax)
    fr.axes(ticks_for(ticks[-1], 6), ticks_for(ymax, 5), "tick", ylabel)
    if gates:
        for label, g in gates:
            fr.hrule(g, label)
    end_labels = []
    for k, t in enumerate(order):
        pts = curves[t]
        if not pts:
            continue
        fr.polyline(pts, SERIES[k], title=tier_label(pot[t]))
        x, y = pts[-1]
        fr.dot(x, y, SERIES[k])
        end_labels.append((fr.sy(y), tier_label(pot[t])))
    for y, label in spread_labels(end_labels):
        svg.text(fr.x1 + 10, y + 4, label, INK2, "start", 11)
    legend(svg, 64, 440, [(tier_label(pot[t]), SERIES[k]) for k, t in enumerate(order)])
    svg.write(os.path.join(out, f"{prefix}{column_file(column)}.svg"))


def column_file(column):
    return {"alive": "alive-by-tier", "mean_iq": "iq-by-tier"}[column]


def plot_fitness_by_iq(fitness, gates_rows, out, prefix):
    rows = [r for r in fitness if abs(fnum(r["scale"]) - 1.0) < 1e-9]
    if not rows:
        return
    pooled = defaultdict(lambda: [0.0, 0.0, 0.0])  # bin lo -> [n, offspring sum, lifespan sum]
    for r in rows:
        n = fnum(r["lifetimes"])
        p = pooled[fnum(r["iq_lo"])]
        p[0] += n
        p[1] += fnum(r["mean_offspring"]) * n
        p[2] += fnum(r["mean_lifespan"]) * n
    seeds = len({r["seed"] for r in rows})
    total = sum(p[0] for p in pooled.values())
    vals = {lo: (p[1] / p[0] if p[0] else 0.0, p[0]) for lo, p in pooled.items()}
    top = max((v for v, n in vals.values() if n >= MIN_LIFETIMES), default=1.0)
    ymax = nice_max(top * 1.15)
    svg = Svg(
        880, 460, "Lifetime offspring by realized IQ",
        f"Offspring credited per agent whose IQ had crystallized and that died in the run; "
        f"{int(total)} lifetimes over {seeds} seed{'s' if seeds != 1 else ''}. "
        f"Hollow bars rest on fewer than {MIN_LIFETIMES}.",
    )
    fr = Frame(svg, 64, 70, 840, 380, 0.0, 1.0, 0.0, ymax)
    fr.axes([i / 10 for i in range(11)], ticks_for(ymax, 5), "realized IQ (0.05 bins)",
            "offspring per matured lifetime")
    g = gates_rows[0] if gates_rows else None
    if g:
        reqs = [fnum(g["practice_req"])] + [fnum(g[f"era_req_{k}"]) for k in range(1, 5)]
        last_px = -1e9
        row = 0
        for (label, idx) in GATE_LABELS:
            px = fr.sx(reqs[idx])
            # Rules closer than a label's width stagger their labels downward.
            row = row + 1 if px - last_px < 72 else 0
            fr.vrule(reqs[idx], f"{label} {reqs[idx]:.2f}", row=row)
            last_px = px
    width = 0.05
    for lo in sorted(vals):
        v, n = vals[lo]
        if n == 0:
            continue
        hollow = n < MIN_LIFETIMES
        fr.bar(lo, lo + width, v, SEQ_HOLLOW if hollow else SEQ, title=f"IQ {lo:.2f}-{lo + width:.2f}: "
               f"{v:.2f} offspring over {int(n)} lifetimes", hollow=hollow)
        if not hollow:
            svg.text(fr.sx(lo + width / 2), fr.sy(v) - 6, f"{v:.2f}", INK2, "middle", 10)
    # Lifetime counts as a muted row under the axis, so the weight behind each bar shows.
    svg.text(fr.x0 - 8, 436, "lifetimes", MUTED, "end", 10)
    for lo in sorted(vals):
        v, n = vals[lo]
        if n:
            svg.text(fr.sx(lo + width / 2), 436, str(int(n)), MUTED, "middle", 10)
    svg.write(os.path.join(out, f"{prefix}fitness-by-iq.svg"))


def plot_fitness_by_gate(tiers, out, prefix):
    scales = sorted({fnum(r["scale"]) for r in tiers})
    if len(scales) < 2:
        return
    order, pot = tiers_in_order(tiers)
    pooled = defaultdict(lambda: [0.0, 0.0])  # (tier, scale) -> [lifetimes, offspring sum]
    for r in tiers:
        n = fnum(r["lifetimes"])
        p = pooled[(r["tier"], fnum(r["scale"]))]
        p[0] += n
        p[1] += fnum(r["mean_offspring"]) * n
    seeds = len({r["seed"] for r in tiers})
    curves, top = {}, 0.0
    for t in order:
        pts = []
        for s in scales:
            n, off = pooled[(t, s)]
            if n >= MIN_LIFETIMES:
                v = off / n
                pts.append((s, v))
                top = max(top, v)
        curves[t] = pts
    ymax = nice_max(top * 1.1)
    svg = Svg(
        880, 460, "Lifetime offspring per tier by gate height",
        f"The whole IQ ladder scaled by the factor on the x axis (1 = the default gates; "
        f"0 = every gate open). Pooled over {seeds} seed{'s' if seeds != 1 else ''}.",
    )
    fr = Frame(svg, 64, 70, 780, 380, scales[0], scales[-1], 0.0, ymax)
    fr.axes(scales, ticks_for(ymax, 5), "gate scale", "offspring per matured lifetime")
    end_labels = []
    for k, t in enumerate(order):
        pts = curves[t]
        if not pts:
            continue
        fr.polyline(pts, SERIES[k], title=tier_label(pot[t]))
        for x, y in pts:
            fr.dot(x, y, SERIES[k])
        x, y = pts[-1]
        end_labels.append((fr.sy(y), tier_label(pot[t])))
    for y, label in spread_labels(end_labels):
        svg.text(fr.x1 + 10, y + 4, label, INK2, "start", 11)
    legend(svg, 64, 440, [(tier_label(pot[t]), SERIES[k]) for k, t in enumerate(order)])
    svg.write(os.path.join(out, f"{prefix}fitness-by-gate.svg"))


def plot_births_by_gate(gates_rows, out, prefix):
    scales = sorted({fnum(r["scale"]) for r in gates_rows})
    if len(scales) < 2:
        return
    acc = defaultdict(list)
    era = defaultdict(list)
    for r in gates_rows:
        acc[fnum(r["scale"])].append(fnum(r["births_total"]))
        era[fnum(r["scale"])].append(fnum(r["mean_era"]))
    means = {s: sum(v) / len(v) for s, v in acc.items()}
    seeds = len({r["seed"] for r in gates_rows})
    ymax = nice_max(max(means.values()) * 1.15)
    svg = Svg(
        720, 420, "Births per run by gate height",
        f"Whole population, mean over {seeds} seed{'s' if seeds != 1 else ''}; "
        f"the mean tech era held at the end is written under each bar.",
    )
    n = len(scales)
    fr = Frame(svg, 64, 70, 680, 340, -0.5, n - 0.5, 0.0, ymax)
    fr.axes([], ticks_for(ymax, 5), "", "births per run")
    svg.text((fr.x0 + fr.x1) / 2, fr.y1 + 48, "gate scale", INK2, "middle")
    for i, s in enumerate(scales):
        v = means[s]
        fr.bar(i - 0.5, i + 0.5, v, SEQ, maxw=48, title=f"scale {s:g}: {v:.0f} births")
        svg.text(fr.sx(i), fr.sy(v) - 6, f"{v:.0f}", INK2, "middle", 10)
        svg.text(fr.sx(i), fr.y1 + 16, f"{s:g}", MUTED, "middle", 11)
        e = sum(era[s]) / len(era[s])
        svg.text(fr.sx(i), fr.y1 + 30, f"era {e:.2f}", MUTED, "middle", 10)
    svg.write(os.path.join(out, f"{prefix}births-by-gate.svg"))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir", help="directory `anabios-headless cognition --out` wrote")
    ap.add_argument("--out", help="directory for the SVGs (default: the input directory)")
    ap.add_argument("--prefix", default="", help="file-name prefix for the SVGs")
    args = ap.parse_args()
    out = args.out or args.dir
    os.makedirs(out, exist_ok=True)

    series = read_csv(os.path.join(args.dir, "series.csv"))
    fitness = read_csv(os.path.join(args.dir, "fitness.csv"))
    tiers = read_csv(os.path.join(args.dir, "tiers.csv"))
    gates = read_csv(os.path.join(args.dir, "gates.csv"))
    default_gates = [r for r in gates if abs(fnum(r["scale"]) - 1.0) < 1e-9] or gates

    plot_series(series, out, args.prefix, "alive", "agents alive", "Population by founder tier",
                "Each tier is one founding lineage of 40; the gene is its CognitivePotential.")
    gate_lines = None
    if default_gates:
        g = default_gates[0]
        reqs = [fnum(g["practice_req"])] + [fnum(g[f"era_req_{k}"]) for k in range(1, 5)]
        gate_lines = [(f"{label} gate {reqs[idx]:.2f}", reqs[idx]) for label, idx in GATE_LABELS]
    plot_series(series, out, args.prefix, "mean_iq", "mean realized IQ", "Realized IQ by founder tier",
                "The rules are the gates: a tier below one cannot learn that era's inventions.",
                gates=gate_lines, ymax=1.0)
    plot_fitness_by_iq(fitness, default_gates, out, args.prefix)
    plot_fitness_by_gate(tiers, out, args.prefix)
    plot_births_by_gate(gates, out, args.prefix)
    print(f"wrote SVGs to {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
