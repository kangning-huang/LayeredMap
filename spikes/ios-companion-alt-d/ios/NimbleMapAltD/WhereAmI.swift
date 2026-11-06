import CoreLocation
import Foundation

/// Where the phone is.
///
/// The crosshair button in earlier builds was labelled "Show all data" and fitted the camera to
/// the layers' bounds — a reasonable thing, wearing an icon that promises a different one. There
/// was no CoreLocation in the app at all: no `NSLocationWhenInUseUsageDescription`, no
/// `showsUserLocation`. So "locate me" could never have worked; it was always going to fly to the
/// data instead, which is exactly what it looked like it wasn't doing.
///
/// This asks properly, once, and reports what happened in words rather than leaving the button
/// looking broken. A refusal is a normal answer, not an error.
@MainActor
final class WhereAmI: NSObject, ObservableObject, CLLocationManagerDelegate {
    @Published private(set) var isLocating = false
    /// A plain sentence when it could not be done, for an alert.
    @Published var problem: String?
    /// Where to fly, once known. `CLLocationCoordinate2D` is not `Equatable`, so views watch
    /// `foundKey` instead — it changes once per answer, including two answers in the same place.
    @Published var found: CLLocationCoordinate2D?
    @Published private(set) var foundKey = 0

    private let manager = CLLocationManager()

    override init() {
        super.init()
        manager.delegate = self
        manager.desiredAccuracy = kCLLocationAccuracyHundredMeters
    }

    func locate() {
        guard !isLocating else { return }
        switch manager.authorizationStatus {
        case .notDetermined:
            isLocating = true
            manager.requestWhenInUseAuthorization()
        case .restricted, .denied:
            problem = "NimbleMap doesn't have permission to use your location. You can turn it on in Settings › Privacy › Location Services."
        case .authorizedWhenInUse, .authorizedAlways:
            isLocating = true
            manager.requestLocation()
        @unknown default:
            problem = "Your iPhone wouldn't say whether NimbleMap can use your location."
        }
    }

    nonisolated func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        Task { @MainActor in
            switch manager.authorizationStatus {
            case .authorizedWhenInUse, .authorizedAlways:
                // The permission sheet is how a first tap gets answered; carry on with it.
                if isLocating { manager.requestLocation() }
            case .denied, .restricted:
                isLocating = false
                problem = "NimbleMap doesn't have permission to use your location."
            default:
                break
            }
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        let last = locations.last?.coordinate
        Task { @MainActor in
            isLocating = false
            guard let last else {
                problem = "Your iPhone couldn't work out where you are just now."
                return
            }
            found = last
            foundKey &+= 1
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
        Task { @MainActor in
            isLocating = false
            // On the simulator with no location set this is the usual answer, and it is not a bug.
            problem = "Your iPhone couldn't work out where you are just now."
        }
    }
}
