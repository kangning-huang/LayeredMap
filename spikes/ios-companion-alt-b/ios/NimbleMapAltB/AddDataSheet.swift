import SwiftUI

/// "+" → Add data, ordered by what the person wants, not by mechanism: data library, search,
/// From Files, Paste link (redesign › 6).
struct AddDataSheet: View {
    @ObservedObject var doc: MapDocument
    var onClose: () -> Void
    var onAddLibrary: (LibraryItem) -> Void
    var onStarted: () -> Void

    @State private var search = ""
    @State private var showFiles = false
    @State private var pasteFailed = false

    var body: some View {
        VStack(spacing: 0) {
            CardHeader(title: "Add data", subtitle: "Put more on this map", onClose: onClose)
                .padding(.horizontal, 18)
                .padding(.top, 18)
            List {
                Section {
                    HStack(spacing: 8) {
                        Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                        TextField("Search the data library", text: $search)
                            .autocorrectionDisabled()
                    }
                }
                Section("Data library") {
                    let items = Library.search(search)
                    if items.isEmpty {
                        Text("No ready-to-map data for “\(search)”. Found it on a web page? Share the page to NimbleMap.")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(items) { item in
                        let onMap = doc.layers.contains { $0.name == item.title }
                        LibraryRow(item: item, onAdd: { onAddLibrary(item) })
                            .disabled(onMap)
                            .opacity(onMap ? 0.5 : 1)
                    }
                }
                Section {
                    Button {
                        showFiles = true
                    } label: {
                        Label("From Files", systemImage: "folder")
                    }
                    HStack {
                        Label("Paste link", systemImage: "link")
                        Spacer()
                        PasteButton(payloadType: String.self) { strings in
                            guard let s = strings.first?.trimmingCharacters(in: .whitespacesAndNewlines),
                                  let url = URL(string: s), url.scheme?.hasPrefix("http") == true else {
                                pasteFailed = true
                                return
                            }
                            onStarted()
                            doc.importing(Importer.displayName(for: url)) { await Importer.download(url) }
                        }
                        .labelStyle(.titleOnly)
                        .buttonBorderShape(.capsule)
                    }
                } footer: {
                    Text("Got a file in Mail or WeChat? Tap Share, then NimbleMap.")
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
        }
        .fileImporter(isPresented: $showFiles, allowedContentTypes: Importer.contentTypes) { result in
            guard case .success(let url) = result else { return }
            onStarted()
            doc.importing(Importer.displayName(for: url)) { await Importer.openExternal(url) }
        }
        .alert("That isn't a link", isPresented: $pasteFailed) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("Copy a web link that starts with https://, then tap Paste.")
        }
    }
}
