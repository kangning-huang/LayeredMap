import SwiftUI

/// A idle (map-first floating pills) → B Layers sheet → C symbology. See docs/lm/ios-layers-symbology.md.
struct ContentView: View {
    @StateObject private var layerStore = LayerStore()
    @State private var showOpenURL = false
    @State private var showLayers = false
    @State private var statusText = "Paste a dataset URL to open"
    @State private var overlayGeoJSON: String?
    @State private var isBusy = false

    var body: some View {
        ZStack(alignment: .bottom) {
            MapScreen(overlayGeoJSON: overlayGeoJSON)
                .ignoresSafeArea()

            VStack(spacing: 0) {
                topChrome
                Spacer()
                bottomChrome
            }
        }
        .sheet(isPresented: $showOpenURL) {
            OpenURLSheet { url in
                openURL(url)
            }
        }
        .sheet(isPresented: $showLayers) {
            LayersSheet(store: layerStore)
        }
    }

    /// State A: floating glass pills only. Layers opens B; Ask stays a pill/composer, not the layers sheet.
    private var topChrome: some View {
        HStack {
            glassPillButton(title: "Open", systemImage: "link") {
                showOpenURL = true
            }

            Spacer()

            FpsOverlay()

            if isBusy {
                ProgressView()
                    .controlSize(.small)
                    .padding(8)
                    .background(.ultraThinMaterial, in: Capsule())
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 12)
    }

    private var bottomChrome: some View {
        VStack(spacing: 10) {
            HStack(alignment: .bottom, spacing: 10) {
                glassPillButton(title: "Layers", systemImage: "square.3.layers.3d") {
                    showLayers = true
                }

                AskBar()
            }
            .padding(.horizontal, 12)

            Text(statusText)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 16)
                .padding(.bottom, 10)
                .background(.ultraThinMaterial)
        }
    }

    private func glassPillButton(title: String, systemImage: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(title, systemImage: systemImage)
                .font(.subheadline.weight(.semibold))
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
                .background(.ultraThinMaterial, in: Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
    }

    private func openURL(_ raw: String) {
        guard let url = URL(string: raw.trimmingCharacters(in: .whitespacesAndNewlines)),
              let scheme = url.scheme?.lowercased(),
              scheme == "http" || scheme == "https"
        else {
            statusText = "Need an http(s) URL"
            return
        }

        isBusy = true
        statusText = "Downloading…"
        overlayGeoJSON = nil

        Task.detached(priority: .userInitiated) {
            do {
                let (data, response) = try await URLSession.shared.data(from: url)
                if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
                    await MainActor.run {
                        isBusy = false
                        statusText = "Download failed (HTTP \(http.statusCode))"
                    }
                    return
                }

                let dir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first
                    ?? FileManager.default.temporaryDirectory
                let file = dir.appendingPathComponent("spike-download-\(UUID().uuidString).geojson")
                try data.write(to: file, options: .atomic)

                let decoded = DecodeBridge.decodeGeoJSONFile(at: file.path)
                await MainActor.run {
                    isBusy = false
                    switch decoded {
                    case .success(let result):
                        overlayGeoJSON = result.json
                        statusText = "Opened \(result.featureCount) feature(s)"
                    case .failure(let err):
                        statusText = "Decode: \(err)"
                    }
                }
            } catch {
                await MainActor.run {
                    isBusy = false
                    statusText = "Download error: \(error.localizedDescription)"
                }
            }
        }
    }
}
