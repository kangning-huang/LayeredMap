import SwiftUI
import CoreLocation

/// Tap anything → card, like a place card in Apple Maps: name, kind and size, up to three insight
/// rows, up to three actions (redesign › Tap anything → card).
struct FeatureCard: View {
    var insight: FeatureInsight
    var layer: MapLayer
    var onClose: () -> Void
    var onColorBy: (String) -> Void
    var onZoom: () -> Void
    var onList: () -> Void

    @State private var showDetails = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                CardHeader(title: insight.title, subtitle: insight.subtitle, onClose: onClose)
                if !insight.rows.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(Array(insight.rows.enumerated()), id: \.offset) { i, row in
                            Text(row)
                                .font(i == 0 ? .title3.weight(.semibold) : .body)
                                .foregroundStyle(i == 0 ? .primary : .secondary)
                        }
                        if !insight.strip.isEmpty {
                            DotStrip(values: insight.strip, highlight: insight.stripHighlight)
                                .frame(height: 22)
                                .padding(.top, 4)
                        }
                    }
                    .padding(14)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                }
                HStack(spacing: 8) {
                    if let field = insight.field {
                        ActionButton(title: "Color by \(field.label.lowercased())", systemImage: "paintpalette") {
                            onColorBy(field.name)
                        }
                    }
                    ActionButton(title: "Zoom to", systemImage: "plus.magnifyingglass", action: onZoom)
                    ActionButton(title: "See all", systemImage: "list.number", action: onList)
                }
                if !insight.details.isEmpty {
                    DisclosureGroup("More details", isExpanded: $showDetails) {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(Array(insight.details.enumerated()), id: \.offset) { _, d in
                                HStack(alignment: .firstTextBaseline) {
                                    Text(d.0).foregroundStyle(.secondary)
                                    Spacer()
                                    Text(d.1).multilineTextAlignment(.trailing)
                                }
                                .font(.subheadline)
                            }
                        }
                        .padding(.top, 6)
                    }
                    .tint(.primary)
                }
                SourceLine(dataset: layer.dataset)
            }
            .padding(.horizontal, 18)
            .padding(.top, 18)
            .padding(.bottom, 24)
        }
    }
}

/// Tap a data row → data card: legend, assumptions (purple dot), summary, three actions.
struct DataCard: View {
    var layer: MapLayer
    var onClose: () -> Void
    var onChangeLook: () -> Void
    var onList: () -> Void
    var onZoom: () -> Void

    var body: some View {
        let summary = Insights.dataset(layer.dataset)
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack(alignment: .top, spacing: 12) {
                    LegendSwatch(kind: layer.dataset.kind, colors: layer.scheme.swatch, opacity: layer.style.opacity)
                    CardHeader(title: layer.name, subtitle: summary.headline, onClose: onClose)
                }
                if case .single = layer.scheme {} else {
                    VStack(alignment: .leading, spacing: 4) {
                        if let by = layer.style.showBy, let f = layer.dataset.field(named: by) {
                            Text("Colored by \(f.label.lowercased())").font(.caption).foregroundStyle(.secondary)
                        }
                        LegendBar(scheme: layer.scheme)
                    }
                }
                if !layer.dataset.assumptions.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(layer.dataset.assumptions, id: \.self) { a in
                            HStack(alignment: .firstTextBaseline, spacing: 8) {
                                Circle().fill(Color.purple).frame(width: 8, height: 8)
                                Text(a).font(.subheadline)
                            }
                        }
                    }
                }
                if !summary.rows.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(summary.rows, id: \.self) { Text($0) }
                    }
                    .padding(14)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                }
                HStack(spacing: 8) {
                    ActionButton(title: "Change look", systemImage: "paintpalette", action: onChangeLook)
                    ActionButton(title: "View as list", systemImage: "list.bullet", action: onList)
                    ActionButton(title: "Zoom to", systemImage: "plus.magnifyingglass", action: onZoom)
                }
                SourceLine(dataset: layer.dataset)
            }
            .padding(.horizontal, 18)
            .padding(.top, 18)
            .padding(.bottom, 24)
        }
    }
}

/// Long-press drops a pin and shows the value of every visible layer at that point.
struct PinCard: View {
    @ObservedObject var doc: MapDocument
    var coordinate: CLLocationCoordinate2D?
    var hits: [MapHit]
    var onClose: () -> Void
    var onOpen: (MapHit) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                CardHeader(title: "Dropped pin", subtitle: coordText, onClose: onClose)
                if hits.isEmpty {
                    Text("No data right here.").foregroundStyle(.secondary)
                }
                ForEach(hits, id: \.self) { hit in
                    if case let (layer, f)? = doc.feature(hit) {
                        Button {
                            onOpen(hit)
                        } label: {
                            HStack {
                                LegendSwatch(kind: layer.dataset.kind, colors: layer.scheme.swatch, opacity: layer.style.opacity)
                                VStack(alignment: .leading) {
                                    Text(layer.dataset.title(of: f)).foregroundStyle(.primary)
                                    Text(valueText(layer, f)).font(.caption).foregroundStyle(.secondary)
                                }
                                Spacer()
                                Image(systemName: "chevron.right").foregroundStyle(.tertiary)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .padding(18)
        }
    }

    private var coordText: String {
        guard let c = coordinate else { return "" }
        return String(format: "%.3f°, %.3f°", c.latitude, c.longitude)
    }

    private func valueText(_ layer: MapLayer, _ f: GeoFeature) -> String {
        guard let field = layer.dataset.primaryField, let v = f.properties[field.name], let d = GeoValue.number(v) else {
            return layer.name
        }
        return "\(layer.name) · \(field.label) \(NumberWords.compact(d))"
    }
}

/// View as list: like Contacts on iPhone. Search, sort, tap a row → its card.
struct FeatureList: View {
    var layer: MapLayer
    var onClose: () -> Void
    var onPick: (Int) -> Void

    @State private var search = ""
    @State private var byValue = true

    var body: some View {
        let ds = layer.dataset
        let field = ds.primaryField
        let rows = sortedRows(ds, field)
        VStack(spacing: 0) {
            CardHeader(title: ds.name, subtitle: ds.countPhrase, onClose: onClose)
                .padding(.horizontal, 18)
                .padding(.top, 18)
            HStack {
                TextField("Search \(ds.name.lowercased())", text: $search)
                    .padding(.horizontal, 12)
                    .frame(height: 40)
                    .background(Color(.tertiarySystemFill), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                if let field {
                    Menu {
                        Picker("Sort", selection: $byValue) {
                            Text("By \(field.label.lowercased())").tag(true)
                            Text("By name").tag(false)
                        }
                    } label: {
                        Image(systemName: "arrow.up.arrow.down").frame(width: 44, height: 40)
                    }
                    .accessibilityLabel("Sort")
                }
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 10)
            List(rows, id: \.index) { f in
                Button {
                    onPick(f.index)
                } label: {
                    HStack {
                        Circle().fill(Color(hex: layer.scheme.color(for: layer.style.showBy.flatMap { f.properties[$0] })))
                            .frame(width: 10, height: 10)
                        Text(ds.title(of: f)).foregroundStyle(.primary)
                        Spacer()
                        if let field, let v = f.properties[field.name], let d = GeoValue.number(v) {
                            Text(NumberWords.compact(d)).foregroundStyle(.secondary).monospacedDigit()
                        }
                    }
                }
            }
            .listStyle(.plain)
        }
    }

    private func sortedRows(_ ds: GeoDataset, _ field: FieldInfo?) -> [GeoFeature] {
        let q = search.lowercased()
        var rows = q.isEmpty ? ds.features : ds.features.filter { ds.title(of: $0).lowercased().contains(q) }
        if byValue, let field {
            rows.sort { a, b in
                (a.properties[field.name].flatMap(GeoValue.number) ?? -.infinity) > (b.properties[field.name].flatMap(GeoValue.number) ?? -.infinity)
            }
        } else {
            rows.sort { ds.title(of: $0) < ds.title(of: $1) }
        }
        return Array(rows.prefix(400))
    }
}

// MARK: Pieces

struct CardHeader: View {
    var title: String
    var subtitle: String
    var onClose: () -> Void

    var body: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.title2.weight(.bold)).lineLimit(2)
                Text(subtitle).font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer()
            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.subheadline.weight(.bold))
                    .foregroundStyle(.secondary)
                    .frame(width: 32, height: 32)
                    .background(Color(.tertiarySystemFill), in: Circle())
                    .frame(width: 44, height: 44)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Close")
        }
    }
}

struct ActionButton: View {
    var title: String
    var systemImage: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 5) {
                Image(systemName: systemImage).font(.body.weight(.semibold))
                Text(title).font(.caption.weight(.semibold)).lineLimit(2).multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity, minHeight: 58)
            .background(Color.accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .foregroundStyle(Color.accentColor)
        }
        .buttonStyle(.plain)
    }
}

/// Where this one sits among all the others: one dot each, this one large.
struct DotStrip: View {
    var values: [Double]
    var highlight: Double?

    var body: some View {
        Canvas { ctx, size in
            let y = size.height / 2
            ctx.fill(Path(CGRect(x: 0, y: y - 0.5, width: size.width, height: 1)), with: .color(.secondary.opacity(0.4)))
            for v in values {
                let x = 4 + v * (size.width - 8)
                ctx.fill(Path(ellipseIn: CGRect(x: x - 2, y: y - 2, width: 4, height: 4)), with: .color(.secondary.opacity(0.5)))
            }
            if let h = highlight {
                let x = 4 + h * (size.width - 8)
                ctx.fill(Path(ellipseIn: CGRect(x: x - 7, y: y - 7, width: 14, height: 14)), with: .color(.accentColor))
            }
        }
        .accessibilityHidden(true)
    }
}

struct SourceLine: View {
    var dataset: GeoDataset

    var body: some View {
        Text("Source: \(dataset.credit ?? dataset.sourceName)")
            .font(.caption)
            .foregroundStyle(.tertiary)
    }
}
