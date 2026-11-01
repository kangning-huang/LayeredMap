import Foundation
import SwiftUI

// Shared by Alt A and Alt B: the data on the map, adding, removing, undo, camera requests.

/// One data set on the map with its look. The basemap is never a layer (always on, never listed).
struct MapLayer: Identifiable, Equatable {
    let id: UUID
    let dataset: GeoDataset
    var visible: Bool
    var style: LayerStyle

    var name: String { dataset.name }
    var scheme: ColorRule { ColorRule.make(style: style, dataset: dataset) }

    static func == (a: MapLayer, b: MapLayer) -> Bool {
        a.id == b.id && a.visible == b.visible && a.style == b.style
    }
}

struct CameraRequest: Equatable {
    let id = UUID()
    let bounds: GeoBounds
    /// Zoom for a single point (bounds of zero size).
    var pointZoom: Double = 4
}

struct Toast: Identifiable, Equatable {
    let id = UUID()
    let message: String
    let canUndo: Bool
}

@MainActor
final class MapDocument: ObservableObject {
    /// Draw order: index 0 is the topmost data layer (same as the desktop list).
    @Published var layers: [MapLayer] = []
    @Published var camera: CameraRequest?
    /// Shown while a file downloads or decodes ("Adding Earthquakes…").
    @Published var busy: String?
    @Published var toast: Toast?
    @Published var error: String?

    private var undoStack: [() -> Void] = []
    private var colorCounter = 0

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

    /// Add on top and fly to it (redesign › Files from other apps: "fly the camera to the new data").
    func add(_ dataset: GeoDataset, showBy: String? = nil) {
        var style = LayerStyle.default(kind: dataset.kind, colorIndex: colorCounter)
        colorCounter += 1
        if let showBy, dataset.field(named: showBy) != nil { style.showBy = showBy }
        let layer = MapLayer(id: UUID(), dataset: dataset, visible: true, style: style)
        layers.insert(layer, at: 0)
        fly(to: dataset.bounds)
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

    func undo() {
        guard let last = undoStack.popLast() else { return }
        last()
        toast = nil
    }

    var canUndo: Bool { !undoStack.isEmpty }

    /// Run an import off the main thread; the map keeps panning.
    func importing(_ label: String, _ work: @escaping @Sendable () async -> Result<GeoDataset, ImportError>, showBy: String? = nil) {
        busy = "Adding \(label)…"
        Task {
            let result = await work()
            self.busy = nil
            switch result {
            case .success(let ds):
                self.add(ds, showBy: showBy)
            case .failure(let e):
                self.error = e.message
            }
        }
    }
}
