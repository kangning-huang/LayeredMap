import Foundation

/// Thin Swift wrapper around the spike C ABI (`include/lm_decode.h`).
enum DecodeBridge {
    struct Result {
        let json: String
        let featureCount: Int
    }

    enum DecodeError: Error, LocalizedError {
        case nullArg
        case empty
        case invalidUTF8
        case parse(String)
        case io(String)
        case unknown(Int32)

        var errorDescription: String? {
            switch self {
            case .nullArg: return "null argument"
            case .empty: return "empty input"
            case .invalidUTF8: return "invalid UTF-8"
            case .parse(let m): return m
            case .io(let m): return m
            case .unknown(let c): return "decode status \(c)"
            }
        }
    }

    static func decodeGeoJSONFile(at path: String) -> Swift.Result<Result, DecodeError> {
        path.withCString { cPath in
            var out: UnsafeMutablePointer<CChar>?
            var count: Int = 0
            let status = lm_decode_geojson_path(cPath, &out, &count)
            defer {
                if let out {
                    lm_string_free(out)
                }
            }
            let message = out.map { String(cString: $0) } ?? ""
            switch status {
            case 0:
                return .success(Result(json: message, featureCount: count))
            case 1:
                return .failure(.empty)
            case 2:
                return .failure(.invalidUTF8)
            case 3:
                return .failure(.parse(message.isEmpty ? "parse error" : message))
            case 4:
                return .failure(.io(message.isEmpty ? "io error" : message))
            case 5:
                return .failure(.nullArg)
            default:
                return .failure(.unknown(status))
            }
        }
    }
}
