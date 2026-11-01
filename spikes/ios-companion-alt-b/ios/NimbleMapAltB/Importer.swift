import Foundation
import UniformTypeIdentifiers

// Shared by Alt A and Alt B: every way data gets in ends here. Copy into the app, decode in Rust
// off the main thread, plain-language errors.

struct ImportError: Error {
    let message: String
}

enum Importer {
    /// Phone limit for one file (redesign › File check: "the size is under the phone limit").
    static let maxBytes: Int64 = 60 * 1024 * 1024

    static let extensions = ["geojson", "json", "csv", "tsv", "txt", "kml", "kmz", "gpx", "zip", "gpkg"]

    static var contentTypes: [UTType] {
        var types: [UTType] = [.json, .commaSeparatedText, .tabSeparatedText, .zip, .plainText]
        for ext in ["geojson", "kml", "kmz", "gpx", "gpkg"] {
            if let t = UTType(filenameExtension: ext) { types.append(t) }
        }
        return types
    }

    /// The app's copy of everything added (copy on open; works offline; Recents read from here).
    static var dataFolder: URL {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        let dir = docs.appendingPathComponent("Data", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    struct Recent: Identifiable {
        let url: URL
        let name: String
        let date: Date
        var id: URL { url }
    }

    static func recents(limit: Int = 6) -> [Recent] {
        let keys: [URLResourceKey] = [.contentModificationDateKey]
        let files = (try? FileManager.default.contentsOfDirectory(at: dataFolder, includingPropertiesForKeys: keys)) ?? []
        return files
            .filter { extensions.contains($0.pathExtension.lowercased()) }
            .map { url in
                let date = (try? url.resourceValues(forKeys: Set(keys)).contentModificationDate) ?? .distantPast
                return Recent(url: url, name: displayName(for: url), date: date)
            }
            .sorted { $0.date > $1.date }
            .prefix(limit)
            .map { $0 }
    }

    static func displayName(for url: URL) -> String {
        let stem = url.deletingPathExtension().lastPathComponent
        let tidy = stem.replacingOccurrences(of: "_", with: " ").replacingOccurrences(of: "-", with: " ")
        return tidy.prefix(1).uppercased() + tidy.dropFirst()
    }

    // MARK: Opening

    /// A file picked in Files or sent from another app (Open In / Share). Copies it in first.
    static func openExternal(_ url: URL) async -> Result<GeoDataset, ImportError> {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let ext = url.pathExtension.lowercased()
        if ext == "shp" || ext == "dbf" || ext == "shx" {
            return .failure(ImportError(message: "This map file is missing its partner files. Ask the sender to share the .zip."))
        }
        guard extensions.contains(ext) else {
            return .failure(ImportError(message: "NimbleMap can't open .\(ext) files yet."))
        }
        let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init) ?? 0
        if size > maxBytes {
            return .failure(ImportError(message: "This file is too big for the phone (\(ByteCountFormatter.string(fromByteCount: size, countStyle: .file)))."))
        }
        let dest = uniqueDestination(for: url.lastPathComponent)
        do {
            try FileManager.default.copyItem(at: url, to: dest)
        } catch {
            return .failure(ImportError(message: "Couldn't read this file: \(error.localizedDescription)"))
        }
        return await decode(dest, name: displayName(for: url), primary: nil, sourceName: "Your file", credit: nil)
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
        return await decode(url, name: item.title, primary: item.primaryField, sourceName: item.source, credit: item.credit)
    }

    static func reopen(_ recent: Recent) async -> Result<GeoDataset, ImportError> {
        await decode(recent.url, name: recent.name, primary: nil, sourceName: "Your file", credit: nil)
    }

    // MARK: Decode

    static func decode(_ file: URL, name: String, primary: String?, sourceName: String, credit: String?) async -> Result<GeoDataset, ImportError> {
        await Task.detached(priority: .userInitiated) { () -> Result<GeoDataset, ImportError> in
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
        if m.contains("shp on its own") { return "This map file is missing its partner files. Ask the sender to share the .zip." }
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
            candidate = dataFolder.appendingPathComponent("\(base) \(n).\(ext)")
            n += 1
        }
        return candidate
    }
}
