# Draft: GitHub Discussion for opengeos/RustGIS (WP0.1)

*For the owner to post (it is public and in your name). Edit freely.*

**Title:** A second, map-first front end on RustGIS's engine crates — would pieces of it be welcome upstream?

Hi, and thank you for RustGIS. I teach at NYU Shanghai and am building **LayeredMap**, a
deliberately small, map-first desktop app for novice students: open a file, click something, see
where it stands ("3rd hottest of 17 districts"), and ask one of three suggested questions with a
preview and a plain-language recipe. It is a separate egui front end on top of the `geom`, `data`,
`map`, `tiles`, `render`, `analysis` and `engine` crates; it does not replace the ribbon UI.

For now it lives in a fork with all product code in new crates, and I would like to keep the fork
thin. Two questions:

1. **An extracted map-view crate.** Any second front end needs the map widget (camera, gestures,
   texture caching, tile placeholders, hit testing). Would you welcome a PR that extracts the map
   view from `ui-egui` into its own crate that `ui-egui` then uses?
2. **A second front end upstream.** Alternatively, would you consider hosting a second front end
   (as another app under `apps/`) in the main repository?

Later we expect to offer back a GCJ-02/BD-09 coordinate conversion and a fractional-coverage,
area-weighted zonal summary, each with reference tests against GeoPandas/exactextract/pyproj.

Happy to adapt to whatever shape suits the project.
