import SwiftUI

/// State B: data layers only (no basemap). Reorder, eye, live swatch, swipe Remove → C on tap.
struct LayersSheet: View {
    @ObservedObject var store: LayerStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section {
                    if store.layers.isEmpty {
                        Text("No data layers yet")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(store.layers) { layer in
                            layerRow(layer)
                        }
                        .onMove(perform: store.move)
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
                        .disabled(store.layers.count < 2)
                }
            }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
    }

    private func deleteAtOffsets(_ indexSet: IndexSet) {
        let ids = indexSet.map { store.layers[$0].id }
        for id in ids {
            store.remove(id: id)
        }
    }

    @ViewBuilder
    private func layerRow(_ layer: MapLayer) -> some View {
        HStack(spacing: 12) {
            if let binding = store.binding(for: layer.id) {
                NavigationLink {
                    SymbologySheet(layer: binding)
                } label: {
                    HStack(spacing: 12) {
                        LegendSwatch(geometry: layer.geometry, symbology: layer.symbology)
                        Text(layer.name)
                            .foregroundStyle(.primary)
                        Spacer(minLength: 0)
                    }
                }
            }

            Button {
                store.setVisible(id: layer.id, !layer.visible)
            } label: {
                Image(systemName: layer.visible ? "eye" : "eye.slash")
                    .foregroundStyle(layer.visible ? Color.accentColor : Color.secondary)
                    .frame(minWidth: 32, minHeight: 32)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(layer.visible ? "Hide layer" : "Show layer")
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
            Button(role: .destructive) {
                store.remove(id: layer.id)
            } label: {
                Text("Remove")
            }
        }
    }
}
