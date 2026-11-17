import Foundation
import MapLibre

/// A big point layer, handed to MapLibre one tile at a time: the dots themselves where a tile holds
/// few enough to draw, and a density grid where it doesn't.
///
/// Given all 154,400 of the white storks as one GeoJSON source, MapLibre on an iPhone used about
/// 2.5 GB, nearly everything iOS allows an app, and the app was ended as soon as it was coloured.
/// Here MapLibre is never given more than [`dotsPerTile`] points or [`cells`]² squares per tile,
/// so what it holds follows what is on screen, not the size of the file. Zoomed out, a tile of
/// Western Europe is a grid of squares shaded by how many places each holds; zoomed in, each tile
/// is real dots, which can be tapped and coloured by a field like any other.
final class PointTiles: NSObject, MLNComputedShapeSourceDataSource {
    /// Layers with more points than this are drawn tile by tile.
    static let threshold = 50_000
    /// The most dots in one tile; a tile with more is drawn as density.
    static let dotsPerTile = 4_000
    /// Density squares per tile side (a 512-point tile: 8-point squares).
    static let cells = 64
    /// The index's grid: 2^9 × 2^9 Web Mercator cells, each a list of its points.
    private static let gridZoom = 9

    /// The attribute holding a density square's count.
    static let countKey = "_lm_n"

    private let mx: [Float]
    private let my: [Float]
    /// Row number of each indexed point (rows without a place are left out).
    private let rows: [Int32]
    /// Grid cell `c` holds `order[start[c] ..< start[c + 1]]` (indexes into `mx`/`my`/`rows`).
    private let start: [Int32]
    private let order: [Int32]
    /// The colour field's value per row, when coloured by one.
    private let field: String?
    private let values: [Any?]

    /// Tiles answered and the most features in one, for the memory log. MapLibre asks from its
    /// own queue, so behind a lock.
    private let lock = NSLock()
    private var served = 0
    private var most = 0
    var stats: (tiles: Int, most: Int) {
        lock.lock()
        defer { lock.unlock() }
        return (served, most)
    }

    init(dataset: GeoDataset, field: String?) {
        var mx: [Float] = [], my: [Float] = [], rows: [Int32] = []
        mx.reserveCapacity(dataset.features.count)
        my.reserveCapacity(dataset.features.count)
        rows.reserveCapacity(dataset.features.count)
        for f in dataset.features where f.bounds.isValid {
            let p = Self.mercator(f.center)
            mx.append(p.x)
            my.append(p.y)
            rows.append(Int32(f.index))
        }
        // Counting sort into the grid.
        let n = 1 << Self.gridZoom
        var count = [Int32](repeating: 0, count: n * n + 1)
        var cellOf = [Int32](repeating: 0, count: mx.count)
        for i in 0..<mx.count {
            let c = Self.cell(mx[i], n) + Self.cell(my[i], n) * n
            cellOf[i] = Int32(c)
            count[c + 1] += 1
        }
        for c in 0..<(n * n) { count[c + 1] += count[c] }
        var fill = count
        var order = [Int32](repeating: 0, count: mx.count)
        for i in 0..<mx.count {
            let c = Int(cellOf[i])
            order[Int(fill[c])] = Int32(i)
            fill[c] += 1
        }
        self.mx = mx
        self.my = my
        self.rows = rows
        self.start = count
        self.order = order
        self.field = field
        if let field {
            values = dataset.features.map { $0.properties[field] }
        } else {
            values = []
        }
        super.init()
    }

    // MARK: Web Mercator, 0…1 from the west and the north

    private static func mercator(_ c: CLLocationCoordinate2D) -> (x: Float, y: Float) {
        let lat = max(-85.051_128, min(85.051_128, c.latitude)) * .pi / 180
        let x = (c.longitude + 180) / 360
        let y = (1 - log(tan(lat) + 1 / cos(lat)) / .pi) / 2
        return (Float(min(max(x, 0), 1)), Float(min(max(y, 0), 1)))
    }

    private static func coordinate(_ x: Double, _ y: Double) -> CLLocationCoordinate2D {
        let lon = x * 360 - 180
        let lat = atan(sinh(.pi * (1 - 2 * y))) * 180 / .pi
        return CLLocationCoordinate2D(latitude: lat, longitude: lon)
    }

    private static func cell(_ v: Float, _ n: Int) -> Int {
        min(n - 1, max(0, Int(v * Float(n))))
    }

    // MARK: Tiles

    func featuresInTileAt(x: UInt, y: UInt, zoomLevel: UInt) -> [MLNShape & MLNFeature] {
        let z = Int(min(zoomLevel, 24))
        let scale = Double(1 << z)
        let x0 = Double(x) / scale, y0 = Double(y) / scale
        let x1 = x0 + 1 / scale, y1 = y0 + 1 / scale
        let points = pointsIn(x0, y0, x1, y1, z: z, x: Int(x), y: Int(y))

        let out: [MLNShape & MLNFeature]
        if points.count <= Self.dotsPerTile {
            out = points.map { i in
                let p = MLNPointFeature()
                p.coordinate = Self.coordinate(Double(mx[i]), Double(my[i]))
                let row = Int(rows[i])
                var attrs: [String: Any] = ["_lm_i": row]
                if let field, row < values.count, let v = values[row] { attrs[field] = v }
                p.attributes = attrs
                return p
            }
        } else {
            out = density(points, x0, y0, scale)
        }
        lock.lock()
        served += 1
        most = max(most, out.count)
        lock.unlock()
        return out
    }

    /// Indexes of the points inside a tile, from the grid cells it covers.
    private func pointsIn(_ x0: Double, _ y0: Double, _ x1: Double, _ y1: Double, z: Int, x: Int, y: Int) -> [Int] {
        let gz = Self.gridZoom
        let n = 1 << gz
        var out: [Int] = []
        if z <= gz {
            // The tile covers whole grid cells: take them all, row by row.
            let span = 1 << (gz - z)
            let cx0 = x * span, cy0 = y * span
            for cy in cy0..<min(n, cy0 + span) {
                let a = Int(start[cy * n + cx0]), b = Int(start[cy * n + min(n, cx0 + span) - 1 + 1])
                guard b > a else { continue }
                out.reserveCapacity(out.count + (b - a))
                for k in a..<b { out.append(Int(order[k])) }
            }
        } else {
            // Inside one grid cell: keep its points that fall in this tile.
            let shift = z - gz
            let c = (x >> shift) + (y >> shift) * n
            guard c + 1 < start.count else { return [] }
            let fx0 = Float(x0), fy0 = Float(y0), fx1 = Float(x1), fy1 = Float(y1)
            for k in Int(start[c])..<Int(start[c + 1]) {
                let i = Int(order[k])
                if mx[i] >= fx0 && mx[i] < fx1 && my[i] >= fy0 && my[i] < fy1 { out.append(i) }
            }
        }
        return out
    }

    /// Squares on a `cells` × `cells` grid over the tile, each with how many points it holds.
    private func density(_ points: [Int], _ x0: Double, _ y0: Double, _ scale: Double) -> [MLNShape & MLNFeature] {
        let k = Self.cells
        var counts = [Int32](repeating: 0, count: k * k)
        let s = Double(k) * scale
        for i in points {
            let cx = min(k - 1, max(0, Int((Double(mx[i]) - x0) * s)))
            let cy = min(k - 1, max(0, Int((Double(my[i]) - y0) * s)))
            counts[cx + cy * k] += 1
        }
        var out: [MLNShape & MLNFeature] = []
        let step = 1 / s
        for cy in 0..<k {
            for cx in 0..<k where counts[cx + cy * k] > 0 {
                let ax = x0 + Double(cx) * step, ay = y0 + Double(cy) * step
                var ring = [
                    Self.coordinate(ax, ay), Self.coordinate(ax + step, ay),
                    Self.coordinate(ax + step, ay + step), Self.coordinate(ax, ay + step),
                    Self.coordinate(ax, ay),
                ]
                let sq = MLNPolygonFeature(coordinates: &ring, count: UInt(ring.count))
                sq.attributes = [Self.countKey: Int(counts[cx + cy * k])]
                out.append(sq)
            }
        }
        return out
    }
}
