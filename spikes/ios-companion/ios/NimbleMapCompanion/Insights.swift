import Foundation

// Shared by Alt A and Alt B. Every number here comes from the data, never from drawn pixels.

/// The desktop object card's content (spec › Inspector: object card): name, kind and size, up to
/// three insight rows, a strip of where this one sits among the rest.
struct FeatureInsight: Identifiable {
    let id = UUID()
    let layerID: UUID
    let featureIndex: Int
    let title: String
    let subtitle: String
    let rows: [String]
    /// Values of the insight field scaled to 0…1, and this feature's position, for the dot strip.
    let strip: [Double]
    let stripHighlight: Double?
    let field: FieldInfo?
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
        var subtitleParts = [dataset.name]
        if let category = dataset.fields.first(where: { $0.kind == .category && $0.name.lowercased() != "capital" }),
           let v = feature.properties[category.name], let s = GeoValue.text(v) {
            subtitleParts.append(s)
        }
        if let a = feature.areaKm2, a > 0 {
            subtitleParts.append(NumberWords.area(a))
        } else if let l = feature.lengthKm, l > 0 {
            subtitleParts.append(NumberWords.length(l))
        }

        var rows: [String] = []
        var strip: [Double] = []
        var highlight: Double?
        let field = dataset.primaryField
        if let field, let raw = feature.properties[field.name], let value = GeoValue.number(raw) {
            let values = dataset.numbers(for: field).map(\.value)
            let n = values.count
            rows.append("\(field.label) \(NumberWords.compact(value))")
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

        let details: [(String, String)] = dataset.fields
            .filter { $0.name != dataset.titleField && $0.name != field?.name }
            .compactMap { f in
                guard let v = feature.properties[f.name], let s = GeoValue.text(v), !s.isEmpty else { return nil }
                let shown = f.kind == .number ? (GeoValue.number(v).map(NumberWords.compact) ?? s) : s
                return (f.label, shown)
            }

        return FeatureInsight(
            layerID: layerID,
            featureIndex: feature.index,
            title: dataset.title(of: feature),
            subtitle: subtitleParts.joined(separator: " · "),
            rows: Array(rows.prefix(3)),
            strip: strip,
            stripHighlight: highlight,
            field: field,
            details: details
        )
    }

    static func dataset(_ dataset: GeoDataset) -> DatasetSummary {
        var rows: [String] = []
        if let field = dataset.primaryField {
            let pairs = dataset.numbers(for: field)
            if let top = pairs.max(by: { $0.value < $1.value }), let bottom = pairs.min(by: { $0.value < $1.value }),
               let topF = dataset.features[safe: top.index], let bottomF = dataset.features[safe: bottom.index] {
                rows.append("Highest \(field.label.lowercased()): \(dataset.title(of: topF)) (\(NumberWords.compact(top.value)))")
                rows.append("Lowest: \(dataset.title(of: bottomF)) (\(NumberWords.compact(bottom.value)))")
                rows.append("Middle value: \(NumberWords.compact(median(pairs.map(\.value))))")
            }
        }
        if rows.isEmpty, let category = dataset.fields.first(where: { $0.kind == .category }) {
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
