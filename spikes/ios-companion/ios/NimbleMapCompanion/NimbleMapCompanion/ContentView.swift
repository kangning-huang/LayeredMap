import SwiftUI

/// Full-screen map + stub Ask bar + Open URL sheet. Butter-smooth shell: UI never waits on decode.
struct ContentView: View {
    @State private var showOpenURL = false
    @State private var statusText = "Paste a dataset URL to open"
    @State private var overlayGeoJSON: String?
    @State private var isBusy = false

    var body: some View {
        ZStack(alignment: .bottom) {
            MapScreen(overlayGeoJSON: overlayGeoJSON)
                .ignoresSafeArea()

            VStack(spacing: 0) {
                HStack {
                    Button {
                        showOpenURL = true
                    } label: {
                        Label("Open URL", systemImage: "link")
                            .font(.subheadline.weight(.semibold))
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)

                    Spacer()

                    if isBusy {
                        ProgressView()
                            .controlSize(.small)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.top, 12)

                Spacer()

                AskBar()
                    .padding(.horizontal, 12)
                    .padding(.bottom, 8)

                Text(statusText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 10)
                    .background(.ultraThinMaterial)
            }
        }
        .sheet(isPresented: $showOpenURL) {
            OpenURLSheet { url in
                openURL(url)
            }
        }
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
