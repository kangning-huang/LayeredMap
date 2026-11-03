import SwiftUI

/// A's Layers list (eye, Edit to reorder, swipe Remove), where a row opens B's data card: legend,
/// what's in it ("Most common category: Food (8)"), Change look, View as list, Zoom to.
struct LayersSheet: View {
    @ObservedObject var doc: MapDocument
    var onLook: (UUID) -> Void
    var onList: (UUID) -> Void
    var onOpen: () -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var path: [UUID] = []

    var body: some View {
        NavigationStack(path: $path) {
            List {
                Section {
                    if doc.layers.isEmpty {
                        Button("Open a map file", action: onOpen)
                    }
                    ForEach(doc.layers) { layer in
                        row(layer)
                    }
                    .onMove(perform: doc.move)
                    .onDelete { offsets in
                        offsets.compactMap { doc.layers[safe: $0]?.id }.forEach(doc.remove)
                    }
                } footer: {
                    Text("Top of the list draws on top. New places go above areas so they're never covered. Swipe left to remove; Edit to reorder.")
                }
            }
            .navigationTitle("Layers")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } }
                ToolbarItem(placement: .primaryAction) { EditButton().disabled(doc.layers.count < 2) }
            }
            .navigationDestination(for: UUID.self) { id in
                if let layer = doc.layer(id) {
                    DataCard(
                        layer: layer,
                        onClose: { path.removeAll() },
                        onChangeLook: { onLook(id) },
                        onList: { onList(id) },
                        onZoom: { doc.fly(to: layer.dataset.bounds) }
                    )
                    .toolbar(.hidden, for: .navigationBar)
                } else {
                    Text("This layer was removed.").foregroundStyle(.secondary)
                }
            }
        }
        .presentationDetents([.medium, .large])
        .presentationBackgroundInteraction(.enabled(upThrough: .medium))
        .presentationDragIndicator(.visible)
    }

    /// The eye sits at the leading edge, well away from the ">" (Walt tapped the arrow to "look" and
    /// switched his layer off); everything else in the row, arrow included, opens the layer card.
    private func row(_ layer: MapLayer) -> some View {
        VStack(alignment: .leading, spacing: 2) {
        HStack(spacing: 4) {
            Button {
                doc.setVisible(layer.id, !layer.visible)
            } label: {
                Image(systemName: layer.visible ? "eye" : "eye.slash")
                    .font(.body.weight(.semibold))
                    .foregroundStyle(layer.visible ? Color.accentColor : Color.secondary)
                    .frame(width: 44, height: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(layer.visible ? "Hide \(layer.name)" : "Show \(layer.name)")

            Button {
                path.append(layer.id)
            } label: {
                HStack(spacing: 12) {
                    LegendSwatch(kind: layer.dataset.kind, colors: layer.scheme.swatch, opacity: layer.visible ? layer.style.opacity : 0.15)
                    VStack(alignment: .leading, spacing: 2) {
                        HStack(spacing: 6) {
                            Text(layer.name).foregroundStyle(layer.visible ? .primary : .secondary)
                            if layer.id == doc.sampleID {
                                Text("Sample")
                                    .font(.caption2.weight(.semibold))
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(Color(.tertiarySystemFill), in: Capsule())
                            }
                        }
                        Text(detail(layer)).font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 8)
                    Image(systemName: "chevron.right")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.tertiary)
                }
                .frame(minHeight: 44)
                .contentShape(Rectangle())
            }
            .buttonStyle(.borderless)
            .tint(.primary)
            .accessibilityHint("Shows what's in this layer")
        }
            // Map images fade right here (Priya hunted for it three screens deep).
            if layer.dataset.raster != nil && layer.visible {
                HStack(spacing: 8) {
                    Text("See-through").font(.caption).foregroundStyle(.secondary)
                    Slider(
                        value: Binding(
                            get: { 1 - layer.style.opacity },
                            set: { v in
                                var s = layer.style
                                s.opacity = 1 - v
                                doc.setStyle(layer.id, s)
                            }
                        ),
                        in: 0...1
                    )
                    .accessibilityLabel("See-through for \(layer.name)")
                    Text("\(Int(((1 - layer.style.opacity) * 100).rounded()))%")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                        .frame(width: 38, alignment: .trailing)
                }
                .padding(.leading, 48)
            }
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
            Button(role: .destructive) {
                doc.remove(layer.id)
            } label: {
                Text("Remove")
            }
        }
    }

    private func detail(_ layer: MapLayer) -> String {
        var s = layer.dataset.countPhrase
        if let by = layer.style.showBy, let f = layer.dataset.field(named: by) {
            s += " · by \(f.label.lowercased())"
        }
        return s
    }
}
