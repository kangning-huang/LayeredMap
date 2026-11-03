import SwiftUI
import UIKit

// From the A/B shared file (Alt C copy). Ramps follow spec › Data palettes: viridis by default, "Heat" for
// temperature-like data, Okabe–Ito for categories. No ramp editor.

enum Palette: String, CaseIterable, Identifiable, Codable {
    case viridis
    case heat
    case blues
    /// Grey, for a value grid over a busy map where colour would fight the streets.
    case greys

    var id: String { rawValue }

    var label: String {
        switch self {
        case .viridis: return "Standard"
        case .heat: return "Heat"
        case .blues: return "Blues"
        case .greys: return "Grey"
        }
    }

    /// Low → high.
    var hex: [UInt32] {
        switch self {
        case .viridis: return [0x440154, 0x3B528B, 0x21918C, 0x5EC962, 0xFDE725]
        case .heat: return [0xFFF5B0, 0xFECC5C, 0xFD8D3C, 0xF03B20, 0xBD0026]
        case .blues: return [0xEFF3FF, 0xBDD7E7, 0x6BAED6, 0x3182BD, 0x08519C]
        case .greys: return [0xF7F7F7, 0xCCCCCC, 0x969696, 0x636363, 0x252525]
        }
    }

    var colors: [Color] { hex.map(Color.init(hex:)) }
}

/// Okabe & Ito (2008), colour-blind safe, for categories.
let okabeIto: [UInt32] = [0xE69F00, 0x56B4E9, 0x009E73, 0xF0E442, 0x0072B2, 0xD55E00, 0xCC79A7, 0x999999]

/// One-color choices; first-added layers take them in turn.
let singleColors: [UInt32] = [0x2F6FDE, 0xE4572E, 0x17A398, 0x8E5BD9, 0xF2A541, 0x5C6B73]

enum Classify: String, CaseIterable, Identifiable, Codable {
    /// Smooth ramp through the data's quartiles (robust to one giant value).
    case auto
    /// Five classes with the same number of features each.
    case quantile
    /// Five classes of equal width.
    case equal

    var id: String { rawValue }

    var label: String {
        switch self {
        case .auto: return "Smooth"
        case .quantile: return "Equal counts"
        case .equal: return "Equal steps"
        }
    }
}

struct LayerStyle: Equatable, Codable {
    var color: UInt32
    /// Field name to color by; nil = one color.
    var showBy: String?
    var palette: Palette = .viridis
    var classify: Classify = .auto
    var opacity: Double
    /// Point radius, line width or area outline width, in points.
    var size: Double

    static func `default`(kind: GeometryKind, colorIndex: Int) -> LayerStyle {
        let c = singleColors[colorIndex % singleColors.count]
        switch kind {
        case .point: return LayerStyle(color: c, opacity: 0.85, size: 4)
        case .line: return LayerStyle(color: c, opacity: 0.9, size: 2)
        case .area, .table: return LayerStyle(color: c, opacity: 0.6, size: 0.5)
        case .raster: return LayerStyle(color: c, opacity: 0.85, size: 0)
        }
    }
}

/// How a style colors a data set: one color, a ramp over numbers, or categories.
enum ColorRule {
    case single(UInt32)
    /// `years`: label stops "1851", not "1,851".
    case ramp(stops: [(Double, UInt32)], stepped: Bool, years: Bool)
    /// `more`: some values fell outside the named colors, so the legend lists "Other".
    case categories([(String, UInt32)], other: UInt32, more: Bool)

    static let missing: UInt32 = 0xC8C8C8

    static func make(style: LayerStyle, dataset: GeoDataset) -> ColorRule {
        if let r = dataset.raster {
            // A grid of values is drawn on the Standard ramp between its 2nd and 98th percentiles.
            guard r.look == .values, r.high > r.low else { return .single(0x9A7B4F) }
            let colors = Palette.viridis.hex
            let stops = colors.enumerated().map { (r.low + (r.high - r.low) * Double($0.offset) / Double(colors.count - 1), $0.element) }
            return .ramp(stops: stops, stepped: false, years: false)
        }
        guard let name = style.showBy, let field = dataset.field(named: name) else { return .single(style.color) }
        switch field.kind {
        case .number:
            let values = dataset.numbers(for: field).map(\.value).sorted()
            guard let lo = values.first, let hi = values.last, hi > lo else { return .single(style.color) }
            let colors = style.palette.hex
            let breaks: [Double]
            switch style.classify {
            case .equal:
                breaks = (0..<colors.count).map { lo + (hi - lo) * Double($0) / Double(colors.count - 1) }
            case .auto, .quantile:
                breaks = (0..<colors.count).map { i in
                    let q = Double(i) / Double(colors.count - 1)
                    let pos = q * Double(values.count - 1)
                    return values[Swift.min(values.count - 1, Int(pos.rounded()))]
                }
            }
            // MapLibre needs strictly ascending stops.
            var stops: [(Double, UInt32)] = []
            for (b, c) in zip(breaks, colors) where stops.last.map({ b > $0.0 }) ?? true {
                stops.append((b, c))
            }
            guard stops.count >= 2 else { return .single(style.color) }
            return .ramp(stops: stops, stepped: style.classify != .auto, years: field.isYear)
        case .category:
            var counts: [String: Int] = [:]
            for f in dataset.features {
                if let v = f.properties[field.name], let s = GeoValue.text(v) { counts[s, default: 0] += 1 }
            }
            let ordered = counts.sorted { $0.value == $1.value ? $0.key < $1.key : $0.value > $1.value }.map(\.key)
            let named = ordered.prefix(okabeIto.count - 1)
            let own = field.name == dataset.fileColorsField ? dataset.fileColors : [:]
            // The file's own colors (a My Maps export) win; Okabe–Ito fills the gaps.
            let pairs = named.enumerated().map { ($0.element, own[$0.element] ?? okabeIto[$0.offset]) }
            return .categories(Array(pairs), other: okabeIto[okabeIto.count - 1], more: ordered.count > named.count)
        case .text:
            return .single(style.color)
        }
    }

    /// MapLibre style-spec JSON for the color.
    func expressionJSON(field: String?) -> Any {
        switch self {
        case .single(let c):
            return rgba(c)
        case .ramp(let stops, let stepped, _):
            guard let field else { return rgba(ColorRule.missing) }
            let input: [Any] = ["get", field]
            var body: [Any] = []
            if stepped {
                body = ["step", input, rgba(stops[0].1)]
                for (b, c) in stops.dropFirst() {
                    body.append(b)
                    body.append(rgba(c))
                }
            } else {
                body = ["interpolate", ["linear"], input]
                for (b, c) in stops {
                    body.append(b)
                    body.append(rgba(c))
                }
            }
            return ["case", ["==", ["typeof", input], "number"], body, rgba(ColorRule.missing)]
        case .categories(let pairs, let other, _):
            guard let field, !pairs.isEmpty else { return rgba(other) }
            var body: [Any] = ["match", ["to-string", ["get", field]]]
            for (label, c) in pairs {
                body.append(label)
                body.append(rgba(c))
            }
            body.append(rgba(other))
            return body
        }
    }

    /// The color one value gets, as MapLibre draws it (for thumbnails and list dots).
    func color(for value: Any?) -> UInt32 {
        switch self {
        case .single(let c):
            return c
        case .ramp(let stops, let stepped, _):
            guard let value, let x = GeoValue.number(value), let first = stops.first, let last = stops.last else {
                return ColorRule.missing
            }
            if x <= first.0 { return first.1 }
            if x >= last.0 { return last.1 }
            for i in 1..<stops.count where x < stops[i].0 {
                if stepped { return stops[i - 1].1 }
                let (b0, c0) = stops[i - 1]
                let (b1, c1) = stops[i]
                return lerp(c0, c1, (x - b0) / (b1 - b0))
            }
            return last.1
        case .categories(let pairs, let other, _):
            guard let value, let s = GeoValue.text(value) else { return other }
            return pairs.first { $0.0 == s }?.1 ?? other
        }
    }

    private func lerp(_ a: UInt32, _ b: UInt32, _ t: Double) -> UInt32 {
        func ch(_ c: UInt32, _ shift: UInt32) -> Double { Double((c >> shift) & 0xFF) }
        func mix(_ shift: UInt32) -> UInt32 { UInt32((ch(a, shift) + (ch(b, shift) - ch(a, shift)) * t).rounded()) & 0xFF }
        return (mix(16) << 16) | (mix(8) << 8) | mix(0)
    }

    /// Every color this rule can draw.
    var hexes: [UInt32] {
        switch self {
        case .single(let c): return [c]
        case .ramp(let stops, _, _): return stops.map(\.1)
        case .categories(let pairs, let other, _): return pairs.map(\.1) + [other]
        }
    }

    /// The one-color choice farthest from colors already on the map, so a new layer stands out
    /// (Walt's teal places vanished into the earthquakes' teal).
    static func freshColor(avoiding used: [UInt32], fallback: Int) -> UInt32 {
        guard !used.isEmpty else { return singleColors[fallback % singleColors.count] }
        func distance(_ a: UInt32, _ b: UInt32) -> Double {
            let d = [16, 8, 0].map { Double(Int((a >> UInt32($0)) & 0xFF) - Int((b >> UInt32($0)) & 0xFF)) }
            return (d[0] * d[0] * 2 + d[1] * d[1] * 4 + d[2] * d[2] * 3).squareRoot()
        }
        return singleColors.max { a, b in
            (used.map { distance(a, $0) }.min() ?? 0) < (used.map { distance(b, $0) }.min() ?? 0)
        } ?? singleColors[0]
    }

    /// Swatch colors for legends and list rows (the list matches the map).
    var swatch: [Color] {
        switch self {
        case .single(let c): return [Color(hex: c)]
        case .ramp(let stops, _, _): return stops.map { Color(hex: $0.1) }
        case .categories(let pairs, _, _): return pairs.prefix(4).map { Color(hex: $0.1) }
        }
    }

    /// Legend rows: "low → high" ends, or category labels.
    var legend: [(String, Color)] {
        switch self {
        case .single: return []
        case .ramp(let stops, _, let years):
            return stops.map { (ColorRule.label($0.0, years: years), Color(hex: $0.1)) }
        case .categories(let pairs, let other, let more):
            return pairs.map { ($0.0, Color(hex: $0.1)) } + (more ? [("Other", Color(hex: other))] : [])
        }
    }

    static func label(_ v: Double, years: Bool) -> String {
        years ? NumberWords.year(v) : NumberWords.compact(v)
    }

    /// Low and high ends of a ramp, for legends.
    var ends: (String, String)? {
        switch self {
        case .ramp(let stops, _, let years):
            guard let lo = stops.first, let hi = stops.last else { return nil }
            return (ColorRule.label(lo.0, years: years), ColorRule.label(hi.0, years: years))
        default:
            return nil
        }
    }

    private func rgba(_ c: UInt32) -> [Any] {
        ["rgba", Double((c >> 16) & 0xFF), Double((c >> 8) & 0xFF), Double(c & 0xFF), 1.0]
    }
}

extension Color {
    init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255
        )
    }
}

extension UIColor {
    convenience init(hex: UInt32, alpha: CGFloat = 1) {
        self.init(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: alpha
        )
    }
}

/// Live legend sample for a data row: the current look, never a type glyph.
struct LegendSwatch: View {
    var kind: GeometryKind
    var colors: [Color]
    var opacity: Double

    private let box: CGFloat = 30

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(Color(.secondarySystemFill))
            sample.opacity(max(0.35, opacity))
        }
        .frame(width: box, height: box)
        .accessibilityHidden(true)
    }

    private var fill: AnyShapeStyle {
        if colors.count > 1 {
            return AnyShapeStyle(LinearGradient(colors: colors, startPoint: .bottomLeading, endPoint: .topTrailing))
        }
        return AnyShapeStyle(colors.first ?? .blue)
    }

    @ViewBuilder
    private var sample: some View {
        switch kind {
        case .point:
            if colors.count > 1 {
                HStack(spacing: 2) {
                    ForEach(Array(colors.prefix(3).enumerated()), id: \.offset) { _, c in
                        Circle().fill(c).frame(width: 7, height: 7)
                    }
                }
            } else {
                Circle().fill(fill).frame(width: 13, height: 13)
            }
        case .line:
            Capsule().fill(fill).frame(width: 20, height: 4).rotationEffect(.degrees(-30))
        case .area, .table:
            RoundedRectangle(cornerRadius: 3, style: .continuous).fill(fill).frame(width: 18, height: 18)
        case .raster:
            Image(systemName: colors.count > 1 ? "square.grid.3x3.fill" : "photo")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(colors.count > 1 ? fill : AnyShapeStyle(Color.brown))
        }
    }
}

/// Small horizontal legend bar for a ramp or categories.
struct LegendBar: View {
    var scheme: ColorRule
    /// Words for the two ends ("Low 12 m", "High 227 m"); the numbers alone otherwise.
    var ends: (String, String)?

    var body: some View {
        switch scheme {
        case .single:
            EmptyView()
        case .ramp(let stops, let stepped, _):
            VStack(alignment: .leading, spacing: 3) {
                if stepped {
                    HStack(spacing: 0) {
                        ForEach(Array(stops.enumerated()), id: \.offset) { _, s in
                            Rectangle().fill(Color(hex: s.1))
                        }
                    }
                    .frame(height: 10)
                    .clipShape(Capsule())
                } else {
                    Capsule()
                        .fill(LinearGradient(colors: stops.map { Color(hex: $0.1) }, startPoint: .leading, endPoint: .trailing))
                        .frame(height: 10)
                }
                HStack {
                    Text(ends?.0 ?? scheme.ends?.0 ?? "")
                    Spacer()
                    Text(ends?.1 ?? scheme.ends?.1 ?? "")
                }
                .font(.caption2)
                .foregroundStyle(.secondary)
            }
        case .categories:
            FlowLegend(items: scheme.legend)
        }
    }
}

struct FlowLegend: View {
    var items: [(String, Color)]

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 96), alignment: .leading)], alignment: .leading, spacing: 4) {
            ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                HStack(spacing: 5) {
                    Circle().fill(item.1).frame(width: 9, height: 9)
                    Text(item.0).font(.caption2).lineLimit(1)
                }
            }
        }
    }
}
