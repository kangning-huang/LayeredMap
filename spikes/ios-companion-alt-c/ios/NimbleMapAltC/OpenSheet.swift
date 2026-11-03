import SwiftUI
import UniformTypeIdentifiers

/// One Open sheet, ordered by what the study's personas reached for: Browse map files, then the
/// ranked map files on this iPhone (A's best moment: "Parishes 1851 · Shapefile"), the other doors,
/// then B's in-app data library with Add buttons (no web view).
struct OpenSheet: View {
    @ObservedObject var doc: MapDocument
    var onLibrary: (LibraryItem) -> Void
    var onPhoneFile: (PhoneFile) -> Void
    var onRecent: (Importer.Recent) -> Void
    var onPicked: ([URL]) -> Void
    var onLink: (URL) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var picking = false
    @State private var job = FilePick.listFolder
    @State private var refresh = 0
    @State private var pasteFailed = false
    /// The Files picker opens where the last pick came from (Priya: it started at On My iPhone).
    @AppStorage("lm.lastPickFolder") private var lastFolder = ""

    var body: some View {
        NavigationStack {
            List {
                // The usual door first and always in view (Priya's second visit had to scroll for it).
                Section {
                    Button {
                        pick(.files)
                    } label: {
                        Label("Browse map files…", systemImage: "doc.text.magnifyingglass")
                            .font(.body.weight(.semibold))
                    }
                }
                // Files opened before are listed here once (not again under a recents heading).
                PhoneFilesSection(limit: 5, refresh: refresh, onOpen: onPhoneFile, onChooseFolder: { pick(.listFolder) })
                Section {
                    Button {
                        pick(.openFolder)
                    } label: {
                        Label("A Shapefile folder…", systemImage: "folder")
                    }
                    Button {
                        pick(.anyFile)
                    } label: {
                        Label("Any file (Google Drive…)", systemImage: "doc.questionmark")
                    }
                    HStack {
                        Label("Paste a link", systemImage: "link")
                        Spacer()
                        PasteButton(payloadType: String.self) { strings in
                            guard let s = strings.first?.trimmingCharacters(in: .whitespacesAndNewlines),
                                  let url = URL(string: s), url.scheme?.hasPrefix("http") == true else {
                                pasteFailed = true
                                return
                            }
                            onLink(url)
                        }
                        .labelStyle(.titleOnly)
                        .buttonBorderShape(.capsule)
                    }
                } header: {
                    Text("More ways to open")
                } footer: {
                    Text("A Shapefile is several files (.shp, .dbf, .shx…) that belong together. Tap “A Shapefile folder”, then choose the folder they're in: the files inside look greyed out, and that's expected. If Google Drive greys out a map file, use Any file.")
                }
                Section("Data library") {
                    ForEach(Library.items) { item in
                        let onMap = doc.layers.contains { $0.name == item.title }
                        LibraryRow(item: item, added: onMap, onAdd: { onLibrary(item) })
                    }
                    NavigationLink {
                        DataWebPage(onDataLink: onLink)
                    } label: {
                        Label("More data on nimblemap.com", systemImage: "globe")
                    }
                }
            }
            .navigationTitle("Open")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
        // One picker for every door (SwiftUI drops a second one on the same view).
        .fileImporter(isPresented: $picking, allowedContentTypes: job.types, allowsMultipleSelection: job.several) { result in
            guard case .success(let urls) = result, let first = urls.first else { return }
            let isFolder = (try? first.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true
            lastFolder = (isFolder && job == .listFolder ? first : first.deletingLastPathComponent()).path
            if job == .listFolder {
                urls.forEach { PhoneFiles.addFolder($0) }
                refresh += 1
                return
            }
            onPicked(urls)
        }
        .fileDialogDefaultDirectory(lastFolder.isEmpty ? nil : URL(fileURLWithPath: lastFolder, isDirectory: true))
        .alert("That isn't a link", isPresented: $pasteFailed) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("Copy a web link that starts with https://, then tap Paste.")
        }
    }

    private func pick(_ next: FilePick) {
        job = next
        picking = true
    }
}

struct LibraryRow: View {
    var item: LibraryItem
    var added = false
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
            Button(added ? "On map" : "Add", action: onAdd)
                .buttonStyle(.borderedProminent)
                .buttonBorderShape(.capsule)
                .controlSize(.small)
                .disabled(added)
        }
        .padding(.vertical, 2)
    }
}
