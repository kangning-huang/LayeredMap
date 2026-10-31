import SwiftUI

/// Geometry kind for legend swatches (shared habit with Alt A).
enum LayerGeometryKind: String, CaseIterable, Identifiable, Hashable {
    case point
    case line
    case fill

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .point: return "Points"
        case .line: return "Lines"
        case .fill: return "Areas"
        }
    }
}

struct LayerSymbology: Hashable {
    var color: Color
    var size: Double
    var opacity: Double
}

/// One data set on the map. Basemap is never modeled here.
struct MapLayer: Identifiable, Hashable {
    let id: UUID
    var name: String
    var visible: Bool
    var geometry: LayerGeometryKind
    var symbology: LayerSymbology
    /// Plain-language kind + size for the data card.
    var kindSummary: String
    var insightStubs: [String]

    init(
        id: UUID = UUID(),
        name: String,
        visible: Bool = true,
        geometry: LayerGeometryKind,
        symbology: LayerSymbology,
        kindSummary: String,
        insightStubs: [String] = []
    ) {
        self.id = id
        self.name = name
        self.visible = visible
        self.geometry = geometry
        self.symbology = symbology
        self.kindSummary = kindSummary
        self.insightStubs = insightStubs
    }
}

@MainActor
final class LayerStore: ObservableObject {
    @Published var layers: [MapLayer]

    init(layers: [MapLayer] = LayerStore.demoLayers) {
        self.layers = layers
    }

    /// First-run example data already on the map (Alt B build order step 1).
    static let demoLayers: [MapLayer] = [
        MapLayer(
            name: "Cities",
            geometry: .point,
            symbology: LayerSymbology(color: .blue, size: 8, opacity: 0.9),
            kindSummary: "Points · 5 places",
            insightStubs: [
                "Avg. temperature 27.2 °C",
                "Hottest: Lagos (32.4 °C)",
                "Coolest: London (18.5 °C)",
            ]
        ),
        MapLayer(
            name: "Roads",
            geometry: .line,
            symbology: LayerSymbology(color: Color(white: 0.45), size: 3, opacity: 1.0),
            kindSummary: "Lines · example",
            insightStubs: [
                "Demo roads (no geometry in this spike pass)",
                "Hide with the eye to declutter",
            ]
        ),
        MapLayer(
            name: "Parks",
            geometry: .fill,
            symbology: LayerSymbology(color: .green, size: 1, opacity: 0.45),
            kindSummary: "Areas · example",
            insightStubs: [
                "Demo parks (no geometry in this spike pass)",
                "Tap Change look later for Show by",
            ]
        ),
    ]

    var visibleCount: Int { layers.filter(\.visible).count }

    func move(from source: IndexSet, to destination: Int) {
        layers.move(fromOffsets: source, toOffset: destination)
    }

    func remove(id: UUID) {
        layers.removeAll { $0.id == id }
    }

    func setVisible(id: UUID, _ visible: Bool) {
        guard let index = layers.firstIndex(where: { $0.id == id }) else { return }
        layers[index].visible = visible
    }

    func layer(id: UUID) -> MapLayer? {
        layers.first { $0.id == id }
    }
}

/// Stub insight for a map tap (Alt B: tap → card).
struct FeatureInsight: Identifiable, Hashable {
    let id: UUID
    var title: String
    var subtitle: String
    var rows: [String]

    init(id: UUID = UUID(), title: String, subtitle: String, rows: [String]) {
        self.id = id
        self.title = title
        self.subtitle = subtitle
        self.rows = rows
    }

    static func stubNear(latitude: Double, longitude: Double) -> FeatureInsight {
        FeatureInsight(
            title: "District 7",
            subtitle: String(format: "Area · near %.2f, %.2f", latitude, longitude),
            rows: [
                "Avg. temperature 31.2 °C",
                "3rd hottest of 17",
                "+2.1 °C above the median",
            ]
        )
    }
}
