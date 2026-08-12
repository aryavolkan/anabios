#!/usr/bin/env python3
"""Offline builder for the checked-in Earth biome rasters.

Outputs three 256x256 u8 files (elevation, temperature, precip), each a
value in [0,1] quantized to a byte. NOT run by the simulation -- the sim
embeds the .u8 outputs via include_bytes!. Determinism lives in the bytes,
not this script.

Usage:
  # Real-world rasters (see crates/anabios-core/assets/earth/README.md for
  # exact source URLs). --temp and --precip each take 12 monthly GeoTIFF
  # paths (Jan..Dec) which are averaged into a mean-annual field before
  # normalizing -- both source datasets are published as monthly rasters,
  # there is no single "mean annual" file to point at.
  build_earth_map.py --source real --elev ETOPO_bed.tif \\
      --temp jan.tif feb.tif ... dec.tif \\
      --precip jan.tif feb.tif ... dec.tif

  build_earth_map.py --source synthetic   # no external data; coarse Earth
"""
import argparse
import sys
from pathlib import Path

RES = 256
OUT = Path(__file__).resolve().parents[1] / "crates/anabios-core/assets/earth"
SEA_LEVEL_NORM = 0.35  # must match biome::SEA_LEVEL

# Land-side elevation normalization ceiling, in meters. `0 m` always maps to
# SEA_LEVEL_NORM; a real elevation of ELEV_CEILING_M (or higher) maps to 1.0.
# This is a *contrast* knob, not a literal "highest point on Earth" figure:
# real peaks (Everest, 8850 m) are rare, bilinear-resampling to
# 256x256 smooths them away, and biome::ROCK_LINE = 0.78 needs a plausible
# fraction of real mountainous terrain (East African Rift, Andes, Himalaya,
# ...) to actually cross it. A high ceiling (e.g. 8850 m, the literal Everest
# height) compresses nearly all land into the 0.35-0.50 band and produces 0%
# Rock terrain -- see docs/superpowers/specs/2026-08-11-ooa-earth-emergence-
# probe-findings.md. 3000 m was validated by that probe to restore realistic
# Rock coverage (including obsidian-bearing Rock near the East African Rift
# cradle) without over-mountainizing the whole map. Overridable via
# --elev-ceiling for retuning.
ELEV_CEILING_M = 3000.0


def write_u8(name, grid):
    assert len(grid) == RES * RES, f"{name}: {len(grid)} != {RES * RES}"
    b = bytes(max(0, min(255, round(v * 255))) for v in grid)
    (OUT / f"{name}.u8").write_bytes(b)
    print(f"wrote {name}.u8 ({len(b)} bytes)")


def _require_real_deps():
    try:
        import numpy  # noqa: F401
    except ImportError:
        sys.exit("--source real requires numpy: pip install numpy")
    try:
        import tifffile  # noqa: F401
    except ImportError:
        sys.exit(
            "--source real requires tifffile (+ imagecodecs for LZW/deflate "
            "float TIFF support): pip install tifffile imagecodecs\n"
            "(plain PIL cannot reliably decode these 32-bit-float, "
            "LZW/deflate-compressed GeoTIFFs -- it silently returns NaN or "
            "refuses very large images as a decompression-bomb guard)"
        )
    try:
        from PIL import Image  # noqa: F401
    except ImportError:
        sys.exit("--source real requires Pillow (for resampling): pip install Pillow")


def _load_raster(path):
    """Load a 2D float32 array from a GeoTIFF.

    Uses tifffile rather than PIL. Verified empirically against the actual
    NOAA ETOPO2022 / NASA NEO rasters this scenario uses: PIL's TIFF decoder
    mis-decodes the 32-bit-float, LZW-compressed NEO rasters into an
    all-NaN array, and refuses to even open the larger deflate-compressed
    ETOPO2022 elevation raster (DecompressionBombError). tifffile decodes
    both correctly. PIL is still used afterwards, but only to resample an
    already-correctly-decoded numpy array -- never to decode the TIFF
    itself.
    """
    import tifffile

    return tifffile.imread(str(path)).astype("float32")


def _resample(arr, res=RES):
    """Bilinear-resample a 2D float32 array to res x res via PIL, operating
    on the decoded numpy array (see _load_raster for why TIFF decoding
    itself does not go through PIL)."""
    import numpy as np
    from PIL import Image

    img = Image.fromarray(arr.astype("float32"), mode="F")
    return np.asarray(img.resize((res, res), Image.BILINEAR), dtype="float32")


def _monthly_mean(paths, is_fill):
    """Average N monthly rasters (same grid) into one full-resolution
    mean-annual field, masking per-pixel fill/no-data values before
    averaging so a single bad month's ocean sentinel doesn't pollute the
    mean. `is_fill(stacked_array)` returns a boolean mask of invalid cells
    (in addition to NaN, which is always treated as invalid).

    Returns (mean, valid_mask) where valid_mask is False for cells that
    had zero valid observations across all months (e.g. genuine ocean
    no-data sentinel in every month).
    """
    import numpy as np

    stack = np.stack([_load_raster(p) for p in paths])
    invalid = np.isnan(stack) | is_fill(stack)
    stack = np.where(invalid, np.nan, stack)
    with np.errstate(invalid="ignore"):
        mean = np.nanmean(stack, axis=0)
    valid_mask = ~np.isnan(mean)
    return mean, valid_mask


def _normalize_elevation(elev_m, ceiling_m):
    """Map raw elevation (meters, negative = ocean depth) to [0,1].

    0 m -> SEA_LEVEL_NORM (unchanged, matches biome::SEA_LEVEL). Land
    stretches SEA_LEVEL_NORM -> 1.0 over `ceiling_m` meters (a contrast knob,
    see ELEV_CEILING_M above -- NOT the literal highest point on Earth); land
    elevation at or above the ceiling clips to 1.0. Ocean stretches
    SEA_LEVEL_NORM -> 0.0 over the observed real-world min (~-11000 m,
    Mariana Trench), unchanged by the ceiling knob.
    """
    import numpy as np

    return np.where(
        elev_m >= 0,
        SEA_LEVEL_NORM + (elev_m / ceiling_m) * (1.0 - SEA_LEVEL_NORM),
        SEA_LEVEL_NORM * (1.0 + np.clip(elev_m, -11000, 0) / 11000.0),
    ).clip(0, 1)


def build_elevation_only(elev_path, ceiling_m):
    """Build and write ONLY elevation.u8 from an elevation GeoTIFF, skipping
    temperature/precipitation entirely -- no NEO monthly rasters needed. Used
    to retune the elevation normalization without re-downloading/re-averaging
    the (unrelated, unchanged) temperature and precip sources."""
    elev_m = _load_raster(elev_path)
    elev_m = _resample(elev_m)
    en = _normalize_elevation(elev_m, ceiling_m)
    write_u8("elevation", en.flatten().tolist())


def build_real(elev_path, temp_paths, precip_paths, ceiling_m=ELEV_CEILING_M):
    import numpy as np

    elev_m = _load_raster(elev_path)

    # NASA NEO MODIS land-surface-temperature: ocean/no-data cells are a
    # sentinel of 99999. (Land cells occasionally saturate at exactly
    # -25.0C, a real -- if clipped -- cold reading, not a fill value; only
    # 99999 is treated as invalid here.)
    temp_c, temp_valid = _monthly_mean(temp_paths, is_fill=lambda a: a >= 90000.0)
    # NASA NEO GPM IMERG precipitation: no explicit sentinel observed in
    # practice, but a thin band of NaN appears at the extreme poles (outside
    # the ~60S..60N native IMERG coverage) -- NaN is already masked above.
    precip_mm, precip_valid = _monthly_mean(precip_paths, is_fill=lambda a: a <= -1.0)

    # Cells with no valid observation in ANY month (persistent ocean/no-data
    # for temperature; the handful of polar-edge rows for precip) get a
    # sensible constant instead of the fill sentinel, so the bilinear
    # resample near coastlines and poles doesn't get dragged toward a
    # garbage value (e.g. 99999 bleeding a "boiling hot" reading into
    # nearby coastal land pixels).
    temp_c = np.where(temp_valid, temp_c, 15.0)  # ~global mean sea-surface temp, deg C
    precip_mm = np.where(precip_valid, precip_mm, 0.0)  # polar ice-cap edge rows only

    elev_m = _resample(elev_m)
    temp_c = _resample(temp_c)
    precip_mm = _resample(precip_mm)

    # Elevation: 0 m -> SEA_LEVEL_NORM; deep ocean -> ~0.05; land >= ceiling_m -> 1.0.
    en = _normalize_elevation(elev_m, ceiling_m)
    tn = ((temp_c + 40.0) / 80.0).clip(0, 1)  # -40..40 C -> 0..1
    pn = (np.log1p(precip_mm.clip(0, None)) / np.log1p(4000.0)).clip(0, 1)
    for name, arr in (("elevation", en), ("temperature", tn), ("precip", pn)):
        write_u8(name, arr.flatten().tolist())


# --- synthetic fallback (stdlib only, no network) -------------------------

# Coarse land/ocean approximation as latitude/longitude bounding boxes
# (deliberately crude -- this path exists only as a self-contained,
# deterministic fallback when real rasters are unavailable; the checked-in
# assets are built from --source real, see README.md).
_LAND_BOXES = [
    (-35, 38, -18, 52),  # Africa
    (10, 82, -170, -50),  # North America (incl. loose Central America)
    (-56, 13, -82, -34),  # South America
    (35, 82, -12, 180),  # Eurasia (wraps to antimeridian on the east edge)
    (-45, -10, 112, 154),  # Australia
    (-90, -60, -180, 180),  # Antarctica
]


def _is_land(lat, lon):
    for lat0, lat1, lon0, lon1 in _LAND_BOXES:
        if lat0 <= lat <= lat1 and lon0 <= lon <= lon1:
            return True
    return False


def build_synthetic():
    import math

    elev, temp, precip = [], [], []
    for row in range(RES):
        v = row / RES  # 0=north .. 1=south
        lat = 90.0 - v * 180.0
        t = max(0.0, min(1.0, 1.0 - abs(lat) / 90.0))  # hot equator, cold poles
        # wet equator, dry ~25 deg, wet temperate, dry poles
        p = max(0.0, min(1.0, 0.5 + 0.5 * math.cos(math.radians(lat) * 3.0)))
        for col in range(RES):
            lon = -180.0 + (col / RES) * 360.0
            is_land = _is_land(lat, lon)
            elev.append(SEA_LEVEL_NORM + 0.25 if is_land else 0.15)
            temp.append(t)
            precip.append(p if is_land else min(1.0, p + 0.2))
    write_u8("elevation", elev)
    write_u8("temperature", temp)
    write_u8("precip", precip)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", choices=["real", "synthetic"], required=True)
    ap.add_argument("--elev", help="ETOPO (or similar) elevation GeoTIFF, meters")
    ap.add_argument(
        "--temp",
        nargs="+",
        help="12 monthly land-surface-temperature GeoTIFFs (Jan..Dec), degrees C",
    )
    ap.add_argument(
        "--precip",
        nargs="+",
        help="12 monthly precipitation GeoTIFFs (Jan..Dec), mm",
    )
    ap.add_argument(
        "--elev-only",
        action="store_true",
        help=(
            "Build and write ONLY elevation.u8 from --elev, skipping "
            "temperature/precip entirely (no --temp/--precip needed, no NEO "
            "re-download). Use this to retune the elevation ceiling without "
            "touching the unrelated, unchanged temperature/precip assets."
        ),
    )
    ap.add_argument(
        "--elev-ceiling",
        type=float,
        default=ELEV_CEILING_M,
        help=(
            "Land-side elevation normalization ceiling in meters (see "
            f"ELEV_CEILING_M in this file for rationale; default {ELEV_CEILING_M:.0f})."
        ),
    )
    a = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    if a.source == "real":
        if not a.elev:
            ap.error("--source real requires --elev")
        _require_real_deps()
        if a.elev_only:
            build_elevation_only(a.elev, a.elev_ceiling)
        else:
            if not (a.temp and a.precip):
                ap.error("--source real requires --temp (12 files), --precip (12 files) unless --elev-only")
            build_real(a.elev, a.temp, a.precip, a.elev_ceiling)
    else:
        build_synthetic()
