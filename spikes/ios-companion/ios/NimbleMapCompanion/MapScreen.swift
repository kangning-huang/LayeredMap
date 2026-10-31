import SwiftUI
import MapLibre

/// Native MapLibre Metal map. Pan/pinch stay on the map view; overlays update without blocking gestures.
///
/// Symbology chrome (A→B→C) drives `LayerStore` + legend swatches in SwiftUI. Wiring those styles into
/// MapLibre paint properties per data layer is intentionally stubbed in this pass — keep chrome
/// exercisable with demo layers without blocking on style-paint plumbing.
struct MapScreen: UIViewRepresentable {
    var overlayGeoJSON: String?

    func makeUIView(context: Context) -> MLNMapView {
        let map = MLNMapView(frame: .zero)
        // Public demo tiles; replace with NimbleMap basemap hosting later.
        map.styleURL = URL(string: "https://demotiles.maplibre.org/style.json")
        map.logoView.isHidden = false
        map.attributionButton.isHidden = false
        map.allowsRotating = true
        map.allowsTilting = true
        map.setCenter(CLLocationCoordinate2D(latitude: 20, longitude: 0), zoomLevel: 1.5, animated: false)
        map.delegate = context.coordinator
        return map
    }

    func updateUIView(_ map: MLNMapView, context: Context) {
        guard let overlayGeoJSON, overlayGeoJSON != context.coordinator.lastOverlay else { return }
        context.coordinator.lastOverlay = overlayGeoJSON
        context.coordinator.applyOverlay(overlayGeoJSON, on: map)
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    final class Coordinator: NSObject, MLNMapViewDelegate {
        var lastOverlay: String?
        private let sourceId = "spike-overlay"
        private let circleLayerId = "spike-overlay-circle"
        private let lineLayerId = "spike-overlay-line"
        private let fillLayerId = "spike-overlay-fill"

        func mapView(_ mapView: MLNMapView, didFinishLoading style: MLNStyle) {
            if let lastOverlay {
                applyOverlay(lastOverlay, on: mapView)
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
