import CoreLocation
import Foundation
import os

// From the A/B shared file (Alt C copy). Thin Swift wrapper around the spike C ABI (`include/lm_decode.h`).
/// Whether there is room to take in what the reader produced, asked of iOS at the moment of opening
/// (`os_proc_available_memory`): it knows this phone and what is already on the map. A simulator
/// has the Mac's memory and says yes to anything; a phone ended the app instead — mid-parse
/// (the tornado tracks on top of earlier layers) or in MapLibre's worker (154,400 storks).
enum MemoryRoom {
    /// What the app holds once a layer is on the map, measured on an iPhone 18 Pro (2026-11-06):
    /// a small record per feature and its cells as typed columns (Geofabrik's Taiwan, 920,000
    /// features, about 400 MB), plus what MapLibre takes for a line or an area (Taiwan's 530,000
    /// took it about 1.5 GB more at the first draw). Points cost MapLibre only the tiles on
    /// screen (`PointTiles`).
    static let perFeature = 500.0
    static let perDrawnShape = 3_000.0
    static let perCell = 12.0

    /// What the reader itself needs at its peak, from the file's size, before it starts: it holds a
    /// table whole while it reads it (NYC's street trees, a 210 MB CSV: 1.85 GB on the phone; the
    /// white storks, 51 MB: 578 MB). A zip is read a member at a time, so it isn't judged by its
    /// size here; its layers are checked once read.
    static let readerPerByte = 10.0

    static func readerRefusal(path: String) -> String? {
        let available = Double(os_proc_available_memory())
        guard available > 0 else { return nil }
        let ext = (path as NSString).pathExtension.lowercased()
        guard ["csv", "tsv", "txt", "geojson", "json", "kml", "gpx"].contains(ext),
              let size = (try? FileManager.default.attributesOfItem(atPath: path)[.size] as? NSNumber)?.doubleValue
        else { return nil }
        let need = size * readerPerByte
        guard need > available * 0.85 else { return nil }
        let mb = { (b: Double) in Int(b / 1_048_576) }
        return "This file is too big to read on this iPhone right now: reading it takes about \(mb(need)) MB and there is room for \(mb(available * 0.85)) MB. Remove a layer or two and try again, open it on the Mac, or download a smaller part (most data portals let you filter by area or date)."
    }

    /// `shapes`: the lines and areas among `features`.
    static func refusal(features: Int, shapes: Int, cells: Int) -> String? {
        let available = Double(os_proc_available_memory())
        // 0 when unknown (a simulator): fall back on the reader's own size limits.
        guard available > 0 else { return nil }
        let need = Double(features) * perFeature + Double(shapes) * perDrawnShape + Double(cells) * perCell
        guard need > available * 0.7 else { return nil }
        let mb = { (b: Double) in Int(b / 1_048_576) }
        return "This file is too big to open on this iPhone right now: it needs about \(mb(need)) MB and there is room for \(mb(available * 0.7)) MB. Remove a layer or two and try again, open it on the Mac, or download a smaller part (most data portals let you filter by area or date)."
    }
}

enum DecodeBridge {
    struct Envelope {
        let name: String
        let geometry: String
        let skipped: Int
        let messages: [String]
        let geojson: Any
        let count: Int
        /// How data that wasn't in longitude/latitude was placed (see `placement.rs`): "how" is
        /// prj, guess, chosen or unplaced; nil when it already was.
        let placement: [String: Any]?
        /// The layer's path inside a .zip; nil for a file of its own.
        var member: String? = nil
        /// The layer as files (map, idx, geo, cols) when it was written to disk (`openAll`).
        var files: [String: String] = [:]
        var fields: Int = 0
    }

    struct DecodeError: Error, LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    /// Open any file `rustgis-data` reads by extension. Call off the main thread.
    /// Every layer of a file: one per layer of a .zip, one for anything else (`lm_open_path_all`).
    /// Each layer is written into `outDir` as files (see `store.rs`); the envelopes name them.
    static func openAll(path: String, outDir: URL, near: CLLocationCoordinate2D? = nil, epsg: Int? = nil) -> Result<[Envelope], DecodeError> {
        try? FileManager.default.createDirectory(at: outDir, withIntermediateDirectories: true)
        if let refusal = MemoryRoom.readerRefusal(path: path) {
            return .failure(DecodeError(message: refusal))
        }
        return path.withCString { cPath in outDir.path.withCString { cDir in
            var out: UnsafeMutablePointer<CChar>?
            var count: Int = 0
            let status = lm_open_path_all(
                cPath, near == nil ? 0 : 1, near?.longitude ?? 0, near?.latitude ?? 0,
                UInt32(clamping: epsg ?? 0), cDir, &out, &count
            )
            defer {
                if let out { lm_string_free(out) }
            }
            guard let out else { return .failure(DecodeError(message: "decode status \(status)")) }
            let data = Data(bytesNoCopy: out, count: strlen(out), deallocator: .none)
            guard status == 0 else {
                return .failure(DecodeError(message: String(data: data, encoding: .utf8) ?? "decode status \(status)"))
            }
            guard let list = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else {
                return .failure(DecodeError(message: "unreadable decoder output"))
            }
            let cells = list.reduce(0) { $0 + (($1["count"] as? NSNumber)?.intValue ?? 0) * max(1, ($1["fields"] as? NSNumber)?.intValue ?? 1) }
            let shapes = list.reduce(0) { n, o in
                let g = o["geometry"] as? String
                return n + (g == "line" || g == "area" ? (o["count"] as? NSNumber)?.intValue ?? 0 : 0)
            }
            if let refusal = MemoryRoom.refusal(features: count, shapes: shapes, cells: cells) {
                return .failure(DecodeError(message: refusal))
            }
            return .success(list.map { obj in
                let geojson = obj["geojson"] ?? [:]
                let n = (obj["count"] as? NSNumber)?.intValue ?? (((geojson as? [String: Any])?["features"] as? [Any])?.count ?? 0)
                return Envelope(
                    name: obj["name"] as? String ?? "",
                    geometry: obj["geometry"] as? String ?? "table",
                    skipped: (obj["skipped"] as? NSNumber)?.intValue ?? 0,
                    messages: obj["messages"] as? [String] ?? [],
                    geojson: geojson,
                    count: n,
                    placement: obj["placement"] as? [String: Any],
                    member: obj["member"] as? String,
                    files: obj["files"] as? [String: String] ?? [:],
                    fields: (obj["fields"] as? NSNumber)?.intValue ?? 0
                )
            })
        } }
    }

    /// `near`: where to guess a grid for data in metres (the map, or the person); `epsg`: a grid
    /// the person chose.
    static func open(path: String, near: CLLocationCoordinate2D? = nil, epsg: Int? = nil) -> Result<Envelope, DecodeError> {
        path.withCString { cPath in
            var out: UnsafeMutablePointer<CChar>?
            var count: Int = 0
            let status = lm_open_path_at(
                cPath, near == nil ? 0 : 1, near?.longitude ?? 0, near?.latitude ?? 0,
                UInt32(clamping: epsg ?? 0), &out, &count
            )
            defer {
                if let out {
                    lm_string_free(out)
                }
            }
            guard let out else { return .failure(DecodeError(message: "decode status \(status)")) }
            let len = strlen(out)
            let data = Data(bytesNoCopy: out, count: len, deallocator: .none)
            guard status == 0 else {
                return .failure(DecodeError(message: String(data: data, encoding: .utf8) ?? "decode status \(status)"))
            }
            guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                return .failure(DecodeError(message: "unreadable decoder output"))
            }
            return .success(
                Envelope(
                    name: obj["name"] as? String ?? "",
                    geometry: obj["geometry"] as? String ?? "table",
                    skipped: (obj["skipped"] as? NSNumber)?.intValue ?? 0,
                    messages: obj["messages"] as? [String] ?? [],
                    geojson: obj["geojson"] ?? [:],
                    count: count,
                    placement: obj["placement"] as? [String: Any]
                )
            )
        }
    }
}
