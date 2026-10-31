import SwiftUI

/// Persistent bottom sheet for Alt B: small = search+; medium = data on this map; large = library stub.
struct PersistentMapSheet: View {
    var mapName: String
    @Binding var searchText: String
    @ObservedObject var store: LayerStore
    var onAdd: () -> Void
    var onSelectLayer: (UUID) -> Void

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                searchRow
                    .padding(.horizontal, 16)
                    .padding(.top, 8)
                    .padding(.bottom, 10)

                List {
                    Section {
                        if store.layers.isEmpty {
                            Text("No data on this map yet")
                                .foregroundStyle(.secondary)
                        } else {
                            ForEach(store.layers) { layer in
                                layerRow(layer)
                            }
                            .onMove(perform: store.move)
                            .onDelete(perform: deleteAtOffsets)
                        }
                    } header: {
                        Text(dataHeader)
                    }

                    Section("Your maps") {
                        Text("Library list — stub for this spike")
                            .foregroundStyle(.secondary)
                    }
                    Section("Recent data") {
                        Text("Recent — stub for this spike")
                            .foregroundStyle(.secondary)
                    }
                }
                .listStyle(.insetGrouped)
            }
            .navigationTitle(mapName)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    EditButton()
                        .disabled(store.layers.count < 2)
                }
            }
        }
    }

    private var dataHeader: String {
        let n = store.visibleCount
        return "\(n) data set\(n == 1 ? "" : "s") on this map"
    }

    private var searchRow: some View {
        HStack(spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                TextField("Search places or data", text: $searchText)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(Color(.secondarySystemFill), in: RoundedRectangle(cornerRadius: 12, style: .continuous))

            Button(action: onAdd) {
                Image(systemName: "plus")
                    .font(.body.weight(.semibold))
                    .frame(width: 44, height: 44)
                    .background(Color.accentColor.opacity(0.15), in: Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Add data")
        }
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
            Button {
                onSelectLayer(layer.id)
            } label: {
                HStack(spacing: 12) {
                    LegendSwatch(geometry: layer.geometry, symbology: layer.symbology)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(layer.name)
                            .foregroundStyle(.primary)
                        Text(layer.kindSummary)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("\(layer.name), open data card")

            Button {
                store.setVisible(id: layer.id, !layer.visible)
            } label: {
                Image(systemName: layer.visible ? "eye" : "eye.slash")
                    .foregroundStyle(layer.visible ? Color.accentColor : Color.secondary)
                    .frame(minWidth: 44, minHeight: 44)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(layer.visible ? "Hide" : "Show")
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
