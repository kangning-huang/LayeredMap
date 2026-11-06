import Foundation

// From the A/B shared file (Alt C copy). Every number here comes from the data, never from drawn pixels.

/// The desktop object card's content (spec › Inspector: object card): name, kind and size, up to
/// three insight rows, a strip of where this one sits among the rest.
struct FeatureInsight: Identifiable {
    let id = UUID()
    let layerID: UUID
    let featureIndex: Int
    let title: String
    let subtitle: String
    /// The subtitle's facts without the layer name, for one-line labels.
    let facts: [String]
    /// The person's own words about this place, shown first (Priya: "That's the whole point").
    let notes: String?
    let rows: [String]
    /// Values of the insight field scaled to 0…1, and this feature's position, for the dot strip.
    let strip: [Double]
    let stripHighlight: Double?
    let field: FieldInfo?
    /// What "Color by …" on the card offers: the insight number, else the first category.
    let colorField: FieldInfo?
    /// Other attributes, in plain labels, for "More details".
    let details: [(String, String)]
}

/// Data card content for a whole data set.
struct DatasetSummary {
    let headline: String
    let rows: [String]
}

enum Insights {
    static func feature(_ feature: GeoFeature, in dataset: GeoDataset, layerID: UUID) -> FeatureInsight {
        var subtitleParts: [String] = []
        if let category = dataset.visibleFields.first(where: { $0.kind == .category && $0.name.lowercased() != "capital" }),
           let v = feature.properties[category.name], let s = GeoValue.text(v) {
            subtitleParts.append(s)
        }
        // Who, then when and where: "Born · 1812 · Haworth" (Walt: "Tell me who, not just where").
        var shownInSubtitle: Set<String> = []
        if let year = dataset.yearField, let v = feature.properties[year.name], let d = GeoValue.number(v) {
            subtitleParts.append(NumberWords.year(d))
            shownInSubtitle.insert(year.name)
        }
        if !dataset.nameFields.isEmpty, let place = dataset.titleField, let v = feature.properties[place], let s = GeoValue.text(v), !s.isEmpty {
            subtitleParts.append(s)
            shownInSubtitle.insert(place)
        }
        // A size measured from the drawing only when the file doesn't state one itself.
        if let a = feature.areaKm2, a > 0, dataset.areaField == nil {
            subtitleParts.append(NumberWords.area(a))
        } else if let l = feature.lengthKm, l > 0, dataset.areaField == nil, dataset.lengthField == nil {
            subtitleParts.append(NumberWords.length(l))
        }
        // Never the title or the layer's name twice ("Keighley · Keighley · Parishes 1851").
        var seen: Set<String> = [dataset.title(of: feature), dataset.name]
        let facts = subtitleParts.filter { seen.insert($0).inserted }
        subtitleParts = facts + [dataset.name]

        var rows: [String] = []
        var strip: [Double] = []
        var highlight: Double?
        let field = dataset.primaryField
        if let field, let raw = feature.properties[field.name], let value = GeoValue.number(raw) {
            let values = dataset.numbers(for: field).map(\.value)
            let n = values.count
            // "Population 66.8 million", but "Population 1851: 10,308" (no number run into a number).
            let joiner = field.label.last.map { $0.isNumber || $0 == ")" } == true ? ": " : " "
            rows.append("\(field.label)\(joiner)\(NumberWords.compact(value))")
            if n >= 3 {
                let higher = values.filter { $0 > value }.count
                let lower = values.filter { $0 < value }.count
                if higher <= lower {
                    let rank = higher + 1
                    rows.append(rank == 1 ? "Highest of \(NumberWords.integer(n))" : "\(NumberWords.ordinal(rank)) highest of \(NumberWords.integer(n))")
                } else {
                    let rank = lower + 1
                    rows.append(rank == 1 ? "Lowest of \(NumberWords.integer(n))" : "\(NumberWords.ordinal(rank)) lowest of \(NumberWords.integer(n))")
                }
                let m = median(values)
                let diff = value - m
                if abs(diff) < 1e-9 {
                    rows.append("Right at the middle value")
                } else {
                    let word = diff > 0 ? "above" : "below"
                    rows.append("\(NumberWords.compact(abs(diff))) \(word) the middle value")
                }
                // Rank space, so one giant does not squash everyone else into a corner.
                let sorted = values.sorted()
                let denom = Double(max(1, n - 1))
                strip = sorted.enumerated().map { Double($0.offset) / denom }
                highlight = Double(lower) / denom
            }
        }

        let titleNames = Set(dataset.nameFields.isEmpty ? [dataset.titleField].compactMap { $0 } : dataset.nameFields)
        let details: [(String, String)] = dataset.visibleFields
            .filter { !titleNames.contains($0.name) && $0.name != field?.name && !shownInSubtitle.contains($0.name) }
            .compactMap { f in
                guard let v = feature.properties[f.name], let s = GeoValue.text(v), !s.isEmpty else { return nil }
                let shown: String
                if f.isYear, let d = GeoValue.number(v) {
                    shown = NumberWords.year(d)
                } else {
                    shown = f.kind == .number ? (GeoValue.number(v).map(NumberWords.compact) ?? s) : s
                }
                return (f.label, shown)
            }
        let notes = dataset.notesField
            .flatMap { feature.properties[$0] }
            .flatMap(GeoValue.text)
            .map(plainText)
            .flatMap { $0.isEmpty ? nil : $0 }

        return FeatureInsight(
            layerID: layerID,
            featureIndex: feature.index,
            title: dataset.title(of: feature),
            subtitle: subtitleParts.joined(separator: " · "),
            facts: facts,
            notes: notes,
            rows: Array(rows.prefix(3)),
            strip: strip,
            stripHighlight: highlight,
            field: field,
            colorField: field ?? dataset.colorFields.first { $0.kind == .category },
            details: details
        )
    }

    static func dataset(_ dataset: GeoDataset) -> DatasetSummary {
        var rows: [String] = []
        if let r = dataset.raster {
            let size = "Each square is about \(NumberWords.length(r.cellMeters / 1000)) across."
            if r.look == .values {
                rows = ["\(r.quantity) from \(r.format(r.min)) to \(r.format(r.max))", "Tap the map to read the \(r.quantity.lowercased()) at any spot.", size]
            } else {
                rows = ["A picture of a map, placed where it belongs.", "Change look › See-through to compare it with today's map.", size]
            }
            return DatasetSummary(headline: dataset.kind.plural, rows: rows)
        }
        if let field = dataset.primaryField {
            let pairs = dataset.numbers(for: field)
            if let top = pairs.max(by: { $0.value < $1.value }), let bottom = pairs.min(by: { $0.value < $1.value }),
               let topF = dataset.features[safe: top.index], let bottomF = dataset.features[safe: bottom.index] {
                rows.append("Highest \(field.label.lowercased()): \(dataset.title(of: topF)) (\(NumberWords.compact(top.value)))")
                rows.append("Lowest: \(dataset.title(of: bottomF)) (\(NumberWords.compact(bottom.value)))")
                rows.append("Middle value: \(NumberWords.compact(median(pairs.map(\.value))))")
            }
        }
        if rows.isEmpty, let category = dataset.visibleFields.first(where: { $0.kind == .category }) {
            var counts: [String: Int] = [:]
            for f in dataset.features {
                if let v = f.properties[category.name], let s = GeoValue.text(v) { counts[s, default: 0] += 1 }
            }
            if let most = counts.max(by: { $0.value < $1.value }) {
                rows.append("Most common \(category.label.lowercased()): \(most.key) (\(NumberWords.integer(most.value)))")
            }
        }
        return DatasetSummary(headline: "\(dataset.countPhrase) · \(dataset.kind.plural)", rows: rows)
    }

    /// A one-line label for "N things here" that tells similar ones apart: "Thomas Ackroyd · Born ·
    /// 1812", "Ascension Island region · Magnitude 6.1 · 1998-03-14". The caller adds the layer name.
    static func shortLabel(_ feature: GeoFeature, in dataset: GeoDataset) -> String {
        let insight = Insights.feature(feature, in: dataset, layerID: UUID())
        // Two short facts at most; sizes ("16.1 km²") don't tell neighbours apart.
        let sizes = [" km²", " ha", " km", " m"]
        var extras = insight.facts.filter { f in !sizes.contains { f.hasSuffix($0) } }
        if let first = insight.rows.first { extras = Array(extras.prefix(1)) + [first] }
        if dataset.yearField == nil, let date = dataset.fields.first(where: { ["date", "time", "datetime"].contains($0.name.lowercased()) }),
           let v = feature.properties[date.name].flatMap(GeoValue.text), !v.isEmpty {
            extras.append(String(v.prefix(10)))
        }
        // People keep their town ("Mary Ackroyd · Born · 1815 · Keighley"): without it two rows
        // read as if they were at the same place.
        let room = dataset.nameFields.isEmpty ? 2 : 3
        return ([insight.title] + extras.prefix(room)).joined(separator: " · ")
    }

    /// KML descriptions are often HTML ("<b>Café</b>"): show the words.
    static func plainText(_ s: String) -> String {
        var out = s.replacingOccurrences(of: "<br>", with: "\n", options: .caseInsensitive)
        out = out.replacingOccurrences(of: "<[^>]+>", with: "", options: .regularExpression)
        for (entity, char) in [("&amp;", "&"), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&#39;", "'"), ("&nbsp;", " ")] {
            out = out.replacingOccurrences(of: entity, with: char)
        }
        return out.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func median(_ values: [Double]) -> Double {
        guard !values.isEmpty else { return 0 }
        let s = values.sorted()
        let mid = s.count / 2
        return s.count % 2 == 0 ? (s[mid - 1] + s[mid]) / 2 : s[mid]
    }
}

extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
