import SwiftUI
import QuartzCore

/// Spike-only CADisplayLink FPS badge. Measures display refresh delivery to UIKit/SwiftUI, not MapLibre GPU fill rate.
final class FpsSampler: ObservableObject {
    @Published var fps: Double = 0
    private var link: CADisplayLink?
    private var frames = 0
    private var lastTimestamp: CFTimeInterval = 0

    func start() {
        guard link == nil else { return }
        let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common)
        self.link = link
    }

    func stop() {
        link?.invalidate()
        link = nil
    }

    @objc private func tick(_ link: CADisplayLink) {
        if lastTimestamp == 0 {
            lastTimestamp = link.timestamp
            return
        }
        frames += 1
        let elapsed = link.timestamp - lastTimestamp
        if elapsed >= 0.5 {
            fps = Double(frames) / elapsed
            frames = 0
            lastTimestamp = link.timestamp
        }
    }
}

struct FpsOverlay: View {
    @StateObject private var sampler = FpsSampler()

    var body: some View {
        Text(String(format: "%.0f FPS", sampler.fps))
            .font(.caption.monospacedDigit().weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(.black.opacity(0.55), in: Capsule())
            .foregroundStyle(.white)
            .onAppear { sampler.start() }
            .onDisappear { sampler.stop() }
            .accessibilityLabel("Frames per second")
    }
}
