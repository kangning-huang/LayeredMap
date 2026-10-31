import SwiftUI

/// Geometry kind for legend swatches and lean symbology controls.
enum LayerGeometryKind: String, CaseIterable, Identifiable, Hashable {
    case point
    case line
    case fill

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .point: return "Point"
        case .line: return "Line"
        case .fill: return "Fill"
        }
    }
}

/// Lean symbology for the spike: color, size/width, opacity.
/// MapLibre paint updates from this model are stubbed in MapScreen for this pass.
struct LayerSymbology: Hashable {
    var color: Color
    /// Point radius or line width (points). Fill uses it as outline weight.
    var size: Double
    var opacity: Double

    static let pointDefault = LayerSymbology(color: .blue, size: 8, opacity: 0.9)
    static let lineDefault = LayerSymbology(color: .gray, size: 3, opacity: 1.0)
    static let fillDefault = LayerSymbology(color: .green, size: 1, opacity: 0.45)
}

/// One data layer. Basemap is never modeled here (always on; never listed in B).
struct MapLayer: Identifiable, Hashable {
    let id: UUID
    var name: String
    var visible: Bool
    var geometry: LayerGeometryKind
    var symbology: LayerSymbology

    init(
        id: UUID = UUID(),
        name: String,
        visible: Bool = true,
        geometry: LayerGeometryKind,
        symbology: LayerSymbology
    ) {
        self.id = id
        self.name = name
        self.visible = visible
        self.geometry = geometry
        self.symbology = symbology
    }
}

/// In-memory layer stack for the A→B→C chrome spike. Demo layers only — no dataset import yet.
@MainActor
final class LayerStore: ObservableObject {
    /// Draw order: index 0 is topmost data layer (matches desktop list lean).
    @Published var layers: [MapLayer]

    init(layers: [MapLayer] = LayerStore.demoLayers) {
        self.layers = layers
    }

    static let demoLayers: [MapLayer] = [
        MapLayer(
            name: "Cities",
            geometry: .point,
            symbology: LayerSymbology(color: .blue, size: 8, opacity: 0.9)
        ),
        MapLayer(
            name: "Roads",
            geometry: .line,
            symbology: LayerSymbology(color: Color(white: 0.45), size: 3, opacity: 1.0)
        ),
        MapLayer(
            name: "Parks",
            geometry: .fill,
            symbology: LayerSymbology(color: .green, size: 1, opacity: 0.45)
        ),
    ]

    func move(from source: IndexSet, to destination: Int) {
        layers.move(fromOffsets: source, toOffset: destination)
    }

    func remove(id: UUID) {
        layers.removeAll { $0.id == id }
    }

    func binding(for id: UUID) -> Binding<MapLayer>? {
        guard let index = layers.firstIndex(where: { $0.id == id }) else { return nil }
        return Binding(
            get: { self.layers[index] },
            set: { self.layers[index] = $0 }
        )
    }

    func updateSymbology(id: UUID, _ symbology: LayerSymbology) {
        guard let index = layers.firstIndex(where: { $0.id == id }) else { return }
        layers[index].symbology = symbology
    }

    func setVisible(id: UUID, _ visible: Bool) {
        guard let index = layers.firstIndex(where: { $0.id == id }) else { return }
        layers[index].visible = visible
    }
}
