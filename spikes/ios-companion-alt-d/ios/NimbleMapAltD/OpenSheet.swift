import SwiftUI
import UniformTypeIdentifiers

/// **Alternative D** — "Add data", rebuilt from the Marisol persona study (2026-11-03): a map
/// enthusiast who knows exactly *where* data lives (60 bookmarks: data.gov, TIGER/Line, Geofabrik,
/// her city's portal) but has never once managed to open any of it.
///
/// Alt C ordered this sheet by the mechanism the developer had to hand: Browse map files, the phone
/// list, "More ways to open", and only then the Data library — the catalogue last, below the fold of
/// a half-height sheet, behind 65 words about `.shp`/`.dbf`/`.shx`. Someone arriving with an
/// appetite rather than a file saw nothing but file management and left.
///
/// D orders it by **what the person already has**: an appetite, a link, or a download.
///
/// 1. **Search** — searches the catalogue. (`Library.search` existed in C and had no callers.)
/// 2. **Browse data** — a horizontal strip of Add tiles; the catalogue, visible without scrolling.
/// 3. **I have a link** — a real, visible field. C hid `Importer.download` behind a bare system
///    Paste button inside "More ways to open"; it is the one skill the persona actually has
///    ("hold the download button → Copy Link"), and it works on every portal she has bookmarked.
/// 4. **My downloads** — the phone list, named for where files really come from.
/// 5. A collapsed disclosure at the bottom holds the Shapefile/zip plumbing, so the two people who
///    need it can find it and nobody else ever reads it.
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
    @State private var search = ""
    @State private var link = ""
    @State private var linkFailed = false
    @State private var showPlumbing = false
    @FocusState private var linkFocused: Bool
    /// Where the online search has got to. `.found([])` is a real answer, not a missing one.
    @State private var online = Online.idle
    @State private var scope = Catalog.Scope.portals

    enum Online: Equatable {
        case idle
        case searching
        case found([Catalog.Item])
        case failed
    }
    /// The Files picker opens where the last pick came from (Priya: it started at On My iPhone).
    @AppStorage("lm.lastPickFolder") private var lastFolder = ""

    var body: some View {
        NavigationStack {
            List {
                if searching {
                    searchResults
                } else {
                    browseSection
                    linkSection
                    downloadsSection
                    plumbingSection
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Add data")
            .navigationBarTitleDisplayMode(.inline)
            .searchable(text: $search, placement: .navigationBarDrawer(displayMode: .always),
                        prompt: "Search data (bike lanes, earthquakes…)")
            // New words, new question: old results must not sit under a different search.
            .onChange(of: search) { _, _ in online = .idle }
            // Spike-only: LM_Q=<words> types a search and runs it, so the online results can be
            // screenshotted headlessly.
            .task {
                guard let q = ProcessInfo.processInfo.environment["LM_Q"], !q.isEmpty, search.isEmpty else { return }
                search = q
                try? await Task.sleep(nanoseconds: 400_000_000)
                runOnlineSearch()
            }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            }
        }
        .presentationDetents([.large])
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
        .alert("That didn't look like a web address", isPresented: $linkFailed) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("Go back to the site, hold the download button, and choose Copy Link. Then paste it here.")
        }
    }

    private var searching: Bool {
        !search.trimmingCharacters(in: .whitespaces).isEmpty
    }

    // MARK: 1 · Search the catalogue

    /// C shipped `Library.search` and never called it, so the four bundled sets were unfindable.
    /// A miss is not a dead end here: it says where that kind of data is actually published.
    @ViewBuilder
    private var searchResults: some View {
        let hits = Library.search(search)
        if !hits.isEmpty {
            Section("In the data library") {
                ForEach(hits) { item in
                    LibraryRow(item: item, added: onMap(item), onAdd: { onLibrary(item) })
                }
            }
        }
        if hits.isEmpty {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Nothing in the built-in library for “\(search)”.")
                        .font(.body.weight(.semibold))
                    Text("NimbleMap can look for map files on the web. Tap a result and it opens straight on your map.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 2)
            }
        }
        onlineSection
        if hits.isEmpty {
            linkSection
        }
    }

    // MARK: 1b · Looking on the web

    /// The catalogues, then a web search if they come back empty. Nothing runs until the person
    /// asks: a search box that fires off network calls on every keystroke spends their data and
    /// shows them half-typed nonsense.
    @ViewBuilder
    private var onlineSection: some View {
        Section {
            Button {
                runOnlineSearch()
            } label: {
                HStack(spacing: 10) {
                    if online == .searching {
                        ProgressView()
                    } else {
                        Image(systemName: "globe").foregroundStyle(Color.accentColor)
                    }
                    Text(online == .searching ? "Looking…" : "Look for “\(search)” map files")
                        .font(.body.weight(.semibold))
                        .lineLimit(1)
                }
            }
            .disabled(online == .searching)

            Picker("Search in", selection: $scope) {
                ForEach(Catalog.Scope.allCases) { s in Text(s.label).tag(s) }
            }
            .pickerStyle(.segmented)
            .onChange(of: scope) { _, _ in
                // The chips are a different question, not a filter on the old answer.
                if case .found = online { online = .idle }
            }
        } header: {
            Text("Look on the web")
        } footer: {
            Text("We look for files NimbleMap can open — GeoJSON, CSV, KML, Shapefile and more.")
        }

        switch online {
        case .found(let items) where !items.isEmpty:
            Section("Found on the web") {
                ForEach(items) { item in
                    CatalogRow(item: item, onAdd: { onLink(item.url) })
                }
            }
        case .found:
            // A true empty answer. Say so plainly, and say why, or it reads as a broken app.
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text("No map files found for “\(search)”.")
                        .font(.body.weight(.semibold))
                    Text("Some subjects aren't published as free map files — shop and business locations often aren't.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 2)
                if scope != .anyFile {
                    Button {
                        scope = .anyFile
                        runOnlineSearch()
                    } label: {
                        Label("Try “Any map file” instead", systemImage: "arrow.triangle.2.circlepath")
                    }
                }
                if let web = Catalog.webSearch(search, scope: scope) {
                    NavigationLink {
                        DataWebPage(url: web, title: "Search", onDataLink: onLink)
                    } label: {
                        Label("Search the web for “\(search)”", systemImage: "magnifyingglass")
                    }
                }
            }
        case .failed:
            Section {
                Text("Couldn't reach the data portals just now. Check your connection and try again.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        case .idle, .searching:
            EmptyView()
        }
    }

    private func runOnlineSearch() {
        let text = search
        let where_ = scope
        online = .searching
        Task {
            let items = await Catalog.search(text, scope: where_)
            // The person has typed on since; their newer words win.
            guard text == search, where_ == scope else { return }
            online = .found(items)
        }
    }

    // MARK: 2 · Browse data (the catalogue, above the fold)

    private var browseSection: some View {
        Section {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(Library.items) { item in
                        LibraryTile(item: item, added: onMap(item), onAdd: { onLibrary(item) })
                    }
                }
                .padding(.vertical, 4)
            }
            .listRowInsets(EdgeInsets(top: 6, leading: 16, bottom: 6, trailing: 0))
            // A full-width row, not the last tile in the strip: the catalogue is the answer for
            // anyone whose data isn't one of the four bundled sets, so it must never need a
            // sideways scroll to find. (In C it was the last line of the last section.)
            NavigationLink {
                DataWebPage(onDataLink: onLink)
            } label: {
                MoreDataRow()
            }
        } header: {
            Text("Browse data")
        } footer: {
            Text("Tap Add and it goes straight on the map.")
        }
    }

    // MARK: 3 · I have a link

    /// The whole point of D. `Importer.download` already fetches the file and guesses the format
    /// from the MIME type when the URL lies — in C that capability sat behind a bare Paste button
    /// in a section called "More ways to open", below the fold.
    private var linkSection: some View {
        Section {
            HStack(spacing: 8) {
                Image(systemName: "link").foregroundStyle(.secondary)
                TextField("https://…/bike_lanes.geojson", text: $link)
                    .focused($linkFocused)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                    .submitLabel(.go)
                    .onSubmit(openLink)
                if !link.isEmpty {
                    Button(action: { link = "" }) {
                        Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Clear link")
                }
            }
            HStack {
                PasteButton(payloadType: String.self) { strings in
                    guard let s = strings.first?.trimmingCharacters(in: .whitespacesAndNewlines) else { return }
                    link = s
                    openLink()
                }
                .labelStyle(.titleOnly)
                .buttonBorderShape(.capsule)
                Spacer()
                Button("Add to map", action: openLink)
                    .buttonStyle(.borderedProminent)
                    .buttonBorderShape(.capsule)
                    .controlSize(.small)
                    .disabled(link.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        } header: {
            Text("I have a link")
        } footer: {
            Text("On a data portal: hold the download button and choose Copy Link. Works with GeoJSON, CSV, KML, GPX and zipped Shapefiles.")
        }
    }

    private func openLink() {
        let raw = link.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URL(string: raw), url.scheme?.hasPrefix("http") == true, url.host != nil else {
            linkFailed = true
            return
        }
        linkFocused = false
        onLink(url)
    }

    // MARK: 4 · My downloads

    private var downloadsSection: some View {
        PhoneFilesSection(limit: 5, refresh: refresh, onOpen: onPhoneFile, onChooseFolder: { pick(.listFolder) })
    }

    // MARK: 5 · The plumbing, collapsed

    /// Everything that made the persona freeze, behind one disclosure. The zip truth leads, because
    /// it is the single most useful fact in here and C never stated it: a zipped Shapefile opens
    /// as it is (`Importer.extensions` has `zip`; the decoder handles it).
    private var plumbingSection: some View {
        Section {
            DisclosureGroup(isExpanded: $showPlumbing) {
                Text("A .zip works as it is — just open it, no unzipping.")
                    .font(.footnote.weight(.semibold))
                Button {
                    pick(.files)
                } label: {
                    Label("Browse all files…", systemImage: "doc.text.magnifyingglass")
                }
                Button {
                    pick(.openFolder)
                } label: {
                    Label("Several files that belong together…", systemImage: "folder")
                }
                Button {
                    pick(.anyFile)
                } label: {
                    Label("Can't tap your file? Try this", systemImage: "doc.questionmark")
                }
                Text("A Shapefile arrives as several files (.shp, .dbf, .shx…). If you have the .zip, open the .zip. If you have the loose files, tap “Several files that belong together” and choose the folder holding them — the files inside look greyed out, and that's expected.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } label: {
                Label("Downloaded a .zip, or several files?", systemImage: "questionmark.circle")
                    .font(.subheadline)
            }
        }
    }

    private func onMap(_ item: LibraryItem) -> Bool {
        doc.layers.contains { $0.name == item.title }
    }

    private func pick(_ next: FilePick) {
        job = next
        picking = true
    }
}

/// A tile in the horizontal Browse strip: picture, name, count, one Add button — the App Store
/// shape the persona already understands, and the part of C she called the best thing in the app.
struct LibraryTile: View {
    var item: LibraryItem
    var added: Bool
    var onAdd: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Image(systemName: item.symbol)
                .font(.title2)
                .foregroundStyle(.white)
                .frame(width: 124, height: 62)
                .background(Color(hex: item.tint).gradient, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            Text(item.title)
                .font(.subheadline.weight(.semibold))
                .lineLimit(1)
            Text(item.blurb)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(2)
                .frame(height: 28, alignment: .top)
            Button(added ? "On map" : "Add", action: onAdd)
                .buttonStyle(.borderedProminent)
                .buttonBorderShape(.capsule)
                .controlSize(.small)
                .disabled(added)
        }
        .frame(width: 124)
    }
}

/// The way into the nimblemap.com catalogue: a full-width row directly under the Add tiles, named
/// by what is actually in it. C put this behind the words "More data on nimblemap.com" as the last
/// row of the last section of a half-height sheet, and the persona never found it.
struct MoreDataRow: View {
    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "globe")
                .font(.title3)
                .foregroundStyle(.white)
                .frame(width: 48, height: 48)
                .background(Color.accentColor.gradient, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text("Browse all data").font(.body.weight(.semibold))
                Text("Earthquakes live, wind turbines, world ports, Roman roads, meteorites…")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
        }
        .padding(.vertical, 2)
    }
}

/// One result found on the web. The format badge is the catalogue's own word, never guessed from
/// the URL, so "GeoJSON" is a promise the app can keep.
struct CatalogRow: View {
    var item: Catalog.Item
    var onAdd: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "globe")
                .font(.title3)
                .foregroundStyle(.white)
                .frame(width: 48, height: 48)
                .background(Color.accentColor.gradient, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(item.title).font(.body.weight(.semibold)).lineLimit(2)
                if !item.blurb.isEmpty {
                    Text(item.blurb).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
                Text("\(item.source) · \(item.format)")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
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
