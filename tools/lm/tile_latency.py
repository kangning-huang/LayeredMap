#!/usr/bin/env python3
"""WP0.6: measure basemap reachability and latency from where students are.

Reads a PMTiles archive the way the app does — HTTP range requests — and reports time to the
first tile and the 50th/95th-percentile latency of tile-sized reads. Standard library only, so it
runs on any Mac:

    python3 tools/lm/tile_latency.py URL [URL ...] [--n 60] [--label campus]

Run it on each connection the plan names (campus network, home broadband, mobile data), once per
candidate host, and paste the printed table into docs/lm/basemap-hosting.md. Results are also
appended to tools/lm/tile_latency_results.csv.

Gate 0 exit criterion: at least one host reaches a 95th-percentile tile latency ≤ 500 ms from
campus.
"""
import csv, random, ssl, statistics, sys, time, urllib.request, datetime, pathlib, struct

def ranged(url, start, length, timeout=15):
    req = urllib.request.Request(url, headers={"Range": f"bytes={start}-{start + length - 1}", "User-Agent": "LayeredMap-latency/1"})
    t = time.perf_counter()
    with urllib.request.urlopen(req, timeout=timeout, context=ssl.create_default_context()) as r:
        first = r.read(1)
        ttfb = time.perf_counter() - t
        body = first + r.read()
        total = time.perf_counter() - t
        return r.status, ttfb, total, body

def measure(url, n):
    # The 127-byte PMTiles v3 header gives the tile data section; sample reads from inside it.
    status, ttfb, total, header = ranged(url, 0, 16384)
    if header[:7] != b"PMTiles":
        raise SystemExit(f"{url}: not a PMTiles archive (status {status})")
    data_off, data_len = struct.unpack_from("<QQ", header, 56)
    first_tile = total
    rng = random.Random(42)
    lat = []
    errors = 0
    for _ in range(n):
        off = data_off + rng.randrange(max(1, data_len - 65536))
        try:
            _, _, t, _ = ranged(url, off, rng.choice([8192, 16384, 32768]))
            lat.append(t)
        except Exception:
            errors += 1
    lat.sort()
    p = lambda q: lat[min(len(lat) - 1, int(round((len(lat) - 1) * q)))] if lat else float("nan")
    return {"first_ms": first_tile * 1000, "p50_ms": p(0.5) * 1000, "p95_ms": p(0.95) * 1000, "errors": errors, "n": n}

def main():
    args = sys.argv[1:]
    n, label, urls = 60, "unlabelled", []
    while args:
        a = args.pop(0)
        if a == "--n":
            n = int(args.pop(0))
        elif a == "--label":
            label = args.pop(0)
        else:
            urls.append(a)
    if not urls:
        print(__doc__)
        return 2
    out = pathlib.Path(__file__).with_name("tile_latency_results.csv")
    new = not out.exists()
    with out.open("a", newline="") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["utc", "label", "url", "first_ms", "p50_ms", "p95_ms", "errors", "n"])
        print(f"| connection | host | first tile (ms) | p50 (ms) | p95 (ms) | errors |")
        print(f"|---|---|---|---|---|---|")
        for u in urls:
            r = measure(u, n)
            print(f"| {label} | {u} | {r['first_ms']:.0f} | {r['p50_ms']:.0f} | {r['p95_ms']:.0f} | {r['errors']}/{n} |")
            w.writerow([datetime.datetime.utcnow().isoformat(timespec="seconds"), label, u, f"{r['first_ms']:.0f}", f"{r['p50_ms']:.0f}", f"{r['p95_ms']:.0f}", r["errors"], n])
    return 0

if __name__ == "__main__":
    sys.exit(main())
