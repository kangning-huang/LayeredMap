import SwiftUI

/// State C: lean symbology for one data layer, nested under B; Done/Back collapses. v1 scope:
/// field to color by, about three color themes, classify (auto + two presets), opacity.
struct SymbologySheet: View {
    @ObservedObject var doc: MapDocument
    var layerID: UUID
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        if let layer = doc.layer(layerID) {
            form(layer)
        } else {
            Text("This layer was removed.").foregroundStyle(.secondary)
        }
    }

    private func form(_ layer: MapLayer) -> some View {
        let ds = layer.dataset
        let fields = ds.fields.filter { $0.kind != .text }
        let field = layer.style.showBy.flatMap { ds.field(named: $0) }
        return Form {
            Section {
                HStack {
                    LegendSwatch(kind: ds.kind, colors: layer.scheme.swatch, opacity: layer.style.opacity)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(layer.name).font(.headline)
                        Text(ds.countPhrase).font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                }
                if case .single = layer.scheme {} else {
                    LegendBar(scheme: layer.scheme)
                }
            }

            Section("Color by") {
                Picker("Field", selection: binding(layer, \.showBy)) {
                    Text("One color").tag(String?.none)
                    ForEach(fields) { f in
                        Text(f.label).tag(Optional(f.name))
                    }
                }
                if field == nil {
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10), count: 6), spacing: 10) {
                        ForEach(singleColors, id: \.self) { c in
                            Circle()
                                .fill(Color(hex: c))
                                .frame(width: 30, height: 30)
                                .overlay {
                                    if layer.style.color == c {
                                        Circle().strokeBorder(.primary, lineWidth: 2).padding(-3)
                                    }
                                }
                                .onTapGesture {
                                    var s = layer.style
                                    s.color = c
                                    doc.setStyle(layerID, s)
                                }
                                .accessibilityLabel("Color")
                        }
                    }
                    .padding(.vertical, 4)
                } else if field?.kind == .number {
                    Picker("Colors", selection: binding(layer, \.palette)) {
                        ForEach(Palette.allCases) { p in
                            Text(p.label).tag(p)
                        }
                    }
                    Picker("Classify", selection: binding(layer, \.classify)) {
                        ForEach(Classify.allCases) { c in
                            Text(c.label).tag(c)
                        }
                    }
                }
            }

            Section {
                Slider(value: binding(layer, \.opacity), in: 0.1...1.0) {
                    Text("Opacity")
                } minimumValueLabel: {
                    Text("0")
                } maximumValueLabel: {
                    Text("100%")
                }
                Slider(value: binding(layer, \.size), in: sizeRange(ds.kind), step: 0.5) {
                    Text(sizeLabel(ds.kind))
                }
            } header: {
                Text("\(sizeLabel(ds.kind)) & opacity")
            } footer: {
                Text("Live preview: the legend swatch and the map both update as you change style.")
            }
        }
        .navigationTitle("Style")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Done") { dismiss() }
            }
        }
    }

    private func binding<T>(_ layer: MapLayer, _ key: WritableKeyPath<LayerStyle, T>) -> Binding<T> {
        Binding(
            get: { doc.layer(layerID)?.style[keyPath: key] ?? layer.style[keyPath: key] },
            set: { v in
                guard var s = doc.layer(layerID)?.style else { return }
                s[keyPath: key] = v
                doc.setStyle(layerID, s)
            }
        )
    }

    private func sizeLabel(_ kind: GeometryKind) -> String {
        switch kind {
        case .point: return "Size"
        case .line: return "Width"
        case .area, .table: return "Outline"
        }
    }

    private func sizeRange(_ kind: GeometryKind) -> ClosedRange<Double> {
        switch kind {
        case .point: return 2...12
        case .line: return 0.5...8
        case .area, .table: return 0...3
        }
    }
}
