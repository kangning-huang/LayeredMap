import CoreLocation
import Foundation
import os

// From the A/B shared file (Alt C copy). Thin Swift wrapper around the spike C ABI (`include/lm_decode.h`).
/// Whether there is room to take in what the reader produced, asked of iOS at the moment of opening
/// (`os_proc_available_memory`): it knows this phone and what is already on the map. A simulator
/// has the Mac's memory and says yes to anything; a phone ended the app instead — mid-parse
/// (the tornado tracks on top of earlier layers) or in MapLibre's worker (154,400 storks).
enum MemoryRoom {
    /// What the app holds for each feature and each attribute cell once a layer is on the map,
    /// now that layers arrive as files (`store.rs`): a small record per feature, its cells as
    /// typed columns, and MapLibre's own index of the shapes. First estimates, from the phone.
    static let perFeature = 2_500.0
    static let perCell = 10.0

    static func refusal(features: Int, cells: Int) -> String? {
        let available = Double(os_proc_available_memory())
        // 0 when unknown (a simulator): fall back on the reader's own size limits.
        guard available > 0 else { return nil }
        let need = Double(features) * perFeature + Double(cells) * perCell
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
            if let refusal = MemoryRoom.refusal(features: count, cells: cells) {
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
