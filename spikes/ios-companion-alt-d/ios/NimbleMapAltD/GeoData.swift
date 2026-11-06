import Foundation
import CoreLocation

// Copied from the file Alt A and Alt B share; Alt C adds the persona-study fixes (README › Alt C).

/// What a data set is drawn as. Plain words for people; never "polygon" or "polyline".
enum GeometryKind: String {
    case point
    case line
    case area
    case table
    /// A map image or a grid of values (Alt C).
    case raster

    /// "Points", "Lines", "Areas" — the kind in plain words (redesign › Result card).
    var plural: String {
        switch self {
        case .point: return "Points"
        case .line: return "Lines"
        case .area: return "Areas"
        case .table: return "Table"
        case .raster: return "Map image"
        }
    }
}

enum FieldKind {
    case number
    case category
    case text
}

struct FieldInfo: Identifiable, Hashable {
    let name: String
    let label: String
    let kind: FieldKind
    /// A year column ("Year", "built_year"): colorable as a number, never an insight or a "1,851".
    var isYear = false
    var id: String { name }
}

/// One feature: where it is and how big it is, and a window onto its attributes in the layer's
/// columns. It used to carry a dictionary of every attribute and its whole shape: for 154,400
/// white storks that was millions of objects, and a phone ended the app. Now it is a few numbers;
/// attributes are looked up in `LayerStore`, and the shape is read from disk when it is needed.
struct GeoFeature {
    let index: Int
    let store: LayerStore?
    let center: CLLocationCoordinate2D
    let bounds: GeoBounds
    let areaKm2: Double?
    let lengthKm: Double?

    /// `feature.properties["name"]`, as before, without a dictionary per feature.
    var properties: Row { Row(store: store, index: index) }

    /// The feature as GeoJSON with its shape (for the outline drawn when it's tapped).
    var raw: [String: Any] { ["type": "Feature", "geometry": store?.geometry(index) ?? NSNull()] }
}

/// One feature's attributes, read from the layer's columns.
struct Row {
    let store: LayerStore?
    let index: Int
    subscript(key: String) -> Any? { store?.value(index, key) }
    var keys: [String] { store?.names ?? [] }
}

/// A layer's attributes by column, and its shapes on disk (written by the reader: `store.rs`).
/// Numbers are kept as `Double`s; any other column as codes into its distinct values, stored once.
final class LayerStore: @unchecked Sendable {
    enum Column {
        case number([Double])
        case text([String], [UInt32])
    }

    let names: [String]
    let columns: [Column]
    private let byName: [String: Int]
    let rows: Int
    let mapURL: URL
    private let idx: Data

    init?(cols: URL, map: URL, idx idxURL: URL) {
        guard let data = try? Data(contentsOf: cols, options: .alwaysMapped),
              let idx = try? Data(contentsOf: idxURL, options: .alwaysMapped) else { return nil }
        var at = 0
        func u32() -> UInt32? {
            guard at + 4 <= data.count else { return nil }
            defer { at += 4 }
            return data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: at, as: UInt32.self) }
        }
        func string() -> String? {
            guard let n = u32().map(Int.init), at + n <= data.count else { return nil }
            defer { at += n }
            return String(decoding: data[at..<(at + n)], as: UTF8.self)
        }
        guard data.count >= 12, data.prefix(4) == Data("LMC1".utf8) else { return nil }
        at = 4
        guard let rows = u32().map(Int.init), let ncols = u32().map(Int.init) else { return nil }
        var names: [String] = []
        var columns: [Column] = []
        for _ in 0..<ncols {
            guard let name = string(), at < data.count else { return nil }
            let kind = data[at]
            at += 1
            if kind == 0 {
                guard at + rows * 8 <= data.count else { return nil }
                let values = data.withUnsafeBytes { raw in
                    (0..<rows).map { raw.loadUnaligned(fromByteOffset: at + $0 * 8, as: Double.self) }
                }
                at += rows * 8
                columns.append(.number(values))
            } else {
                guard let n = u32().map(Int.init) else { return nil }
                var dict: [String] = []
                dict.reserveCapacity(n)
                for _ in 0..<n {
                    guard let v = string() else { return nil }
                    dict.append(v)
                }
                guard at + rows * 4 <= data.count else { return nil }
                let codes = data.withUnsafeBytes { raw in
                    (0..<rows).map { raw.loadUnaligned(fromByteOffset: at + $0 * 4, as: UInt32.self) }
                }
                at += rows * 4
                columns.append(.text(dict, codes))
            }
            names.append(name)
        }
        self.names = names
        self.columns = columns
        self.byName = Dictionary(names.enumerated().map { ($1, $0) }, uniquingKeysWith: { a, _ in a })
        self.rows = rows
        self.mapURL = map
        self.idx = idx
    }

    /// A cell as the old GeoJSON gave it: an NSNumber, a String, or nil when empty.
    func value(_ row: Int, _ key: String) -> Any? {
        guard let c = byName[key], row >= 0, row < rows else { return nil }
        switch columns[c] {
        case .number(let values):
            let v = values[row]
            return v.isNaN ? nil : NSNumber(value: v)
        case .text(let dict, let codes):
            let k = codes[row]
            return k == UInt32.max ? nil : dict[Int(k)]
        }
    }

    /// Where feature `row` is in the map file.
    func span(_ row: Int) -> (offset: UInt64, length: Int)? {
        guard row >= 0, (row + 1) * 12 <= idx.count else { return nil }
        return idx.withUnsafeBytes { raw in
            (raw.loadUnaligned(fromByteOffset: row * 12, as: UInt64.self), Int(raw.loadUnaligned(fromByteOffset: row * 12 + 8, as: UInt32.self)))
        }
    }

    /// One feature's GeoJSON geometry, read from the map file.
    func geometry(_ row: Int) -> Any? {
        guard let (offset, length) = span(row), let h = try? FileHandle(forReadingFrom: mapURL) else { return nil }
        defer { try? h.close() }
        guard (try? h.seek(toOffset: offset)) != nil, let bytes = try? h.read(upToCount: length),
              let obj = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any] else { return nil }
        return obj["geometry"]
    }

    /// The map file with one more property on every feature: the field a layer is coloured by
    /// (MapLibre reads it from the source). Spliced in before each feature's closing `}}`; made
    /// once per field and kept beside the map file.
    func mapURL(showing field: String?) -> URL? {
        guard let field, let c = byName[field] else { return mapURL }
        let safe = field.unicodeScalars.map { CharacterSet.alphanumerics.contains($0) ? String($0) : "_" }.joined()
        let out = mapURL.deletingPathExtension().appendingPathExtension("by-\(safe).geojson")
        if FileManager.default.fileExists(atPath: out.path) { return out }
        guard let map = try? Data(contentsOf: mapURL, options: .alwaysMapped) else { return nil }
        FileManager.default.createFile(atPath: out.path, contents: nil)
        guard let h = try? FileHandle(forWritingTo: out) else { return nil }
        defer { try? h.close() }
        let key = Self.json(field)
        var buf = Data(#"{"type":"FeatureCollection","features":["#.utf8)
        buf.reserveCapacity(1 << 20)
        for row in 0..<rows {
            guard let (offset, length) = span(row), length >= 2, Int(offset) + length <= map.count else { continue }
            if row > 0 { buf.append(0x2C) }
            let start = Int(offset)
            buf.append(map[start..<(start + length - 2)])
            switch columns[c] {
            case .number(let values) where values[row].isFinite:
                buf.append(contentsOf: Array(",\(key):\(values[row])}}".utf8))
            case .text(let dict, let codes) where codes[row] != UInt32.max:
                buf.append(contentsOf: Array(",\(key):\(Self.json(dict[Int(codes[row])]))}}".utf8))
            default:
                buf.append(contentsOf: Array("}}".utf8))
            }
            if buf.count > 1 << 20 {
                h.write(buf)
                buf.removeAll(keepingCapacity: true)
            }
        }
        buf.append(contentsOf: Array("]}".utf8))
        h.write(buf)
        return out
    }

    /// A JSON string literal.
    static func json(_ s: String) -> String {
        var out = "\""
        for u in s.unicodeScalars {
            switch u {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default:
                if u.value < 0x20 { out += String(format: "\\u%04x", u.value) } else { out.unicodeScalars.append(u) }
            }
        }
        return out + "\""
    }
}

struct GeoBounds: Equatable {
    var minLon: Double
    var minLat: Double
    var maxLon: Double
    var maxLat: Double

    static let empty = GeoBounds(minLon: .infinity, minLat: .infinity, maxLon: -.infinity, maxLat: -.infinity)

    var isValid: Bool { minLon <= maxLon && minLat <= maxLat }

    mutating func add(lon: Double, lat: Double) {
        guard lon.isFinite, lat.isFinite else { return }
        minLon = min(minLon, lon)
        minLat = min(minLat, lat)
        maxLon = max(maxLon, lon)
        maxLat = max(maxLat, lat)
    }

    mutating func add(_ other: GeoBounds) {
        guard other.isValid else { return }
        add(lon: other.minLon, lat: other.minLat)
        add(lon: other.maxLon, lat: other.maxLat)
    }

    var center: CLLocationCoordinate2D {
        CLLocationCoordinate2D(latitude: (minLat + maxLat) / 2, longitude: (minLon + maxLon) / 2)
    }
}

/// A decoded data set. Built off the main thread, then read-only.
/// How data in a local grid was put on the map (see `placement.rs`).
struct Placement {
    /// The app's copy of the file, read again for another grid.
    let file: URL
    /// "prj", "guess" or "chosen".
    let how: String
    let name: String
    /// Other grids that land it near `near`, nearest first.
    let alternatives: [(epsg: Int, name: String)]
    let near: CLLocationCoordinate2D?
}

final class GeoDataset: Identifiable {
    let id = UUID()
    let name: String
    let kind: GeometryKind
    let features: [GeoFeature]
    let fields: [FieldInfo]
    let bounds: GeoBounds
    /// The attribute columns and the shapes on disk (nil for a map image).
    let store: LayerStore?

    /// The GeoJSON file for MapLibre: each feature's shape, its index (`_lm_i`, for taps) and only
    /// the field the layer is coloured by (see `LayerStore.mapURL(showing:)`).
    func mapFile(showing field: String?) -> URL? { store?.mapURL(showing: field) }
    /// Plain-language assumptions and skips from the reader (purple dot on the phone too).
    let assumptions: [String]
    /// The field used for insights ("3rd highest of 17"), if any number field exists.
    let primaryField: FieldInfo?
    let titleField: String?
    /// "First name" + "Surname" columns: the title is the person, not the place (Walt).
    let nameFields: [String]
    /// Notes written by people ("description", "notes"): shown first on the card (Priya).
    let notesField: String?
    /// Plumbing a novice never needs: style ids, a folder that repeats another column.
    let hiddenFields: Set<String>
    /// Colors the file itself asked for (KML styles), by value of `fileColorsField`.
    var fileColors: [String: UInt32] = [:]
    var fileColorsField: String?
    /// Where it came from, so the map can be reopened: a library item, or a file in the app.
    var libraryID: String?
    var savedPath: String?
    /// Data that came in a local grid (metres, not longitude/latitude): how it was placed, so the
    /// layer card can offer another grid.
    var placement: Placement?
    /// The layer's path inside the .zip it came from (several layers share one file).
    var member: String?
    /// Set for map images and grids of values; they have no features.
    var raster: RasterInfo?

    /// Takes another read of the same file's cells — a different NetCDF time step — keeping this
    /// dataset's identity, name and fields so the layer's place, style and card do not change.
    func adoptCells(from other: GeoDataset) {
        guard let next = other.raster else { return }
        raster = next
    }
    let sourceName: String
    let credit: String?

    init(
        name: String,
        kind: GeometryKind,
        features: [GeoFeature],
        fields: [FieldInfo],
        bounds: GeoBounds,
        store: LayerStore? = nil,
        assumptions: [String],
        primaryField: FieldInfo?,
        titleField: String?,
        nameFields: [String] = [],
        notesField: String? = nil,
        hiddenFields: Set<String> = [],
        sourceName: String,
        credit: String?
    ) {
        self.name = name
        self.kind = kind
        self.features = features
        self.fields = fields
        self.bounds = bounds
        self.store = store
        self.assumptions = assumptions
        self.primaryField = primaryField
        self.titleField = titleField
        self.nameFields = nameFields
        self.notesField = notesField
        self.hiddenFields = hiddenFields
        self.sourceName = sourceName
        self.credit = credit
    }

    func field(named name: String) -> FieldInfo? {
        fields.first { $0.name == name }
    }

    /// Fields a person picks from or reads: not hidden plumbing, not the notes.
    var visibleFields: [FieldInfo] {
        fields.filter { !hiddenFields.contains($0.name) && $0.name != notesField }
    }

    /// Fields "Color by" offers: categories and numbers, years included.
    var colorFields: [FieldInfo] {
        visibleFields.filter { $0.kind != .text }
    }

    var yearField: FieldInfo? { fields.first { $0.isYear } }

    /// The file's own area column ("AREA_HA"): when there is one, the card shows it and not a
    /// size measured from the drawing, so it never states two areas (Priya: "42.1 ha" vs "354.8").
    var areaField: FieldInfo? {
        fields.first { f in
            let n = f.name.lowercased()
            return f.kind == .number && (n.hasPrefix("area") || n.hasSuffix("area") || n.hasPrefix("shape_area") || n == "hectares" || n == "acres")
        }
    }

    var lengthField: FieldInfo? {
        fields.first { f in
            let n = f.name.lowercased()
            return f.kind == .number && (n.hasPrefix("length") || n.hasPrefix("len_") || n.hasSuffix("length") || n == "km" || n == "miles")
        }
    }

    func title(of feature: GeoFeature) -> String {
        let person = nameFields.compactMap { feature.properties[$0].flatMap(GeoValue.text) }.filter { !$0.isEmpty }
        if !person.isEmpty { return person.joined(separator: " ") }
        if let titleField, let v = feature.properties[titleField], let s = GeoValue.text(v), !s.isEmpty {
            return s
        }
        return "\(singularNoun.capitalized) \(feature.index + 1)"
    }

    /// "country", "city", "earthquake" — from the name when it is a plain plural.
    var singularNoun: String {
        let lower = name.lowercased()
        if !lower.contains(" ") {
            if lower.hasSuffix("ies") { return String(lower.dropLast(3)) + "y" }
            if lower.hasSuffix("oes") { return String(lower.dropLast(2)) }
            if lower.hasSuffix("s") && !lower.hasSuffix("ss") { return String(lower.dropLast()) }
        }
        switch kind {
        case .point: return "place"
        case .line: return "line"
        case .area: return "area"
        case .table: return "row"
        case .raster: return "map image"
        }
    }

    /// "177 countries", "1,251 cities"; "22 places" for "Lisbon trip" (not "22 lisbon trip").
    /// Rows kept in the table that have no place on the map (blank coordinates, a 0, 0 stand-in,
    /// a find on Mars). Their `center` is 0, 0 only as a placeholder.
    var unlocated: Int {
        kind == .table || raster != nil ? 0 : features.lazy.filter { !$0.bounds.isValid }.count
    }

    var countPhrase: String {
        let missing = unlocated
        let n = features.count - missing
        let lower = name.lowercased()
        let plainPlural = !lower.contains(" ") && lower.hasSuffix("s") && !lower.hasSuffix("ss")
        let noun: String
        if plainPlural {
            noun = n == 1 ? singularNoun : lower
        } else {
            switch kind {
            case .point: noun = n == 1 ? "place" : "places"
            case .line: noun = n == 1 ? "line" : "lines"
            case .area: noun = n == 1 ? "area" : "areas"
            case .table: noun = n == 1 ? "row" : "rows"
            case .raster: noun = ""
            }
        }
        if let raster {
            return raster.look == .values ? "\(raster.quantity.lowercased()) grid" : "map image"
        }
        // NASA's meteorites: 45,716 rows, but 13,530 have no usable place; saying 45,716 places
        // promised dots that aren't there.
        let more = missing > 0 ? " (\(NumberWords.integer(missing)) more without a location)" : ""
        return "\(NumberWords.integer(n)) \(noun)\(more)"
    }

    func numbers(for field: FieldInfo) -> [(index: Int, value: Double)] {
        features.compactMap { f in
            guard let v = f.properties[field.name], let d = GeoValue.number(v) else { return nil }
            return (f.index, d)
        }
    }
}

enum GeoValue {
    static func number(_ v: Any) -> Double? {
        if let n = v as? NSNumber {
            // JSON booleans arrive as NSNumber too; they are categories, not numbers.
            if CFGetTypeID(n) == CFBooleanGetTypeID() { return nil }
            let d = n.doubleValue
            return d.isFinite ? d : nil
        }
        return nil
    }

    static func text(_ v: Any) -> String? {
        if let s = v as? String { return s }
        if let n = v as? NSNumber {
            if CFGetTypeID(n) == CFBooleanGetTypeID() { return n.boolValue ? "Yes" : "No" }
            return NumberWords.plain(n.doubleValue)
        }
        return nil
    }
}

/// Human labels for common field names. Unknown names are tidied ("depth_km" → "Depth km").
enum FieldLabels {
    static let known: [String: String] = [
        "pop_est": "Population",
        "population": "Population",
        "pop_max": "Population",
        "gdp_md": "Economy (million US$)",
        "continent": "Continent",
        "country": "Country",
        "capital": "Capital",
        "magnitude": "Magnitude",
        "mag": "Magnitude",
        "depth_km": "Depth (km)",
        "elevation_m": "Height (m)",
        "morphology": "Volcano type",
        "status": "Status",
        "type": "Type",
        "name": "Name",
        "date": "Date",
        "place": "Place",
        "year": "Year",
        "description": "Notes",
        "folder": "Folder",
    ]

    /// Words inside column names, for names nobody listed ("POP1851" → "Population 1851").
    static let words: [String: String] = [
        "pop": "population", "popn": "population", "area": "area", "ha": "(ha)", "km2": "(km²)", "sqkm": "(km²)",
        "reg": "registration", "dist": "district", "tipo": "type", "nome": "name", "nom": "name", "typ": "type",
        "desc": "description", "elev": "height", "len": "length", "no": "number", "num": "number",
    ]

    /// "AREA_HA" → "Area (ha)", "POP_2021" → "Population 2021", "depth_km" → "Depth km". Names
    /// shouted in capitals are written in sentence case; a known name wins.
    static func label(for name: String) -> String {
        if let k = known[name.lowercased()] { return k }
        // Split on underscores, spaces, and letter/digit boundaries.
        var tokens: [String] = []
        var current = ""
        for ch in name {
            if ch == "_" || ch == " " || ch == "-" {
                if !current.isEmpty { tokens.append(current) }
                current = ""
                continue
            }
            if let last = current.last, last.isNumber != ch.isNumber {
                tokens.append(current)
                current = ""
            }
            current.append(ch)
        }
        if !current.isEmpty { tokens.append(current) }
        let shouting = name == name.uppercased()
        let mapped = tokens.map { t -> String in
            if let w = words[t.lowercased()] { return w }
            return shouting ? t.lowercased() : t
        }
        let spaced = mapped.joined(separator: " ")
        return spaced.prefix(1).uppercased() + spaced.dropFirst()
    }
}

enum GeoParse {
    enum ParseError: Error {
        case notGeoJSON
        case noFeatures
    }

    /// Build a dataset from the reader's files (see `store.rs`): columns, measures, map file.
    /// `primary` names the insight field when known.
    static func dataset(
        name: String,
        files: [String: String],
        count: Int,
        kindHint: GeometryKind?,
        assumptions: [String],
        primary: String?,
        sourceName: String,
        credit: String?
    ) throws -> GeoDataset {
        guard let cols = files["cols"], let map = files["map"], let idx = files["idx"], let geoPath = files["geo"],
              let store = LayerStore(cols: URL(fileURLWithPath: cols), map: URL(fileURLWithPath: map), idx: URL(fileURLWithPath: idx)),
              let geo = try? Data(contentsOf: URL(fileURLWithPath: geoPath), options: .alwaysMapped) else {
            throw ParseError.notGeoJSON
        }
        let rows = min(count, store.rows, geo.count / 64)
        var features: [GeoFeature] = []
        features.reserveCapacity(rows)
        var all = GeoBounds.empty
        geo.withUnsafeBytes { raw in
            func d(_ row: Int, _ k: Int) -> Double { raw.loadUnaligned(fromByteOffset: (row * 8 + k) * 8, as: Double.self) }
            for row in 0..<rows {
                let b = GeoBounds(minLon: d(row, 2), minLat: d(row, 3), maxLon: d(row, 4), maxLat: d(row, 5))
                let valid = b.minLon.isFinite && b.isValid
                if valid { all.add(b) }
                let area = d(row, 6), length = d(row, 7)
                features.append(GeoFeature(
                    index: row,
                    store: store,
                    center: valid ? CLLocationCoordinate2D(latitude: d(row, 1), longitude: d(row, 0)) : CLLocationCoordinate2D(latitude: 0, longitude: 0),
                    bounds: valid ? b : .empty,
                    areaKm2: area.isNaN ? nil : area,
                    lengthKm: length.isNaN ? nil : length
                ))
            }
        }
        guard !features.isEmpty else { throw ParseError.noFeatures }
        let kind: GeometryKind = kindHint.flatMap { $0 == .table ? nil : $0 } ?? .point

        let fields = inferFields(features)
        let primaryField: FieldInfo? = {
            if let primary, let f = fields.first(where: { $0.name == primary && $0.kind == .number }) { return f }
            // A size column is a fact about the shape; prefer what the data is about (population…).
            func sizeLike(_ f: FieldInfo) -> Bool {
                let n = f.name.lowercased()
                return n.hasPrefix("area") || n.hasSuffix("area") || n.hasPrefix("shape") || n.hasPrefix("len")
            }
            let numbers = fields.filter { $0.kind == .number && !$0.isYear }
            return numbers.first { !sizeLike($0) } ?? numbers.first
        }()
        let titleField = ["name", "title", "place", "label"].compactMap { key in
            fields.first { $0.name.lowercased() == key }?.name
        }.first ?? fields.first { $0.kind == .text }?.name
        func named(_ keys: [String]) -> String? {
            fields.first { keys.contains($0.name.lowercased().replacingOccurrences(of: "_", with: " ")) }?.name
        }
        let first = named(["first name", "firstname", "given name", "forename", "first"])
        let last = named(["surname", "last name", "lastname", "family name"])
        let nameFields = first != nil && last != nil ? [first, last].compactMap { $0 } : []
        let notesField = named(["description", "notes", "note", "comment", "comments", "notes from friends"])
        let hiddenFields = plumbing(fields, features)

        return GeoDataset(
            name: name,
            kind: kind,
            features: features,
            fields: fields,
            bounds: all,
            store: store,
            assumptions: assumptions,
            primaryField: primaryField,
            titleField: titleField,
            nameFields: nameFields,
            notesField: notesField,
            hiddenFields: hiddenFields,
            sourceName: sourceName,
            credit: credit
        )
    }

    /// Style ids and file bookkeeping, and any column that repeats an earlier one value for value
    /// (a My Maps "folder" next to its "Category").
    static func plumbing(_ fields: [FieldInfo], _ features: [GeoFeature]) -> Set<String> {
        var hidden = Set(fields.map(\.name).filter { ["style", "styleurl", "visibility", "open", "snippet", "_lm_i"].contains($0.lowercased()) })
        for (i, f) in fields.enumerated() where !hidden.contains(f.name) {
            let repeats = fields.prefix(i).contains { g in
                !hidden.contains(g.name) && features.allSatisfy { feature in
                    let a = feature.properties[f.name].flatMap(GeoValue.text) ?? ""
                    let b = feature.properties[g.name].flatMap(GeoValue.text) ?? ""
                    return a == b
                }
            }
            if repeats { hidden.insert(f.name) }
        }
        return hidden
    }

    /// Number when nearly every value is a number with at least two distinct values; category when
    /// a few distinct labels repeat; text otherwise. Year/id columns are not insight material.
    static func inferFields(_ features: [GeoFeature]) -> [FieldInfo] {
        var order: [String] = []
        var seen = Set<String>()
        for f in features.prefix(200) {
            for key in f.properties.keys.sorted() where !seen.contains(key) {
                seen.insert(key)
                order.append(key)
            }
        }
        return order.compactMap { key in
            var numbers = Set<Double>()
            var labels = Set<String>()
            var numberCount = 0
            var present = 0
            for f in features {
                guard let v = f.properties[key], !(v is NSNull) else { continue }
                present += 1
                if let d = GeoValue.number(v) {
                    numberCount += 1
                    if numbers.count < 64 { numbers.insert(d) }
                } else if let s = GeoValue.text(v), labels.count < 64 {
                    labels.insert(s)
                }
            }
            guard present > 0 else { return nil }
            let lower = key.lowercased()
            let coordinate = ["latitude", "longitude", "lat", "lon", "lng", "x", "y"].contains(lower)
            let isYear = lower.hasSuffix("year") && numbers.allSatisfy { $0 >= 0 && $0 <= 3000 && $0 == $0.rounded() }
            let idLike = coordinate || lower == "id" || lower.hasSuffix("_id") || lower.hasPrefix("iso")
            let kind: FieldKind
            if Double(numberCount) >= 0.8 * Double(present), numbers.count >= 2, !idLike {
                kind = .number
            } else if numberCount == 0, labels.count >= 2, labels.count <= 12, present >= labels.count * 2 {
                kind = .category
            } else {
                kind = .text
            }
            return FieldInfo(name: key, label: FieldLabels.label(for: key), kind: kind, isYear: isYear && kind == .number)
        }
    }
}

/// Spherical sizes for the card ("Areas · 652,860 km²"). Same formula family as the desktop's
/// geodesic measure, accurate to well under a percent at country scale.
enum GeoMeasure {
    static let earthRadiusKm = 6371.0088

    struct Result {
        var kind: GeometryKind?
        var bounds = GeoBounds.empty
        var center = CLLocationCoordinate2D(latitude: 0, longitude: 0)
        var areaKm2: Double?
        var lengthKm: Double?
    }

    static func measure(_ geometry: [String: Any]?) -> Result {
        var r = Result()
        guard let geometry, let type = geometry["type"] as? String else { return r }
        let coords = geometry["coordinates"]
        switch type {
        case "Point":
            r.kind = .point
            if let p = point(coords) {
                r.bounds.add(lon: p.0, lat: p.1)
            }
        case "MultiPoint":
            r.kind = .point
            for p in points(coords) { r.bounds.add(lon: p.0, lat: p.1) }
        case "LineString":
            r.kind = .line
            let line = points(coords)
            for p in line { r.bounds.add(lon: p.0, lat: p.1) }
            r.lengthKm = length(line)
        case "MultiLineString":
            r.kind = .line
            var total = 0.0
            for line in (coords as? [Any] ?? []).map(points) {
                for p in line { r.bounds.add(lon: p.0, lat: p.1) }
                total += length(line)
            }
            r.lengthKm = total
        case "Polygon":
            r.kind = .area
            let rings = (coords as? [Any] ?? []).map { points($0) }
            for p in rings.first ?? [] { r.bounds.add(lon: p.0, lat: p.1) }
            r.areaKm2 = polygonArea(rings)
        case "MultiPolygon":
            r.kind = .area
            var total = 0.0
            var biggest = 0.0
            var biggestBounds = GeoBounds.empty
            for poly in coords as? [Any] ?? [] {
                let rings = (poly as? [Any] ?? []).map { points($0) }
                var b = GeoBounds.empty
                for p in rings.first ?? [] {
                    b.add(lon: p.0, lat: p.1)
                    r.bounds.add(lon: p.0, lat: p.1)
                }
                let a = polygonArea(rings)
                total += a
                if a > biggest {
                    biggest = a
                    biggestBounds = b
                }
            }
            r.areaKm2 = total
            if biggestBounds.isValid {
                r.center = biggestBounds.center
                return r
            }
        default:
            break
        }
        if r.bounds.isValid { r.center = r.bounds.center }
        return r
    }

    private static func point(_ any: Any?) -> (Double, Double)? {
        guard let a = any as? [Any], a.count >= 2, let x = GeoValue.number(a[0]), let y = GeoValue.number(a[1]) else {
            return nil
        }
        return (x, y)
    }

    private static func points(_ any: Any?) -> [(Double, Double)] {
        (any as? [Any] ?? []).compactMap(point)
    }

    private static func rad(_ d: Double) -> Double { d * .pi / 180 }

    static func length(_ line: [(Double, Double)]) -> Double {
        guard line.count > 1 else { return 0 }
        var total = 0.0
        for i in 1..<line.count {
            let (lon1, lat1) = line[i - 1]
            let (lon2, lat2) = line[i]
            let dLat = rad(lat2 - lat1)
            let dLon = rad(lon2 - lon1)
            let h = sin(dLat / 2) * sin(dLat / 2) + cos(rad(lat1)) * cos(rad(lat2)) * sin(dLon / 2) * sin(dLon / 2)
            total += 2 * earthRadiusKm * asin(min(1, sqrt(h)))
        }
        return total
    }

    /// Outer ring minus holes.
    static func polygonArea(_ rings: [[(Double, Double)]]) -> Double {
        guard let outer = rings.first else { return 0 }
        var a = ringArea(outer)
        for hole in rings.dropFirst() { a -= ringArea(hole) }
        return max(0, a)
    }

    /// Chamberlain & Duquette, "Some algorithms for polygons on a sphere" (JPL, 2007).
    static func ringArea(_ ring: [(Double, Double)]) -> Double {
        guard ring.count > 2 else { return 0 }
        var total = 0.0
        for i in 0..<ring.count {
            let (lon1, lat1) = ring[i]
            let (lon2, lat2) = ring[(i + 1) % ring.count]
            total += rad(lon2 - lon1) * (2 + sin(rad(lat1)) + sin(rad(lat2)))
        }
        return abs(total * earthRadiusKm * earthRadiusKm / 2)
    }
}

/// Numbers in words a novice reads at a glance: "1.41 billion", "652,860", "6.9".
enum NumberWords {
    static func integer(_ n: Int) -> String {
        let f = NumberFormatter()
        f.numberStyle = .decimal
        return f.string(from: NSNumber(value: n)) ?? String(n)
    }

    static func plain(_ d: Double) -> String {
        let f = NumberFormatter()
        f.numberStyle = .decimal
        f.maximumFractionDigits = abs(d) < 10 ? 2 : (abs(d) < 1000 ? 1 : 0)
        return f.string(from: NSNumber(value: d)) ?? String(d)
    }

    /// "1851", never "1,851".
    static func year(_ d: Double) -> String { String(Int(d.rounded())) }

    static func compact(_ d: Double) -> String {
        let a = abs(d)
        let sign = d < 0 ? "−" : ""
        func fmt(_ v: Double, _ unit: String) -> String {
            let f = NumberFormatter()
            f.numberStyle = .decimal
            f.maximumFractionDigits = v < 10 ? 2 : (v < 100 ? 1 : 0)
            return sign + (f.string(from: NSNumber(value: v)) ?? String(v)) + " " + unit
        }
        if a >= 1e9 { return fmt(a / 1e9, "billion") }
        if a >= 1e6 { return fmt(a / 1e6, "million") }
        if a >= 1e5 { return fmt(a / 1e3, "thousand") }
        return sign + plain(a)
    }

    static func ordinal(_ n: Int) -> String {
        let f = NumberFormatter()
        f.numberStyle = .ordinal
        return f.string(from: NSNumber(value: n)) ?? "\(n)th"
    }

    static func area(_ km2: Double) -> String {
        if km2 < 1 { return plain(km2 * 100) + " ha" }
        return compact(km2) + " km²"
    }

    static func length(_ km: Double) -> String {
        km < 1 ? plain(km * 1000) + " m" : compact(km) + " km"
    }
}
