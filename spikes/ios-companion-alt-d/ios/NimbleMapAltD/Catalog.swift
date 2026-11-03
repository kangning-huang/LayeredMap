import Foundation

/// Searching the open web for data, from the Add-data sheet.
///
/// The persona study's owner asked whether the search box could reach the web, appending the
/// formats the app opens to whatever the person typed. Tested against real engines (24 searches,
/// 2026-11-03), that works for abundant governmental subjects — "bike lanes" went from 0 of 9
/// openable results to 8 of 9 once "shapefile" was appended — but it fails badly elsewhere, and
/// the failures are the loud kind:
///
/// - Short format words collide with product codes. `Walmart gpkg` returns nine walmart.com pages
///   for Garbage Pail Kids cards; `Walmart kml` returns a Cuisinart KML-KO3B kitchen scale.
/// - OR-ing formats makes the engine think the question is *about* formats: seven of ten results
///   for `Walmart (shapefile OR geojson OR gpkg)` were format-converter tools.
/// - `filetype:` is not honoured at all — `Walmart filetype:geojson` returns Wikipedia's GeoJSON
///   article and a walmart.com sitemap.
///
/// So the default here is **not** a web search. Open-data catalogues have real APIs that return the
/// format and a direct file URL as data, need no key, and cost nothing. They also answer honestly:
/// Socrata returns zero results for "Walmart", which is true, where a web search returns ten
/// confident-looking shopping pages. A web search stays available as a clearly-labelled fallback
/// for the subjects catalogues genuinely don't hold.
enum Catalog {
    /// One result, already reduced to what a row needs and what the importer can use.
    struct Item: Identifiable, Hashable {
        let id: String
        let title: String
        let blurb: String
        /// The catalogue's own word for the format ("GeoJSON"), never guessed from the URL.
        let format: String
        /// Where it came from, for the row's second line ("data.cityofnewyork.us").
        let source: String
        let url: URL
        /// Whether the link hands back the data or a page to pick a download from. Checked by
        /// content type when the catalogue was built, never guessed from the URL: Dataverse serves
        /// `application/zipped-shapefile` from a URL with no extension at all, and plenty of URLs
        /// ending in `.csv` serve an HTML landing page.
        var kind: Kind = .file
        /// Only the bundled catalogue has these; they are what offline search matches on.
        var keywords: [String] = []
        var size = ""

        enum Kind: String, Hashable {
            /// Tap Add and it goes on the map.
            case file
            /// Tap through, pick a download there.
            case page
        }
    }

    // MARK: The bundled catalogue

    /// Datasets shipped with the app, lifted from the site's data page and checked when the
    /// catalogue was built. Searching these costs nothing: no network, no waiting, no model, and
    /// it works on a train. Most searches should end here, which is the point — the online
    /// catalogues and the model are for what this list does not hold.
    static let bundled: [Item] = {
        guard let url = Bundle.main.url(forResource: "catalog", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let rows = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return [] }
        return rows.compactMap { row in
            guard let id = row["id"] as? String, let title = row["title"] as? String,
                  let link = row["url"] as? String, let file = URL(string: link)
            else { return nil }
            return Item(
                id: "bundled:\(id)",
                title: title,
                blurb: (row["blurb"] as? String) ?? "",
                format: (row["format"] as? String) ?? "",
                source: (row["source"] as? String) ?? "",
                url: file,
                kind: Item.Kind(rawValue: (row["kind"] as? String) ?? "file") ?? .file,
                keywords: (row["keywords"] as? [String]) ?? [],
                size: (row["size"] as? String) ?? ""
            )
        }
    }()

    /// Offline search over the bundled catalogue. Every word typed must appear somewhere in the
    /// entry — title, what it is, who publishes it, its section, or its keywords — so "roman
    /// roads" finds the Roman Empire roads and "roads" alone does not drown in everything else.
    /// A title match sorts first, because someone typing "earthquakes" means the one called that.
    static func searchBundled(_ text: String) -> [Item] {
        let words = text.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init)
        guard !words.isEmpty else { return [] }
        return bundled.compactMap { item -> (Item, Int)? in
            let title = item.title.lowercased()
            let haystack = ([title, item.blurb, item.source, item.format] + item.keywords)
                .joined(separator: " ").lowercased()
            guard words.allSatisfy({ haystack.contains($0) }) else { return nil }
            let score = words.contains(where: { title.contains($0) }) ? 0 : 1
            return (item, score)
        }
        .sorted { $0.1 < $1.1 }
        .map(\.0)
    }

    /// Where to look. Three, because one query cannot serve both "bike lanes" (abundant, published
    /// by cities) and "Walmart" (commercial, mostly not free).
    enum Scope: String, CaseIterable, Identifiable {
        case portals
        case cityMaps
        case anyFile

        var id: String { rawValue }

        /// What the chip says. No "query", "operator", "API" or "filetype" anywhere a person reads.
        var label: String {
            switch self {
            case .portals: return "Open data portals"
            case .cityMaps: return "City & county maps"
            case .anyFile: return "Any map file"
            }
        }
    }

    /// Everything found for `text`, bundled catalogue first.
    ///
    /// Order matters more than it looks: the bundled list answers instantly, offline and free, so
    /// it is tried before anything that costs a request. The model is asked for better search
    /// words only when the bundled list came back empty — if "earthquakes" already matched, there
    /// is nothing to improve and no reason to spend anything.
    static func search(_ text: String, scope: Scope) async -> [Item] {
        let q = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard q.count >= 2 else { return [] }

        // Catalogue indexes are keyword matches, so a typed sentence finds nothing. Ask a model
        // for better words only when the bundled catalogue already came up short — if
        // "earthquakes" matched offline there is nothing to improve and nothing to spend.
        // `let`, not `var`: the two calls below capture it concurrently, which Swift 6 rejects
        // for a mutable binding.
        let terms: String
        if searchBundled(q).isEmpty, Assist.available, let better = await Assist.searchTerms(for: q) {
            terms = better
        } else {
            terms = q
        }

        async let socrata = scope == .cityMaps ? [] : fromSocrata(terms)
        async let hub = scope == .portals ? [] : fromArcGISHub(terms)
        // The bundled matches are already on screen; this is only what the web adds to them.
        let alreadyShown = Set(searchBundled(q).map(\.url.absoluteString))
        var seen = alreadyShown
        return ((await socrata) + (await hub)).filter { seen.insert($0.url.absoluteString).inserted }
    }

    // MARK: Socrata

    /// Socrata's Discovery API: no key, no cost, and it labels each column's type, so a dataset
    /// with no geometry can be dropped before anyone taps it.
    private static func fromSocrata(_ q: String) async -> [Item] {
        var c = URLComponents(string: "https://api.us.socrata.com/api/catalog/v1")
        c?.queryItems = [
            URLQueryItem(name: "q", value: q),
            URLQueryItem(name: "only", value: "dataset"),
            URLQueryItem(name: "limit", value: "20"),
        ]
        guard let url = c?.url, let root = await json(url) else { return [] }
        let results = (root["results"] as? [[String: Any]]) ?? []
        return results.compactMap { row -> Item? in
            guard let r = row["resource"] as? [String: Any],
                  let id = r["id"] as? String,
                  let name = r["name"] as? String,
                  let domain = (row["metadata"] as? [String: Any])?["domain"] as? String
            else { return nil }
            // Only datasets that actually hold places: a Socrata table of budget lines is
            // honestly labelled and completely useless on a map.
            let types = Set((r["columns_datatype"] as? [String]) ?? [])
            guard !types.isDisjoint(with: geometryTypes) else { return nil }
            guard let file = URL(string: "https://\(domain)/resource/\(id).geojson") else { return nil }
            let blurb = (r["description"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            return Item(id: "socrata:\(domain):\(id)", title: name, blurb: blurb, format: "GeoJSON", source: domain, url: file)
        }
    }

    /// Socrata's names for columns that hold a place.
    private static let geometryTypes: Set<String> = [
        "Point", "MultiPoint", "Line", "MultiLine", "LineString", "MultiLineString",
        "Polygon", "MultiPolygon", "Location", "Geospatial",
    ]

    // MARK: ArcGIS Hub

    /// ArcGIS Hub: where most city and county maps live. Its `downloadable` flag is not
    /// trustworthy — of eight datasets flagged downloadable in testing, one served a file and
    /// seven answered 404 — so every link is checked before it is shown (`verified`).
    private static func fromArcGISHub(_ q: String) async -> [Item] {
        var c = URLComponents(string: "https://hub.arcgis.com/api/v3/datasets")
        c?.queryItems = [
            URLQueryItem(name: "q", value: q),
            URLQueryItem(name: "page[size]", value: "12"),
        ]
        guard let url = c?.url, let root = await json(url) else { return [] }
        let data = (root["data"] as? [[String: Any]]) ?? []
        let candidates: [Item] = data.compactMap { row in
            guard let id = row["id"] as? String, let a = row["attributes"] as? [String: Any],
                  let name = a["name"] as? String
            else { return nil }
            // Hub refuses these outright; no point offering them.
            let kind = (a["type"] as? String) ?? ""
            guard !["Web Map", "Web Mapping Application", "KML", "Document Link"].contains(kind) else { return nil }
            guard var f = URLComponents(string: "https://hub.arcgis.com/api/v3/datasets/\(id)/downloads/data") else { return nil }
            // geojson, csv and kml are the only formats Hub will produce; "shapefile" is a 500.
            f.queryItems = [
                URLQueryItem(name: "format", value: "geojson"),
                URLQueryItem(name: "spatialRefId", value: "4326"),
                URLQueryItem(name: "where", value: "1=1"),
            ]
            guard let file = f.url else { return nil }
            let blurb = plainText((a["description"] as? String) ?? "")
            let org = (a["orgTitle"] as? String) ?? "ArcGIS Hub"
            return Item(id: "hub:\(id)", title: name, blurb: blurb, format: "GeoJSON", source: org, url: file)
        }
        return await verified(candidates)
    }

    /// Keeps only the links that really answer. A row that cannot be opened is worse than no row:
    /// it makes a promise the app then breaks, which is the whole failure this spike exists to fix.
    private static func verified(_ items: [Item]) async -> [Item] {
        await withTaskGroup(of: Item?.self) { group in
            for item in items {
                group.addTask { await reachable(item.url) ? item : nil }
            }
            var kept: [Item] = []
            for await item in group {
                if let item { kept.append(item) }
            }
            // The task group finishes out of order; keep the catalogue's ranking.
            let order = Dictionary(uniqueKeysWithValues: items.enumerated().map { ($0.element.id, $0.offset) })
            return kept.sorted { (order[$0.id] ?? 0) < (order[$1.id] ?? 0) }
        }
    }

    /// HEAD, which Hub answers in full (200 for a live dataset, 404 for a dead one) without
    /// sending the file.
    ///
    /// Not a ranged GET: Hub drops the connection on `Range: bytes=0-0` rather than answering, so
    /// a range check marks every dataset dead, including the ones that work. Measured against the
    /// same six "bike lanes" datasets — every one "dead" by range, one alive by HEAD and by a
    /// plain GET.
    private static func reachable(_ url: URL) async -> Bool {
        var r = URLRequest(url: url)
        r.httpMethod = "HEAD"
        r.timeoutInterval = 10
        guard let (_, response) = try? await URLSession.shared.data(for: r),
              let http = response as? HTTPURLResponse
        else { return false }
        return (200..<300).contains(http.statusCode)
    }

    // MARK: Web search (the fallback)

    /// A web search for subjects the catalogues do not hold.
    ///
    /// **Google, opened in Safari** — `SFSafariViewController`, which is a real out-of-process
    /// Safari with its own address bar, not an embedded web view. Google's block applies to
    /// `WKWebView`-based browsers, and the usual way round it is to spoof a Safari user agent,
    /// which is deliberate evasion and an App Review risk. Opening Safari needs no spoofing, no
    /// key and no money, and the person can see where they are.
    ///
    /// The cost of that honesty: Safari owns the download. A file saved there lands in Files, and
    /// the person brings it back with Share → NimbleMap or "Show my Downloads folder". The app
    /// cannot intercept it the way it can in its own web view.
    ///
    /// The words appended are the measured best: `open data download geojson shapefile`. They are
    /// space-separated on purpose — the parenthesised OR form scored worse on every subject tested
    /// — and the short format words (`gpkg`, `kml`, `shp`) are deliberately left out, because
    /// `Walmart gpkg` returns Garbage Pail Kids cards and `Walmart kml` a KML-brand kitchen scale.
    static func webSearch(_ text: String, scope: Scope) -> URL? {
        let q = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else { return nil }
        let terms: String
        switch scope {
        case .portals: terms = "\(q) open data download geojson shapefile"
        case .cityMaps: terms = "\(q) (site:hub.arcgis.com OR site:opendata.arcgis.com OR site:catalog.data.gov)"
        case .anyFile: terms = "\(q) geojson shapefile csv download"
        }
        var c = URLComponents(string: "https://www.google.com/search")
        c?.queryItems = [URLQueryItem(name: "q", value: terms)]
        return c?.url
    }

    // MARK: Plumbing

    private static func json(_ url: URL) async -> [String: Any]? {
        var r = URLRequest(url: url)
        r.timeoutInterval = 12
        guard let (data, response) = try? await URLSession.shared.data(for: r),
              let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return root
    }

    /// Portal descriptions arrive full of HTML and runs of whitespace.
    private static func plainText(_ s: String) -> String {
        let stripped = s.replacingOccurrences(of: "<[^>]+>", with: " ", options: .regularExpression)
        return stripped.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
