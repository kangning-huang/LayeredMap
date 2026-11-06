import Foundation
import CoreLocation

/// Alt C: map images and grids of values (GeoTIFF, ASCII grid). Rust reads and reprojects them and
/// writes a picture on a Web Mercator grid (MapLibre stretches an image source linearly), plus the
/// values on the same grid so a tap can read "Height here: 112 m".
struct RasterInfo {
    enum Look: String {
        /// A scanned map or photo: shown as it is.
        case picture
        /// Measurements (height, rainfall…): colored on the Standard ramp.
        case values
    }

    let look: Look
    let png: URL
    let values: URL?
    let gridWidth: Int
    let gridHeight: Int
    let bounds: GeoBounds
    /// The 2nd and 98th percentiles: the ends of the color ramp.
    let low: Double
    let high: Double
    let min: Double
    let max: Double
    let cellMeters: Double
    let bandCount: Int

    /// What the numbers are, guessed from the name: "Height" in metres for an elevation grid.
    let quantity: String
    let unit: String

    /// A NetCDF time axis in words ("July 2023"), one per step; empty when the file has no time
    /// axis, which is what hides the slider.
    var times: [String] = []
    /// Which step is drawn, into `times`.
    var time: Int = 0
    /// The file this came from, so another step can be read from it.
    var source: URL?
    /// The ramp this picture was drawn with. A raster is a picture, not styled features, so
    /// changing its colours means drawing it again — this is what to draw it with next time.
    var ramp: Palette = .viridis
    /// The part of the world this picture covers. Equal to `bounds` for a whole-file drawing;
    /// smaller once it has been redrawn for a zoomed-in view.
    var window: GeoBounds?

    var hasTime: Bool { times.count > 1 }

    var timeLabel: String { times.indices.contains(time) ? times[time] : "" }

    /// The value under a point, read from the grid (nil outside or on no-data).
    func value(at c: CLLocationCoordinate2D) -> Double? {
        guard let values, c.longitude >= bounds.minLon, c.longitude <= bounds.maxLon,
              c.latitude >= bounds.minLat, c.latitude <= bounds.maxLat else { return nil }
        func y(_ lat: Double) -> Double { log(tan(.pi / 4 + lat * .pi / 360)) }
        let fx = (c.longitude - bounds.minLon) / (bounds.maxLon - bounds.minLon)
        let fy = (y(bounds.maxLat) - y(c.latitude)) / (y(bounds.maxLat) - y(bounds.minLat))
        let col = Swift.min(gridWidth - 1, Swift.max(0, Int(fx * Double(gridWidth))))
        let row = Swift.min(gridHeight - 1, Swift.max(0, Int(fy * Double(gridHeight))))
        guard let handle = try? FileHandle(forReadingFrom: values) else { return nil }
        defer { try? handle.close() }
        try? handle.seek(toOffset: UInt64((row * gridWidth + col) * 4))
        guard let data = try? handle.read(upToCount: 4), data.count == 4 else { return nil }
        let bits = data.withUnsafeBytes { $0.loadUnaligned(as: UInt32.self) }
        let v = Double(Float(bitPattern: UInt32(littleEndian: bits)))
        return v.isFinite ? v : nil
    }

    /// The share of the grid's values below `v` (0…1), from the grid itself.
    func share(below v: Double) -> Double? {
        guard let values, let data = try? Data(contentsOf: values) else { return nil }
        var below = 0
        var total = 0
        data.withUnsafeBytes { raw in
            for i in stride(from: 0, to: raw.count - 3, by: 4) {
                let x = Double(Float(bitPattern: UInt32(littleEndian: raw.loadUnaligned(fromByteOffset: i, as: UInt32.self))))
                guard x.isFinite else { continue }
                total += 1
                if x < v { below += 1 }
            }
        }
        return total > 0 ? Double(below) / Double(total) : nil
    }

    func format(_ v: Double) -> String {
        let n = abs(v) >= 100 ? String(Int(v.rounded())) : NumberWords.plain(v)
        return unit.isEmpty ? n : "\(n) \(unit)"
    }

    /// CF unit strings are written for machines ("degC", "degrees_Celsius", "m s-1"). Show the
    /// common ones the way a person writes them, and leave anything else exactly as the file said
    /// rather than mangling it.
    static func readableUnit(_ raw: String) -> String {
        switch raw.lowercased().replacingOccurrences(of: " ", with: "_") {
        case "degc", "degrees_celsius", "degree_celsius", "celsius": return "°C"
        case "degk", "k", "kelvin", "degrees_kelvin": return "K"
        case "degf", "degrees_fahrenheit", "fahrenheit": return "°F"
        case "m", "metre", "meter", "metres", "meters": return "m"
        case "km", "kilometre", "kilometer", "kilometres", "kilometers": return "km"
        case "mm": return "mm"
        case "percent", "%": return "%"
        case "1", "none", "dimensionless", "unitless": return ""
        default: return raw
        }
    }

    static func guessQuantity(_ name: String) -> (String, String) {
        let n = name.lowercased()
        if ["elev", "height", "dem", "dtm", "altitude", "terrain", "altimetr"].contains(where: n.contains) { return ("Height", "m") }
        if ["temp"].contains(where: n.contains) { return ("Temperature", "°C") }
        if ["rain", "precip"].contains(where: n.contains) { return ("Rainfall", "mm") }
        if ["slope"].contains(where: n.contains) { return ("Slope", "°") }
        return ("Value", "")
    }
}

enum RasterDecode {
    // NetCDF climate and population grids (`.nc`) are read by the spike's own pure-Rust reader;
    // they arrive here as a grid of values like any GeoTIFF.
    static let extensions = ["tif", "tiff", "asc", "nc", "nc4", "cdf", "netcdf"]

    static func isRaster(_ url: URL) -> Bool { extensions.contains(url.pathExtension.lowercased()) }

    /// Read a raster file (call off the main thread) into a dataset with no features.
    ///
    /// `time` picks the step of a NetCDF time axis; `range` is the colour range to keep, so
    /// stepping through time does not restretch the ramp and make every month look the same.
    static func open(
        _ file: URL,
        name: String,
        sourceName: String,
        time: Int = 0,
        range: (Double, Double)? = nil,
        ramp: Palette = .viridis,
        window: GeoBounds? = nil
    ) -> Result<GeoDataset, ImportError> {
        let cache = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first ?? FileManager.default.temporaryDirectory
        let stem = cache.appendingPathComponent("raster-\(UUID().uuidString)").path
        // A window of (0,0,0,0) means "all of it": Rust treats a zero-size window as absent.
        let w = window ?? GeoBounds(minLon: 0, minLat: 0, maxLon: 0, maxLat: 0)
        let result: Result<[String: Any], DecodeBridge.DecodeError> = file.path.withCString { cPath in
            stem.withCString { cStem in
                ramp.rawValue.withCString { cRamp in
                var out: UnsafeMutablePointer<CChar>?
                let status = lm_open_raster_step(
                    cPath, cStem, Int32(time), range?.0 ?? 1, range?.1 ?? 0,
                    cRamp, w.minLon, w.minLat, w.maxLon, w.maxLat, &out
                )
                defer { if let out { lm_string_free(out) } }
                guard let out else { return .failure(DecodeBridge.DecodeError(message: "decode status \(status)")) }
                let text = String(cString: out)
                guard status == 0 else { return .failure(DecodeBridge.DecodeError(message: text)) }
                guard let obj = (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any] else {
                    return .failure(DecodeBridge.DecodeError(message: "unreadable decoder output"))
                }
                return .success(obj)
                }
            }
        }
        switch result {
        case .failure(let e):
            return .failure(ImportError(message: "Couldn't open this map image: \(Importer.plain(e.message))"))
        case .success(let o):
            let b = (o["bounds"] as? [NSNumber])?.map(\.doubleValue) ?? []
            let grid = (o["grid"] as? [NSNumber])?.map(\.intValue) ?? []
            guard b.count == 4, grid.count == 2, let png = o["png"] as? String else {
                return .failure(ImportError(message: "Couldn't open this map image."))
            }
            let look = RasterInfo.Look(rawValue: o["look"] as? String ?? "") ?? .picture
            // A NetCDF file states what it measures and in what unit (CF `long_name`/`units`), so
            // use those; a GeoTIFF says nothing, so fall back to guessing from the file name.
            let said = (o["quantity"] as? String).map { $0.trimmingCharacters(in: .whitespaces) } ?? ""
            let saidUnit = (o["unit"] as? String).map { $0.trimmingCharacters(in: .whitespaces) } ?? ""
            let guessed = RasterInfo.guessQuantity(name)
            let quantity = said.isEmpty ? guessed.0 : said
            let unit = said.isEmpty ? guessed.1 : RasterInfo.readableUnit(saidUnit)
            func num(_ k: String) -> Double { (o[k] as? NSNumber)?.doubleValue ?? 0 }
            // Pulled out of the initializer: with the time-axis fields added, type-checking the
            // whole call in one expression takes the compiler an unreasonable amount of time.
            let pngURL = URL(fileURLWithPath: png)
            let valuesURL: URL? = (o["values"] as? String).map { URL(fileURLWithPath: $0) }
            let bounds = GeoBounds(minLon: b[0], minLat: b[1], maxLon: b[2], maxLat: b[3])
            let bands: Int = (o["bands"] as? NSNumber)?.intValue ?? 1
            let stepNames: [String] = (o["times"] as? [String]) ?? []
            let stepIndex: Int = (o["time"] as? NSNumber)?.intValue ?? 0
            let info = RasterInfo(
                look: look,
                png: pngURL,
                values: valuesURL,
                gridWidth: grid[0],
                gridHeight: grid[1],
                bounds: bounds,
                low: num("low"),
                high: num("high"),
                min: num("min"),
                max: num("max"),
                cellMeters: num("cell_m"),
                bandCount: bands,
                quantity: quantity,
                unit: unit,
                times: stepNames,
                time: stepIndex,
                source: file,
                ramp: ramp,
                window: window
            )
            var assumptions: [String] = []
            if look == .values {
                assumptions.append("Colors stretch from \(info.format(info.low)) to \(info.format(info.high)); the few values beyond take the end colors.")
                if unit.isEmpty {
                    assumptions.append("The file doesn't say what its numbers measure.")
                } else if said.isEmpty {
                    // The unit came from the file name, not the file. Saying so matters: a GeoTIFF
                    // called sst_template.tif was reporting "287 °C" with no hedge at all.
                    assumptions.append("This file doesn't state its units — “\(quantity.lowercased()), \(unit)” is a guess from its name.")
                }
            }
            // What the NetCDF reader had to assume: a wrapped longitude range, which grid of
            // several was shown. Plain sentences from Rust, shown under More details.
            assumptions.append(contentsOf: (o["notes"] as? [String]) ?? [])
            let ds = GeoDataset(
                name: name,
                kind: .raster,
                features: [],
                fields: [],
                bounds: info.bounds,
                assumptions: assumptions,
                primaryField: nil,
                titleField: nil,
                sourceName: sourceName,
                credit: nil
            )
            ds.raster = info
            return .success(ds)
        }
    }
}
