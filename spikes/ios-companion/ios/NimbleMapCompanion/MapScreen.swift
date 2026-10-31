import SwiftUI
import MapLibre
import UIKit

/// Native MapLibre Metal map. Pan/pinch stay on the map view; overlays update without blocking gestures.
///
/// **Alt A:** demo data-layer GeoJSON + `LayerStore` symbology drive MapLibre paint (color / size /
/// opacity / visibility). URL-opened overlays still use the separate `spike-overlay` source.
struct MapScreen: UIViewRepresentable {
    var overlayGeoJSON: String?
    var layers: [MapLayer]

    func makeUIView(context: Context) -> MLNMapView {
        let map = MLNMapView(frame: .zero)
        // Public demo tiles; replace with NimbleMap basemap hosting later.
        map.styleURL = URL(string: "https://demotiles.maplibre.org/style.json")
        map.logoView.isHidden = false
        map.attributionButton.isHidden = false
        map.allowsRotating = true
        map.allowsTilting = true
        // Frame demo Cities / Roads / Parks around NYC.
        map.setCenter(
            CLLocationCoordinate2D(latitude: 40.74, longitude: -73.98),
            zoomLevel: 11,
            animated: false
        )
        map.delegate = context.coordinator
        return map
    }

    func updateUIView(_ map: MLNMapView, context: Context) {
        context.coordinator.syncDataLayers(layers, on: map)

        guard let overlayGeoJSON, overlayGeoJSON != context.coordinator.lastOverlay else { return }
        context.coordinator.lastOverlay = overlayGeoJSON
        context.coordinator.applyOverlay(overlayGeoJSON, on: map)
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    final class Coordinator: NSObject, MLNMapViewDelegate {
        var lastOverlay: String?
        var lastLayers: [MapLayer] = []
        private var knownStyleIds = Set<String>()
        private let sourceId = "spike-overlay"
        private let circleLayerId = "spike-overlay-circle"
        private let lineLayerId = "spike-overlay-line"
        private let fillLayerId = "spike-overlay-fill"

        func mapView(_ mapView: MLNMapView, didFinishLoading style: MLNStyle) {
            knownStyleIds.removeAll()
            if !lastLayers.isEmpty {
                syncDataLayers(lastLayers, on: mapView)
            }
            if let lastOverlay {
                applyOverlay(lastOverlay, on: mapView)
            }
        }

        func syncDataLayers(_ layers: [MapLayer], on map: MLNMapView) {
            lastLayers = layers
            guard let style = map.style else { return }

            let activeIds = Set(layers.map(\.styleId))
            for stale in knownStyleIds.subtracting(activeIds) {
                removeLayer(styleId: stale, from: style)
            }
            knownStyleIds = activeIds

            // Bottom of list draws under top; MapLibre add order: add bottom-first.
            for layer in layers.reversed() {
                upsert(layer, on: style)
            }
        }

        private func removeLayer(styleId: String, from style: MLNStyle) {
            for suffix in ["-fill", "-line", "-circle"] {
                if let layer = style.layer(withIdentifier: styleId + suffix) {
                    style.removeLayer(layer)
                }
            }
            if let source = style.source(withIdentifier: styleId) {
                style.removeSource(source)
            }
        }

        private func upsert(_ layer: MapLayer, on style: MLNStyle) {
            guard let data = layer.demoGeoJSON.data(using: .utf8),
                  let shape = try? MLNShape(data: data, encoding: String.Encoding.utf8.rawValue)
            else { return }

            let source: MLNShapeSource
            if let existing = style.source(withIdentifier: layer.styleId) as? MLNShapeSource {
                existing.shape = shape
                source = existing
            } else {
                source = MLNShapeSource(identifier: layer.styleId, shape: shape, options: nil)
                style.addSource(source)
            }

            let uiColor = UIColor(layer.symbology.color)
            let opacity = NSNumber(value: layer.symbology.opacity)
            let size = NSNumber(value: layer.symbology.size)
            let visible = layer.visible

            switch layer.geometry {
            case .point:
                let id = layer.styleId + "-circle"
                let circle: MLNCircleStyleLayer
                if let existing = style.layer(withIdentifier: id) as? MLNCircleStyleLayer {
                    circle = existing
                } else {
                    circle = MLNCircleStyleLayer(identifier: id, source: source)
                    style.addLayer(circle)
                }
                circle.circleColor = NSExpression(forConstantValue: uiColor)
                circle.circleRadius = NSExpression(forConstantValue: size)
                circle.circleOpacity = NSExpression(forConstantValue: opacity)
                circle.isVisible = visible
            case .line:
                let id = layer.styleId + "-line"
                let line: MLNLineStyleLayer
                if let existing = style.layer(withIdentifier: id) as? MLNLineStyleLayer {
                    line = existing
                } else {
                    line = MLNLineStyleLayer(identifier: id, source: source)
                    style.addLayer(line)
                }
                line.lineColor = NSExpression(forConstantValue: uiColor)
                line.lineWidth = NSExpression(forConstantValue: size)
                line.lineOpacity = NSExpression(forConstantValue: opacity)
                line.isVisible = visible
            case .fill:
                let fillId = layer.styleId + "-fill"
                let outlineId = layer.styleId + "-line"
                let fill: MLNFillStyleLayer
                if let existing = style.layer(withIdentifier: fillId) as? MLNFillStyleLayer {
                    fill = existing
                } else {
                    fill = MLNFillStyleLayer(identifier: fillId, source: source)
                    style.addLayer(fill)
                }
                fill.fillColor = NSExpression(
                    forConstantValue: uiColor.withAlphaComponent(CGFloat(layer.symbology.opacity))
                )
                fill.fillOpacity = NSExpression(forConstantValue: opacity)
                fill.isVisible = visible

                let outline: MLNLineStyleLayer
                if let existing = style.layer(withIdentifier: outlineId) as? MLNLineStyleLayer {
                    outline = existing
                } else {
                    outline = MLNLineStyleLayer(identifier: outlineId, source: source)
                    style.addLayer(outline)
                }
                outline.lineColor = NSExpression(forConstantValue: uiColor)
                outline.lineWidth = NSExpression(forConstantValue: size)
                outline.lineOpacity = NSExpression(forConstantValue: NSNumber(value: 1.0))
                outline.isVisible = visible
            }
        }

        func applyOverlay(_ geojson: String, on map: MLNMapView) {
            guard let style = map.style,
                  let data = geojson.data(using: .utf8),
                  let shape = try? MLNShape(data: data, encoding: String.Encoding.utf8.rawValue)
            else { return }

            if let existing = style.source(withIdentifier: sourceId) as? MLNShapeSource {
                existing.shape = shape
            } else {
                let source = MLNShapeSource(identifier: sourceId, shape: shape, options: nil)
                style.addSource(source)

                let fill = MLNFillStyleLayer(identifier: fillLayerId, source: source)
                fill.fillColor = NSExpression(forConstantValue: UIColor.systemBlue.withAlphaComponent(0.25))
                fill.fillOutlineColor = NSExpression(forConstantValue: UIColor.systemBlue)
                style.addLayer(fill)

                let line = MLNLineStyleLayer(identifier: lineLayerId, source: source)
                line.lineColor = NSExpression(forConstantValue: UIColor.systemBlue)
                line.lineWidth = NSExpression(forConstantValue: NSNumber(value: 2))
                style.addLayer(line)

                let circle = MLNCircleStyleLayer(identifier: circleLayerId, source: source)
                circle.circleColor = NSExpression(forConstantValue: UIColor.systemBlue)
                circle.circleRadius = NSExpression(forConstantValue: NSNumber(value: 6))
                circle.circleOpacity = NSExpression(forConstantValue: NSNumber(value: 0.9))
                style.addLayer(circle)
            }
        }
    }
}
