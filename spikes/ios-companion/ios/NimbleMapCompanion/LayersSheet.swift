import SwiftUI

/// State B: data layers only (no basemap). Reorder, eye, live swatch, swipe Remove → C on tap.
/// The map above still reacts while it is open; a pan or pinch returns to A.
struct LayersSheet: View {
    @ObservedObject var doc: MapDocument
    var startStyle: UUID?
    @Environment(\.dismiss) private var dismiss
    @State private var path: [UUID] = []

    var body: some View {
        NavigationStack(path: $path) {
            List {
                Section {
                    if doc.layers.isEmpty {
                        Text("No data layers yet")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(doc.layers) { layer in
                            layerRow(layer)
                        }
                        .onMove(perform: doc.move)
                        .onDelete(perform: deleteAtOffsets)
                    }
                } footer: {
                    Text("Basemap stays on and is not listed. Swipe left to Remove. Edit to reorder.")
                }
            }
            .navigationTitle("Layers")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
                ToolbarItem(placement: .primaryAction) {
                    EditButton()
                        .disabled(doc.layers.count < 2)
                }
            }
            .navigationDestination(for: UUID.self) { id in
                SymbologySheet(doc: doc, layerID: id)
            }
        }
        .presentationDetents([.medium, .large])
        .presentationBackgroundInteraction(.enabled(upThrough: .medium))
        .presentationDragIndicator(.visible)
        .onAppear {
            if let startStyle, path.isEmpty { path = [startStyle] }
        }
    }

    private func deleteAtOffsets(_ indexSet: IndexSet) {
        let ids = indexSet.compactMap { doc.layers[safe: $0]?.id }
        for id in ids {
            doc.remove(id)
        }
    }

    private func layerRow(_ layer: MapLayer) -> some View {
        HStack(spacing: 12) {
            NavigationLink(value: layer.id) {
                HStack(spacing: 12) {
                    LegendSwatch(kind: layer.dataset.kind, colors: layer.scheme.swatch, opacity: layer.style.opacity)
                    Text(layer.name)
                        .foregroundStyle(.primary)
                    Spacer(minLength: 0)
                }
            }

            Button {
                doc.setVisible(layer.id, !layer.visible)
            } label: {
                Image(systemName: layer.visible ? "eye" : "eye.slash")
                    .foregroundStyle(layer.visible ? Color.accentColor : Color.secondary)
                    .frame(minWidth: 44, minHeight: 44)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(layer.visible ? "Hide layer" : "Show layer")
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
            Button(role: .destructive) {
                doc.remove(layer.id)
            } label: {
                Text("Remove")
            }
        }
    }
}
