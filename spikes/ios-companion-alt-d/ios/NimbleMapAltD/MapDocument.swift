import Foundation
import SwiftUI

// From the A/B shared file (Alt C copy): the data on the map, adding, removing, undo, camera requests.

/// One data set on the map with its look. The basemap is never a layer (always on, never listed).
struct MapLayer: Identifiable, Equatable {
    let id: UUID
    let dataset: GeoDataset
    var visible: Bool
    var style: LayerStyle
    /// Bumped when the dataset's cells are replaced (a new NetCDF time step). `dataset` is a
    /// class, so without this the layer compares equal to itself and the map never redraws.
    var dataRev: Int = 0

    var name: String { dataset.name }
    var scheme: ColorRule { ColorRule.make(style: style, dataset: dataset) }

    /// A grid of values names its key's ends with units (Priya: "11.5 ━ 226.5" said nothing).
    var legendEnds: (String, String)? {
        guard let r = dataset.raster, r.look == .values else { return nil }
        return ("Low \(r.format(r.low))", "High \(r.format(r.high))")
    }

    static func == (a: MapLayer, b: MapLayer) -> Bool {
        a.id == b.id && a.visible == b.visible && a.style == b.style && a.dataRev == b.dataRev
    }
}

struct CameraRequest: Equatable {
    let id = UUID()
    let bounds: GeoBounds
    /// Zoom for a single point (bounds of zero size).
    var pointZoom: Double = 4
    /// Show exactly these bounds, no chrome padding, no animation (returning to a saved view).
    var exact = false
}

struct Toast: Identifiable, Equatable {
    let id = UUID()
    let message: String
    let canUndo: Bool
    /// A second button ("Show all"); its work is `MapDocument.toastAction`.
    var actionTitle: String?
}

@MainActor
final class MapDocument: ObservableObject {
    /// Draw order: index 0 is the topmost data layer (same as the desktop list).
    @Published var layers: [MapLayer] = [] {
        // The sample is never saved: a relaunch brings back the person's map, or the sample again.
        didSet { if !restoring { SavedMap.save(layers.filter { $0.id != sampleID }) } }
    }
    /// The first-run sample map (B's instant map). It steps aside when the person's own data arrives.
    @Published var sampleID: UUID?
    /// What the map shows now (set as the camera settles).
    var viewBounds: GeoBounds?
    /// The work of the toast's second button.
    var toastAction: (() -> Void)?
    @Published var camera: CameraRequest?
    /// Shown while a file downloads or decodes ("Adding Earthquakes…").
    @Published var busy: String?
    /// 0…1 while a download reports its size; nil when the server won't say how big it is, so the
    /// bar stays indeterminate rather than inventing a number.
    @Published var busyFraction: Double?
    /// "2.4 MB of 11 MB" under the bar.
    @Published var busyDetail: String?
    /// The running import, so a Stop button can end it.
    private var busyTask: Task<Void, Never>?
    /// Counts time-step reads so a slow one can't overwrite a newer one.
    private var timeRequest = 0
    /// The same, for redraws at another ramp or zoom.
    private var rasterRequest = 0
    @Published var toast: Toast?
    @Published var error: String?

    private var undoStack: [() -> Void] = []
    private var colorCounter = 0
    private var restoring = false

    var isEmpty: Bool { layers.isEmpty }

    func layer(_ id: UUID) -> MapLayer? { layers.first { $0.id == id } }

    func index(of id: UUID) -> Int? { layers.firstIndex { $0.id == id } }

    func feature(_ hit: MapHit) -> (MapLayer, GeoFeature)? {
        guard let layer = layer(hit.layerID), let f = layer.dataset.features[safe: hit.featureIndex] else { return nil }
        return (layer, f)
    }

    /// One feature as GeoJSON, for the outline drawn while its card is open.
    func outline(_ hit: MapHit?) -> Data? {
        guard let hit, case let (_, f)? = feature(hit) else { return nil }
        return try? JSONSerialization.data(withJSONObject: f.raw)
    }

    /// Add and fly to it (redesign › Files from other apps: "fly the camera to the new data").
    /// Alt C: a new layer goes on top of its own kind, so places stay above lines and lines above
    /// areas whatever order they came in (Priya: "Don't cover my pins"). The list still matches
    /// the map, and Edit can still reorder.
    func add(_ dataset: GeoDataset, showBy: String? = nil, style saved: LayerStyle? = nil, visible: Bool = true, fly flyTo: Bool = true, sample: Bool = false) {
        var style = saved ?? LayerStyle.default(kind: dataset.kind, colorIndex: colorCounter)
        colorCounter += 1
        if saved == nil {
            let goingAway = dataset.libraryID == nil ? sampleID : nil
            let used = layers.filter { $0.visible && $0.id != goingAway }.flatMap { $0.scheme.hexes }
            style.color = ColorRule.freshColor(avoiding: used, fallback: colorCounter - 1)
            if let showBy, dataset.field(named: showBy) != nil {
                style.showBy = showBy
            } else if let own = dataset.fileColorsField {
                // A My Maps export opens in its own colors.
                style.showBy = own
            }
        }
        let layer = MapLayer(id: UUID(), dataset: dataset, visible: visible, style: style)
        let rank = MapDocument.rank(dataset.kind)
        var putAway: MapLayer?
        if dataset.libraryID == nil, let sampleID, let i = index(of: sampleID) {
            // The person's own file: the sample stops tinting the map and doubling every tap.
            putAway = layers.remove(at: i)
            self.sampleID = nil
        }
        let at = layers.firstIndex { MapDocument.rank($0.dataset.kind) >= rank } ?? layers.count
        if sample { sampleID = layer.id }
        layers.insert(layer, at: Swift.min(at, layers.count))
        if !sample {
            // Say it arrived, and how to take it back (Walt: "tell me it's there").
            undoStack.append { [weak self] in
                guard let self else { return }
                self.layers.removeAll { $0.id == layer.id }
                if let putAway {
                    self.sampleID = putAway.id
                    self.layers.append(putAway)
                }
            }
            var message = "Added \(dataset.name) · \(dataset.countPhrase)"
            if putAway != nil { message += " · sample put away" }
            // Added on top of a map the person is already looking at: stay put if any of the new
            // data is in view (Walt was thrown out to the whole Atlantic by two New Jersey rows),
            // and say how much is in view, with "Show all" one tap away.
            let others = layers.contains { $0.id != layer.id }
            if flyTo, others, let view = viewBounds, let inView = MapDocument.countInView(dataset, view), inView > 0 {
                let total = dataset.raster == nil ? dataset.features.count : 1
                if inView < total {
                    message = "Added \(dataset.name) · \(NumberWords.integer(inView)) of \(NumberWords.integer(total)) in view"
                    toastAction = { [weak self] in self?.fly(to: dataset.bounds) }
                    toast = Toast(message: message, canUndo: true, actionTitle: "Show all")
                } else {
                    toastAction = nil
                    toast = Toast(message: message, canUndo: true)
                }
                return
            }
            toastAction = nil
            toast = Toast(message: message, canUndo: true)
        }
        if flyTo { fly(to: dataset.bounds) }
    }

    /// How many of the data's features sit in `view` (a map image counts as one); nil if unknown.
    static func countInView(_ ds: GeoDataset, _ view: GeoBounds) -> Int? {
        guard view.isValid else { return nil }
        func inside(_ lon: Double, _ lat: Double) -> Bool {
            lon >= view.minLon && lon <= view.maxLon && lat >= view.minLat && lat <= view.maxLat
        }
        if let r = ds.raster {
            let b = r.bounds
            let overlaps = b.minLon <= view.maxLon && b.maxLon >= view.minLon && b.minLat <= view.maxLat && b.maxLat >= view.minLat
            return overlaps ? 1 : 0
        }
        return ds.features.filter { inside($0.center.longitude, $0.center.latitude) }.count
    }

    static func rank(_ kind: GeometryKind) -> Int {
        switch kind {
        case .point: return 0
        case .line: return 1
        case .area, .table: return 2
        case .raster: return 3
        }
    }

    // MARK: Saved map (Alt C: the map comes back on relaunch)

    /// Open the map that was on screen last time, in the same order and look.
    func restore() async -> Bool {
        let saved = SavedMap.load()
        guard !saved.isEmpty else { return false }
        busy = "Opening your map…"
        restoring = true
        var opened: [(SavedMap.Layer, GeoDataset)] = []
        for entry in saved {
            if case .success(let ds)? = await entry.open() { opened.append((entry, ds)) }
        }
        // Saved top-first; add bottom-first so each lands on top of the last.
        var all = GeoBounds.empty
        for (entry, ds) in opened.reversed() {
            let style = entry.style
            let layer = MapLayer(id: UUID(), dataset: ds, visible: entry.visible, style: style)
            layers.insert(layer, at: 0)
            if entry.visible { all.add(ds.bounds) }
        }
        restoring = false
        busy = nil
        SavedMap.save(layers)
        if let view = SavedMap.view, view.isValid {
            camera = CameraRequest(bounds: view, exact: true)
        } else {
            fly(to: all)
        }
        return !opened.isEmpty
    }

    func fly(to bounds: GeoBounds, pointZoom: Double = 4) {
        guard bounds.isValid else { return }
        camera = CameraRequest(bounds: bounds, pointZoom: pointZoom)
    }

    func remove(_ id: UUID) {
        guard let i = index(of: id) else { return }
        let removed = layers.remove(at: i)
        undoStack.append { [weak self] in
            guard let self else { return }
            self.layers.insert(removed, at: Swift.min(i, self.layers.count))
        }
        if removed.id == sampleID { sampleID = nil }
        toast = Toast(message: "Removed \(removed.name)", canUndo: true)
    }

    func move(from source: IndexSet, to destination: Int) {
        layers.move(fromOffsets: source, toOffset: destination)
    }

    func setVisible(_ id: UUID, _ visible: Bool) {
        guard let i = index(of: id) else { return }
        layers[i].visible = visible
    }

    func setStyle(_ id: UUID, _ style: LayerStyle) {
        guard let i = index(of: id) else { return }
        layers[i].style = style
    }

    // MARK: Time (NetCDF)

    /// The step a layer's cells show, which may lag the slider while a read is in flight.
    func shownTime(_ id: UUID) -> Int { layer(id)?.dataset.raster?.time ?? 0 }

    /// Shows step `to` of a grid with a time axis. The slider runs ahead of the file: drags land
    /// faster than a step can be read, so only the newest request is kept and anything older is
    /// dropped on arrival (`timeRequest`). The colour range is the one the layer already has, so
    /// the ramp means the same at every step.
    func setTime(_ id: UUID, _ to: Int) {
        guard let layer = layer(id), let r = layer.dataset.raster, r.hasTime, let source = r.source else { return }
        let step = max(0, min(to, r.times.count - 1))
        guard step != r.time else { return }
        timeRequest += 1
        let token = timeRequest
        let name = layer.name
        let range = (r.low, r.high)
        Task.detached(priority: .userInitiated) {
            let result = RasterDecode.open(source, name: name, sourceName: "", time: step, range: range)
            await MainActor.run {
                // A newer drag already won; this one is stale.
                guard token == self.timeRequest, case .success(let ds) = result, let i = self.index(of: id) else { return }
                self.layers[i].dataset.adoptCells(from: ds)
                self.layers[i].dataRev &+= 1
            }
        }
    }

    /// Redraws a grid of values on another colour ramp.
    ///
    /// A raster is one picture on the map, not styled features, so there is nothing on the phone
    /// to recolour — the picture has to be drawn again. That is why this is a round trip through
    /// Rust rather than a style change like every other layer's colours.
    func setRasterRamp(_ id: UUID, _ ramp: Palette) {
        guard let layer = layer(id), let r = layer.dataset.raster, r.look == .values, r.ramp != ramp else { return }
        redrawRaster(id, ramp: ramp, window: r.window)
    }

    /// How much smaller the view has to be than the drawn window before redrawing is worth it.
    ///
    /// This is the speed/detail trade-off. A redraw reads the file again and writes a new PNG, so
    /// doing it on every nudge of the map would make panning feel like wading. Waiting for a 4×
    /// difference in width means a redraw happens roughly once every two zoom levels — about as
    /// often as the detail visibly runs out — and never while someone is just sliding around at
    /// the same scale.
    private static let redrawGain = 4.0

    /// Redraw for what is on screen, if that would actually gain detail.
    ///
    /// Only ever called when the camera has settled (`onCameraIdle`), never mid-gesture.
    func refineRaster(_ id: UUID, view: GeoBounds) {
        guard let layer = layer(id), let r = layer.dataset.raster, r.source != nil else { return }
        let drawn = r.window ?? r.bounds
        let drawnWidth = drawn.maxLon - drawn.minLon
        // Clamp to the data: asking for more than the file covers gains nothing.
        let wanted = GeoBounds(
            minLon: Swift.max(view.minLon, r.bounds.minLon),
            minLat: Swift.max(view.minLat, r.bounds.minLat),
            maxLon: Swift.min(view.maxLon, r.bounds.maxLon),
            maxLat: Swift.min(view.maxLat, r.bounds.maxLat)
        )
        let wantedWidth = wanted.maxLon - wanted.minLon
        guard wantedWidth > 0, drawnWidth / wantedWidth >= Self.redrawGain else { return }
        // A margin, so a small pan after the redraw does not immediately fall off the picture.
        let padX = wantedWidth * 0.25
        let padY = (wanted.maxLat - wanted.minLat) * 0.25
        let padded = GeoBounds(
            minLon: Swift.max(wanted.minLon - padX, r.bounds.minLon),
            minLat: Swift.max(wanted.minLat - padY, r.bounds.minLat),
            maxLon: Swift.min(wanted.maxLon + padX, r.bounds.maxLon),
            maxLat: Swift.min(wanted.maxLat + padY, r.bounds.maxLat)
        )
        redrawRaster(id, ramp: r.ramp, window: padded)
    }

    /// Draw a raster again with different settings, keeping its colour range so only the thing
    /// that was asked for changes.
    private func redrawRaster(_ id: UUID, ramp: Palette, window: GeoBounds?) {
        guard let layer = layer(id), let r = layer.dataset.raster, let source = r.source else { return }
        rasterRequest += 1
        let token = rasterRequest
        let name = layer.name
        let range = (r.low, r.high)
        let step = r.time
        Task.detached(priority: .userInitiated) {
            let result = RasterDecode.open(source, name: name, sourceName: "", time: step, range: range, ramp: ramp, window: window)
            await MainActor.run {
                guard token == self.rasterRequest, case .success(let ds) = result, let i = self.index(of: id) else { return }
                self.layers[i].dataset.adoptCells(from: ds)
                self.layers[i].dataRev &+= 1
            }
        }
    }

    /// Steps a grid's time by `delta`, stopping at the ends.
    func stepTime(_ id: UUID, _ delta: Int) {
        guard let r = layer(id)?.dataset.raster, r.hasTime else { return }
        setTime(id, r.time + delta)
    }

    func undo() {
        guard let last = undoStack.popLast() else { return }
        last()
        toast = nil
    }

    var canUndo: Bool { !undoStack.isEmpty }

    /// Run an import off the main thread; the map keeps panning.
    /// Several at once (files picked together, a folder): each dataset is added; the first
    /// problem is shown, with how many more there were.
    func importingAll(_ label: String, _ work: @escaping @Sendable () async -> [Result<GeoDataset, ImportError>]) {
        startBusy("Adding \(label)…")
        busyTask = Task {
            let results = await work()
            self.endBusy()
            var problems: [String] = []
            for result in results {
                switch result {
                case .success(let ds):
                    self.add(ds, showBy: nil)
                case .failure(let e):
                    problems.append(e.message)
                }
            }
            if let first = problems.first {
                self.error = problems.count > 1 ? "\(first) (\(problems.count - 1) more couldn't open.)" : first
            }
        }
    }

    func importing(_ label: String, _ work: @escaping @Sendable () async -> Result<GeoDataset, ImportError>, showBy: String? = nil) {
        startBusy("Adding \(label)…")
        busyTask = Task {
            let result = await work()
            self.endBusy()
            switch result {
            case .success(let ds):
                self.add(ds, showBy: showBy)
            case .failure(let e):
                // A Stop the person pressed is not an error.
                if e.message != Importer.cancelled { self.error = e.message }
            }
        }
    }

    // MARK: Progress

    private func startBusy(_ label: String) {
        busy = label
        busyFraction = nil
        busyDetail = nil
    }

    private func endBusy() {
        busy = nil
        busyFraction = nil
        busyDetail = nil
        busyTask = nil
    }

    /// Handed to `Importer.download` as its progress callback. `total` is 0 when the server sent no
    /// Content-Length, which leaves the bar indeterminate instead of guessing.
    func downloadProgress(_ received: Int64, _ total: Int64) {
        guard busy != nil else { return }
        if total > 0 {
            busyFraction = min(1, Double(received) / Double(total))
            busyDetail = "\(Self.bytes(received)) of \(Self.bytes(total))"
        } else {
            busyFraction = nil
            busyDetail = Self.bytes(received)
        }
    }

    /// The Stop button on the progress capsule.
    func cancelBusy() {
        busyTask?.cancel()
        endBusy()
    }

    private static func bytes(_ n: Int64) -> String {
        ByteCountFormatter.string(fromByteCount: n, countStyle: .file)
    }
}
