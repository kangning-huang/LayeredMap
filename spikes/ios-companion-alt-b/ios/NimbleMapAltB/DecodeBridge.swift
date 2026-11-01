import Foundation

// Shared by Alt A and Alt B. Thin Swift wrapper around the spike C ABI (`include/lm_decode.h`).
enum DecodeBridge {
    struct Envelope {
        let name: String
        let geometry: String
        let skipped: Int
        let messages: [String]
        let geojson: Any
        let count: Int
    }

    struct DecodeError: Error, LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    /// Open any file `rustgis-data` reads by extension. Call off the main thread.
    static func open(path: String) -> Result<Envelope, DecodeError> {
        path.withCString { cPath in
            var out: UnsafeMutablePointer<CChar>?
            var count: Int = 0
            let status = lm_open_path(cPath, &out, &count)
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
                    count: count
                )
            )
        }
    }
}
