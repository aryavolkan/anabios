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
- Planned normalization: raw elevation is meters relative to sea level (negative =
  ocean depth). Map to `[0,1]` such that **0 m → 0.35** (matching
  `biome::SEA_LEVEL = 0.35`), with land stretching `0.35 → 1.0` over the observed max
  elevation (~8849 m, Everest) and ocean stretching `0.35 → 0.0` over the observed min
  (~-10935 m, Mariana Trench), then quantize to `u8`.

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
