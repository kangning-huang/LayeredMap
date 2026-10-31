import SwiftUI

/// Map tap → insight card (Apple Maps–style). Stub rows for the Alt B compare.
struct FeatureInsightCard: View {
    var insight: FeatureInsight
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(insight.title)
                            .font(.title2.weight(.semibold))
                        Text(insight.subtitle)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 4)
                }

                Section("Insights") {
                    ForEach(insight.rows, id: \.self) { row in
                        Text(row)
                    }
                }

                Section {
                    HStack(spacing: 10) {
                        actionChip("Summarize heat")
                        actionChip("Compare")
                        actionChip("Measure")
                    }
                    .listRowBackground(Color.clear)
                }

                Section {
                    Text("Stub card for Alternative B. Real feature hit-test and desktop object-card insights come later.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Place")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
    }

    private func actionChip(_ title: String) -> some View {
        Text(title)
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color.accentColor.opacity(0.12), in: Capsule())
            .foregroundStyle(Color.accentColor)
            .accessibilityLabel(title)
    }
}
