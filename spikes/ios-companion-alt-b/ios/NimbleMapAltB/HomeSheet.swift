import SwiftUI

/// Alt B sheet, home: search + "+" (small), data on this map (medium), your maps and recent data
/// (large). The title is the map's name; the word "Layers" is not needed.
struct HomeSheet: View {
    @ObservedObject var doc: MapDocument
    var mapName: String
    var onAdd: () -> Void
    var onOpenLayer: (UUID) -> Void
    var onOpenFeature: (MapHit) -> Void
    var onAddLibrary: (LibraryItem) -> Void
    var onSearchFocus: () -> Void

    @State private var search = ""
    @FocusState private var searchFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            searchRow
                .padding(.horizontal, 16)
                .padding(.top, 18)
            Text(subtitle)
                .font(.footnote)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 20)
                .padding(.top, 8)
                .padding(.bottom, 4)
            if search.isEmpty {
                dataList
            } else {
                SearchResults(doc: doc, text: search, onOpenFeature: onOpenFeature, onAddLibrary: { item in
                    search = ""
                    searchFocused = false
                    onAddLibrary(item)
                })
            }
        }
        .onChange(of: searchFocused) { _, focused in
            if focused { onSearchFocus() }
        }
    }

    private var subtitle: String {
        let n = doc.layers.count
        return "\(mapName) · \(n) data set\(n == 1 ? "" : "s")"
    }

    private var searchRow: some View {
        HStack(spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField("Search places or data", text: $search)
                    .focused($searchFocused)
                    .submitLabel(.search)
                    .autocorrectionDisabled()
                if !search.isEmpty {
                    Button {
                        search = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Clear search")
                }
            }
            .padding(.horizontal, 12)
            .frame(height: 44)
            .background(Color(.tertiarySystemFill), in: RoundedRectangle(cornerRadius: 12, style: .continuous))

            Button(action: onAdd) {
                Image(systemName: "plus")
                    .font(.title3.weight(.semibold))
                    .frame(width: 44, height: 44)
                    .background(Color.accentColor, in: Circle())
                    .foregroundStyle(.white)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Add data")
        }
    }

    private var dataList: some View {
        List {
            Section("On this map") {
                if doc.layers.isEmpty {
                    Button("Add data to this map", action: onAdd)
                }
                ForEach(doc.layers) { layer in
                    row(layer)
                }
                .onMove(perform: doc.move)
            }
            Section("Recent data") {
                let recents = Importer.recents()
                if recents.isEmpty {
                    Text("Files you add show up here.").foregroundStyle(.secondary)
                }
                ForEach(recents) { r in
                    Button {
                        doc.importing(r.name) { await Importer.reopen(r) }
                    } label: {
                        Label(r.name, systemImage: "doc")
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
    }

    private func row(_ layer: MapLayer) -> some View {
        HStack(spacing: 12) {
            Button {
                onOpenLayer(layer.id)
            } label: {
                HStack(spacing: 12) {
                    LegendSwatch(kind: layer.dataset.kind, colors: layer.scheme.swatch, opacity: layer.style.opacity)
                    VStack(alignment: .leading, spacing: 2) {
                        HStack(spacing: 6) {
                            Text(layer.name).foregroundStyle(.primary)
                            if !layer.dataset.assumptions.isEmpty {
                                Circle().fill(Color.purple).frame(width: 7, height: 7)
                                    .accessibilityLabel("Has assumptions")
                            }
                        }
                        Text(rowDetail(layer))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            Button {
                doc.setVisible(layer.id, !layer.visible)
            } label: {
                Image(systemName: layer.visible ? "eye" : "eye.slash")
                    .foregroundStyle(layer.visible ? Color.accentColor : Color.secondary)
                    .frame(width: 44, height: 44)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(layer.visible ? "Hide \(layer.name)" : "Show \(layer.name)")
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
            Button(role: .destructive) {
                doc.remove(layer.id)
            } label: {
                Text("Remove")
            }
        }
    }

    private func rowDetail(_ layer: MapLayer) -> String {
        var s = layer.dataset.countPhrase
        if let by = layer.style.showBy, let f = layer.dataset.field(named: by) {
            s += " · by \(f.label.lowercased())"
        }
        return s
    }
}

/// One search field, result groups: on this map, ready to map; plain words when nothing is found.
struct SearchResults: View {
    @ObservedObject var doc: MapDocument
    var text: String
    var onOpenFeature: (MapHit) -> Void
    var onAddLibrary: (LibraryItem) -> Void

    var body: some View {
        let places = matches
        let library = Library.search(text).filter { item in !doc.layers.contains { $0.name == item.title } }
        List {
            if !places.isEmpty {
                Section("On this map") {
                    ForEach(places, id: \.hit) { m in
                        Button {
                            onOpenFeature(m.hit)
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(m.title).foregroundStyle(.primary)
                                Text(m.layer).font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
            if !library.isEmpty {
                Section("Ready to map") {
                    ForEach(library) { item in
                        LibraryRow(item: item, onAdd: { onAddLibrary(item) })
                    }
                }
            }
            if places.isEmpty && library.isEmpty {
                Section {
                    Text("No ready-to-map data for “\(text)”. Found it on a web page? Share the page to NimbleMap.")
                        .foregroundStyle(.secondary)
                }
            }
            Section {
                Label("Ask about this map", systemImage: "sparkles")
                    .foregroundStyle(.secondary)
                Text("Ask comes later, after tap → card works.").font(.caption).foregroundStyle(.tertiary)
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
    }

    private struct Match {
        let hit: MapHit
        let title: String
        let layer: String
    }

    private var matches: [Match] {
        let q = text.trimmingCharacters(in: .whitespaces).lowercased()
        guard q.count >= 2 else { return [] }
        var out: [Match] = []
        for layer in doc.layers where layer.visible {
            for f in layer.dataset.features {
                let title = layer.dataset.title(of: f)
                if title.lowercased().hasPrefix(q) || (q.count >= 3 && title.lowercased().contains(q)) {
                    out.append(Match(hit: MapHit(layerID: layer.id, featureIndex: f.index), title: title, layer: layer.name))
                    if out.count >= 6 { return out }
                }
            }
        }
        return out
    }
}

struct LibraryRow: View {
    var item: LibraryItem
    var onAdd: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: item.symbol)
                .font(.title3)
                .foregroundStyle(.white)
                .frame(width: 48, height: 48)
                .background(Color(hex: item.tint).gradient, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(item.title).font(.body.weight(.semibold))
                Text(item.blurb).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                Text("\(item.kind.plural) · \(item.size) · \(item.license)")
                    .font(.caption2).foregroundStyle(.secondary)
            }
            Spacer(minLength: 4)
            Button("Add", action: onAdd)
                .buttonStyle(.borderedProminent)
                .buttonBorderShape(.capsule)
                .controlSize(.small)
        }
        .padding(.vertical, 2)
    }
}
