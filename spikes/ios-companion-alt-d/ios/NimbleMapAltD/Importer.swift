import Compression
import CoreLocation
import Foundation
import UniformTypeIdentifiers

// From the A/B shared file (Alt C copy): every way data gets in ends here. Copy into the app, decode in Rust
// off the main thread, plain-language errors.

struct ImportError: Error {
    let message: String
    /// Data in a local grid with nothing to guess the grid from: the app's copy, to read again
    /// once the person says roughly where it is.
    var needsPlace: URL? = nil
}

/// Where a grid for data in metres should be guessed near: the map, when it is zoomed in on
/// somewhere, else where the person last was. Read off the main thread by the importer.
final class NearHint: @unchecked Sendable {
    static let shared = NearHint()
    private let lock = NSLock()
    private var map: CLLocationCoordinate2D?
    private var me: CLLocationCoordinate2D?

    /// The visible area, from the map: only a view of a region or smaller says "here".
    func mapView(_ b: GeoBounds) {
        lock.lock()
        defer { lock.unlock() }
        map = b.isValid && (b.maxLon - b.minLon) < 30 && (b.maxLat - b.minLat) < 30 ? b.center : nil
    }

    func person(_ c: CLLocationCoordinate2D) {
        lock.lock()
        defer { lock.unlock() }
        me = c
    }

    var current: CLLocationCoordinate2D? {
        lock.lock()
        defer { lock.unlock() }
        return map ?? me
    }
}

enum Importer {
    /// Above this a file is worth a word of warning — not a refusal.
    ///
    /// There used to be a 60 MB cap that turned anything larger away, and it was checked *after*
    /// the whole file had come down, so a 220 MB download spent the person's data and then told
    /// them no. Six of the catalogue's own entries were over it. A phone can open these; the only
    /// honest problems are that it takes a while and takes up room, and both are the owner's call
    /// to make, not the app's.
    static let bigBytes: Int64 = 60 * 1024 * 1024

    /// Is this worth warning about before it starts?
    static func isBig(_ bytes: Int64) -> Bool { bytes > bigBytes }

    static func sizeText(_ bytes: Int64) -> String {
        ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
    }

    static let extensions = ["geojson", "json", "csv", "tsv", "txt", "kml", "kmz", "gpx", "zip", "gpkg", "shp", "xlsx", "xls"] + RasterDecode.extensions

    /// The kind of file inside: "nc" for "gistemp.nc.gz", the extension itself otherwise.
    static func innerExtension(_ url: URL) -> String {
        let ext = url.pathExtension.lowercased()
        return ext == "gz" ? url.deletingPathExtension().pathExtension.lowercased() : ext
    }

    /// A file NimbleMap opens, including one gzipped around a kind it opens ("x.nc.gz").
    static func canOpen(_ url: URL) -> Bool {
        let inner = innerExtension(url)
        return extensions.contains(inner) && (url.pathExtension.lowercased() != "gz" || inner != "gz")
    }

    /// A Shapefile's other parts: they travel with its .shp and never open on their own.
    static let shapefileParts = ["shx", "dbf", "prj", "cpg"]

    /// What the Files picker lets you tap (everything else is greyed out): the kinds NimbleMap
    /// opens, a Shapefile's parts (picked together with Select) and folders (the folder a
    /// Shapefile is in). Plain text is left out: it is mostly notes, not tables.
    static var contentTypes: [UTType] {
        var types: [UTType] = [.commaSeparatedText, .tabSeparatedText, .json, .zip, .folder, .tiff, .gzip]
        for ext in ["geojson", "kml", "kmz", "gpx", "gpkg", "shp", "asc", "nc", "nc4", "cdf", "xlsx", "xls"] + shapefileParts {
            if let t = UTType(filenameExtension: ext), !types.contains(t) { types.append(t) }
        }
        // NetCDF has no system UTI, so the app declares one (Info.plist). Without this the Files
        // picker greys out every .nc file.
        if let nc = UTType("org.unidata.netcdf"), !types.contains(nc) { types.append(nc) }
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
        var stem = url.deletingPathExtension().lastPathComponent
        if url.pathExtension.lowercased() == "gz" { stem = (stem as NSString).deletingPathExtension }
        let tidy = stem.replacingOccurrences(of: "_", with: " ").replacingOccurrences(of: "-", with: " ")
        return tidy.prefix(1).uppercased() + tidy.dropFirst()
    }

    // MARK: Opening

    // D: C's copy told people to tap "Open a folder…", a button that has never existed in any
    // spike (the label was "A Shapefile folder…"). Name the real control, and lead with the .zip.
    static let lonePartsMessage = "This map comes as several files that belong together. The easiest fix is to download it again as a .zip — a .zip opens as it is. Otherwise, under “Downloaded a .zip, or several files?”, tap “Several files that belong together” and choose the folder holding them."

    /// A file sent from another app (Open In / Share). Copies it in first.
    static func openExternal(_ url: URL) async -> [Result<GeoDataset, ImportError>] {
        let all = await openPicked([url])
        return all.isEmpty ? [.failure(ImportError(message: "Couldn't read this file."))] : all
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
                results.append(contentsOf: await openOne(url))
            }
        }
        return results
    }

    private static func openOne(_ url: URL) async -> [Result<GeoDataset, ImportError>] {
        if url.pathExtension.lowercased() == "gz" || Gzip.isGzip(url) {
            return [await openGzip(url, name: Gzip.innerName(url.lastPathComponent), sourceName: "Your file", named: nil)]
        }
        let ext = url.pathExtension.lowercased()
        guard extensions.contains(ext) else {
            return [.failure(ImportError(message: "NimbleMap can't open .\(ext) files yet."))]
        }
        let dest = uniqueDestination(for: url.lastPathComponent)
        do {
            try coordinatedCopy(url, to: dest)
        } catch {
            return [.failure(ImportError(message: "Couldn't read this file: \(error.localizedDescription)"))]
        }
        // A table in a local grid can come with a .prj that says which: it travels with the copy
        // (renamed to match), or the reader never sees it.
        let prj = url.deletingPathExtension().appendingPathExtension("prj")
        if FileManager.default.fileExists(atPath: prj.path) {
            try? coordinatedCopy(prj, to: dest.deletingPathExtension().appendingPathExtension("prj"))
        }
        touch(dest)
        return await decodeAll(dest, name: displayName(for: url), primary: nil, sourceName: "Your file", credit: nil)
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
            if shapefileParts.contains(ext) || (ext == "gz" && canOpen(url)) || (extensions.contains(ext) && PhoneFiles.looksLikeMap(url, ext: ext)) {
                out.append(url)
            }
        }
        return out
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
    ///
    /// D reports progress while it runs: a bare "Adding …" capsule with no bar and no way out
    /// reads as a hang on anything bigger than a second or two, and the catalogue links files of
    /// tens of megabytes. `onProgress` is called on the main actor with bytes received and the
    /// total the server promised (0 when it won't say).
    static func download(
        _ url: URL,
        /// What the person tapped. A catalogue row knows its own name ("Rivers of the world");
        /// without this the layer is named from the file ("Ne 10m rivers lake centerlines"), which
        /// reads as though the wrong thing was downloaded.
        named: String? = nil,
        onProgress: @escaping @Sendable (Int64, Int64) -> Void = { _, _ in }
    ) async -> [Result<GeoDataset, ImportError>] {
        guard let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http" else {
            return [.failure(ImportError(message: "That isn't a web link."))]
        }
        do {
            let (temp, response) = try await Downloader(onProgress: onProgress).run(url)
            if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
                return [.failure(ImportError(message: "The link didn't work (the site answered \(http.statusCode))."))]
            }
            let mime = response.mimeType?.lowercased() ?? ""
            if mime.contains("html") {
                return [.failure(ImportError(message: "This link opens a web page, not a map file."))]
            }
            let size = (try? FileManager.default.attributesOfItem(atPath: temp.path)[.size] as? NSNumber)?.int64Value ?? 0
            var fileName = response.suggestedFilename ?? url.lastPathComponent
            if fileName.lowercased().hasSuffix(".gz") || Gzip.isGzip(temp) {
                defer { try? FileManager.default.removeItem(at: temp) }
                var inner = Gzip.innerName(fileName)
                if inner == fileName { inner = Gzip.innerName(url.lastPathComponent) }
                return [await openGzip(temp, name: inner, sourceName: url.host ?? "The web", named: named)]
            }
            var ext = (fileName as NSString).pathExtension.lowercased()
            if !extensions.contains(ext) {
                ext = guessExtension(mime: mime, url: url)
                fileName = (fileName as NSString).deletingPathExtension + "." + ext
            }
            let dest = uniqueDestination(for: fileName)
            try FileManager.default.moveItem(at: temp, to: dest)
            return await decodeAll(dest, name: named ?? displayName(for: dest), primary: nil, sourceName: url.host ?? "The web", credit: nil)
        } catch is CancellationError {
            return [.failure(ImportError(message: cancelled))]
        } catch let error as URLError where error.code == .cancelled {
            return [.failure(ImportError(message: cancelled))]
        } catch {
            return [.failure(ImportError(message: "Couldn't download: \(error.localizedDescription)"))]
        }
    }

    /// Unpacks a gzip file into the app's data folder as `name` ("gistemp.nc") and opens that.
    /// Runs off the main actor: a 26 MB NetCDF unpacks to well over 100 MB.
    private static func openGzip(_ gz: URL, name: String, sourceName: String, named: String?) async -> Result<GeoDataset, ImportError> {
        let ext = (name as NSString).pathExtension.lowercased()
        guard extensions.contains(ext) else {
            let what = ext.isEmpty ? "this file" : ".\(ext) files"
            return .failure(ImportError(message: "This is a compressed (.gz) file, and NimbleMap can't open \(what) inside it yet."))
        }
        let dest = uniqueDestination(for: name)
        let unpacked: Result<Void, ImportError> = await Task.detached(priority: .userInitiated) {
            do {
                try Gzip.inflate(gz, to: dest)
                return .success(())
            } catch let e as Gzip.Failure {
                try? FileManager.default.removeItem(at: dest)
                return .failure(ImportError(message: "Couldn't unpack this .gz file: \(e.message)"))
            } catch {
                try? FileManager.default.removeItem(at: dest)
                return .failure(ImportError(message: "Couldn't unpack this .gz file: \(error.localizedDescription)"))
            }
        }.value
        if case .failure(let e) = unpacked { return .failure(e) }
        touch(dest)
        return await decode(dest, name: named ?? displayName(for: dest), primary: nil, sourceName: sourceName, credit: nil)
    }

    /// A Stop that the person pressed is not an error worth an alert; the caller drops it.
    static let cancelled = "__lm_cancelled__"

    /// Downloads a URL while reporting progress.
    ///
    /// This owns its own `URLSession` on purpose: `URLSession.shared` does not deliver per-task
    /// delegate callbacks, so `shared.download(from:delegate:)` runs to completion with the bar
    /// never moving. A session that owns the delegate gets `didWriteData` as the bytes arrive.
    private final class Downloader: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
        private let onProgress: @Sendable (Int64, Int64) -> Void
        private let lock = NSLock()
        private var continuation: CheckedContinuation<(URL, URLResponse), Error>?
        private var task: URLSessionDownloadTask?
        private lazy var session = URLSession(configuration: .default, delegate: self, delegateQueue: nil)

        init(onProgress: @escaping @Sendable (Int64, Int64) -> Void) {
            self.onProgress = onProgress
        }

        func run(_ url: URL) async throws -> (URL, URLResponse) {
            try await withTaskCancellationHandler {
                try await withCheckedThrowingContinuation { c in
                    lock.lock()
                    continuation = c
                    let t = session.downloadTask(with: url)
                    task = t
                    lock.unlock()
                    t.resume()
                }
            } onCancel: {
                lock.lock()
                let t = task
                lock.unlock()
                t?.cancel()
            }
        }

        /// Resumes the continuation exactly once; later callbacks for the same task are ignored.
        private func finish(_ result: Result<(URL, URLResponse), Error>) {
            lock.lock()
            let c = continuation
            continuation = nil
            lock.unlock()
            guard let c else { return }
            c.resume(with: result)
            session.finishTasksAndInvalidate()
        }

        func urlSession(
            _ session: URLSession,
            downloadTask: URLSessionDownloadTask,
            didWriteData bytesWritten: Int64,
            totalBytesWritten: Int64,
            totalBytesExpectedToWrite: Int64
        ) {
            // NSURLSessionTransferSizeUnknown (-1) when the server sends no Content-Length
            // (chunked or gzipped replies); 0 keeps the bar indeterminate instead of guessing.
            let total = max(0, totalBytesExpectedToWrite)
            let send = onProgress
            Task { @MainActor in send(totalBytesWritten, total) }
        }

        func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
            // URLSession deletes `location` as soon as this returns, so move it first.
            let keep = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            do {
                try FileManager.default.moveItem(at: location, to: keep)
                finish(.success((keep, downloadTask.response ?? URLResponse())))
            } catch {
                finish(.failure(error))
            }
        }

        func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
            if let error { finish(.failure(error)) }
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

    static func reopen(_ recent: Recent) async -> [Result<GeoDataset, ImportError>] {
        let parent = recent.url.deletingLastPathComponent()
        touch(parent.standardizedFileURL == dataFolder.standardizedFileURL ? recent.url : parent)
        return await decodeAll(recent.url, name: recent.name, primary: nil, sourceName: "Your file", credit: nil)
    }

    // MARK: Decode

    /// Where the reader writes each opened layer's files (`store.rs`). Cleared at launch: layers
    /// are read again from the copies in `dataFolder` when a saved map comes back.
    static var layerFolder: URL {
        let caches = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first ?? FileManager.default.temporaryDirectory
        return caches.appendingPathComponent("layers", isDirectory: true)
    }

    static func clearLayerFolder() {
        try? FileManager.default.removeItem(at: layerFolder)
    }

    /// The first layer of a file (most files have one). `near` and `epsg` are for data in a local
    /// grid (see `NearHint`, `Placement`): where to guess its grid, or the grid the person chose.
    static func decode(
        _ file: URL, name: String, primary: String?, sourceName: String, credit: String?,
        near: CLLocationCoordinate2D? = NearHint.shared.current, epsg: Int? = nil
    ) async -> Result<GeoDataset, ImportError> {
        await decodeAll(file, name: name, primary: primary, sourceName: sourceName, credit: credit, near: near, epsg: epsg).first
            ?? .failure(ImportError(message: "This file has no map features in it."))
    }

    /// Every layer of a file: one per layer of a .zip (the eclipse's eight Shapefiles, Geofabrik's
    /// twenty), one for anything else. `member` picks one layer of a .zip (a saved map reopening).
    static func decodeAll(
        _ file: URL, name: String, primary: String?, sourceName: String, credit: String?,
        near: CLLocationCoordinate2D? = NearHint.shared.current, epsg: Int? = nil, member: String? = nil
    ) async -> [Result<GeoDataset, ImportError>] {
        await Task.detached(priority: .userInitiated) { () -> [Result<GeoDataset, ImportError>] in
            let data = dataFolder.standardizedFileURL.path + "/"
            let path = file.standardizedFileURL.path
            let saved = path.hasPrefix(data) ? String(path.dropFirst(data.count)) : nil
            if RasterDecode.isRaster(file) {
                let result = RasterDecode.open(file, name: name, sourceName: sourceName)
                if case .success(let ds) = result { ds.savedPath = saved }
                return [result]
            }
            switch DecodeBridge.openAll(path: file.path, outDir: layerFolder.appendingPathComponent(UUID().uuidString), near: near, epsg: epsg) {
            case .failure(let e):
                return [.failure(ImportError(message: plain(e.localizedDescription)))]
            case .success(var envelopes):
                if let member { envelopes = envelopes.filter { $0.member == member } }
                let several = envelopes.count > 1
                return envelopes.map { envelope in
                    // One layer keeps the file's name; several are named by their own files.
                    let layerName = several ? displayName(for: URL(fileURLWithPath: envelope.name + ".x")) : name
                    let result = dataset(from: envelope, file: file, name: layerName, primary: primary,
                                         sourceName: sourceName, credit: credit, near: near, single: !several && envelope.member == nil)
                    if case .success(let ds) = result {
                        ds.savedPath = saved
                        ds.member = envelope.member
                    }
                    return result
                }
            }
        }.value
    }

    /// One layer from the reader, as the map's dataset.
    private static func dataset(
        from envelope: DecodeBridge.Envelope, file: URL, name: String, primary: String?, sourceName: String,
        credit: String?, near: CLLocationCoordinate2D?, single: Bool
    ) -> Result<GeoDataset, ImportError> {
        guard envelope.count > 0 else {
            return .failure(ImportError(message: "This file has no map features in it."))
        }
        if envelope.geometry == "table" {
            return .failure(ImportError(message: "This table has no places in it: no columns like latitude and longitude, only names or codes. NimbleMap on iPhone can't join a table to places yet; on the Mac you can join it to the countries layer."))
        }
        let placed = envelope.placement
        let how = placed?["how"] as? String
        if how == "unplaced" {
            return .failure(ImportError(
                message: "“\(name)” uses coordinates in a local grid, not latitude and longitude, and NimbleMap needs to know roughly where it is.",
                needsPlace: single ? file : nil
            ))
        }
        var assumptions = envelope.messages
        if ["csv", "tsv", "txt"].contains(file.pathExtension.lowercased()) && how == nil
            && !assumptions.contains(where: { $0.hasPrefix("Places from") }) {
            assumptions.insert("Places come from the latitude and longitude columns.", at: 0)
        }
        var placement: Placement?
        if let placed, let how {
            let grid = placed["name"] as? String ?? "a local grid"
            let others = (placed["alternatives"] as? [[String: Any]] ?? []).compactMap { a -> (epsg: Int, name: String)? in
                guard let code = (a["epsg"] as? NSNumber)?.intValue, let n = a["name"] as? String else { return nil }
                return (code, n)
            }
            switch how {
            case "prj": assumptions.insert("Placed with the grid named in its .prj file: \(grid).", at: 0)
            case "chosen": assumptions.insert("Placed using \(grid), the grid you chose.", at: 0)
            default: assumptions.insert("Its coordinates are in a local grid, not latitude and longitude. Placed using \(grid): the grid that puts it nearest to where your map was. If it's in the wrong place, try another grid below.", at: 0)
            }
            // Another grid reads the file again: only for a file that is this one layer.
            if single { placement = Placement(file: file, how: how, name: grid, alternatives: others, near: near) }
        }
        if envelope.skipped > 0 {
            assumptions.append("\(NumberWords.integer(envelope.skipped)) rows were left out: they had no readable place.")
        }
        do {
            let ds = try GeoParse.dataset(
                name: name,
                files: envelope.files,
                count: envelope.count,
                kindHint: GeometryKind(rawValue: envelope.geometry),
                assumptions: assumptions,
                primary: primary,
                sourceName: sourceName,
                credit: credit
            )
            ds.placement = placement
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

    /// Reader messages are written for the desktop; soften the few a phone user meets most.
    static func plain(_ message: String) -> String {
        // Already written for a person by the phone's own reader (too big, nothing in a zip…).
        if (message.contains("NimbleMap") || message.contains("this iPhone")) && !message.hasPrefix("/") { return message }
        let m = message.lowercased()
        if m.contains("shp on its own") { return lonePartsMessage }
        if m.contains("coordinate") || m.contains("latitude") { return "This table has no places in it: no columns like latitude and longitude." }
        if m.contains("too large") { return "This file is too big for this phone to open." }
        if m.contains("unsupported") { return "NimbleMap can't open this kind of file yet." }
        if m.contains("never closed") || m.contains("unexpected end") || m.contains("unexpected eof") || m.contains("truncated") {
            return "This file looks cut short or damaged. Try downloading it again."
        }
        // The reader names the file by its full path inside the app ("/var/mobile/…/x.kml: …"),
        // which means nothing to the person and pushes the actual reason off the screen.
        var reason = message
        if reason.hasPrefix("/"), let r = reason.range(of: ": ") { reason = String(reason[r.upperBound...]) }
        return "Couldn't open this file. (\(reason))"
    }

    private static func guessExtension(mime: String, url: URL) -> String {
        let path = url.absoluteString.lowercased()
        if mime.contains("csv") || path.contains(".csv") || path.contains("csv?") { return "csv" }
        if mime.contains("zip") && !mime.contains("gzip") { return "zip" }
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

// MARK: Gzip

/// A `.gz` around a file NimbleMap opens (the site's GISTEMP link is `.nc.gz`). Nothing in the app
/// unpacked gzip, so the link was not recognised as data, and a file that did arrive was refused
/// as ".gz" — or worse, renamed `.zip` because "x-gzip" contains "zip".
enum Gzip {
    /// The two bytes every gzip member starts with.
    static func isGzip(_ url: URL) -> Bool {
        guard let h = try? FileHandle(forReadingFrom: url) else { return false }
        defer { try? h.close() }
        let magic = (try? h.read(upToCount: 2)) ?? Data()
        return magic.count == 2 && magic[magic.startIndex] == 0x1f && magic[magic.startIndex + 1] == 0x8b
    }

    /// "gistemp.nc.gz" → "gistemp.nc"; "x.tgz"/"x.gz" with nothing inside the name → "x".
    static func innerName(_ name: String) -> String {
        let lower = name.lowercased()
        if lower.hasSuffix(".gz") { return String(name.dropLast(3)) }
        return name
    }

    struct Failure: Error { let message: String }

    /// Streams `source` through raw DEFLATE into `dest`. Uncompressed NetCDF grids run to
    /// hundreds of megabytes, so neither side is ever held in memory whole.
    static func inflate(_ source: URL, to dest: URL) throws {
        let input = try FileHandle(forReadingFrom: source)
        defer { try? input.close() }
        let header = try input.read(upToCount: 64 * 1024) ?? Data()
        let start = try headerLength(header)
        try input.seek(toOffset: UInt64(start))

        FileManager.default.createFile(atPath: dest.path, contents: nil)
        let output = try FileHandle(forWritingTo: dest)
        defer { try? output.close() }

        let chunk = 1 << 20
        let dst = UnsafeMutablePointer<UInt8>.allocate(capacity: chunk)
        defer { dst.deallocate() }
        let streamPtr = UnsafeMutablePointer<compression_stream>.allocate(capacity: 1)
        defer { streamPtr.deallocate() }
        guard compression_stream_init(streamPtr, COMPRESSION_STREAM_DECODE, COMPRESSION_ZLIB) == COMPRESSION_STATUS_OK else {
            throw Failure(message: "Couldn't start unpacking.")
        }
        defer { compression_stream_destroy(streamPtr) }

        var done = false
        var wrote = false
        while !done {
            let data = try input.read(upToCount: chunk) ?? Data()
            let last = data.isEmpty
            try data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
                streamPtr.pointee.src_ptr = raw.bindMemory(to: UInt8.self).baseAddress ?? UnsafePointer(dst)
                streamPtr.pointee.src_size = data.count
                repeat {
                    streamPtr.pointee.dst_ptr = dst
                    streamPtr.pointee.dst_size = chunk
                    let status = compression_stream_process(streamPtr, last ? Int32(COMPRESSION_STREAM_FINALIZE.rawValue) : 0)
                    let produced = chunk - streamPtr.pointee.dst_size
                    if produced > 0 {
                        output.write(Data(bytes: dst, count: produced))
                        wrote = true
                    }
                    switch status {
                    case COMPRESSION_STATUS_END:
                        // The DEFLATE body is over; what follows is the gzip trailer (CRC, size).
                        done = true
                        return
                    case COMPRESSION_STATUS_OK:
                        break
                    default:
                        throw Failure(message: "The compressed file is damaged or cut short. Try downloading it again.")
                    }
                } while streamPtr.pointee.src_size > 0 || streamPtr.pointee.dst_size == 0
            }
            if last && !done {
                // The data ran out before the compressed stream said it was finished: a download
                // that stopped part-way (NASA's server does this on slow connections).
                throw Failure(message: wrote ? "The compressed file is cut short. Try downloading it again." : "The compressed file is empty or cut short. Try downloading it again.")
            }
        }
    }

    /// RFC 1952: 10 fixed bytes, then optional extra field, file name, comment and header CRC.
    private static func headerLength(_ d: Data) throws -> Int {
        let b = [UInt8](d)
        guard b.count >= 10, b[0] == 0x1f, b[1] == 0x8b, b[2] == 8 else {
            throw Failure(message: "This isn't a gzip file.")
        }
        let flags = b[3]
        var i = 10
        if flags & 0x04 != 0 {
            guard b.count >= i + 2 else { throw Failure(message: "The compressed file is damaged.") }
            i += 2 + (Int(b[i]) | (Int(b[i + 1]) << 8))
        }
        for bit in [UInt8(0x08), 0x10] where flags & bit != 0 {
            while i < b.count && b[i] != 0 { i += 1 }
            i += 1
        }
        if flags & 0x02 != 0 { i += 2 }
        guard i <= b.count else { throw Failure(message: "The compressed file is damaged.") }
        return i
    }
}
