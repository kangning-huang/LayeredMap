import SwiftUI
import UniformTypeIdentifiers

/// "+" → Add data, ordered by what the person wants, not by mechanism: data library, search,
/// On this iPhone, From Files, Paste link (redesign › 6).
struct AddDataSheet: View {
    @ObservedObject var doc: MapDocument
    var onClose: () -> Void
    var onAddLibrary: (LibraryItem) -> Void
    var onStarted: () -> Void

    @State private var search = ""
    @State private var showFiles = false
    @State private var picking = FilePick.files
    @State private var refresh = 0
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
                PhoneFilesSection(
                    limit: 5,
                    refresh: refresh,
                    onOpen: { f in
                        onStarted()
                        doc.importingAll(f.name) { await PhoneFiles.open(f) }
                    },
                    onChooseFolder: {
                        picking = .listFolder
                        showFiles = true
                    }
                )
                Section {
                    Button {
                        picking = .files
                        showFiles = true
                    } label: {
                        Label("From Files", systemImage: "doc")
                    }
                    Button {
                        picking = .openFolder
                        showFiles = true
                    } label: {
                        Label("A folder (Shapefile)…", systemImage: "folder")
                    }
                    Button {
                        picking = .anyFile
                        showFiles = true
                    } label: {
                        Label("Any file (Google Drive…)", systemImage: "doc.questionmark")
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
                    Text("From Files shows only map files. A Shapefile is several files: choose the folder they're in. Some cloud apps (Google Drive) grey out map files there: use Any file. Got a file in Mail or WeChat? Tap Share, then NimbleMap.")
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
        }
        // One picker for both doors (SwiftUI drops a second one on the same view).
        .fileImporter(
            isPresented: $showFiles,
            allowedContentTypes: picking.types,
            allowsMultipleSelection: picking.several
        ) { result in
            guard case .success(let urls) = result, !urls.isEmpty else { return }
            if picking == .listFolder {
                urls.forEach { PhoneFiles.addFolder($0) }
                refresh += 1
                return
            }
            onStarted()
            doc.importingAll(Importer.pickedLabel(urls)) { await Importer.openPicked(urls) }
        }
        .alert("That isn't a link", isPresented: $pasteFailed) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("Copy a web link that starts with https://, then tap Paste.")
        }
    }
}
