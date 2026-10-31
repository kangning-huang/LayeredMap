import SwiftUI

/// State C: lean symbology for one data layer. Nested under B; Done/Back collapses.
struct SymbologySheet: View {
    @Binding var layer: MapLayer
    @Environment(\.dismiss) private var dismiss

    private let colorPresets: [Color] = [
        .blue, .red, .orange, .yellow, .green, .mint, .teal, .purple, .pink, .gray, .primary,
    ]

    var body: some View {
        Form {
            Section {
                HStack {
                    LegendSwatch(geometry: layer.geometry, symbology: layer.symbology)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(layer.name)
                            .font(.headline)
                        Text(layer.geometry.displayName)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                }
            }

            Section("Color") {
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10), count: 6), spacing: 10) {
                    ForEach(Array(colorPresets.enumerated()), id: \.offset) { _, color in
                        Circle()
                            .fill(color)
                            .frame(width: 28, height: 28)
                            .overlay {
                                if colorsMatch(color, layer.symbology.color) {
                                    Circle()
                                        .strokeBorder(.primary, lineWidth: 2)
                                        .padding(1)
                                }
                            }
                            .onTapGesture {
                                layer.symbology.color = color
                            }
                            .accessibilityLabel("Color preset")
                    }
                }
                .padding(.vertical, 4)
            }

            Section {
                sizeControl
                Slider(value: $layer.symbology.opacity, in: 0.05...1.0) {
                    Text("Opacity")
                } minimumValueLabel: {
                    Text("0")
                } maximumValueLabel: {
                    Text("100%")
                }
                Text("Opacity \(Int((layer.symbology.opacity * 100).rounded()))%")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } header: {
                Text(sizeHeader)
            } footer: {
                Text("Live preview: the legend swatch on the Layers list updates as you change style. MapLibre paint from this model is stubbed in this spike pass.")
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

    private var sizeHeader: String {
        switch layer.geometry {
        case .point: return "Size & opacity"
        case .line: return "Width & opacity"
        case .fill: return "Outline & opacity"
        }
    }

    @ViewBuilder
    private var sizeControl: some View {
        switch layer.geometry {
        case .point:
            Slider(value: $layer.symbology.size, in: 4...20, step: 1) {
                Text("Size")
            }
            Text("Size \(Int(layer.symbology.size))")
                .font(.caption)
                .foregroundStyle(.secondary)
        case .line:
            Slider(value: $layer.symbology.size, in: 1...12, step: 0.5) {
                Text("Width")
            }
            Text("Width \(layer.symbology.size, specifier: "%.1f")")
                .font(.caption)
                .foregroundStyle(.secondary)
        case .fill:
            Slider(value: $layer.symbology.size, in: 0...4, step: 0.5) {
                Text("Outline")
            }
            Text("Outline \(layer.symbology.size, specifier: "%.1f")")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private func colorsMatch(_ a: Color, _ b: Color) -> Bool {
        // Spike: compare description strings; enough for preset chips.
        String(describing: a) == String(describing: b)
    }
}
