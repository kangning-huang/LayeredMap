import SwiftUI

/// Stub Ask composer. Non-functional in this spike; layout only.
struct AskBar: View {
    @State private var draft = ""

    var body: some View {
        HStack(spacing: 10) {
            TextField("Ask about these layers…", text: $draft)
                .textFieldStyle(.plain)
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
                .background(.regularMaterial, in: Capsule())

            Button {
                // Stub: no assistant wiring in this spike.
            } label: {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.title)
                    .symbolRenderingMode(.hierarchical)
            }
            .disabled(true)
            .accessibilityLabel("Send (stub)")
        }
        .padding(.horizontal, 4)
    }
}
