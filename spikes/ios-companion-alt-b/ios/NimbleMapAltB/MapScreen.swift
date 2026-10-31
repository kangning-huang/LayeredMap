import SwiftUI
import MapLibre
import CoreLocation

/// Native MapLibre Metal map. Tap reports a coordinate for the Alt B insight card stub.
struct MapScreen: UIViewRepresentable {
    var overlayGeoJSON: String?
    var onMapTap: ((CLLocationCoordinate2D) -> Void)?

    func makeUIView(context: Context) -> MLNMapView {
        let map = MLNMapView(frame: .zero)
        map.styleURL = URL(string: "https://demotiles.maplibre.org/style.json")
        map.logoView.isHidden = false
        map.attributionButton.isHidden = false
        map.allowsRotating = true
        map.allowsTilting = true
        map.setCenter(CLLocationCoordinate2D(latitude: 20, longitude: 20), zoomLevel: 1.4, animated: false)
        map.delegate = context.coordinator

        let tap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleTap(_:)))
        tap.numberOfTapsRequired = 1
        tap.cancelsTouchesInView = false
        map.addGestureRecognizer(tap)
        context.coordinator.tapRecognizer = tap
        context.coordinator.onMapTap = onMapTap

        return map
    }

    func updateUIView(_ map: MLNMapView, context: Context) {
        context.coordinator.onMapTap = onMapTap
        guard let overlayGeoJSON, overlayGeoJSON != context.coordinator.lastOverlay else { return }
        context.coordinator.lastOverlay = overlayGeoJSON
        context.coordinator.applyOverlay(overlayGeoJSON, on: map)
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    final class Coordinator: NSObject, MLNMapViewDelegate {
        var lastOverlay: String?
        var onMapTap: ((CLLocationCoordinate2D) -> Void)?
        var tapRecognizer: UITapGestureRecognizer?
        private let sourceId = "altb-overlay"
        private let circleLayerId = "altb-overlay-circle"
        private let lineLayerId = "altb-overlay-line"
        private let fillLayerId = "altb-overlay-fill"

        func mapView(_ mapView: MLNMapView, didFinishLoading style: MLNStyle) {
            if let lastOverlay {
                applyOverlay(lastOverlay, on: mapView)
            }
        }

        @objc func handleTap(_ gesture: UITapGestureRecognizer) {
            guard let map = gesture.view as? MLNMapView else { return }
            let point = gesture.location(in: map)
            let coordinate = map.convert(point, toCoordinateFrom: map)
            onMapTap?(coordinate)
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
