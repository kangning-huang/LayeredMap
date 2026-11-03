import Foundation
import UniformTypeIdentifiers

// From the A/B shared file (Alt C copy): every way data gets in ends here. Copy into the app, decode in Rust
// off the main thread, plain-language errors.

struct ImportError: Error {
    let message: String
}

enum Importer {
    /// Phone limit for one file (redesign › File check: "the size is under the phone limit").
    static let maxBytes: Int64 = 60 * 1024 * 1024

    static let extensions = ["geojson", "json", "csv", "tsv", "txt", "kml", "kmz", "gpx", "zip", "gpkg", "shp"] + RasterDecode.extensions

    /// A Shapefile's other parts: they travel with its .shp and never open on their own.
    static let shapefileParts = ["shx", "dbf", "prj", "cpg"]

    /// What the Files picker lets you tap (everything else is greyed out): the kinds NimbleMap
    /// opens, a Shapefile's parts (picked together with Select) and folders (the folder a
    /// Shapefile is in). Plain text is left out: it is mostly notes, not tables.
    static var contentTypes: [UTType] {
        var types: [UTType] = [.commaSeparatedText, .tabSeparatedText, .json, .zip, .folder, .tiff]
        for ext in ["geojson", "kml", "kmz", "gpx", "gpkg", "shp", "asc"] + shapefileParts {
            if let t = UTType(filenameExtension: ext), !types.contains(t) { types.append(t) }
        }
        return types
    }

    /// The app's copy of everything added (copy on open; works offline; Recents read from here).
    /// Kept out of Documents, which Files shows as "NimbleMap C" for the user's own files.
    static var dataFolder: URL {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        let dir = support.appendingPathComponent("Data", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    struct Recent: Identifiable {
        let url: URL
        let name: String
        let date: Date
        var id: URL { url }
    }

    /// Newest first by when it was last opened here (`touch` on every open; a copy keeps the
    /// source's own date otherwise). A Shapefile is kept as a folder of its parts.
    static func recents(limit: Int = 6) -> [Recent] {
        let keys: [URLResourceKey] = [.contentModificationDateKey, .isDirectoryKey]
        let entries = (try? FileManager.default.contentsOfDirectory(at: dataFolder, includingPropertiesForKeys: keys)) ?? []
        return entries
            .compactMap { url -> Recent? in
                let values = try? url.resourceValues(forKeys: Set(keys))
                let date = values?.contentModificationDate ?? .distantPast
                if values?.isDirectory == true {
                    let inside = (try? FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: nil)) ?? []
                    guard let shp = inside.first(where: { $0.pathExtension.lowercased() == "shp" }) else { return nil }
                    return Recent(url: shp, name: displayName(for: shp), date: date)
                }
                guard extensions.contains(url.pathExtension.lowercased()) else { return nil }
                return Recent(url: url, name: displayName(for: url), date: date)
            }
            .sorted { $0.date > $1.date }
            .prefix(limit)
            .map { $0 }
    }

    /// "Roads" for one file or one Shapefile's parts, "3 files" otherwise.
    static func pickedLabel(_ urls: [URL]) -> String {
        let sets = Set(urls.map(setKey))
        if sets.count == 1, let first = urls.first { return displayName(for: first) }
        return "\(urls.count) files"
    }

    static func displayName(for url: URL) -> String {
        let stem = url.deletingPathExtension().lastPathComponent
        let tidy = stem.replacingOccurrences(of: "_", with: " ").replacingOccurrences(of: "-", with: " ")
        return tidy.prefix(1).uppercased() + tidy.dropFirst()
    }

    // MARK: Opening

    // D: C's copy told people to tap "Open a folder…", a button that has never existed in any
    // spike (the label was "A Shapefile folder…"). Name the real control, and lead with the .zip.
    static let lonePartsMessage = "This map comes as several files that belong together. The easiest fix is to download it again as a .zip — a .zip opens as it is. Otherwise, under “Downloaded a .zip, or several files?”, tap “Several files that belong together” and choose the folder holding them."

    /// A file sent from another app (Open In / Share). Copies it in first.
    static func openExternal(_ url: URL) async -> Result<GeoDataset, ImportError> {
        await openPicked([url]).first ?? .failure(ImportError(message: "Couldn't read this file."))
    }

    /// What was picked in Files: one file, several (Select), or a folder. A Shapefile's parts
    /// travel together; everything else opens one dataset per file. Copies everything in first.
    static func openPicked(_ urls: [URL]) async -> [Result<GeoDataset, ImportError>] {
        let scoped = urls.filter { $0.startAccessingSecurityScopedResource() }
        defer { scoped.forEach { $0.stopAccessingSecurityScopedResource() } }
        var files: [URL] = []
        var results: [Result<GeoDataset, ImportError>] = []
        for url in urls {
            if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
                let found = mapFiles(in: url)
                if found.isEmpty {
                    results.append(.failure(ImportError(message: "There are no map files NimbleMap can open in “\(url.lastPathComponent)”.")))
                }
                files += found
            } else {
                files.append(url)
            }
        }
        let shps = Set(files.filter { $0.pathExtension.lowercased() == "shp" }.map(setKey))
        for url in files.prefix(30) {
            let ext = url.pathExtension.lowercased()
            if shapefileParts.contains(ext) {
                // Opened with its .shp; on its own it is half a map.
                if !shps.contains(setKey(url)) { results.append(.failure(ImportError(message: lonePartsMessage))) }
                continue
            }
            if ext == "shp" {
                results.append(await openShapefile(url, picked: files))
            } else {
                results.append(await openOne(url))
            }
        }
        return results
    }

    private static func openOne(_ url: URL) async -> Result<GeoDataset, ImportError> {
        let ext = url.pathExtension.lowercased()
        guard extensions.contains(ext) else {
            return .failure(ImportError(message: "NimbleMap can't open .\(ext) files yet."))
        }
        if let tooBig = sizeProblem([url]) { return .failure(tooBig) }
        let dest = uniqueDestination(for: url.lastPathComponent)
        do {
            try coordinatedCopy(url, to: dest)
        } catch {
            return .failure(ImportError(message: "Couldn't read this file: \(error.localizedDescription)"))
        }
        touch(dest)
        return await decode(dest, name: displayName(for: url), primary: nil, sourceName: "Your file", credit: nil)
    }

    /// The .shp with its parts: picked with it, or next to it in a folder NimbleMap can read
    /// (a picked folder, or one chosen for "On this iPhone").
    private static func openShapefile(_ shp: URL, picked: [URL]) async -> Result<GeoDataset, ImportError> {
        let key = setKey(shp)
        var parts = picked.filter { shapefileParts.contains($0.pathExtension.lowercased()) && setKey($0) == key }
        if !hasRequiredParts(parts) {
            parts = PhoneFiles.withAccess(to: shp) { siblingParts(of: shp) } ?? siblingParts(of: shp)
        }
        guard hasRequiredParts(parts) else { return .failure(ImportError(message: lonePartsMessage)) }
        if let tooBig = sizeProblem([shp] + parts) { return .failure(tooBig) }
        let stem = shp.deletingPathExtension().lastPathComponent
        let folder = uniqueDestination(for: stem)
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            for part in [shp] + parts {
                // One stem for every part, as the reader pairs them by name.
                let name = stem + "." + part.pathExtension.lowercased()
                try coordinatedCopy(part, to: folder.appendingPathComponent(name))
            }
        } catch {
            try? FileManager.default.removeItem(at: folder)
            return .failure(ImportError(message: "Couldn't read this Shapefile: \(error.localizedDescription)"))
        }
        touch(folder)
        let copied = folder.appendingPathComponent(stem + ".shp")
        return await decode(copied, name: displayName(for: shp), primary: nil, sourceName: "Your file", credit: nil)
    }

    private static func hasRequiredParts(_ parts: [URL]) -> Bool {
        let exts = Set(parts.map { $0.pathExtension.lowercased() })
        return exts.contains("dbf") && exts.contains("shx")
    }

    /// Same folder and same name: a .shp and its parts (case-insensitive, as Shapefiles from
    /// Windows often mix ROADS.SHP and roads.dbf).
    static func setKey(_ url: URL) -> String {
        url.deletingPathExtension().path.lowercased()
    }

    private static func siblingParts(of shp: URL) -> [URL] {
        let key = setKey(shp)
        let inside = (try? FileManager.default.contentsOfDirectory(at: shp.deletingLastPathComponent(), includingPropertiesForKeys: nil)) ?? []
        return inside.filter { shapefileParts.contains($0.pathExtension.lowercased()) && setKey($0) == key }
    }

    /// The map files in a picked folder (two levels deep, at most 200 looked at). A file iCloud
    /// Drive hasn't downloaded yet shows as ".name.icloud"; its real name is read (and so
    /// downloaded) through the coordinator.
    private static func mapFiles(in folder: URL) -> [URL] {
        guard let walk = FileManager.default.enumerator(at: folder, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsPackageDescendants]) else { return [] }
        var out: [URL] = []
        var looked = 0
        for case var url as URL in walk {
            looked += 1
            if looked > 200 { break }
            var name = url.lastPathComponent
            if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
                if name.hasPrefix(".") || walk.level >= 2 { walk.skipDescendants() }
                continue
            }
            if name.hasPrefix(".") {
                guard name.hasSuffix(".icloud") else { continue }
                name = String(name.dropFirst().dropLast(".icloud".count))
                url = url.deletingLastPathComponent().appendingPathComponent(name)
            }
            let ext = url.pathExtension.lowercased()
            if shapefileParts.contains(ext) || (extensions.contains(ext) && PhoneFiles.looksLikeMap(url, ext: ext)) {
                out.append(url)
            }
        }
        return out
    }

    private static func sizeProblem(_ urls: [URL]) -> ImportError? {
        let size = urls.reduce(Int64(0)) { sum, url in
            sum + Int64((try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0)
        }
        guard size > maxBytes else { return nil }
        return ImportError(message: "This file is too big for the phone (\(ByteCountFormatter.string(fromByteCount: size, countStyle: .file))).")
    }

    /// Reads through the file's provider, so a file that lives in iCloud Drive, Google Drive,
    /// Dropbox, OneDrive or Box is downloaded first instead of failing.
    private static func coordinatedCopy(_ source: URL, to dest: URL) throws {
        var coordination: NSError?
        var copy: Error?
        NSFileCoordinator().coordinate(readingItemAt: source, options: [], error: &coordination) { readable in
            do {
                try FileManager.default.copyItem(at: readable, to: dest)
            } catch {
                copy = error
            }
        }
        if let problem = coordination ?? copy { throw problem }
    }

    /// Marks a copy as just opened (Recents order).
    private static func touch(_ url: URL) {
        try? FileManager.default.setAttributes([.modificationDate: Date()], ofItemAtPath: url.path)
    }

    /// Paste link / a tap in the browser. Downloads off the main thread, then the same pipeline.
    static func download(_ url: URL) async -> Result<GeoDataset, ImportError> {
        guard let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http" else {
            return .failure(ImportError(message: "That isn't a web link."))
        }
        do {
            let (temp, response) = try await URLSession.shared.download(from: url)
            if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
                return .failure(ImportError(message: "The link didn't work (the site answered \(http.statusCode))."))
            }
            let mime = response.mimeType?.lowercased() ?? ""
            if mime.contains("html") {
                return .failure(ImportError(message: "This link opens a web page, not a map file."))
            }
            let size = (try? FileManager.default.attributesOfItem(atPath: temp.path)[.size] as? NSNumber)?.int64Value ?? 0
            if size > maxBytes {
                return .failure(ImportError(message: "This file is too big for the phone (\(ByteCountFormatter.string(fromByteCount: size, countStyle: .file)))."))
            }
            var fileName = response.suggestedFilename ?? url.lastPathComponent
            var ext = (fileName as NSString).pathExtension.lowercased()
            if !extensions.contains(ext) {
                ext = guessExtension(mime: mime, url: url)
                fileName = (fileName as NSString).deletingPathExtension + "." + ext
            }
            let dest = uniqueDestination(for: fileName)
            try FileManager.default.moveItem(at: temp, to: dest)
            return await decode(dest, name: displayName(for: dest), primary: nil, sourceName: url.host ?? "The web", credit: nil)
        } catch {
            return .failure(ImportError(message: "Couldn't download: \(error.localizedDescription)"))
        }
    }

    /// Bundled library file (no copy needed: it is read-only in the app bundle).
    static func openBundled(_ item: LibraryItem) async -> Result<GeoDataset, ImportError> {
        guard let url = Bundle.main.url(forResource: item.file, withExtension: nil) else {
            return .failure(ImportError(message: "This example is missing from the app."))
        }
        let result = await decode(url, name: item.title, primary: item.primaryField, sourceName: item.source, credit: item.credit)
        if case .success(let ds) = result { ds.libraryID = item.id }
        return result
    }

    static func reopen(_ recent: Recent) async -> Result<GeoDataset, ImportError> {
        let parent = recent.url.deletingLastPathComponent()
        touch(parent.standardizedFileURL == dataFolder.standardizedFileURL ? recent.url : parent)
        return await decode(recent.url, name: recent.name, primary: nil, sourceName: "Your file", credit: nil)
    }

    // MARK: Decode

    static func decode(_ file: URL, name: String, primary: String?, sourceName: String, credit: String?) async -> Result<GeoDataset, ImportError> {
        await Task.detached(priority: .userInitiated) { () -> Result<GeoDataset, ImportError> in
            if RasterDecode.isRaster(file) {
                let result = RasterDecode.open(file, name: name, sourceName: sourceName)
                if case .success(let ds) = result {
                    let data = dataFolder.standardizedFileURL.path + "/"
                    let path = file.standardizedFileURL.path
                    if path.hasPrefix(data) { ds.savedPath = String(path.dropFirst(data.count)) }
                }
                return result
            }
            switch DecodeBridge.open(path: file.path) {
            case .failure(let e):
                return .failure(ImportError(message: plain(e.localizedDescription)))
            case .success(let envelope):
                guard envelope.count > 0 else {
                    return .failure(ImportError(message: "This file has no map features in it."))
                }
                if envelope.geometry == "table" {
                    return .failure(ImportError(message: "This table has no places in it: no columns like latitude and longitude."))
                }
                var assumptions = envelope.messages
                if ["csv", "tsv", "txt"].contains(file.pathExtension.lowercased()) {
                    assumptions.insert("Places come from the latitude and longitude columns.", at: 0)
                }
                if envelope.skipped > 0 {
                    assumptions.append("\(NumberWords.integer(envelope.skipped)) rows were left out: they had no readable place.")
                }
                do {
                    let ds = try GeoParse.dataset(
                        name: name,
                        geojson: envelope.geojson,
                        kindHint: GeometryKind(rawValue: envelope.geometry),
                        assumptions: assumptions,
                        primary: primary,
                        sourceName: sourceName,
                        credit: credit
                    )
                    // Alt C: remember where it lives, and keep a KML's own colors.
                    let data = dataFolder.standardizedFileURL.path + "/"
                    let path = file.standardizedFileURL.path
                    if path.hasPrefix(data) { ds.savedPath = String(path.dropFirst(data.count)) }
                    if file.pathExtension.lowercased() == "kml",
                       let (field, colors) = FileColors.match(ds, styles: FileColors.kmlStyles(at: file)) {
                        ds.fileColorsField = field
                        ds.fileColors = colors
                    }
                    return .success(ds)
                } catch {
                    return .failure(ImportError(message: "This file has no map features in it."))
                }
            }
        }.value
    }

    /// Reader messages are written for the desktop; soften the few a phone user meets most.
    static func plain(_ message: String) -> String {
        let m = message.lowercased()
        if m.contains("shp on its own") { return lonePartsMessage }
        if m.contains("coordinate") || m.contains("latitude") { return "This table has no places in it: no columns like latitude and longitude." }
        if m.contains("too large") { return "This file is too big for the phone." }
        if m.contains("unsupported") { return "NimbleMap can't open this kind of file yet." }
        return "Couldn't open this file. (\(message))"
    }

    private static func guessExtension(mime: String, url: URL) -> String {
        let path = url.absoluteString.lowercased()
        if mime.contains("csv") || path.contains(".csv") || path.contains("csv?") { return "csv" }
        if mime.contains("zip") { return "zip" }
        if mime.contains("kml") { return "kml" }
        if mime.contains("gpx") { return "gpx" }
        return "geojson"
    }

    private static func uniqueDestination(for fileName: String) -> URL {
        let base = (fileName as NSString).deletingPathExtension
        let ext = (fileName as NSString).pathExtension
        var candidate = dataFolder.appendingPathComponent(fileName)
        var n = 2
        while FileManager.default.fileExists(atPath: candidate.path) {
            candidate = dataFolder.appendingPathComponent(ext.isEmpty ? "\(base) \(n)" : "\(base) \(n).\(ext)")
            n += 1
        }
        return candidate
    }
}
