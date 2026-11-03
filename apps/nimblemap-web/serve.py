# The NimbleMap web app's preview server (docs/lm/web-app.md): 127.0.0.1 only, application/wasm, no
# caching, brotli like a CDN, and /basemap/* proxied to Protomaps' daily builds (they send no CORS headers), as the
# Cloudflare Pages Function does in production (apps/nimblemap-web/cloudflare).
# Usage: python3 -I apps/nimblemap-web/serve.py DIST_DIR PORT   (brotli needs the `brotli` CLI)
import http.server, socketserver, sys, functools, re, urllib.request, urllib.error, os, hashlib, subprocess, tempfile
CACHE = os.path.join(tempfile.gettempdir(), 'nimblemap-web-brcache')
root, port = sys.argv[1], int(sys.argv[2])
BUILD = re.compile(r"^/basemap/(\d{8}\.pmtiles)$")
class H(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map,
                      ".wasm": "application/wasm", ".js": "text/javascript", ".webmanifest": "application/manifest+json",
                      ".svg": "image/svg+xml", ".gpkg": "application/geopackage+sqlite3", ".nc": "application/x-netcdf"}
    def end_headers(self):
        self.send_header("Cache-Control", "no-cache")
        self.send_header("X-Content-Type-Options", "nosniff")
        super().end_headers()
    def log_message(self, *a): pass
    def do_GET(self):
        path = self.path.split("?")[0]
        if path.startswith("/basemap/"):
            m = BUILD.match(path)
            if path == "/basemap/builds.json":
                url = "https://build-metadata.protomaps.dev/builds.json"
            elif m:
                url = "https://build.protomaps.com/" + m.group(1)
            else:
                self.send_error(404); return
            req = urllib.request.Request(url, headers={"User-Agent": "NimbleMap-web-preview"})
            if self.headers.get("Range"):
                req.add_header("Range", self.headers["Range"])
            try:
                with urllib.request.urlopen(req, timeout=20) as r:
                    body = r.read(); status = r.status; hdrs = r.headers
            except urllib.error.HTTPError as e:
                self.send_error(e.code); return
            except Exception:
                self.send_error(502); return
            self.send_response(status)
            for h in ("Content-Type", "Content-Range", "ETag", "Last-Modified"):
                if hdrs.get(h): self.send_header(h, hdrs[h])
            self.send_header("Content-Length", str(len(body)))
            self.end_headers(); self.wfile.write(body); return
        # Compressed like a CDN would (brotli, cached by file and mtime) for .wasm/.js/.html/.json.
        fs = self.translate_path(path if path != "/" else "/index.html")
        if "br" in self.headers.get("Accept-Encoding", "") and os.path.isfile(fs) and fs.rsplit(".", 1)[-1] in ("wasm", "js", "html", "json", "geojson", "csv", "svg", "tsv", "otf") and not self.headers.get("Range"):
            st = os.stat(fs)
            key = os.path.join(CACHE, hashlib.sha1(f"{fs}{st.st_mtime_ns}".encode()).hexdigest() + ".br")
            if not os.path.exists(key):
                os.makedirs(CACHE, exist_ok=True)
                q = "9" if st.st_size > 4 << 20 else "11"
                subprocess.run(["brotli", "-f", "-q", q, "-o", key, fs], check=True)
            body = open(key, "rb").read()
            self.send_response(200)
            self.send_header("Content-Type", self.guess_type(fs))
            self.send_header("Content-Encoding", "br")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers(); self.wfile.write(body); return
        super().do_GET()
class S(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True
S(("127.0.0.1", port), functools.partial(H, directory=root)).serve_forever()
