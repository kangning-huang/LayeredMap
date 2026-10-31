import SwiftUI

struct OpenURLSheet: View {
    @Environment(\.dismiss) private var dismiss
    @State private var urlText = "https://d2ad6b4ur7yvpq.cloudfront.net/naturalearth-3.3.0/ne_110m_populated_places_simple.geojson"
    var onOpen: (String) -> Void

    var body: some View {
        NavigationStack {
            Form {
                Section("Dataset URL") {
                    TextField("https://…", text: $urlText, axis: .vertical)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                }
                Section {
                    Text("v1 spike: GeoJSON over HTTPS. Download runs off the main thread; Rust decodes in the sandbox.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Open URL")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Open") {
                        onOpen(urlText)
                        dismiss()
                    }
                    .disabled(urlText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
