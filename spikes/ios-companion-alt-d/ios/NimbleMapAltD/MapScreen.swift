import SwiftUI
import MapLibre
import UIKit

// From the A/B shared file (Alt C copy). Native MapLibre (Metal). Gestures never wait on decode: sources load
// from files, which MapLibre parses off the main thread.

struct MapHit: Hashable {
    let layerID: UUID
    let featureIndex: Int
}

enum Basemap: String, CaseIterable, Identifiable {
    case light
    case streets

    var id: String { rawValue }

    var label: String {
        switch self {
        case .light: return "Light"
        case .streets: return "Streets"
        }
    }

    /// OpenFreeMap (OpenStreetMap data, no key). Same basemap in both alternatives.
    var url: URL? {
        switch self {
        case .light: return URL(string: "https://tiles.openfreemap.org/styles/positron")
        case .streets: return URL(string: "https://tiles.openfreemap.org/styles/liberty")
        }
    }
}

struct MapScreen: UIViewRepresentable {
    var layers: [MapLayer]
    var camera: CameraRequest?
    var highlight: Data?
    var basemap: Basemap = .light
    /// Space the chrome covers, so "fit to data" frames the data in the visible part.
    var insets: UIEdgeInsets = UIEdgeInsets(top: 80, left: 40, bottom: 200, right: 40)
    /// Features under the tap, and where it was (Alt C reads map images' values there).
    var onTap: (([MapHit], CLLocationCoordinate2D) -> Void)?
    var onLongPress: ((CLLocationCoordinate2D, [MapHit]) -> Void)?
    /// A pan or pinch by the person (not a programmatic camera move).
    var onUserGesture: (() -> Void)?
    /// The visible area after the camera settles (Alt C saves it, so a relaunch returns there).
    var onCameraIdle: ((GeoBounds) -> Void)?
    var pin: CLLocationCoordinate2D?
    /// Which way the map is turned (degrees, 0 = north up), as it changes. MapLibre's own compass
    /// sat top right among the search bar and the system clock; the app draws its own at the
    /// bottom of the map buttons instead.
    var onHeading: ((Double) -> Void)?
    /// A new value turns the map back to north up.
    var northRequest: UUID?

    func makeUIView(context: Context) -> MLNMapView {
        let map = MLNMapView(frame: .zero)
        map.styleURL = basemap.url
        // ProMotion: ask for 120 (iOS clamps to the screen; .maximum is documented as "typically 60").
        // Needs CADisableMinimumFrameDurationOnPhone in Info.plist, or iPhone caps it at 60.
        map.preferredFramesPerSecond = MLNMapViewPreferredFramesPerSecond(rawValue: 120)
        // Never cover attribution: both alternatives put chrome at the bottom.
        map.logoViewPosition = .topLeft
        map.attributionButtonPosition = .topLeft
        map.logoViewMargins = CGPoint(x: 12, y: 6)
        map.attributionButtonMargins = CGPoint(x: 100, y: 6)
        map.compassView.compassVisibility = .hidden
        map.allowsRotating = true
        map.allowsTilting = false
        map.setCenter(CLLocationCoordinate2D(latitude: 25, longitude: 10), zoomLevel: 0.8, animated: false)
        map.delegate = context.coordinator

        let tap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleTap(_:)))
        for g in map.gestureRecognizers ?? [] {
            if let t = g as? UITapGestureRecognizer, t.numberOfTapsRequired == 2 {
                tap.require(toFail: t)
            }
        }
        map.addGestureRecognizer(tap)
        let long = UILongPressGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleLongPress(_:)))
        map.addGestureRecognizer(long)
        context.coordinator.parent = self
        context.coordinator.basemap = basemap
        return map
    }

    func updateUIView(_ map: MLNMapView, context: Context) {
        let c = context.coordinator
        c.parent = self
        if c.basemap != basemap {
            c.basemap = basemap
            c.styleReset()
            map.styleURL = basemap.url
        }
        c.sync(layers, on: map)
        c.setHighlight(highlight, on: map)
        c.setPin(pin, on: map)
        if let northRequest, northRequest != c.lastNorth {
            c.lastNorth = northRequest
            map.setDirection(0, animated: true)
        }
        if let camera, camera.id != c.lastCamera {
            c.lastCamera = camera.id
            c.apply(camera, on: map)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    final class Coordinator: NSObject, MLNMapViewDelegate {
        var parent: MapScreen?
        var basemap: Basemap = .light
        var lastCamera: UUID?
        var lastNorth: UUID?
        private var lastHeading = 0.0
        private var styleLoaded = false
        private var appliedOrder: [UUID] = []
        private var applied: [UUID: (LayerStyle, Bool)] = [:]
        private var fileURLs: [UUID: URL] = [:]
        /// The field each layer's source carries (see `GeoDataset.mapGeoJSON(showing:)`).
        private var sourceField: [UUID: String?] = [:]
        /// Big point layers, drawn tile by tile (`PointTiles`); kept so the source's data
        /// source lives as long as the source.
        private(set) var pointTiles: [UUID: PointTiles] = [:]

        static func drawsByTile(_ ds: GeoDataset) -> Bool {
            ds.kind == .point && ds.features.count > PointTiles.threshold
                && ProcessInfo.processInfo.environment["LM_TILES"] != "0"
        }
        private var lastHighlight: Data?
        private var pinAnnotation: MLNPointAnnotation?
        private var pendingCamera: CameraRequest?
        private let highlightSource = "lm-highlight"
        let bench = FrameBench()

        // MARK: Style

        func styleReset() {
            styleLoaded = false
            appliedOrder = []
            applied = [:]
            lastHighlight = nil
        }

        func mapView(_ mapView: MLNMapView, didFinishLoading style: MLNStyle) {
            styleLoaded = true
            appliedOrder = []
            applied = [:]
            lastHighlight = nil
            if let parent {
                sync(parent.layers, on: mapView)
                setHighlight(parent.highlight, on: mapView)
            }
            if let pendingCamera {
                self.pendingCamera = nil
                apply(pendingCamera, on: mapView)
            }
            // Spike-only: LM_TOUR="lat,lon,zoom;…" flies to each stop (from 8 s, then every 5 s)
            // and prints what the tile-by-tile point layers handed MapLibre, for screenshots.
            if let tour = ProcessInfo.processInfo.environment["LM_TOUR"] {
                let stops = tour.split(separator: ";").map { $0.split(separator: ",").compactMap { Double($0) } }.filter { $0.count == 3 }
                for (n, s) in stops.enumerated() {
                    let at = Double(ProcessInfo.processInfo.environment["LM_TOUR_AT"] ?? "") ?? 8
                    DispatchQueue.main.asyncAfter(deadline: .now() + at + Double(n) * 5) { [weak self] in
                        mapView.setCenter(CLLocationCoordinate2D(latitude: s[0], longitude: s[1]), zoomLevel: s[2], animated: false)
                        DispatchQueue.main.asyncAfter(deadline: .now() + 3.5) {
                            let st = self?.pointTiles.values.map { "\($0.stats.tiles) tiles, at most \($0.stats.most) features in one" } ?? []
                            print("LM_TOUR stop \(n) z\(s[2]) · \(st.joined(separator: "; ")) · \(os_proc_available_memory() / 1_048_576) MB available")
                            fflush(stdout)
                        }
                    }
                }
            }
            if ProcessInfo.processInfo.environment["LM_BENCH"] == "1" {
                DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
                    self?.bench.run(on: mapView)
                }
            }
        }

        private func ids(_ id: UUID) -> (source: String, fill: String, line: String, circle: String) {
            let p = "lm-\(id.uuidString)"
            return (p, p + "-fill", p + "-line", p + "-circle")
        }

        private func styleLayerIDs(_ layer: MapLayer) -> Set<String> {
            let i = ids(layer.id)
            switch layer.dataset.kind {
            case .point: return [i.circle]
            case .line: return [i.line]
            case .area, .table: return [i.fill, i.circle]
            case .raster: return [i.fill]
            }
        }

        /// Areas too small to see when zoomed out (36,882 solar farms, 44,929 mines: a blank world
        /// at first sight) get a dot at their centre until they are big enough to see: up to the
        /// zoom where a typical one is about 4 points across. nil when they show from the start,
        /// or when there are so many (building footprints) that dots would cost more than help.
        static func dotsMaxZoom(_ ds: GeoDataset) -> Double? {
            guard ds.kind == .area, ds.features.count <= 200_000 else { return nil }
            let step = max(1, ds.features.count / 2000)
            var sizes: [Double] = []
            for f in stride(from: 0, to: ds.features.count, by: step).map({ ds.features[$0] }) where f.bounds.isValid {
                sizes.append(max(f.bounds.maxLon - f.bounds.minLon, f.bounds.maxLat - f.bounds.minLat))
            }
            guard !sizes.isEmpty else { return nil }
            sizes.sort()
            let typical = max(sizes[sizes.count / 2], 1e-7)
            let z = log2(4 * 360 / (512 * typical))
            return z < 1.5 ? nil : min(z, 15)
        }

        private func dotSource(for layer: MapLayer, in style: MLNStyle) -> MLNSource {
            let id = ids(layer.id).source + "-c"
            if let s = style.source(withIdentifier: id) { return s }
            if layer.dataset.features.count > PointTiles.threshold {
                // Taiwan's 125,095 land-use areas as one array of dots was what ended the app.
                let t = PointTiles(dataset: layer.dataset, field: layer.style.showBy)
                pointTiles[layer.id] = t
                let s = MLNComputedShapeSource(identifier: id, dataSource: t, options: [.buffer: 16, .maximumZoomLevel: 18])
                style.addSource(s)
                return s
            }
            let points: [MLNPointFeature] = layer.dataset.features.compactMap { f in
                guard f.bounds.isValid else { return nil }
                let p = MLNPointFeature()
                p.coordinate = f.center
                // Its index (taps) and the colour field only, like the map file.
                var attrs: [String: Any] = ["_lm_i": f.index]
                if let field = layer.style.showBy, let v = f.properties[field] { attrs[field] = v }
                p.attributes = attrs
                return p
            }
            let s = MLNShapeSource(identifier: id, features: points, options: nil)
            style.addSource(s)
            return s
        }

        func sync(_ layers: [MapLayer], on map: MLNMapView) {
            // Spike-only: LM_NOMAP=1 leaves layers off the map, to tell its memory from the app's.
            if ProcessInfo.processInfo.environment["LM_NOMAP"] != nil { return }
            guard styleLoaded, let style = map.style else { return }
            let order = layers.map(\.id)
            // A layer coloured by another field needs that field in its source (it holds only one).
            var rebuild = false
            for layer in layers where layer.dataset.raster == nil && fileURLs[layer.id] != nil && sourceField[layer.id] != layer.style.showBy {
                removeLayers(layer.id, from: style)
                if let s = style.source(withIdentifier: ids(layer.id).source) { style.removeSource(s) }
                if let s = style.source(withIdentifier: ids(layer.id).source + "-c") { style.removeSource(s) }
                fileURLs.removeValue(forKey: layer.id)
                pointTiles.removeValue(forKey: layer.id)
                rebuild = true
            }
            if order != appliedOrder || rebuild {
                // Remove ours and re-add bottom-first so draw order matches the list.
                for id in appliedOrder { removeLayers(id, from: style) }
                if let l = style.layer(withIdentifier: highlightSource + "-line") { style.removeLayer(l) }
                if let l = style.layer(withIdentifier: highlightSource + "-circle") { style.removeLayer(l) }
                applied = [:]
                for layer in layers.reversed() { addLayers(layer, to: style) }
                appliedOrder = order
                lastHighlight = nil
                for id in Set(fileURLs.keys).subtracting(order) {
                    if let s = style.source(withIdentifier: ids(id).source) { style.removeSource(s) }
                    if let s = style.source(withIdentifier: ids(id).source + "-c") { style.removeSource(s) }
                    fileURLs.removeValue(forKey: id)
                    pointTiles.removeValue(forKey: id)
                }
            }
            for layer in layers {
                if let a = applied[layer.id], a.0 == layer.style, a.1 == layer.visible { continue }
                paint(layer, in: style)
                applied[layer.id] = (layer.style, layer.visible)
            }
        }

        private func removeLayers(_ id: UUID, from style: MLNStyle) {
            let i = ids(id)
            for lid in [i.fill, i.line, i.circle, i.circle + "-d"] {
                if let l = style.layer(withIdentifier: lid) { style.removeLayer(l) }
            }
        }

        private func source(for layer: MapLayer, in style: MLNStyle) -> MLNSource? {
            let i = ids(layer.id)
            if let s = style.source(withIdentifier: i.source) { return s }
            if let r = layer.dataset.raster {
                // The picture Rust drew on a Web Mercator grid, pinned at its corners.
                let b = r.bounds
                let quad = MLNCoordinateQuad(
                    topLeft: CLLocationCoordinate2D(latitude: b.maxLat, longitude: b.minLon),
                    bottomLeft: CLLocationCoordinate2D(latitude: b.minLat, longitude: b.minLon),
                    bottomRight: CLLocationCoordinate2D(latitude: b.minLat, longitude: b.maxLon),
                    topRight: CLLocationCoordinate2D(latitude: b.maxLat, longitude: b.maxLon)
                )
                let s = MLNImageSource(identifier: i.source, coordinateQuad: quad, url: r.png)
                style.addSource(s)
                fileURLs[layer.id] = r.png
                return s
            }
            if Self.drawsByTile(layer.dataset) {
                let t = PointTiles(dataset: layer.dataset, field: layer.style.showBy)
                pointTiles[layer.id] = t
                // A marker: the rebuild on a new colour field looks for one.
                fileURLs[layer.id] = layer.dataset.mapFile(showing: nil) ?? URL(fileURLWithPath: "/")
                sourceField[layer.id] = layer.style.showBy
                let s = MLNComputedShapeSource(identifier: i.source, dataSource: t, options: [.buffer: 16, .maximumZoomLevel: 18])
                style.addSource(s)
                return s
            }
            let url: URL
            if let u = fileURLs[layer.id] {
                url = u
            } else {
                // The reader's file (shapes and indexes only), or a copy with the colour field.
                guard let u = layer.dataset.mapFile(showing: layer.style.showBy) else { return nil }
                url = u
                fileURLs[layer.id] = url
                sourceField[layer.id] = layer.style.showBy
            }
            // A 128-point buffer (MapLibre's default) took the stork's dots from 1.3 GB to 2.6 GB
            // on an iPhone; 32 still keeps a dot or a line whole across a tile edge.
            // Spike-only experiments: LM_SRC="maxz=12,buffer=0,tol=1".
            var opts: [MLNShapeSourceOption: Any] = [.buffer: 32]
            for kv in (ProcessInfo.processInfo.environment["LM_SRC"] ?? "").split(separator: ",") {
                let p = kv.split(separator: "=")
                guard p.count == 2, let v = Double(p[1]) else { continue }
                switch p[0] {
                case "maxz": opts[.maximumZoomLevel] = v
                case "buffer": opts[.buffer] = v
                case "tol": opts[.simplificationTolerance] = v
                default: break
                }
            }
            let s = MLNShapeSource(identifier: i.source, url: url, options: opts)
            style.addSource(s)
            return s
        }

        private func addLayers(_ layer: MapLayer, to style: MLNStyle) {
            guard let src = source(for: layer, in: style) else { return }
            let i = ids(layer.id)
            switch layer.dataset.kind {
            case .raster:
                style.addLayer(MLNRasterStyleLayer(identifier: i.fill, source: src))
            case .point:
                if Self.drawsByTile(layer.dataset) {
                    // Density squares where a tile holds too many dots to draw.
                    let d = MLNFillStyleLayer(identifier: i.fill, source: src)
                    d.predicate = NSPredicate(mglJSONObject: ["==", ["geometry-type"], "Polygon"])
                    style.addLayer(d)
                }
                let c = MLNCircleStyleLayer(identifier: i.circle, source: src)
                if Self.drawsByTile(layer.dataset) {
                    c.predicate = NSPredicate(mglJSONObject: ["==", ["geometry-type"], "Point"])
                }
                c.circleStrokeColor = NSExpression(forConstantValue: UIColor.white)
                c.circleStrokeWidth = NSExpression(forConstantValue: 0.6)
                style.addLayer(c)
            case .line:
                let l = MLNLineStyleLayer(identifier: i.line, source: src)
                l.lineCap = NSExpression(forConstantValue: "round")
                l.lineJoin = NSExpression(forConstantValue: "round")
                style.addLayer(l)
            case .area, .table:
                let f = MLNFillStyleLayer(identifier: i.fill, source: src)
                style.addLayer(f)
                let o = MLNLineStyleLayer(identifier: i.line, source: src)
                o.lineColor = NSExpression(forConstantValue: UIColor.white)
                style.addLayer(o)
                if let z = Coordinator.dotsMaxZoom(layer.dataset) {
                    let dots = dotSource(for: layer, in: style)
                    if dots is MLNComputedShapeSource {
                        // Too many dots for a tile: where the small areas are, as density.
                        let d = MLNFillStyleLayer(identifier: i.circle + "-d", source: dots)
                        d.predicate = NSPredicate(mglJSONObject: ["==", ["geometry-type"], "Polygon"])
                        d.maximumZoomLevel = Float(z)
                        style.addLayer(d)
                    }
                    let c = MLNCircleStyleLayer(identifier: i.circle, source: dots)
                    if dots is MLNComputedShapeSource {
                        c.predicate = NSPredicate(mglJSONObject: ["==", ["geometry-type"], "Point"])
                    }
                    c.maximumZoomLevel = Float(z)
                    c.circleStrokeColor = NSExpression(forConstantValue: UIColor.white)
                    c.circleStrokeWidth = NSExpression(forConstantValue: 0.5)
                    style.addLayer(c)
                }
            }
        }

        /// One colour, darker where more places fall in a square (log scale: 1 to about 1,000);
        /// squares hold no field, so a colour-by shows on the dots.
        private func paintDensity(_ d: MLNFillStyleLayer, _ layer: MapLayer) {
            let base = layer.style.showBy == nil ? layer.style.color : 0x3B_52_8B
            d.fillColor = NSExpression(forConstantValue: UIColor(hex: base))
            let o = layer.style.opacity
            d.fillOpacity = NSExpression(mglJSONObject: [
                "interpolate", ["linear"], ["ln", ["get", PointTiles.countKey]],
                0, 0.18 * o, 7, 0.95 * o,
            ] as [Any])
            d.isVisible = layer.visible
        }

        private func paint(_ layer: MapLayer, in style: MLNStyle) {
            let i = ids(layer.id)
            let scheme = layer.scheme
            let color = NSExpression(mglJSONObject: scheme.expressionJSON(field: layer.style.showBy))
            let opacity = NSExpression(forConstantValue: layer.style.opacity)
            let size = NSExpression(forConstantValue: layer.style.size)
            switch layer.dataset.kind {
            case .raster:
                guard let r = style.layer(withIdentifier: i.fill) as? MLNRasterStyleLayer else { return }
                // Another NetCDF time step is a new picture on the same source. `source(for:)`
                // hands back the source it already made, so the swap has to happen here.
                if let r = layer.dataset.raster,
                   fileURLs[layer.id] != r.png,
                   let img = style.source(withIdentifier: i.source) as? MLNImageSource
                {
                    // A redraw for a zoomed-in view covers a smaller patch of world, so the
                    // corners move as well as the picture. Setting the url alone would stretch
                    // the new, more detailed image across the whole original extent.
                    let b = r.bounds
                    img.coordinates = MLNCoordinateQuad(
                        topLeft: CLLocationCoordinate2D(latitude: b.maxLat, longitude: b.minLon),
                        bottomLeft: CLLocationCoordinate2D(latitude: b.minLat, longitude: b.minLon),
                        bottomRight: CLLocationCoordinate2D(latitude: b.minLat, longitude: b.maxLon),
                        topRight: CLLocationCoordinate2D(latitude: b.maxLat, longitude: b.maxLon)
                    )
                    img.url = r.png
                    fileURLs[layer.id] = r.png
                }
                r.rasterOpacity = opacity
                r.isVisible = layer.visible
            case .point:
                guard let c = style.layer(withIdentifier: i.circle) as? MLNCircleStyleLayer else { return }
                c.circleColor = color
                c.circleOpacity = opacity
                c.circleStrokeOpacity = opacity
                // Grow a little with zoom so world-scale points stay readable.
                c.circleRadius = NSExpression(mglJSONObject: [
                    "interpolate", ["linear"], ["zoom"],
                    0, layer.style.size * 0.6,
                    6, layer.style.size * 1.3,
                    12, layer.style.size * 2.2,
                ] as [Any])
                c.isVisible = layer.visible
                if let d = style.layer(withIdentifier: i.fill) as? MLNFillStyleLayer {
                    paintDensity(d, layer)
                }
            case .line:
                guard let l = style.layer(withIdentifier: i.line) as? MLNLineStyleLayer else { return }
                l.lineColor = color
                l.lineOpacity = opacity
                l.lineWidth = size
                l.isVisible = layer.visible
            case .area, .table:
                guard let f = style.layer(withIdentifier: i.fill) as? MLNFillStyleLayer,
                      let o = style.layer(withIdentifier: i.line) as? MLNLineStyleLayer else { return }
                f.fillColor = color
                f.fillOpacity = opacity
                f.isVisible = layer.visible
                o.lineWidth = size
                o.lineOpacity = NSExpression(forConstantValue: min(1, layer.style.opacity + 0.2))
                o.isVisible = layer.visible
                if let c = style.layer(withIdentifier: i.circle) as? MLNCircleStyleLayer {
                    c.circleColor = color
                    c.circleOpacity = NSExpression(forConstantValue: min(1, layer.style.opacity + 0.3))
                    c.circleRadius = NSExpression(forConstantValue: 2.6)
                    c.isVisible = layer.visible
                }
                if let d = style.layer(withIdentifier: i.circle + "-d") as? MLNFillStyleLayer {
                    paintDensity(d, layer)
                }
            }
        }

        // MARK: Highlight, pin, camera

        func setHighlight(_ data: Data?, on map: MLNMapView) {
            guard styleLoaded, let style = map.style, data != lastHighlight else { return }
            lastHighlight = data
            let shape = data.flatMap { try? MLNShape(data: $0, encoding: String.Encoding.utf8.rawValue) }
                ?? (MLNShapeCollectionFeature(shapes: []) as MLNShape)
            if let s = style.source(withIdentifier: highlightSource) as? MLNShapeSource {
                s.shape = shape
            } else {
                let s = MLNShapeSource(identifier: highlightSource, shape: shape, options: nil)
                style.addSource(s)
            }
            guard let s = style.source(withIdentifier: highlightSource) else { return }
            if style.layer(withIdentifier: highlightSource + "-line") == nil {
                let l = MLNLineStyleLayer(identifier: highlightSource + "-line", source: s)
                l.lineColor = NSExpression(forConstantValue: UIColor(hex: 0x111111))
                l.lineWidth = NSExpression(forConstantValue: 3)
                style.addLayer(l)
                let c = MLNCircleStyleLayer(identifier: highlightSource + "-circle", source: s)
                c.circleColor = NSExpression(forConstantValue: UIColor.clear)
                c.circleRadius = NSExpression(forConstantValue: 11)
                c.circleStrokeColor = NSExpression(forConstantValue: UIColor(hex: 0x111111))
                c.circleStrokeWidth = NSExpression(forConstantValue: 3)
                c.predicate = NSPredicate(mglJSONObject: ["==", ["geometry-type"], "Point"] as [Any])
                style.addLayer(c)
            }
        }

        func setPin(_ coordinate: CLLocationCoordinate2D?, on map: MLNMapView) {
            if let coordinate {
                if let pinAnnotation, pinAnnotation.coordinate.latitude == coordinate.latitude,
                   pinAnnotation.coordinate.longitude == coordinate.longitude { return }
                if let pinAnnotation { map.removeAnnotation(pinAnnotation) }
                let a = MLNPointAnnotation()
                a.coordinate = coordinate
                map.addAnnotation(a)
                pinAnnotation = a
            } else if let pinAnnotation {
                map.removeAnnotation(pinAnnotation)
                self.pinAnnotation = nil
            }
        }

        func apply(_ camera: CameraRequest, on map: MLNMapView) {
            guard styleLoaded else {
                pendingCamera = camera
                return
            }
            let b = camera.bounds
            let insets = camera.exact ? .zero : (parent?.insets ?? .zero)
            if b.maxLon - b.minLon < 1e-6 && b.maxLat - b.minLat < 1e-6 {
                // Center the point in the part of the map the chrome leaves open (above a card),
                // not the middle of the screen (Priya: "When I zoom to a place, let me see it").
                var zoom = max(map.zoomLevel, camera.pointZoom)
                if let reach = camera.reachMeters {
                    // Close enough that `reach` still fits in the open part of the map (inside the
                    // chrome, with a margin so the farthest of the few isn't on the edge), never
                    // closer than `pointZoom`, never out past a whole continent.
                    let openW = Double(map.bounds.width - insets.left - insets.right)
                    let openH = Double(map.bounds.height - insets.top - insets.bottom)
                    let half = max(40, min(openW, openH) / 2 * 0.8)
                    let metresPerPointAtZ0 = 40_075_016.686 * cos(b.center.latitude * .pi / 180) / 512
                    let fit = log2(metresPerPointAtZ0 * half / reach)
                    zoom = min(camera.pointZoom, max(2, fit))
                }
                let degreesPerPoint = 360 / (512 * pow(2, zoom))
                let dy = Double(insets.bottom - insets.top) / 2
                let dx = Double(insets.right - insets.left) / 2
                let lat = b.center.latitude
                let center = CLLocationCoordinate2D(
                    latitude: lat - dy * degreesPerPoint * cos(lat * .pi / 180),
                    longitude: b.center.longitude + dx * degreesPerPoint
                )
                map.setCenter(center, zoomLevel: zoom, animated: true)
                return
            }
            let bounds = MLNCoordinateBounds(
                sw: CLLocationCoordinate2D(latitude: max(-80, b.minLat), longitude: max(-180, b.minLon)),
                ne: CLLocationCoordinate2D(latitude: min(84, b.maxLat), longitude: min(180, b.maxLon))
            )
            map.setVisibleCoordinateBounds(bounds, edgePadding: insets, animated: !camera.exact, completionHandler: nil)
        }

        func mapView(_ mapView: MLNMapView, regionIsChangingWith reason: MLNCameraChangeReason) {
            reportHeading(mapView)
        }

        /// Whole degrees only, so a pan doesn't redraw the buttons on every frame.
        private func reportHeading(_ map: MLNMapView) {
            let h = map.direction.rounded()
            guard h != lastHeading else { return }
            lastHeading = h
            parent?.onHeading?(h)
        }

        func mapView(_ mapView: MLNMapView, regionDidChangeWith reason: MLNCameraChangeReason, animated: Bool) {
            reportHeading(mapView)
            let v = mapView.visibleCoordinateBounds
            parent?.onCameraIdle?(GeoBounds(minLon: v.sw.longitude, minLat: v.sw.latitude, maxLon: v.ne.longitude, maxLat: v.ne.latitude))
        }

        // MARK: Gestures

        @objc func handleTap(_ g: UITapGestureRecognizer) {
            guard let map = g.view as? MLNMapView, g.state == .ended else { return }
            let p = g.location(in: map)
            parent?.onTap?(hits(at: p, on: map), map.convert(p, toCoordinateFrom: map))
        }

        @objc func handleLongPress(_ g: UILongPressGestureRecognizer) {
            guard let map = g.view as? MLNMapView, g.state == .began else { return }
            let p = g.location(in: map)
            let coord = map.convert(p, toCoordinateFrom: map)
            parent?.onLongPress?(coord, hits(at: p, on: map, exact: true))
        }

        /// Top layer first; at most one hit per area layer, nearest points first. 44 pt target.
        func hits(at p: CGPoint, on map: MLNMapView, exact: Bool = false) -> [MapHit] {
            guard let parent else { return [] }
            var out: [MapHit] = []
            for layer in parent.layers where layer.visible {
                let small = layer.dataset.kind == .area || exact
                let r: CGFloat = small ? 1 : 22
                let rect = CGRect(x: p.x - r, y: p.y - r, width: r * 2, height: r * 2)
                let features = map.visibleFeatures(in: rect, styleLayerIdentifiers: styleLayerIDs(layer))
                var found: [(Int, CGFloat, CGPoint)] = []
                var seen = Set<Int>()
                for f in features {
                    guard let n = f.attribute(forKey: "_lm_i") as? NSNumber else { continue }
                    let idx = n.intValue
                    guard !seen.contains(idx) else { continue }
                    seen.insert(idx)
                    let fp = map.convert(f.coordinate, toPointTo: map)
                    found.append((idx, hypot(fp.x - p.x, fp.y - p.y), fp))
                }
                found.sort { $0.1 < $1.1 }
                // The nearest one, plus only those drawn on top of it: dots a finger's width apart
                // are different places (Walt's Haworth and Keighley came up as one "2 things here").
                let stacked = found.first.map { near in found.filter { hypot($0.2.x - near.2.x, $0.2.y - near.2.y) <= 8 } } ?? []
                let keep = layer.dataset.kind == .area ? found.prefix(1) : stacked.prefix(5)
                out += keep.map { MapHit(layerID: layer.id, featureIndex: $0.0) }
            }
            return out
        }

        func mapView(_ mapView: MLNMapView, regionWillChangeWith reason: MLNCameraChangeReason, animated: Bool) {
            let gesture: MLNCameraChangeReason = [.gesturePan, .gesturePinch, .gestureRotate, .gestureZoomIn, .gestureZoomOut, .gestureOneFingerZoom]
            if !reason.intersection(gesture).isEmpty {
                parent?.onUserGesture?()
            }
        }

        func mapViewDidFinishRenderingFrame(_ mapView: MLNMapView, fullyRendered: Bool) {
            bench.frame()
        }
    }
}

/// Spike-only: a scripted camera tour (LM_BENCH=1) timing every rendered map frame. Prints one
/// `LM_BENCH …` line to stdout (read it with `simctl launch --console`).
final class FrameBench {
    private var times: [CFTimeInterval] = []
    private var recording = false
    private var hz = 60

    func frame() {
        guard recording else { return }
        times.append(CACurrentMediaTime())
    }

    func run(on map: MLNMapView) {
        let stops: [(Double, Double, Double)] = [
            (35, 105, 3), (31.2, 121.5, 6), (48.8, 2.35, 4), (40.7, -74.0, 5), (-23.5, -46.6, 4), (20, 10, 1.2), (35.7, 139.7, 5),
        ]
        times = []
        hz = max(60, map.window?.windowScene?.screen.maximumFramesPerSecond ?? 60)
        recording = true
        var delay = 0.0
        for (lat, lon, z) in stops {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
                let cam = MLNMapCamera(
                    lookingAtCenter: CLLocationCoordinate2D(latitude: lat, longitude: lon),
                    acrossDistance: 4.0e7 / pow(2, z),
                    pitch: 0,
                    heading: 0
                )
                map.fly(to: cam, withDuration: 2.0, completionHandler: nil)
            }
            delay += 2.2
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + delay + 0.5) { [weak self] in
            self?.report()
        }
    }

    private func report() {
        recording = false
        guard times.count > 2 else {
            print("LM_BENCH frames=\(times.count) (too few)")
            return
        }
        var gaps: [Double] = []
        for i in 1..<times.count { gaps.append((times[i] - times[i - 1]) * 1000) }
        let busy = gaps.filter { $0 < 250 }
        let sorted = busy.sorted()
        let avg = busy.reduce(0, +) / Double(max(1, busy.count))
        let p95 = sorted.isEmpty ? 0 : sorted[min(sorted.count - 1, Int(Double(sorted.count) * 0.95))]
        // late: missed the screen's own refresh (over 1.5 frames at `hz`); slow: a visible stutter (over 33.4 ms).
        let budget = 1000 / Double(hz)
        let late = busy.filter { $0 > budget * 1.5 }.count
        let slow = busy.filter { $0 > 33.4 }.count
        let share = { (n: Int) in 100 * Double(n) / Double(max(1, busy.count)) }
        let line = String(
            format: "LM_BENCH hz=%ld frames=%ld avg_ms=%.1f fps=%.1f p95_ms=%.1f worst_ms=%.1f late_pct=%.1f slow_pct=%.1f",
            hz, times.count, avg, 1000 / max(avg, 0.001), p95, sorted.last ?? 0, share(late), share(slow)
        )
        print(line)
        fflush(stdout)
    }
}
