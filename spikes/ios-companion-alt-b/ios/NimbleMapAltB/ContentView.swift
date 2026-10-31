import SwiftUI
/// Alternative B home: full map + one persistent sheet. See docs/lm/ios-redesign.md.
struct ContentView: View {
    @StateObject private var layerStore = LayerStore()
    @State private var sheetDetent: PresentationDetent = .height(Self.smallDetentHeight)
    @State private var searchText = ""
    @State private var showAddData = false
    @State private var selectedLayerID: UUID?
    @State private var featureInsight: FeatureInsight?
    @State private var showFirstRunHint = true
    @State private var overlayGeoJSON: String? = DemoData.worldCitiesGeoJSON
    @State private var mapName = DemoData.mapName
    @State private var sheetPresented = true

    private static let smallDetentHeight: CGFloat = 136

    private var smallDetent: PresentationDetent { .height(Self.smallDetentHeight) }

    var body: some View {
        ZStack(alignment: .top) {
            MapScreen(
                overlayGeoJSON: overlayGeoJSON,
                onMapTap: { coordinate in
                    showFirstRunHint = false
                    featureInsight = FeatureInsight.stubNear(
                        latitude: coordinate.latitude,
                        longitude: coordinate.longitude
                    )
                }
            )
            .ignoresSafeArea()

            topTrailingChrome

            if showFirstRunHint {
                firstRunHint
                    .padding(.top, 72)
            }
        }
        .sheet(isPresented: $sheetPresented) {
            PersistentMapSheet(
                mapName: mapName,
                searchText: $searchText,
                store: layerStore,
                onAdd: { showAddData = true },
                onSelectLayer: { id in
                    selectedLayerID = id
                }
            )
            .presentationDetents(
                [smallDetent, .medium, .large],
                selection: $sheetDetent
            )
            .presentationBackgroundInteraction(.enabled(upThrough: .medium))
            .interactiveDismissDisabled()
            .presentationDragIndicator(.visible)
            .presentationCornerRadius(16)
        }
        .sheet(item: layerSelection) { layer in
            DataCardView(layer: layer)
                .presentationDetents([.medium, .large])
                .presentationDragIndicator(.visible)
        }
        .sheet(item: $featureInsight) { insight in
            FeatureInsightCard(insight: insight)
                .presentationDetents([.medium, .large])
                .presentationDragIndicator(.visible)
        }
        .sheet(isPresented: $showAddData) {
            AddDataSheet { url in
                openURL(url)
            }
        }
        .onAppear {
            // Ensure the persistent sheet is up; constant binding would also work.
            sheetPresented = true
            sheetDetent = .medium
        }
    }

    private var layerSelection: Binding<MapLayer?> {
        Binding(
            get: {
                guard let selectedLayerID else { return nil }
                return layerStore.layer(id: selectedLayerID)
            },
            set: { newValue in
                selectedLayerID = newValue?.id
            }
        )
    }

    private var topTrailingChrome: some View {
        HStack {
            Spacer()
            VStack(alignment: .trailing, spacing: 8) {
                FpsOverlay()
                Button {
                    // Stub: map style chooser not in this spike pass.
                } label: {
                    Image(systemName: "map")
                        .font(.body.weight(.semibold))
                        .frame(width: 40, height: 40)
                        .background(.ultraThinMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Map style")

                Button {
                    // Stub: my location.
                } label: {
                    Image(systemName: "location")
                        .font(.body.weight(.semibold))
                        .frame(width: 40, height: 40)
                        .background(.ultraThinMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("My location")
            }
            .padding(.trailing, 12)
            .padding(.top, 12)
        }
    }

    private var firstRunHint: some View {
        Text("Tap any area to learn")
            .font(.subheadline.weight(.semibold))
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(.ultraThinMaterial, in: Capsule())
            .accessibilityLabel("Tap any area to learn about it")
    }

    private func openURL(_ raw: String) {
        guard let url = URL(string: raw.trimmingCharacters(in: .whitespacesAndNewlines)),
              let scheme = url.scheme?.lowercased(),
              scheme == "http" || scheme == "https"
        else { return }

        Task.detached(priority: .userInitiated) {
            do {
                let (data, response) = try await URLSession.shared.data(from: url)
                if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
                    return
                }
                let dir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first
                    ?? FileManager.default.temporaryDirectory
                let file = dir.appendingPathComponent("altb-download-\(UUID().uuidString).geojson")
                try data.write(to: file, options: .atomic)
                let decoded = DecodeBridge.decodeGeoJSONFile(at: file.path)
                await MainActor.run {
                    if case .success(let result) = decoded {
                        overlayGeoJSON = result.json
                        showFirstRunHint = false
                        sheetDetent = .medium
                    }
                }
            } catch {
                // Spike: errors surface later as toasts; keep silent here.
            }
        }
    }
}
