"""Shared helpers for the LayeredMap reference scripts.

Import this module FIRST (before pyproj / rasterio / pyogrio): it pins the
PROJ / GDAL data directories to the running interpreter's environment so a
foreign PROJ install on PATH (e.g. an anaconda base env) cannot leak in, turns
off PROJ network access, and fixes the timestamps GDAL would otherwise write
into GeoPackages, so outputs are byte-for-byte reproducible.
"""

from __future__ import annotations

import csv
import io
import json
import math
import os
import sys
import zipfile
from pathlib import Path

_prefix = Path(sys.prefix)
os.environ["PROJ_DATA"] = str(_prefix / "share" / "proj")
os.environ["PROJ_LIB"] = str(_prefix / "share" / "proj")
os.environ["GDAL_DATA"] = str(_prefix / "share" / "gdal")
os.environ.pop("GDAL_DRIVER_PATH", None)
os.environ["PROJ_NETWORK"] = "OFF"
os.environ["GDAL_PAM_ENABLED"] = "NO"  # never write .aux.xml side-cars
os.environ["OGR_CURRENT_DATE"] = "2026-01-01T00:00:00.000Z"  # GPKG last_change
os.environ["OGR_SQLITE_JOURNAL"] = "OFF"

REPO = Path(__file__).resolve().parent.parent
FIX = REPO / "fixtures"

FIXED_ZIP_TIME = (2026, 1, 1, 0, 0, 0)


def fmt(x, sig: int = 12) -> str:
    """Deterministic number formatting; None/NaN -> empty string."""
    if x is None:
        return ""
    if isinstance(x, (bool,)):
        return "true" if x else "false"
    if isinstance(x, int):
        return str(x)
    try:
        xf = float(x)
    except (TypeError, ValueError):
        return str(x)
    if math.isnan(xf):
        return ""
    if xf == int(xf) and abs(xf) < 1e15:
        return str(int(xf))
    return format(xf, f".{sig}g")


def write_csv(path: Path, header: list[str], rows, encoding: str = "utf-8",
              delimiter: str = ",") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    buf = io.StringIO()
    w = csv.writer(buf, delimiter=delimiter, lineterminator="\n")
    w.writerow(header)
    for r in rows:
        w.writerow([v if isinstance(v, str) else fmt(v) for v in r])
    path.write_bytes(buf.getvalue().encode(encoding))


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def write_json(path: Path, obj) -> None:
    write_text(path, json.dumps(obj, ensure_ascii=False, indent=2, sort_keys=False) + "\n")


def write_geojson(path: Path, features: list[dict], name: str | None = None) -> None:
    """One feature per line, UTF-8, no CRS member (RFC 7946 => WGS 84 lon/lat)."""
    head = {"type": "FeatureCollection"}
    if name:
        head["name"] = name
    lines = [json.dumps(head, ensure_ascii=False)[:-1] + ', "features": [']
    for i, f in enumerate(features):
        s = json.dumps(f, ensure_ascii=False, separators=(", ", ": "))
        lines.append(s + ("," if i < len(features) - 1 else ""))
    lines.append("]}")
    write_text(path, "\n".join(lines) + "\n")


def patch_dbf(path: Path, ldid: int | None = None) -> None:
    """Set the dBASE 'last update' date to 2026-01-01 (bytes 1-3) for reproducible
    bytes, and optionally overwrite the language-driver id (byte 29)."""
    b = bytearray(path.read_bytes())
    b[1], b[2], b[3] = 126, 1, 1  # 1900 + 126 = 2026
    if ldid is not None:
        b[29] = ldid
    path.write_bytes(bytes(b))


def deterministic_zip(out: Path, members: list[tuple[str, bytes]]) -> None:
    """Write a zip with fixed timestamps / permissions, members in given order."""
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for name, data in members:
            zi = zipfile.ZipInfo(name, date_time=FIXED_ZIP_TIME)
            zi.compress_type = zipfile.ZIP_DEFLATED
            zi.external_attr = (0o644 << 16) if not name.endswith("/") else ((0o755 << 16) | 0x10)
            zi.create_system = 3
            z.writestr(zi, data)


def rezip_fixed(path: Path) -> None:
    """Rewrite an existing zip container (e.g. .xlsx) with fixed member timestamps."""
    with zipfile.ZipFile(path) as z:
        members = [(i.filename, z.read(i.filename)) for i in z.infolist()]
    deterministic_zip(path, members)


def geod():
    import pyproj
    return pyproj.Geod(ellps="WGS84")


def cell_area_rows(geod_, west: float, east: float, lats_top_to_bottom: list[float],
                   n_dense: int = 2000) -> list[float]:
    """Geodesic (WGS84 ellipsoid) area of one grid cell per row.

    A cell is bounded by two meridians and two PARALLELS. Parallels are not
    geodesics, so the parallel edges are densified with n_dense points before
    pyproj.Geod's polygon area (whose edges are geodesics) is evaluated. Area
    does not depend on longitude, so one value per row suffices.
    """
    out = []
    for top, bottom in zip(lats_top_to_bottom[:-1], lats_top_to_bottom[1:]):
        lons_s = [west + (east - west) * k / n_dense for k in range(n_dense + 1)]
        lons = lons_s + lons_s[::-1]
        lats = [bottom] * (n_dense + 1) + [top] * (n_dense + 1)
        a, _ = geod_.polygon_area_perimeter(lons, lats)
        out.append(abs(a))
    return out


def zone_area_exact(lat1: float, lat2: float, dlon_deg: float,
                    a: float = 6378137.0, f: float = 1 / 298.257223563) -> float:
    """Closed-form ellipsoidal area between two parallels over dlon degrees
    (authalic formula). Used to cross-check the Geod-based cell areas."""
    e2 = f * (2 - f)
    e = math.sqrt(e2)
    b2 = (a * (1 - f)) ** 2

    def q(phi):
        s = math.sin(math.radians(phi))
        return s / (1 - e2 * s * s) + (1 / (2 * e)) * math.log((1 + e * s) / (1 - e * s))

    return abs(b2 / 2 * math.radians(dlon_deg) * (q(lat2) - q(lat1)))
