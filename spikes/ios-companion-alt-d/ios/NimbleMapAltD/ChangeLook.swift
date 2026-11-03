import SwiftUI

/// Change look, like Photos edit mode: the map stays big, a strip sits at the bottom. "Show by"
/// lists the data's fields as small previews of the data colored by each one (redesign › 5).
struct ChangeLookStrip: View {
    var layer: MapLayer
    var onPick: (String?) -> Void
    var onOpacity: (Double) -> Void
    var onRamp: (Palette) -> Void = { _ in }

    var body: some View {
        let ds = layer.dataset
        let choices: [FieldInfo?] = [nil] + ds.colorFields.prefix(8).map { Optional($0) }
        VStack(alignment: .leading, spacing: 12) {
            if ds.raster == nil {
            Text("Show by")
                .font(.headline)
                .padding(.horizontal, 18)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(Array(choices.enumerated()), id: \.offset) { _, field in
                        let selected = layer.style.showBy == field?.name
                        Button {
                            onPick(field?.name)
                        } label: {
                            VStack(spacing: 6) {
                                DataThumbnail(dataset: ds, style: previewStyle(field))
                                    .frame(width: 92, height: 62)
                                    .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                                    .overlay(
                                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                                            .strokeBorder(selected ? Color.accentColor : Color.secondary.opacity(0.25), lineWidth: selected ? 3 : 1)
                                    )
                                Text(field?.label ?? "One color")
                                    .font(.caption.weight(selected ? .bold : .regular))
                                    .lineLimit(1)
                                    .frame(width: 92)
                            }
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(selected ? .isSelected : [])
                    }
                }
                .padding(.horizontal, 18)
            }
            } else {
                // A grid of values gets the same choice of ramps as any other coloured layer.
                // It has to be drawn again rather than restyled, which is why it is a tap and a
                // moment's wait rather than instant like the vector previews above.
                if layer.dataset.raster?.look == .values {
                    Text("Colors").font(.headline).padding(.horizontal, 18)
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 12) {
                            ForEach(Palette.allCases) { p in
                                let chosen = layer.dataset.raster?.ramp == p
                                Button { onRamp(p) } label: {
                                    VStack(spacing: 6) {
                                        LinearGradient(colors: p.colors, startPoint: .leading, endPoint: .trailing)
                                            .frame(width: 92, height: 40)
                                            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                                            .overlay(
                                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                                    .strokeBorder(chosen ? Color.accentColor : Color.secondary.opacity(0.25), lineWidth: chosen ? 3 : 1)
                                            )
                                        Text(p.label).font(.caption.weight(chosen ? .bold : .regular)).frame(width: 92)
                                    }
                                }
                                .buttonStyle(.plain)
                                .accessibilityAddTraits(chosen ? .isSelected : [])
                            }
                        }
                        .padding(.horizontal, 18)
                    }
                }
                Text("See-through").font(.headline).padding(.horizontal, 18)
                Text(layer.dataset.raster?.look == .picture ? "Slide left to see today's map through it." : "Slide left to see the streets through it.").font(.subheadline).foregroundStyle(.secondary).padding(.horizontal, 18)
            }
            HStack(spacing: 10) {
                Image(systemName: "circle.lefthalf.filled").foregroundStyle(.secondary)
                Slider(value: Binding(get: { layer.style.opacity }, set: onOpacity), in: (layer.dataset.raster == nil ? 0.15 : 0)...1)
                    .accessibilityLabel("See-through")
            }
            .padding(.horizontal, 18)
        }
        .padding(.top, 20)
    }

    private func previewStyle(_ field: FieldInfo?) -> LayerStyle {
        var s = layer.style
        s.showBy = field?.name
        s.opacity = 1
        return s
    }
}

/// Alt C: B's strip in a sheet that can always close. Cancel and Done live inside the sheet (B's
/// top bar sat behind the dimmed map and was dead, persona study bug 1); a swipe down keeps the look.
/// The map behind stays live, so the new colors show as you pick.
struct ChangeLookSheet: View {
    var layer: MapLayer
    var onPick: (String?) -> Void
    var onOpacity: (Double) -> Void
    var onCancel: () -> Void
    var onDone: () -> Void
    var onMore: () -> Void
    var onTime: (Int) -> Void = { _ in }
    var onRamp: (Palette) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Button("Cancel", action: onCancel)
                Spacer()
                Text("Change look").font(.headline)
                Spacer()
                Button("Done", action: onDone).fontWeight(.semibold)
            }
            .padding(.horizontal, 18)
            .padding(.top, 16)
            ChangeLookStrip(layer: layer, onPick: onPick, onOpacity: onOpacity, onRamp: onRamp)
            if case .single = layer.scheme {} else {
                LegendBar(scheme: layer.scheme, ends: layer.legendEnds)
                    .padding(.horizontal, 18)
                    .padding(.top, 10)
            }
            // A grid with a time axis: the same slider as the card, so time is reachable from
            // Layers without having to tap the map first.
            if let r = layer.dataset.raster, r.hasTime {
                TimeSlider(raster: r, onSet: onTime, onStep: { onTime(r.time + $0) })
                    .padding(.horizontal, 18)
                    .padding(.top, 12)
            }
            if layer.dataset.raster == nil {
                Button(action: onMore) {
                    Label("More options: colors, steps, size", systemImage: "slider.horizontal.3")
                        .font(.subheadline)
                }
                .padding(.horizontal, 18)
                .padding(.top, 12)
            }
            Spacer(minLength: 0)
        }
        .presentationDetents([.height(360), .large])
        .presentationBackgroundInteraction(.enabled)
        .presentationDragIndicator(.visible)
    }
}

/// A tiny drawing of the data itself, colored as the map would be.
struct DataThumbnail: View, Equatable {
    let dataset: GeoDataset
    let style: LayerStyle

    static func == (a: DataThumbnail, b: DataThumbnail) -> Bool {
        a.dataset.id == b.dataset.id && a.style == b.style
    }

    var body: some View {
        let scheme = ColorRule.make(style: style, dataset: dataset)
        let shapes = ThumbGeometry.shapes(for: dataset)
        let colors = dataset.features.map { f in scheme.color(for: style.showBy.flatMap { f.properties[$0] }) }
        Canvas { ctx, size in
            ctx.fill(Path(CGRect(origin: .zero, size: size)), with: .color(Color(.secondarySystemBackground)))
            for (i, shape) in shapes.enumerated() {
                let c = Color(hex: colors[safe: i] ?? style.color)
                switch shape {
                case .rings(let rings):
                    var p = Path()
                    for ring in rings {
                        guard let first = ring.first else { continue }
                        p.move(to: CGPoint(x: first.x * size.width, y: first.y * size.height))
                        for pt in ring.dropFirst() { p.addLine(to: CGPoint(x: pt.x * size.width, y: pt.y * size.height)) }
                        p.closeSubpath()
                    }
                    ctx.fill(p, with: .color(c))
                case .line(let pts):
                    var p = Path()
                    guard let first = pts.first else { continue }
                    p.move(to: CGPoint(x: first.x * size.width, y: first.y * size.height))
                    for pt in pts.dropFirst() { p.addLine(to: CGPoint(x: pt.x * size.width, y: pt.y * size.height)) }
                    ctx.stroke(p, with: .color(c), lineWidth: 1)
                case .point(let pt):
                    ctx.fill(Path(CGRect(x: pt.x * size.width - 1, y: pt.y * size.height - 1, width: 2, height: 2)), with: .color(c))
                }
            }
        }
        .drawingGroup()
    }
}

/// Feature outlines scaled to 0…1 inside the data's extent, built once per data set.
enum ThumbGeometry {
    enum Shape {
        case rings([[CGPoint]])
        case line([CGPoint])
        case point(CGPoint)
    }

    @MainActor private static var cache: [UUID: [Shape]] = [:]

    @MainActor static func shapes(for ds: GeoDataset) -> [Shape] {
        if let c = cache[ds.id] { return c }
        let b = ds.bounds
        let w = max(1e-9, b.maxLon - b.minLon)
        let h = max(1e-9, b.maxLat - b.minLat)
        func norm(_ a: Any) -> CGPoint? {
            guard let p = a as? [Any], p.count >= 2, let x = GeoValue.number(p[0]), let y = GeoValue.number(p[1]) else { return nil }
            return CGPoint(x: (x - b.minLon) / w, y: 1 - (y - b.minLat) / h)
        }
        func ring(_ a: Any) -> [CGPoint] {
            let all = (a as? [Any] ?? []).compactMap(norm)
            let step = max(1, all.count / 40)
            return stride(from: 0, to: all.count, by: step).map { all[$0] }
        }
        let shapes: [Shape] = ds.features.map { f in
            let g = f.raw["geometry"] as? [String: Any]
            let coords = g?["coordinates"]
            switch g?["type"] as? String {
            case "Polygon":
                return .rings((coords as? [Any] ?? []).prefix(1).map(ring))
            case "MultiPolygon":
                return .rings((coords as? [Any] ?? []).compactMap { ($0 as? [Any])?.first }.map(ring))
            case "LineString":
                return .line(ring(coords as Any))
            case "MultiLineString":
                return .line((coords as? [Any] ?? []).first.map(ring) ?? [])
            default:
                let c = f.center
                return .point(CGPoint(x: (c.longitude - b.minLon) / w, y: 1 - (c.latitude - b.minLat) / h))
            }
        }
        cache[ds.id] = shapes
        return shapes
    }
}
