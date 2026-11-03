import SwiftUI
import UniformTypeIdentifiers
import WebKit
import SafariServices

/// First-run empty state (ios-open-paths › First-run empty state): a light card, not a wall.
/// Once Recents exist, it becomes a short Recents list with the same actions below.
struct OpenCard: View {
    var onBrowse: () -> Void
    var onExamples: () -> Void
    var onFiles: () -> Void
    var onURL: () -> Void
    var onRecent: (Importer.Recent) -> Void

    var body: some View {
        let recents = Importer.recents(limit: 3)
        VStack(alignment: .leading, spacing: 12) {
            Text("Open a map").font(.title3.weight(.bold))
            if !recents.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(recents) { r in
                        Button {
                            onRecent(r)
                        } label: {
                            Label(r.name, systemImage: "clock").frame(maxWidth: .infinity, minHeight: 36, alignment: .leading)
                        }
                    }
                }
            }
            Button(action: onBrowse) {
                Label("Browse NimbleMap data", systemImage: "safari")
                    .font(.body.weight(.semibold))
                    .frame(maxWidth: .infinity, minHeight: 44)
            }
            .buttonStyle(.borderedProminent)
            HStack(spacing: 10) {
                Button(action: onExamples) {
                    Label("Examples", systemImage: "square.grid.2x2").frame(maxWidth: .infinity, minHeight: 36)
                }
                Button(action: onFiles) {
                    Label("On my iPhone", systemImage: "folder").frame(maxWidth: .infinity, minHeight: 36)
                }
            }
            .buttonStyle(.bordered)
            HStack(alignment: .firstTextBaseline) {
                Text("You can also share a file into NimbleMap from other apps.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                Spacer()
                Menu("More") {
                    Button("Open URL…", action: onURL)
                }
                .font(.footnote.weight(.semibold))
            }
        }
        .padding(18)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
    }
}

/// The Open sheet: same doors as the card, plus Examples and On my iPhone (map files NimbleMap
/// can open, newest first; the full Files browser one row below).
struct OpenSheet: View {
    enum Start: String, Equatable {
        case home
        case examples
        case phone
    }

    var start: Start
    @ObservedObject var doc: MapDocument
    var onBrowse: () -> Void
    var onFiles: () -> Void
    var onURL: () -> Void
    var onExample: (LibraryItem) -> Void
    var onPhoneFile: (PhoneFile) -> Void
    var onPicked: ([URL]) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var path: [Start] = []
    @State private var pickingFolder = false
    @State private var folderJob = FilePick.listFolder
    @State private var refresh = 0

    var body: some View {
        NavigationStack(path: $path) {
            List {
                Section {
                    Button(action: onBrowse) {
                        Label("Browse NimbleMap data", systemImage: "safari")
                    }
                    NavigationLink(value: Start.examples) {
                        Label("Examples", systemImage: "square.grid.2x2")
                    }
                    NavigationLink(value: Start.phone) {
                        Label("On my iPhone", systemImage: "folder")
                    }
                }
                PhoneFilesSection(limit: 3, refresh: refresh, onOpen: onPhoneFile, onChooseFolder: { pick(.listFolder) })
                Section {
                    Button("Open URL…", action: onURL)
                } footer: {
                    Text("You can also share a file into NimbleMap from other apps.")
                }
            }
            .navigationTitle("Open")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            }
            .navigationDestination(for: Start.self) { page in
                if page == .phone { phone } else { examples }
            }
        }
        .presentationDetents([.medium, .large])
        .fileImporter(isPresented: $pickingFolder, allowedContentTypes: folderJob.types, allowsMultipleSelection: folderJob.several) { result in
            guard case .success(let urls) = result, !urls.isEmpty else { return }
            if folderJob == .listFolder {
                urls.forEach { PhoneFiles.addFolder($0) }
                refresh += 1
                return
            }
            onPicked(urls)
        }
        .onAppear {
            if start != .home && path.isEmpty { path = [start] }
        }
    }

    private func pick(_ job: FilePick) {
        folderJob = job
        pickingFolder = true
    }

    private var phone: some View {
        List {
            PhoneFilesSection(limit: 30, refresh: refresh, onOpen: onPhoneFile, onChooseFolder: { pick(.listFolder) })
            Section {
                Button(action: onFiles) {
                    Label("Browse map files…", systemImage: "doc.text.magnifyingglass")
                }
                Button {
                    pick(.openFolder)
                } label: {
                    Label("Open a folder (Shapefile)…", systemImage: "folder")
                }
                Button {
                    pick(.anyFile)
                } label: {
                    Label("Browse any file (Google Drive…)", systemImage: "doc.questionmark")
                }
            } footer: {
                Text("Browse map files lets you pick only the kinds NimbleMap opens. A Shapefile is several files: open the folder they're in. Some cloud apps (Google Drive) grey out map files there: use Browse any file. Google Drive, Dropbox, OneDrive and Box appear under Browse once installed.")
            }
        }
        .navigationTitle("On my iPhone")
        .navigationBarTitleDisplayMode(.inline)
    }

    private var examples: some View {
        List(Library.items) { item in
            let onMap = doc.layers.contains { $0.name == item.title }
            HStack(spacing: 12) {
                Image(systemName: item.symbol)
                    .foregroundStyle(.white)
                    .frame(width: 40, height: 40)
                    .background(Color(hex: item.tint).gradient, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title).font(.body.weight(.semibold))
                    Text("\(item.blurb) · \(item.size)").font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Button(onMap ? "On map" : "Open") { onExample(item) }
                    .buttonStyle(.bordered)
                    .disabled(onMap)
            }
        }
        .navigationTitle("Examples")
    }
}

/// Browse-tap (path A): an in-app browser on the NimbleMap site's data page. A tap on a link to a
/// file NimbleMap opens downloads it into the app; anything else stays a web page.
struct BrowseSheet: View {
    var onDataLink: (URL) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var safari = false
    static let page = URL(string: "https://nimblemap.com/data.html")

    var body: some View {
        NavigationStack {
            Group {
                if let page = BrowseSheet.page {
                    DataWebView(url: page) { url in
                        onDataLink(url)
                    }
                    .ignoresSafeArea(edges: .bottom)
                }
            }
            .navigationTitle("NimbleMap data")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } }
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        safari = true
                    } label: {
                        Image(systemName: "safari")
                    }
                    .accessibilityLabel("Open in Safari")
                }
            }
            .sheet(isPresented: $safari) {
                if let page = BrowseSheet.page { SafariView(url: page) }
            }
        }
    }
}

struct DataWebView: UIViewRepresentable {
    var url: URL
    var onDataLink: (URL) -> Void

    func makeUIView(context: Context) -> WKWebView {
        let web = WKWebView(frame: .zero)
        web.navigationDelegate = context.coordinator
        web.load(URLRequest(url: url))
        return web
    }

    func updateUIView(_ web: WKWebView, context: Context) {
        context.coordinator.onDataLink = onDataLink
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onDataLink: onDataLink)
    }

    final class Coordinator: NSObject, WKNavigationDelegate {
        var onDataLink: (URL) -> Void

        init(onDataLink: @escaping (URL) -> Void) {
            self.onDataLink = onDataLink
        }

        static func isData(_ url: URL) -> Bool {
            let ext = url.pathExtension.lowercased()
            if Importer.extensions.contains(ext) && ext != "txt" && ext != "json" { return true }
            let s = url.absoluteString.lowercased()
            return s.contains("rows.csv") || s.hasSuffix(".geojson") || s.contains("format=geojson")
        }

        @MainActor
        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction) async -> WKNavigationActionPolicy {
            if let url = navigationAction.request.url, navigationAction.navigationType == .linkActivated, Coordinator.isData(url) {
                onDataLink(url)
                return .cancel
            }
            return .allow
        }
    }
}

struct SafariView: UIViewControllerRepresentable {
    var url: URL

    func makeUIViewController(context: Context) -> SFSafariViewController {
        SFSafariViewController(url: url)
    }

    func updateUIViewController(_ controller: SFSafariViewController, context: Context) {}
}
