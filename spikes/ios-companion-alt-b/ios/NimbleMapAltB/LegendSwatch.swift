import SwiftUI

/// Live legend sample for a data-layer row in B. Matches current symbology (not type glyphs).
struct LegendSwatch: View {
    var geometry: LayerGeometryKind
    var symbology: LayerSymbology

    private let box: CGFloat = 28

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(Color(.secondarySystemFill))
            sample
                .opacity(symbology.opacity)
        }
        .frame(width: box, height: box)
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private var sample: some View {
        switch geometry {
        case .point:
            Circle()
                .fill(symbology.color)
                .frame(width: min(symbology.size + 4, 18), height: min(symbology.size + 4, 18))
        case .line:
            Capsule()
                .fill(symbology.color)
                .frame(width: 18, height: max(2, min(symbology.size, 6)))
        case .fill:
            RoundedRectangle(cornerRadius: 3, style: .continuous)
                .fill(symbology.color)
                .frame(width: 16, height: 16)
                .overlay(
                    RoundedRectangle(cornerRadius: 3, style: .continuous)
                        .strokeBorder(symbology.color.opacity(1), lineWidth: max(1, min(symbology.size, 2)))
                )
        }
    }
}
