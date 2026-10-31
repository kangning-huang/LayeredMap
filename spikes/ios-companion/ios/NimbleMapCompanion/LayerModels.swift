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
/// MapLibre paint on demo layers follows this model (see MapScreen).
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
    /// Stable MapLibre source/layer id prefix (survives rename).
    let styleId: String
    var name: String
    var visible: Bool
    var geometry: LayerGeometryKind
    var symbology: LayerSymbology
    /// Demo FeatureCollection GeoJSON drawn on the map for this chrome spike.
    var demoGeoJSON: String

    init(
        id: UUID = UUID(),
        styleId: String,
        name: String,
        visible: Bool = true,
        geometry: LayerGeometryKind,
        symbology: LayerSymbology,
        demoGeoJSON: String
    ) {
        self.id = id
        self.styleId = styleId
        self.name = name
        self.visible = visible
        self.geometry = geometry
        self.symbology = symbology
        self.demoGeoJSON = demoGeoJSON
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
            styleId: "demo-cities",
            name: "Cities",
            geometry: .point,
            symbology: LayerSymbology(color: .blue, size: 8, opacity: 0.9),
            demoGeoJSON: DemoGeoJSON.cities
        ),
        MapLayer(
            styleId: "demo-roads",
            name: "Roads",
            geometry: .line,
            symbology: LayerSymbology(color: Color(white: 0.45), size: 3, opacity: 1.0),
            demoGeoJSON: DemoGeoJSON.roads
        ),
        MapLayer(
            styleId: "demo-parks",
            name: "Parks",
            geometry: .fill,
            symbology: LayerSymbology(color: .green, size: 1, opacity: 0.45),
            demoGeoJSON: DemoGeoJSON.parks
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

/// Tiny NYC-ish demo geometries so Alt A symbology changes are visible on the MapLibre demotiles.
enum DemoGeoJSON {
    static let cities = """
    {"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"name":"Midtown"},"geometry":{"type":"Point","coordinates":[-73.9857,40.7484]}},
      {"type":"Feature","properties":{"name":"Downtown"},"geometry":{"type":"Point","coordinates":[-74.0060,40.7128]}},
      {"type":"Feature","properties":{"name":"Brooklyn"},"geometry":{"type":"Point","coordinates":[-73.9442,40.6782]}},
      {"type":"Feature","properties":{"name":"Queens"},"geometry":{"type":"Point","coordinates":[-73.7949,40.7282]}},
      {"type":"Feature","properties":{"name":"Jersey"},"geometry":{"type":"Point","coordinates":[-74.0721,40.7178]}}
    ]}
    """

    static let roads = """
    {"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"name":"Broadway"},"geometry":{"type":"LineString","coordinates":[[-74.0130,40.7020],[-73.9990,40.7220],[-73.9850,40.7480],[-73.9680,40.7790]]}},
      {"type":"Feature","properties":{"name":"FDR"},"geometry":{"type":"LineString","coordinates":[[-73.9760,40.7040],[-73.9720,40.7250],[-73.9600,40.7550],[-73.9440,40.7820]]}},
      {"type":"Feature","properties":{"name":"Cross"},"geometry":{"type":"LineString","coordinates":[[-74.0200,40.7400],[-73.9900,40.7400],[-73.9600,40.7400],[-73.9300,40.7400]]}}
    ]}
    """

    static let parks = """
    {"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"name":"Central Park"},"geometry":{"type":"Polygon","coordinates":[[[-73.9819,40.7681],[-73.9580,40.7681],[-73.9493,40.7968],[-73.9730,40.7968],[-73.9819,40.7681]]]}},
      {"type":"Feature","properties":{"name":"Prospect"},"geometry":{"type":"Polygon","coordinates":[[[-73.9695,40.6602],[-73.9620,40.6550],[-73.9680,40.6480],[-73.9770,40.6530],[-73.9695,40.6602]]]}}
    ]}
    """
}
