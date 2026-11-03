#!/usr/bin/env python3
"""Check every catalogue link and say what it really is.

The catalogue stores links, never data — the app downloads on tap. So the only thing that has to
be true of an entry is that its link works, and the only thing the app needs to know is whether
the link hands back a *file* (tap Add, it goes on the map) or a *page* (tap through and pick a
download yourself).

Neither question can be answered from the URL. Some direct files have no extension at all
(Dataverse's `/api/access/datafile/3048165` serves `application/zipped-shapefile`), and some
URLs ending in a known extension serve an HTML landing page. HEAD is no good either: several of
these hosts answer 403 to HEAD and 200 to GET. So: GET with a browser's user agent, follow
redirects, read the content type, and only download the first few bytes.
"""
import concurrent.futures
import json
import pathlib
import subprocess
import sys

CATALOG = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else
                       "spikes/ios-companion-alt-d/ios/NimbleMapAltD/catalog.json")
UA = ("Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 "
      "(KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1")

# Content types that mean "this is the data", not "this is a web page about the data".
FILE_TYPES = ("application/json", "application/geo+json", "application/vnd.geo+json", "text/csv",
              "application/zip", "application/zipped-shapefile", "application/x-zip",
              "application/octet-stream", "image/tiff", "application/x-netcdf",
              "application/netcdf", "application/geopackage+sqlite3", "application/vnd.google-earth",
              "text/tab-separated-values", "application/gpx", "application/xml", "text/plain")


def probe(item):
    r = subprocess.run(
        ["curl", "-sL", "-A", UA, "--max-time", "30", "--max-filesize", "4000",
         "-o", "/dev/null", "-w", "%{http_code}\t%{content_type}\t%{size_download}\t%{url_effective}",
         item["url"]],
        capture_output=True, text=True)
    parts = (r.stdout or "\t\t\t").split("\t")
    code, ctype = parts[0] or "000", (parts[1] or "").lower()
    # curl aborts the transfer once --max-filesize is passed, which is a success for us.
    alive = code.startswith(("2", "3")) or (code == "000" and ctype)
    is_file = any(t in ctype for t in FILE_TYPES) and "text/html" not in ctype
    return item, code, ctype.split(";")[0], alive, is_file


items = json.loads(CATALOG.read_text())
with concurrent.futures.ThreadPoolExecutor(10) as ex:
    results = list(ex.map(probe, items))

kept, dropped = [], []
for item, code, ctype, alive, is_file in results:
    if not alive:
        dropped.append((item["title"], code, ctype or "no answer"))
        continue
    item["kind"] = "file" if is_file else "page"
    kept.append(item)
    print(f'{"FILE" if is_file else "PAGE"}  {code:>3}  {ctype[:34]:34}  {item["title"][:44]}')

CATALOG.write_text(json.dumps(kept, indent=1, ensure_ascii=False) + "\n")
files = sum(1 for i in kept if i["kind"] == "file")
print(f'\n{len(kept)} kept ({files} tap-to-add, {len(kept) - files} tap-through), {len(dropped)} dropped')
for t, c, ct in dropped:
    print(f"  dropped: {t} ({c} {ct})")
