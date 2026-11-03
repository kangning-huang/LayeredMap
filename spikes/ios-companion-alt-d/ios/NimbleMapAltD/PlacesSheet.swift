import MapKit
import SwiftUI

/// Find a place and go there.
///
/// Two things were being asked of one search box before this: "what data can I put on this map"
/// and "take me to Lisbon". They are different questions with different answers, so they are now
/// different buttons — Add data, and this.
///
/// It uses `MKLocalSearchCompleter` and `MKLocalSearch`, which are Apple's and need no key, no
/// account and no network budget of ours. Nothing is added to the map: this only moves the camera.
struct PlacesSheet: View {
    var onGo: (MKMapItem) -> Void

    @Environment(\.dismiss) private var dismiss
    @StateObject private var finder = PlaceFinder()
    @State private var text = ""

    var body: some View {
        NavigationStack {
            List {
                if text.isEmpty {
                    Section {
                        Text("Type a town, a street, a country or a landmark. This moves the map; it doesn't add anything to it.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                } else if finder.results.isEmpty && !finder.searching {
                    Section {
                        Text("No place called “\(text)”.")
                            .font(.body.weight(.semibold))
                    }
                }
                ForEach(finder.results, id: \.self) { completion in
                    Button {
                        finder.resolve(completion) { item in
                            if let item {
                                onGo(item)
                                dismiss()
                            }
                        }
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(completion.title).foregroundStyle(.primary)
                            if !completion.subtitle.isEmpty {
                                Text(completion.subtitle).font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Search places")
            .navigationBarTitleDisplayMode(.inline)
            .searchable(text: $text, placement: .navigationBarDrawer(displayMode: .always),
                        prompt: "Town, street, country or landmark")
            .onChange(of: text) { _, q in finder.ask(q) }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            }
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
    }
}

/// Apple's place completions, and turning one into a coordinate.
@MainActor
final class PlaceFinder: NSObject, ObservableObject, MKLocalSearchCompleterDelegate {
    @Published private(set) var results: [MKLocalSearchCompletion] = []
    @Published private(set) var searching = false

    private let completer = MKLocalSearchCompleter()

    override init() {
        super.init()
        completer.delegate = self
        completer.resultTypes = [.address, .pointOfInterest, .query]
    }

    func ask(_ text: String) {
        let q = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard q.count >= 2 else {
            results = []
            searching = false
            return
        }
        searching = true
        completer.queryFragment = q
    }

    /// A completion is only words; this turns the chosen one into somewhere on the globe.
    func resolve(_ completion: MKLocalSearchCompletion, then: @escaping (MKMapItem?) -> Void) {
        let request = MKLocalSearch.Request(completion: completion)
        MKLocalSearch(request: request).start { response, _ in
            Task { @MainActor in then(response?.mapItems.first) }
        }
    }

    nonisolated func completerDidUpdateResults(_ completer: MKLocalSearchCompleter) {
        let found = completer.results
        Task { @MainActor in
            results = found
            searching = false
        }
    }

    nonisolated func completer(_ completer: MKLocalSearchCompleter, didFailWithError error: Error) {
        Task { @MainActor in
            results = []
            searching = false
        }
    }
}
