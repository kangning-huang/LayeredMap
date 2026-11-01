import Foundation

// Shared by Alt A and Alt B: the same curated data in both (library-catalog.md, bundled so the
// first minute works offline). Alt A calls it "Examples"; Alt B calls it the "Data library".

struct LibraryItem: Identifiable, Hashable {
    let id: String
    let title: String
    let blurb: String
    let kind: GeometryKind
    let size: String
    let file: String
    let primaryField: String?
    /// Field the map is colored by when added (nil = one color).
    let showBy: String?
    let source: String
    let license: String
    let credit: String?
    let symbol: String
    let tint: UInt32
}

enum Library {
    static let items: [LibraryItem] = [
        LibraryItem(
            id: "countries", title: "Countries", blurb: "177 countries with population and economy",
            kind: .area, size: "250 KB", file: "countries.geojson", primaryField: "pop_est", showBy: "pop_est",
            source: "Natural Earth", license: "Free to use", credit: nil, symbol: "globe.europe.africa.fill", tint: 0x21918C
        ),
        LibraryItem(
            id: "cities", title: "Cities", blurb: "1,251 cities and towns with population",
            kind: .point, size: "240 KB", file: "cities.geojson", primaryField: "population", showBy: nil,
            source: "Natural Earth", license: "Free to use", credit: nil, symbol: "building.2.fill", tint: 0x2F6FDE
        ),
        LibraryItem(
            id: "earthquakes", title: "Earthquakes", blurb: "14,537 strong earthquakes since 1900",
            kind: .point, size: "1 MB", file: "earthquakes.csv", primaryField: "magnitude", showBy: "magnitude",
            source: "U.S. Geological Survey", license: "Free to use", credit: "U.S. Geological Survey",
            symbol: "waveform.path.ecg", tint: 0xF03B20
        ),
        LibraryItem(
            id: "volcanoes", title: "Volcanoes", blurb: "1,609 volcanoes with height and type",
            kind: .point, size: "100 KB", file: "volcanoes.csv", primaryField: "elevation_m", showBy: nil,
            source: "NOAA NCEI", license: "Free to use · credit the source", credit: "NOAA NCEI Global Volcano Locations",
            symbol: "mountain.2.fill", tint: 0xD55E00
        ),
    ]

    static func search(_ text: String) -> [LibraryItem] {
        let q = text.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return items }
        return items.filter { $0.title.lowercased().contains(q) || $0.blurb.lowercased().contains(q) }
    }
}
