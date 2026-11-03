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

/// One feature: its attributes, where it is and how big it is. Numbers come from the data.
struct GeoFeature {
    let index: Int
    let properties: [String: Any]
    let center: CLLocationCoordinate2D
    let bounds: GeoBounds
    let areaKm2: Double?
    let lengthKm: Double?
    let raw: [String: Any]
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
final class GeoDataset: Identifiable {
    let id = UUID()
    let name: String
    let kind: GeometryKind
    let features: [GeoFeature]
    let fields: [FieldInfo]
    let bounds: GeoBounds
    /// GeoJSON for MapLibre, each feature tagged with `_lm_i` (its index) for hit-testing.
    let mapGeoJSON: Data
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
    /// Set for map images and grids of values; they have no features.
    var raster: RasterInfo?
    let sourceName: String
    let credit: String?

    init(
        name: String,
        kind: GeometryKind,
        features: [GeoFeature],
        fields: [FieldInfo],
        bounds: GeoBounds,
        mapGeoJSON: Data,
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
        self.mapGeoJSON = mapGeoJSON
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
    var countPhrase: String {
        let n = features.count
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
        return "\(NumberWords.integer(n)) \(noun)"
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

    /// Build a dataset from a FeatureCollection. `primary` names the insight field when known.
    static func dataset(
        name: String,
        geojson: Any,
        kindHint: GeometryKind?,
        assumptions: [String],
        primary: String?,
        sourceName: String,
        credit: String?
    ) throws -> GeoDataset {
        guard let root = geojson as? [String: Any], let rawFeatures = root["features"] as? [[String: Any]] else {
            throw ParseError.notGeoJSON
        }
        var features: [GeoFeature] = []
        var tagged: [[String: Any]] = []
        var all = GeoBounds.empty
        var counts = [GeometryKind.point: 0, .line: 0, .area: 0]
        features.reserveCapacity(rawFeatures.count)
        tagged.reserveCapacity(rawFeatures.count)

        for raw in rawFeatures {
            let index = features.count
            var props = (raw["properties"] as? [String: Any]) ?? [:]
            props.removeValue(forKey: "_lm_i")
            let geometry = raw["geometry"] as? [String: Any]
            let measured = GeoMeasure.measure(geometry)
            if let kind = measured.kind { counts[kind, default: 0] += 1 }
            all.add(measured.bounds)
            features.append(
                GeoFeature(
                    index: index,
                    properties: props,
                    center: measured.bounds.isValid ? measured.center : CLLocationCoordinate2D(latitude: 0, longitude: 0),
                    bounds: measured.bounds,
                    areaKm2: measured.areaKm2,
                    lengthKm: measured.lengthKm,
                    raw: raw
                )
            )
            var tagProps = props
            tagProps["_lm_i"] = index
            var t = raw
            t["properties"] = tagProps
            t.removeValue(forKey: "id")
            tagged.append(t)
        }
        guard !features.isEmpty else { throw ParseError.noFeatures }

        let kind: GeometryKind = {
            if let kindHint, kindHint != .table { return kindHint }
            let best = counts.max { $0.value < $1.value }
            guard let best, best.value > 0 else { return .table }
            return best.key
        }()

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

        let collection: [String: Any] = ["type": "FeatureCollection", "features": tagged]
        let data = try JSONSerialization.data(withJSONObject: collection)
        return GeoDataset(
            name: name,
            kind: kind,
            features: features,
            fields: fields,
            bounds: all,
            mapGeoJSON: data,
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
