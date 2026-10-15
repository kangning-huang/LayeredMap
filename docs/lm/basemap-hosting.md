# Basemaps and reachability (WP0.6)

**Status (2026-10-15):** the spike reads a pinned Protomaps daily planet build
(`https://build.protomaps.com/20261014.pmtiles`) by byte ranges. That is a download host, not a
production tile host. From the reference Mac on 2026-10-15, a 16 KB range read took about 0.55 s.
The remaining WP0.6 work needs you (or a machine in Shanghai) and an account with a host.

## 1. Build the Shanghai extract

With the `pmtiles` CLI (go-pmtiles, BSD-3-Clause):

```bash
pmtiles extract https://build.protomaps.com/20261014.pmtiles shanghai.pmtiles \
  --bbox=119.9,30.0,122.5,32.2 --maxzoom=15
```

The data is OpenStreetMap (ODbL): attribution "© OpenStreetMap contributors" stays visible on the
map and on exports (the spike and the plan's attribution rules already do this).

## 2. Host it on two candidates

| Candidate | Notes |
|---|---|
| A global CDN (e.g. Cloudflare R2 behind a custom domain) | Cheapest; reachability from mainland China varies by network. |
| A host with mainland-China edge nodes, Hong Kong region | Avoids the ICP filing (备案) that a mainland custom domain requires, which takes weeks. |

Both must allow HTTP range requests (the app reads one tile at a time).

## 3. Measure from Shanghai

On each connection — campus network, home broadband, mobile data — run:

```bash
python3 tools/lm/tile_latency.py https://<candidate-A>/shanghai.pmtiles https://<candidate-B>/shanghai.pmtiles --label campus
```

It prints time to the first tile and p50/p95 tile latency, and appends to
`tools/lm/tile_latency_results.csv`. Paste the tables here.

**Gate 0 exit criterion:** at least one host reaches p95 ≤ 500 ms from campus.

| Connection | Host | First tile (ms) | p50 (ms) | p95 (ms) | Errors |
|---|---|---|---|---|---|
| campus | | | | | |
| home broadband | | | | | |
| mobile data | | | | | |

## 4. Offline basemap

Natural Earth 1:50m land, water, boundaries and populated places, packaged as the offline
basemap; builds distributed in China use Natural Earth's China point-of-view boundary files.
RustGIS already draws an offline world outline in both worldviews (Natural Earth's default and
China's point of view, picked by time zone) — see `docs/user-guide/maps.md` › Worldview. Phase 1
(WP1.3) styles it as the "Quiet" basemap; its size is recorded there.
