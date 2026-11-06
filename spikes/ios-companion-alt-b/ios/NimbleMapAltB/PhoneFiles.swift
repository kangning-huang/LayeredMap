import Foundation
import SwiftUI
import UniformTypeIdentifiers

// Shared by Alt A and Alt B: "On this iPhone", the map files NimbleMap can open, newest first.
// iOS shows an app nothing of the phone it wasn't given, so the list is what NimbleMap can see:
// files opened here (Recents) and folders the person chose once (Downloads, or a folder in
// iCloud Drive, Google Drive, Dropbox, OneDrive or Box: any app that shows up in Files).

struct PhoneFile: Identifiable, Sendable {
    let id: String
    let url: URL
    let name: String
    let ext: String
    let date: Date
    /// "Opened", "Downloaded", "Added" or "Changed".
    let verb: String
    /// The chosen folder it was found in; nil for a file opened here before.
    let folderID: String?
    let folderName: String?
    /// A Shapefile's missing parts ("dbf", "shx"); such a row can't open.
    let missing: [String]
    /// Not downloaded yet (iCloud Drive placeholder): opening downloads it.
    let inCloud: Bool

    var kind: String {
        switch ext {
        case "shp": return "Shapefile"
        case "geojson", "json": return "GeoJSON"
        case "csv", "tsv", "txt": return "Table"
        case "kml", "kmz": return "Google Earth"
        case "gpx": return "GPS track"
        case "gpkg": return "GeoPackage"
        case "zip": return "Zip"
        default: return ext.uppercased()
        }
    }

    var symbol: String {
        switch ext {
        case "csv", "tsv", "txt": return "tablecells"
        case "gpx": return "point.topleft.down.to.point.bottomright.curvepath"
        case "zip": return "doc.zipper"
        default: return "map"
        }
    }
}

enum PhoneFiles {
    struct Folder: Identifiable, Sendable {
        let id: String
        let name: String
        let bookmark: Data
    }

    private static let key = "lm.phoneFolders"

    static var folders: [Folder] {
        let raw = UserDefaults.standard.array(forKey: key) as? [[String: Any]] ?? []
        return raw.compactMap { d in
            guard let id = d["id"] as? String, let name = d["name"] as? String, let mark = d["bookmark"] as? Data else { return nil }
            return Folder(id: id, name: name, bookmark: mark)
        }
    }

    private static func save(_ list: [Folder]) {
        UserDefaults.standard.set(list.map { ["id": $0.id, "name": $0.name, "bookmark": $0.bookmark] }, forKey: key)
    }

    /// A folder picked in Files: remembered (a bookmark), so it is listed on every visit.
    @discardableResult
    static func addFolder(_ url: URL) -> Bool {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        guard let mark = try? url.bookmarkData(options: [], includingResourceValuesForKeys: nil, relativeTo: nil) else { return false }
        var list = folders.filter { resolve($0)?.standardizedFileURL != url.standardizedFileURL }
        list.append(Folder(id: UUID().uuidString, name: url.lastPathComponent, bookmark: mark))
        save(list)
        return true
    }

    static func removeFolder(_ id: String) {
        save(folders.filter { $0.id != id })
    }

    private static func resolve(_ folder: Folder) -> URL? {
        var stale = false
        return try? URL(resolvingBookmarkData: folder.bookmark, options: [], relativeTo: nil, bookmarkDataIsStale: &stale)
    }

    /// Runs `work` with read access to the chosen folder that holds `url`; nil if none does.
    static func withAccess<T>(to url: URL, _ work: () -> T) -> T? {
        let path = url.standardizedFileURL.path
        for folder in folders {
            guard let root = resolve(folder), path.hasPrefix(root.standardizedFileURL.path + "/") else { continue }
            let scoped = root.startAccessingSecurityScopedResource()
            defer { if scoped { root.stopAccessingSecurityScopedResource() } }
            return work()
        }
        return nil
    }

    // MARK: Listing

    static func list(limit: Int = 30) async -> [PhoneFile] {
        await Task.detached(priority: .userInitiated) { scan(limit: limit) }.value
    }

    private static func scan(limit: Int) -> [PhoneFile] {
        var all: [PhoneFile] = Importer.recents(limit: 50).map { r in
            PhoneFile(
                id: "opened:" + r.url.path, url: r.url, name: r.name, ext: r.url.pathExtension.lowercased(),
                date: r.date, verb: "Opened", folderID: nil, folderName: nil, missing: [], inCloud: false
            )
        }
        for folder in folders {
            all += scan(folder)
        }
        // The same file opened here and still in its folder: one row, the newer.
        var newest: [String: PhoneFile] = [:]
        for f in all {
            let k = f.name.lowercased() + "." + f.kind
            if let have = newest[k], have.date >= f.date { continue }
            newest[k] = f
        }
        return newest.values.sorted { $0.date > $1.date }.prefix(limit).map { $0 }
    }

    private struct Entry {
        let url: URL
        let ext: String
        let added: Date
        let changed: Date
        let inCloud: Bool
    }

    private static func scan(_ folder: Folder) -> [PhoneFile] {
        guard let root = resolve(folder) else { return [] }
        let scoped = root.startAccessingSecurityScopedResource()
        defer { if scoped { root.stopAccessingSecurityScopedResource() } }
        let keys: [URLResourceKey] = [.isDirectoryKey, .addedToDirectoryDateKey, .creationDateKey, .contentModificationDateKey, .totalFileAllocatedSizeKey]
        guard let walk = FileManager.default.enumerator(at: root, includingPropertiesForKeys: keys, options: [.skipsPackageDescendants]) else { return [] }
        var entries: [Entry] = []
        var looked = 0
        for case let url as URL in walk {
            looked += 1
            if looked > 3000 { break }
            let values = try? url.resourceValues(forKeys: Set(keys))
            var name = url.lastPathComponent
            var real = url
            var inCloud = false
            if values?.isDirectory == true {
                if name.hasPrefix(".") || walk.level >= 3 { walk.skipDescendants() }
                continue
            }
            if name.hasPrefix(".") {
                // iCloud Drive keeps a file it hasn't downloaded as ".name.ext.icloud".
                guard name.hasSuffix(".icloud") else { continue }
                name = String(name.dropFirst().dropLast(".icloud".count))
                real = url.deletingLastPathComponent().appendingPathComponent(name)
                inCloud = true
            }
            let ext = (name as NSString).pathExtension.lowercased()
            let isPart = Importer.shapefileParts.contains(ext)
            guard Importer.extensions.contains(ext) || isPart else { continue }
            if !isPart && !inCloud && !looksLikeMap(real, ext: ext) { continue }
            let changed = values?.contentModificationDate ?? values?.creationDate ?? .distantPast
            let added = values?.addedToDirectoryDate ?? values?.creationDate ?? changed
            entries.append(Entry(url: real, ext: ext, added: added, changed: changed, inCloud: inCloud))
        }

        let downloads = folder.name.lowercased() == "downloads"
        var parts: [String: [Entry]] = [:]
        for e in entries where Importer.shapefileParts.contains(e.ext) {
            parts[Importer.setKey(e.url), default: []].append(e)
        }
        return entries.filter { !Importer.shapefileParts.contains($0.ext) }.map { e in
            let mine = e.ext == "shp" ? parts[Importer.setKey(e.url)] ?? [] : []
            let added = ([e] + mine).map(\.added).max() ?? e.added
            let changed = ([e] + mine).map(\.changed).max() ?? e.changed
            let have = Set(mine.map(\.ext))
            return PhoneFile(
                id: folder.id + ":" + e.url.path,
                url: e.url,
                name: Importer.displayName(for: e.url),
                ext: e.ext,
                date: max(added, changed),
                verb: changed > added ? "Changed" : (downloads ? "Downloaded" : "Added"),
                folderID: folder.id,
                folderName: folder.name,
                missing: e.ext == "shp" ? ["dbf", "shx"].filter { !have.contains($0) } : [],
                inCloud: e.inCloud || mine.contains(where: \.inCloud)
            )
        }
    }

    /// A cheap look inside a file that is already on the phone, so a Downloads folder full of
    /// other JSON, zips and tables lists only the ones that hold places. The reader decides for
    /// real when it opens; a file still in the cloud is listed unread (reading would download it).
    static func looksLikeMap(_ url: URL, ext: String) -> Bool {
        let local = ((try? url.resourceValues(forKeys: [.totalFileAllocatedSizeKey]).totalFileAllocatedSize) ?? 0) > 0
        guard local, let handle = try? FileHandle(forReadingFrom: url) else { return true }
        defer { try? handle.close() }
        func text(_ data: Data?) -> String { String(decoding: data ?? Data(), as: UTF8.self).lowercased() }
        switch ext {
        case "json":
            let head = text(try? handle.read(upToCount: 16_384))
            return head.contains("\"featurecollection\"") || head.contains("\"feature\"") || head.contains("\"coordinates\"")
        case "csv", "tsv", "txt":
            let head = text(try? handle.read(upToCount: 4_096))
            let header = head.split(whereSeparator: \.isNewline).first.map(String.init) ?? ""
            let names = Set(header.split(whereSeparator: { ",;\t|".contains($0) }).map { $0.trimmingCharacters(in: CharacterSet(charactersIn: " \"'")) })
            let places: Set<String> = ["lat", "latitude", "lon", "lng", "long", "longitude", "x", "y", "wkt", "geometry", "geom", "纬度", "经度"]
            return !names.isDisjoint(with: places)
        case "zip":
            // A zip lists its files at the end: look there for a map file's name.
            let end = (try? handle.seekToEnd()) ?? 0
            try? handle.seek(toOffset: end > 65_536 ? end - 65_536 : 0)
            let tail = text(try? handle.readToEnd())
            return [".shp", ".geojson", ".json", ".kml", ".gpx", ".gpkg", ".csv"].contains { tail.contains($0) }
        default:
            return true
        }
    }

    // MARK: Opening

    static func open(_ file: PhoneFile) async -> [Result<GeoDataset, ImportError>] {
        guard file.folderID != nil else {
            return [await Importer.reopen(Importer.Recent(url: file.url, name: file.name, date: file.date))]
        }
        guard let folder = folders.first(where: { $0.id == file.folderID }), let root = resolve(folder) else {
            return [.failure(ImportError(message: "NimbleMap can no longer see this folder. Choose it again."))]
        }
        let scoped = root.startAccessingSecurityScopedResource()
        defer { if scoped { root.stopAccessingSecurityScopedResource() } }
        return await Importer.openPicked([file.url])
    }
}

// MARK: - Self-test (spike-only)

extension PhoneFiles {
    /// LM_PHONE_TEST=<folder in the app's Documents>: lists that folder as a chosen one, prints
    /// the ranked rows, then opens each and prints what came back (`devicectl … --console`).
    static func runSelfTestIfAsked() {
        guard let name = ProcessInfo.processInfo.environment["LM_PHONE_TEST"],
              let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else { return }
        addFolder(docs.appendingPathComponent(name, isDirectory: true))
        Task.detached {
            for ext in ["shp", "shx", "dbf", "prj", "cpg", "geojson", "gpkg"] {
                let t = UTType(filenameExtension: ext)
                print("LM_PHONE type .\(ext) → \(t?.identifier ?? "nil") data=\(t?.conforms(to: .data) ?? false) picker=\(t.map { Importer.contentTypes.contains($0) } ?? false)")
            }
            let test = docs.appendingPathComponent(name, isDirectory: true)
            for file in (try? FileManager.default.contentsOfDirectory(at: test, includingPropertiesForKeys: [.contentTypeKey])) ?? [] {
                let t = try? file.resourceValues(forKeys: [.contentTypeKey]).contentType
                print("LM_PHONE file \(file.lastPathComponent) → \(t?.identifier ?? "nil")")
            }
            let rows = await list()
            for f in rows {
                print("LM_PHONE row \(f.kind) | \(f.name) | \(f.verb) | missing=\(f.missing) | \(f.folderName ?? "opened here")")
            }
            for f in rows where f.folderID != nil {
                for r in await open(f) {
                    switch r {
                    case .success(let ds): print("LM_PHONE open \(f.name): ok, \(ds.countPhrase)")
                    case .failure(let e): print("LM_PHONE open \(f.name): \(e.message)")
                    }
                }
            }
            for r in await Importer.openPicked([test]) {
                switch r {
                case .success(let ds): print("LM_PHONE folder: ok, \(ds.countPhrase)")
                case .failure(let e): print("LM_PHONE folder: \(e.message)")
                }
            }
            print("LM_PHONE done")
            fflush(stdout)
        }
    }
}

// MARK: - The list

/// What one Files picker is open for. Folders get a picker of their own (only folders), so
/// Open works inside the folder whatever the cloud app says its files are. `anyFile` is for
/// cloud apps that call every map file "data" (Google Drive does), which the map-only picker
/// greys out.
enum FilePick {
    case files
    case anyFile
    case openFolder
    case listFolder

    var types: [UTType] {
        switch self {
        case .files: return Importer.contentTypes
        case .anyFile: return [.item]
        case .openFolder, .listFolder: return [.folder]
        }
    }

    var several: Bool { self == .files || self == .anyFile }
}

/// "On this iPhone": a section for a List. The parent shows the folder picker (one file picker
/// per view: SwiftUI drops a second one), then bumps `refresh`.
struct PhoneFilesSection: View {
    var limit = 6
    var refresh = 0
    var onOpen: (PhoneFile) -> Void
    var onChooseFolder: () -> Void

    @State private var files: [PhoneFile] = []
    @State private var folders: [PhoneFiles.Folder] = []
    @State private var loaded = false

    var body: some View {
        Section {
            if loaded && files.isEmpty {
                Text(folders.isEmpty ? "Files you open show up here." : "No map files in these folders yet.")
                    .foregroundStyle(.secondary)
            }
            ForEach(files.prefix(limit)) { f in
                row(f)
            }
            ForEach(folders) { folder in
                Label(folder.name, systemImage: "folder")
                    .foregroundStyle(.secondary)
                    .swipeActions {
                        Button("Stop listing", role: .destructive) {
                            PhoneFiles.removeFolder(folder.id)
                            Task { await reload() }
                        }
                    }
            }
            Button(action: onChooseFolder) {
                Label(folders.isEmpty ? "List map files from a folder…" : "Add another folder…", systemImage: "folder.badge.plus")
            }
        } header: {
            Text("On this iPhone")
        } footer: {
            Text("Map files NimbleMap can open, newest first. Choose Downloads, or a folder in iCloud Drive, Google Drive, Dropbox, OneDrive or Box.")
        }
        .task(id: refresh) { await reload() }
    }

    private func reload() async {
        folders = PhoneFiles.folders
        files = await PhoneFiles.list()
        loaded = true
    }

    private func row(_ f: PhoneFile) -> some View {
        Button {
            onOpen(f)
        } label: {
            HStack(spacing: 12) {
                Image(systemName: f.inCloud ? "icloud.and.arrow.down" : f.symbol)
                    .foregroundStyle(f.missing.isEmpty ? Color.accentColor : Color.secondary)
                    .frame(width: 28)
                VStack(alignment: .leading, spacing: 2) {
                    Text(f.name).foregroundStyle(.primary).lineLimit(1)
                    Text(subtitle(f)).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
        }
        .disabled(!f.missing.isEmpty)
    }

    private func subtitle(_ f: PhoneFile) -> String {
        if !f.missing.isEmpty {
            return "Shapefile · missing its " + f.missing.map { "." + $0 }.joined(separator: " and ")
        }
        let when = RelativeDateTimeFormatter().localizedString(for: f.date, relativeTo: Date())
        var parts = [f.kind, "\(f.verb) \(when)"]
        if let place = f.folderName { parts.append(place) }
        return parts.joined(separator: " · ")
    }
}
