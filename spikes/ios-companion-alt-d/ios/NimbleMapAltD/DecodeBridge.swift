import CoreLocation
import Foundation

// From the A/B shared file (Alt C copy). Thin Swift wrapper around the spike C ABI (`include/lm_decode.h`).
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
    }

    struct DecodeError: Error, LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    /// Open any file `rustgis-data` reads by extension. Call off the main thread.
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
