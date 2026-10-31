import SwiftUI

/// Lean data card for a sheet row tap. Insight stubs first; Change look is a stub action.
struct DataCardView: View {
    var layer: MapLayer
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section {
                    HStack(spacing: 12) {
                        LegendSwatch(geometry: layer.geometry, symbology: layer.symbology)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(layer.name)
                                .font(.title3.weight(.semibold))
                            Text(layer.kindSummary)
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .padding(.vertical, 4)
                }

                Section("Insights") {
                    if layer.insightStubs.isEmpty {
                        Text("No insights yet")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(layer.insightStubs, id: \.self) { row in
                            Text(row)
                        }
                    }
                }

                Section("Actions") {
                    Button("Change look") {
                        // Stub: Photos-style Show by comes in a later Alt B step.
                    }
                    .disabled(true)
                    Button("View as list") {}
                        .disabled(true)
                    Button("Summarize") {}
                        .disabled(true)
                }

                Section {
                    Text("This spike: card + insight stubs only. Full Change look and actions ship later.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Data")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
    }
}
