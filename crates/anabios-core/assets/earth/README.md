# Earth map data source (out-of-africa-earth scenario)

Task 0 spike outcome (2026-08-11): **Path A — real public-domain rasters.** All three
source rasters below were verified reachable and fetchable via plain `curl` from this
environment (full HTTP download, not just a redirect/landing page — each file's magic
bytes were checked with `file` after download to confirm it is a genuine GeoTIFF, not an
HTML error page). No raw downloads are committed to this repo; only this README records
the decision. The actual 256×256 `.u8` assets are produced by a later task
(`scripts/build_earth_map.py`) from these sources.

## Elevation — NOAA ETOPO 2022, 60 arc-second bedrock elevation GeoTIFF

- URL: `https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_bed_elev_gtif/ETOPO_2022_v1_60s_N90W180_bed.tif`
  (landing page: https://www.ncei.noaa.gov/products/etopo-global-relief-model ; DOI
  10.25921/fd45-gt74)
- License: Public domain — NOAA/NCEI (U.S. government work).
- Original resolution: 60 arc-second, single global GeoTIFF, 21600×10800 px
  (verified: downloaded byte range decodes as a valid TIFF, `21600×10800`, 32 bpp,
  deflate-compressed, origin N90 W180 — i.e. row 0 = north, col 0 = west, matching the
  target projection). Full file is ~478 MB.
- Alternate variant at the same resolution/URL prefix: `60s_surface_elev_gtif` (ice
  surface elevation instead of bedrock) — not fetched in this spike, but reachable via
  the same server if Task 1 prefers ice-surface over bedrock.
- Normalization (current): raw elevation is meters relative to sea level (negative =
  ocean depth). Map to `[0,1]` such that **0 m → 0.35** (matching
  `biome::SEA_LEVEL = 0.35`); land stretches `0.35 → 1.0` over a **contrast ceiling of
  `ELEV_CEILING_M = 3000 m`** (land at or above 3000 m clips to 1.0), and ocean
  stretches `0.35 → 0.0` over the observed min (~-11000 m, Mariana Trench), then
  quantize to `u8`. Retune via `build_earth_map.py --elev-ceiling`.
  - **Why 3000 m, not the literal 8849 m (Everest):** an 8849 m ceiling compressed
    essentially all land into normalized `0.35–0.50`, so `biome::ROCK_LINE = 0.78`
    (≈ 5850 m under that mapping) was never reached — the map generated **0% Rock**,
    hence no obsidian, hence the invention tech-tree (rooted at `stone_tools`, which
    needs obsidian) was materially impossible. The 3000 m contrast ceiling restores
    ~1% Rock globally, including obsidian-bearing Rock in the **East African Rift**
    (~23 sim-units from the cradle at res 256 / world 4096). See
    `docs/superpowers/specs/2026-08-11-ooa-earth-emergence-probe-findings.md`. Only
    `elevation.u8` is affected; `temperature.u8` / `precip.u8` are unchanged, and the
    land/water coastline is unchanged (only the land-elevation distribution shifts).

## Temperature — NASA NEO MOD_LSTD_M / MOD_LSTD_CLIM_M, MODIS Land Surface Temperature (Day), monthly, floating-point GeoTIFF

- URL pattern (single month, verified): `https://neo.gsfc.nasa.gov/archive/geotiff.float/MOD_LSTD_M/MOD_LSTD_M_2026-07.FLOAT.TIFF`
- Recommended for Task 1 (multi-year climatological mean rather than one recent year,
  same directory structure, not individually re-verified but same server/pattern):
  `https://neo.gsfc.nasa.gov/archive/geotiff.float/MOD_LSTD_CLIM_M/MOD_LSTD_CLIM_M_2001-{01..12}.FLOAT.TIFF`
  — average the 12 monthly climatology files to get a mean-annual field.
- Dataset landing page: https://neo.gsfc.nasa.gov/view.php?datasetId=MOD_LSTD_M
- License: Public domain — NASA Earth Observations (NEO); "freely available for public
  use without further permission," attribution to NASA Earth Observations requested.
- Original resolution: 0.1°, 3600×1800 px global grid (verified: downloaded file
  decodes as valid TIFF, 3600×1800, 32 bpp float, LZW-compressed, BlackIsZero — i.e.
  real physical values in °C, not a color-palette image).
- Planned normalization: −40 °C .. 40 °C → 0..1, linear, clamped, then quantize to `u8`.

## Precipitation — NASA NEO GPM_3IMERGM, GPM IMERG monthly precipitation, floating-point GeoTIFF

- URL pattern (verified, single month): `https://neo.gsfc.nasa.gov/archive/geotiff.float/GPM_3IMERGM/3B-MO-L.GIS.IMERG.20260701.V07C.tif`
- Dataset landing page: https://neo.gsfc.nasa.gov/view.php?datasetId=GPM_3IMERGM
- License: Public domain — NASA Earth Observations (NEO), same terms as above.
- Original resolution: 0.1°, 3600×1800 px global grid (verified: downloaded file
  decodes as valid TIFF, 3600×1800, 32 bpp float, LZW-compressed, BlackIsZero — real
  physical values in mm, not a color-palette image).
- Task 1 should average 12 monthly files (one calendar year, or ideally a multi-year
  mean if a `_CLIM_` variant is published for this dataset) to get mean-annual
  precipitation before normalizing.
- Planned normalization: 0..~4000 mm annual, log-scaled (`log1p(mm) / log1p(4000)`),
  clamped to `[0,1]`, then quantize to `u8`.

## What was tried

- `curl -sI` HEAD probes and full/range `curl` downloads were run directly from this
  environment's scratch dir against NOAA NCEI/NGDC and NASA NEO servers; outbound HTTPS
  to `ncei.noaa.gov`, `ngdc.noaa.gov`, and `neo.gsfc.nasa.gov` all succeeded.
- Natural Earth (`naturalearthdata.com`) was also tried as an elevation/shaded-relief
  alternative (per the brief); its direct S3/CDN zip URLs returned 403/404 in this
  environment and were dropped once the NOAA ETOPO 2022 source above was confirmed
  working — no need for a second elevation source.
- WorldClim was deliberately not used (CC-BY, not public domain, excluded by the brief).

## Upgrade path

Not applicable — Path A (real rasters) is in use from the start, so there is no
synthetic-to-real swap needed. If a future scenario needs different variables (e.g.
ice-surface elevation instead of bedrock, or a different precipitation climatology),
swapping the source URL above is a drop-in change to `scripts/build_earth_map.py`: the
output `.u8` format and normalization contract are unaffected.

## Task 1 build notes (2026-08-11)

The three checked-in `.u8` assets were generated by `scripts/build_earth_map.py
--source real` from the sources above. Details that affect reproducibility or
interpretation:

- **Elevation variant: bedrock** (`60s_bed_elev_gtif`), as used in the Task 0 spike.
  Consequence: bedrock is isostatically depressed under thick ice sheets, so parts of
  interior Antarctica and Greenland read as *below* `SEA_LEVEL_NORM` (0.35) in the
  256x256 raster even though they are real, walkable (under-ice) land — e.g. a sample
  at (lat=72, lon=-40) in Greenland reads 0.349, just under the sea-level threshold.
  This is expected for the bedrock variant, not an orientation bug; the ice-surface
  variant (`60s_surface_elev_gtif`, same server/resolution) would avoid it if a future
  task needs polar ice caps to read as land.
- **Temperature climatology reachable and used as planned**: all 12
  `MOD_LSTD_CLIM_M_2001-{01..12}.FLOAT.TIFF` files were verified reachable (HTTP 200)
  and downloaded — no fallback to single-month `MOD_LSTD_M` files was needed.
  Per-pixel ocean/no-data sentinel is `99999.0`; a separate, legitimate saturated-cold
  reading of exactly `-25.0` also occurs in real land data (e.g. Antarctic/Greenland
  interior in local winter) and was *not* treated as fill.
- **Precipitation: no `_CLIM_` climatology variant exists for `GPM_3IMERGM`** (checked;
  404). Used the 12 real monthly rasters for calendar year 2023 (`V07B`, the version
  NEO serves for that year) instead, per the README's original fallback plan ("one
  calendar year ... if a `_CLIM_` variant is not published"). No fill sentinel was
  found in the data; a thin band of `NaN` appears in a few rows at the extreme poles
  (outside GPM IMERG's ~60°S..60°N native coverage) and was masked like any other
  invalid value.
- **Monthly averaging**: both temperature and precipitation were averaged at full
  native resolution (3600x1800) across all 12 months, masking invalid/fill cells
  per-pixel-per-month before averaging (`np.nanmean`), then resampled to 256x256.
  Cells with zero valid observations across all 12 months (persistent ocean sentinel
  for temperature; the polar NaN band for precipitation) were filled with a sensible
  constant (15°C / 0mm) before resampling, so the fill values don't bleed into nearby
  coastal/land cells through the bilinear resample.
- **Loader**: `tifffile` (+ `imagecodecs`), not PIL, was used to decode the GeoTIFFs.
  Verified empirically: PIL's TIFF decoder returns an all-`NaN` array for the
  LZW-compressed float32 NEO rasters, and refuses to open the larger
  deflate-compressed ETOPO2022 raster at all (`DecompressionBombError`). PIL is still
  used, but only to bilinear-resample the already-correctly-decoded numpy array to
  256x256, never to decode the TIFF itself.
- **Validation** (see `task-1-report.md`): global land fraction (elevation ≥ 0.35) ≈
  0.281 (target ~0.25-0.35); central Africa (lat=0, lon=20) reads LAND; mid-Pacific
  (lat=0, lon=-150) reads WATER.
