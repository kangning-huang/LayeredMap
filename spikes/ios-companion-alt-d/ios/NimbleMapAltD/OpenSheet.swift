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
    /// A link to open, and the catalogue's name for it when there is one.
    var onLink: (URL, String?) -> Void
    /// A file the in-app browser downloaded itself.
    var onFile: (URL) -> Void

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
    /// A Google search, opened in Safari rather than a web view (see `Catalog.webSearch`).
    @State private var safariURL: URL?
    /// A catalogue entry that is a page rather than a file. This opens in the app's **own** web
    /// view, not Safari: that one watches for a tap on a data link and brings the file straight
    /// back to the map. Safari would download it into Files and leave the person hunting for it,
    /// which is the exact dead end the "Open" button is meant to avoid.
    @State private var pageURL: URL?
    /// Row index → the model's one line on why this result answers the question. Empty when no
    /// model was asked, or when it was and said nothing usable.
    @State private var reasons: [Int: String] = [:]
    /// A download large enough to be worth mentioning before it starts.
    @State private var big: BigDownload?
    /// The row whose size is being looked up, so its button can say so.
    @State private var checking: String?

    struct BigDownload: Identifiable {
        let url: URL
        let title: String
        let size: String
        var id: URL { url }
    }

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
        .sheet(item: $safariURL) { SafariView(url: $0) }
        .sheet(item: $pageURL) { url in
            NavigationStack {
                DataWebPage(url: url, title: "Pick a download", onDataLink: { link in
                    pageURL = nil
                    onLink(link, nil)
                }, onDataFile: { file in
                    pageURL = nil
                    onFile(file)
                })
            }
        }
        // Not a refusal: the number, the two honest consequences, and the choice.
        .alert("“\(big?.title ?? "")” is \(big?.size ?? "")", isPresented: bigBinding, presenting: big) { item in
            Button("Add anyway") { onLink(item.url, item.title) }
            Button("Not now", role: .cancel) {}
        } message: { _ in
            Text("It may take a while to download and will use that much space on your iPhone.")
        }
        .alert("That didn't look like a web address", isPresented: $linkFailed) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("Go back to the site, hold the download button, and choose Copy Link. Then paste it here.")
        }
    }

    private var bigBinding: Binding<Bool> {
        Binding(get: { big != nil }, set: { if !$0 { big = nil } })
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
        // Everything the app already knows, answered as the person types: no network, no waiting,
        // no model, works on a train. Most searches should end here.
        let catalogue = Catalog.searchBundled(search)
        if !hits.isEmpty {
            Section("Ready to add") {
                ForEach(hits) { item in
                    LibraryRow(item: item, added: onMap(item), onAdd: { onLibrary(item) })
                }
            }
        }
        // Maps and shop branches are different kinds of answer and were sharing one heading, so
        // "mountains" offered five retailers under "Ready to add" and "rivers" put a café among
        // the rivers. Split them: a brand is still here for whoever typed "Walmart", but it never
        // again pretends to be a map of the thing you asked for.
        let maps = catalogue.filter { !$0.isBrand }
        let brands = catalogue.filter(\.isBrand)
        if !maps.isEmpty {
            Section(hits.isEmpty ? "Ready to add" : "More data you can add") {
                ForEach(maps) { item in
                    CatalogRow(
                        item: item,
                        why: nil,
                        busy: checking == item.id,
                        onAdd: { addChecking(item) },
                        onOpenPage: { pageURL = item.url }
                    )
                }
            }
        }
        if !brands.isEmpty {
            Section {
                ForEach(brands.prefix(20)) { item in
                    CatalogRow(
                        item: item,
                        why: nil,
                        busy: checking == item.id,
                        onAdd: { addChecking(item) },
                        onOpenPage: { pageURL = item.url }
                    )
                }
            } header: {
                Text("Shops and brands named “\(search)”")
            } footer: {
                // Saying how many there are, because 5,614 rows under a silent heading reads as
                // the whole app rather than as one shelf of it.
                Text(brands.count > 20
                     ? "\(brands.count) brands match. These are shop and branch locations, not maps of \(search)."
                     : "Shop and branch locations, not maps of \(search).")
            }
        }
        if hits.isEmpty && catalogue.isEmpty {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Nothing built in for “\(search)”.")
                        .font(.body.weight(.semibold))
                    Text("NimbleMap can look for map files on the web. Tap a result and it opens straight on your map.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 2)
            }
        }
        onlineSection(hasLocal: !hits.isEmpty || !catalogue.isEmpty)
        if hits.isEmpty && catalogue.isEmpty {
            linkSection
        }
    }

    // MARK: 1b · Looking on the web

    /// The catalogues, then a web search if they come back empty. Nothing runs until the person
    /// asks: a search box that fires off network calls on every keystroke spends their data and
    /// shows them half-typed nonsense.
    @ViewBuilder
    private func onlineSection(hasLocal: Bool) -> some View {
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
                    Text(online == .searching
                         ? "Looking…"
                         : (hasLocal ? "Also look on the web" : "Look for “\(search)” map files"))
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
                ForEach(Array(items.enumerated()), id: \.element.id) { i, item in
                    CatalogRow(
                        item: item,
                        why: reasons[i],
                        busy: checking == item.id,
                        onAdd: { addChecking(item) },
                        onOpenPage: { pageURL = item.url }
                    )
                }
            }
        case .found:
            // A true empty answer. Say so plainly, and say why, or it reads as a broken app.
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    // Saying "no map files found" under three results the app just offered would
                    // be nonsense; the web simply added nothing to them.
                    Text(hasLocal ? "Nothing more on the web for “\(search)”." : "No map files found for “\(search)”.")
                        .font(.body.weight(.semibold))
                    if !hasLocal {
                        Text("Some subjects aren't published as free map files — shop and business locations often aren't.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
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
                    Button {
                        safariURL = web
                    } label: {
                        Label("Search the web for “\(search)”", systemImage: "magnifyingglass")
                    }
                    // Safari owns anything downloaded there, so say where the file will go
                    // *before* the trip rather than leaving someone hunting for it afterwards.
                    Text("This opens Safari. A file you download there is saved to Files — come back and tap “Show my Downloads folder”.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
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

    /// Add it, but ask first if it is big.
    ///
    /// Nothing is refused for being large — a phone can open a 220 MB file, and whether that is
    /// worth the wait and the storage is the owner's call. What the app owes them is the number
    /// *before* the download rather than after, which is why the size is asked for here instead
    /// of trusted from the catalogue's own text.
    private func addChecking(_ item: Catalog.Item) {
        // The size check is a network round trip, so without this the row sits there looking
        // untouched and people tap Add again. Two testers did exactly that.
        guard checking != item.id else { return }
        checking = item.id
        Task {
            defer { checking = nil }
            guard let bytes = await Catalog.bytes(of: item), Importer.isBig(bytes) else {
                onLink(item.url, item.title)
                return
            }
            big = BigDownload(url: item.url, title: item.title, size: Importer.sizeText(bytes))
        }
    }

    private func runOnlineSearch() {
        let text = search
        let where_ = scope
        online = .searching
        reasons = [:]
        Task {
            let outcome = await Catalog.search(text, scope: where_)
            // The person has typed on since; their newer words win.
            guard text == search, where_ == scope else { return }
            guard case .found(let items) = outcome else {
                // Could not ask, which is not the same as nothing existing.
                online = .failed
                return
            }
            online = .found(items)

            // Ranking is a second, optional pass: the results are already on screen, and a model
            // that is slow or missing just means no "why" lines. It never gates the answer.
            guard Assist.available, items.count > 1 else { return }
            let listed = items.map { Assist.Item(title: $0.title, source: $0.source, blurb: $0.blurb) }
            guard let ranked = await Assist.rank(listed, for: text), text == search, where_ == scope else { return }
            let best = ranked.filter { items.indices.contains($0.index) }
            guard !best.isEmpty else { return }
            let order = best.map(\.index)
            let rest = items.indices.filter { !order.contains($0) }
            let reordered = (order + rest).map { items[$0] }
            var why: [Int: String] = [:]
            for (place, r) in best.enumerated() { why[place] = r.why }
            reasons = why
            online = .found(reordered)
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
                DataWebPage(onDataLink: { onLink($0, nil) }, onDataFile: onFile)
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
        onLink(url, nil)
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
    /// Why this one, in a sentence, when a model was asked to rank the results.
    var why: String?
    /// Its size is being looked up; the button says so rather than looking untouched.
    var busy = false
    var onAdd: () -> Void
    var onOpenPage: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: item.kind == .page ? "safari" : "globe")
                .font(.title3)
                .foregroundStyle(.white)
                .frame(width: 48, height: 48)
                .background(Color.accentColor.gradient, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(item.title).font(.body.weight(.semibold)).lineLimit(2)
                if let why {
                    Text(why).font(.caption).foregroundStyle(Color.accentColor).lineLimit(2)
                } else if !item.blurb.isEmpty {
                    Text(item.blurb).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
                Text(detail)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                // Its own line, never clipped: the size is what decides whether this is worth
                // starting on a phone, so it must survive a narrow screen.
                if !item.size.isEmpty {
                    Label(item.size, systemImage: "arrow.down.circle")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .labelStyle(.titleAndIcon)
                }
            }
            Spacer(minLength: 4)
            // A page cannot promise a map; it promises a place to pick a download. Saying "Add"
            // on one would be the broken promise this spike exists to remove.
            Button(action: item.kind == .page ? onOpenPage : onAdd) {
                if busy {
                    ProgressView().controlSize(.small)
                } else {
                    Text(item.kind == .page ? "Open" : "Add")
                }
            }
            .buttonStyle(.borderedProminent)
            .buttonBorderShape(.capsule)
            .controlSize(.small)
            .disabled(busy)
        }
        .padding(.vertical, 2)
    }

    /// The source and format, which can run on and be clipped without costing anything. The size
    /// is deliberately *not* in here: it used to be the last of three fields on one clipped line,
    /// so on an iPhone the only number that decides whether to spend a data allowance was always
    /// the part that fell off the end.
    private var detail: String {
        [item.source, item.format].filter { !$0.isEmpty }.joined(separator: " · ")
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
