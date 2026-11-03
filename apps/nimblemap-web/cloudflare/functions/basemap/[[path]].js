// Cloudflare Pages Function: /basemap/* on the NimbleMap web app's own origin, proxied to
// Protomaps' daily planet builds, which answer no cross-origin requests (docs/lm/web-app.md ›
// Hosting). Only the build list and YYYYMMDD.pmtiles byte ranges pass; responses are cached at
// the edge by URL + Range.
const BUILD = /^(\d{8})\.pmtiles$/;

export async function onRequestGet({ request, params, waitUntil }) {
  const name = Array.isArray(params.path) ? params.path.join("/") : String(params.path || "");
  let upstream;
  if (name === "builds.json") upstream = "https://build-metadata.protomaps.dev/builds.json";
  else if (BUILD.test(name)) upstream = "https://build.protomaps.com/" + name;
  else return new Response("not found", { status: 404 });

  const range = request.headers.get("Range");
  if (name !== "builds.json" && !range) return new Response("a byte range is required", { status: 416 });
  const cache = caches.default;
  const key = new Request(new URL(request.url).origin + "/basemap-cache/" + name + "?r=" + encodeURIComponent(range || ""));
  const hit = await cache.match(key);
  if (hit) return hit.headers.get("Content-Range") ? new Response(hit.body, { status: 206, headers: hit.headers }) : hit;

  const resp = await fetch(upstream, { headers: range ? { Range: range } : {} });
  if (!resp.ok) return new Response(null, { status: resp.status });
  const headers = new Headers();
  for (const h of ["Content-Type", "Content-Range", "Content-Length", "ETag", "Last-Modified"]) {
    const v = resp.headers.get(h);
    if (v) headers.set(h, v);
  }
  headers.set("Cache-Control", name === "builds.json" ? "public, max-age=3600" : "public, max-age=86400");
  // The edge cache stores 200s only: a 206 is kept under its own key as a 200 and served back with
  // its Content-Range.
  const body = await resp.arrayBuffer();
  const out = new Response(body, { status: resp.status, headers });
  waitUntil(cache.put(key, new Response(body, { status: 200, headers })));
  return out;
}
