import SwiftUI
import CoreLocation

/// **Alternative B** home: full map + one persistent sheet (docs/lm/ios-redesign.md).
/// Tap anything → card; "+" adds data; Change look is Photos-style. Bundle label: NimbleMap B.
struct ContentView: View {
    @StateObject private var doc = MapDocument()
    @State private var mode: SheetMode = .home
    @State private var detent: PresentationDetent = SheetMode.small
    @State private var showHint = true
    @State private var basemap: Basemap = .light
    @State private var pin: CLLocationCoordinate2D?
    @State private var pinHits: [MapHit] = []
    @State private var overlap: [MapHit] = []
    @State private var lookOriginal: LayerStyle?
    @State private var sceneRun = false

    var body: some View {
        ZStack(alignment: .top) {
            MapScreen(
                layers: doc.layers,
                camera: doc.camera,
                highlight: doc.outline(highlightHit),
                basemap: basemap,
                insets: mapInsets,
                onTap: handleTap,
                onLongPress: { coord, hits in
                    showHint = false
                    pin = coord
                    pinHits = hits
                    go(.pin, .medium)
                },
                pin: pin
            )
            .ignoresSafeArea()

            if case .look = mode {
                lookTopBar
            } else {
                topTrailingChrome
            }

            VStack(spacing: 8) {
                if showHint && !doc.isEmpty {
                    hint
                }
                if let toast = doc.toast {
                    ToastView(toast: toast, onUndo: { doc.undo() })
                        .task(id: toast.id) {
                            try? await Task.sleep(nanoseconds: 4_000_000_000)
                            if doc.toast?.id == toast.id { doc.toast = nil }
                        }
                }
                if let busy = doc.busy {
                    Label(busy, systemImage: "arrow.down.circle")
                        .font(.subheadline.weight(.semibold))
                        .padding(.horizontal, 14)
                        .padding(.vertical, 9)
                        .background(.regularMaterial, in: Capsule())
                }
            }
            .padding(.top, 64)
            .animation(.snappy, value: doc.toast)
        }
        .sheet(isPresented: .constant(true)) {
            sheetContent
                .presentationDetents(mode.detents, selection: $detent)
                .presentationBackgroundInteraction(.enabled(upThrough: .medium))
                .interactiveDismissDisabled()
                .presentationDragIndicator(.visible)
                .presentationCornerRadius(18)
                .confirmationDialog("\(overlap.count) things here", isPresented: overlapBinding, titleVisibility: .visible) {
                    ForEach(overlap, id: \.self) { hit in
                        if case let (layer, f)? = doc.feature(hit) {
                            Button("\(layer.dataset.title(of: f)) · \(layer.name)") { openCard(hit) }
                        }
                    }
                }
                .alert("Couldn't add this", isPresented: errorBinding) {
                    Button("OK", role: .cancel) { doc.error = nil }
                } message: {
                    Text(doc.error ?? "")
                }
                .onOpenURL { url in
                    go(.home, SheetMode.small)
                    doc.importing(Importer.displayName(for: url)) { await Importer.openExternal(url) }
                }
        }
        .task {
            // First run: example data already on the map, and one hint (no welcome card).
            guard doc.isEmpty, let countries = Library.items.first else { return }
            switch await Importer.openBundled(countries) {
            case .success(let ds):
                doc.add(ds, showBy: countries.showBy)
                runSceneIfAsked()
            case .failure(let e):
                doc.error = e.message
            }
        }
    }

    // MARK: Sheet

    @ViewBuilder
    private var sheetContent: some View {
        switch mode {
        case .home:
            HomeSheet(
                doc: doc,
                mapName: mapName,
                onAdd: { go(.add, .large) },
                onOpenLayer: { go(.data($0), .medium) },
                onOpenFeature: { hit in
                    openCard(hit)
                    if case let (_, f)? = doc.feature(hit) { doc.fly(to: f.bounds, pointZoom: 5) }
                },
                onAddLibrary: addLibrary,
                onSearchFocus: { if detent == SheetMode.small { detent = .large } }
            )
        case .feature(let hit):
            if case let (layer, f)? = doc.feature(hit) {
                FeatureCard(
                    insight: Insights.feature(f, in: layer.dataset, layerID: layer.id),
                    layer: layer,
                    onClose: { go(.home, SheetMode.small) },
                    onColorBy: { field in startLook(layer.id, showBy: field) },
                    onZoom: { doc.fly(to: f.bounds, pointZoom: 6) },
                    onList: { go(.list(layer.id), .large) }
                )
            } else {
                Color.clear.onAppear { go(.home, SheetMode.small) }
            }
        case .data(let id):
            if let layer = doc.layer(id) {
                DataCard(
                    layer: layer,
                    onClose: { go(.home, .medium) },
                    onChangeLook: { startLook(id, showBy: nil) },
                    onList: { go(.list(id), .large) },
                    onZoom: { doc.fly(to: layer.dataset.bounds) }
                )
            } else {
                Color.clear.onAppear { go(.home, SheetMode.small) }
            }
        case .look(let id):
            if let layer = doc.layer(id) {
                ChangeLookStrip(
                    layer: layer,
                    onPick: { showBy in
                        var s = layer.style
                        s.showBy = showBy
                        doc.setStyle(id, s)
                    },
                    onOpacity: { o in
                        var s = layer.style
                        s.opacity = o
                        doc.setStyle(id, s)
                    }
                )
            }
        case .list(let id):
            if let layer = doc.layer(id) {
                FeatureList(
                    layer: layer,
                    onClose: { go(.data(id), .medium) },
                    onPick: { idx in
                        let hit = MapHit(layerID: id, featureIndex: idx)
                        openCard(hit)
                        if case let (_, f)? = doc.feature(hit) { doc.fly(to: f.bounds, pointZoom: 5) }
                    }
                )
            }
        case .add:
            AddDataSheet(
                doc: doc,
                onClose: { go(.home, SheetMode.small) },
                onAddLibrary: addLibrary,
                onStarted: { go(.home, SheetMode.small) }
            )
        case .pin:
            PinCard(
                doc: doc,
                coordinate: pin,
                hits: pinHits,
                onClose: {
                    pin = nil
                    go(.home, SheetMode.small)
                },
                onOpen: openCard
            )
        }
    }

    private var mapName: String {
        let names = doc.layers.map(\.name)
        guard let first = names.last else { return "New map" }
        return names.count == 1 ? first : "\(first) and more"
    }

    // MARK: Actions

    private func go(_ newMode: SheetMode, _ newDetent: PresentationDetent) {
        if case .look = mode, lookOriginal != nil, newMode != mode {
            lookOriginal = nil
        }
        withAnimation(.snappy) {
            mode = newMode
            detent = newMode.detents.contains(newDetent) ? newDetent : (newMode.detents.first ?? SheetMode.small)
        }
    }

    private func openCard(_ hit: MapHit) {
        showHint = false
        pin = nil
        go(.feature(hit), .medium)
    }

    private func handleTap(_ hits: [MapHit], _ point: CGPoint) {
        showHint = false
        if case .look = mode { return }
        if hits.count > 1 {
            overlap = hits
        } else if let hit = hits.first {
            openCard(hit)
        } else if mode != .home {
            pin = nil
            go(.home, SheetMode.small)
        }
    }

    private func addLibrary(_ item: LibraryItem) {
        go(.home, SheetMode.small)
        doc.importing(item.title, { await Importer.openBundled(item) }, showBy: item.showBy)
    }

    private func startLook(_ id: UUID, showBy: String?) {
        guard let layer = doc.layer(id) else { return }
        lookOriginal = layer.style
        if let showBy {
            var s = layer.style
            s.showBy = showBy
            doc.setStyle(id, s)
        }
        go(.look(id), SheetMode.look)
    }

    private func endLook(keep: Bool) {
        guard case .look(let id) = mode else { return }
        if !keep, let lookOriginal { doc.setStyle(id, lookOriginal) }
        lookOriginal = nil
        go(.data(id), .medium)
    }

    private var highlightHit: MapHit? {
        if case .feature(let hit) = mode { return hit }
        return nil
    }

    private var mapInsets: UIEdgeInsets {
        UIEdgeInsets(top: 90, left: 36, bottom: 170, right: 36)
    }

    private var overlapBinding: Binding<Bool> {
        Binding(get: { !overlap.isEmpty }, set: { if !$0 { overlap = [] } })
    }

    private var errorBinding: Binding<Bool> {
        Binding(get: { doc.error != nil }, set: { if !$0 { doc.error = nil } })
    }

    // MARK: Chrome

    private var topTrailingChrome: some View {
        HStack {
            Spacer()
            VStack(alignment: .trailing, spacing: 10) {
                Menu {
                    Picker("Map style", selection: $basemap) {
                        ForEach(Basemap.allCases) { b in Text(b.label).tag(b) }
                    }
                } label: {
                    Image(systemName: "map")
                        .font(.body.weight(.semibold))
                        .frame(width: 44, height: 44)
                        .background(.regularMaterial, in: Circle())
                }
                .accessibilityLabel("Map style")
                Button {
                    if let first = doc.layers.first { doc.fly(to: first.dataset.bounds) }
                } label: {
                    Image(systemName: "scope")
                        .font(.body.weight(.semibold))
                        .frame(width: 44, height: 44)
                        .background(.regularMaterial, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Show all data")
                if ProcessInfo.processInfo.environment["LM_FPS"] == "1" {
                    FpsOverlay()
                }
            }
            .padding(.trailing, 14)
            .padding(.top, 8)
        }
    }

    private var lookTopBar: some View {
        HStack {
            Button("Cancel") { endLook(keep: false) }
                .buttonStyle(.bordered)
            Spacer()
            Button("Done") { endLook(keep: true) }
                .buttonStyle(.borderedProminent)
        }
        .controlSize(.large)
        .padding(.horizontal, 16)
        .padding(.top, 8)
    }

    private var hint: some View {
        Text("Tap any area to learn about it")
            .font(.subheadline.weight(.semibold))
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(.regularMaterial, in: Capsule())
            .transition(.opacity)
    }

    // MARK: Scenes (spike-only: LM_SCENE=… for headless screenshots)

    private func runSceneIfAsked() {
        guard !sceneRun, let scene = ProcessInfo.processInfo.environment["LM_SCENE"] else { return }
        sceneRun = true
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            let countries = doc.layers.first
            func findHit(_ name: String) -> MapHit? {
                guard let layer = countries,
                      let f = layer.dataset.features.first(where: { layer.dataset.title(of: $0) == name }) else { return nil }
                return MapHit(layerID: layer.id, featureIndex: f.index)
            }
            switch scene {
            case "feature":
                if let hit = findHit("Brazil"), case let (_, f)? = doc.feature(hit) {
                    openCard(hit)
                    doc.fly(to: f.bounds)
                }
            case "data":
                if let id = countries?.id { go(.data(id), .medium) }
            case "look":
                if let id = countries?.id { startLook(id, showBy: nil) }
            case "add":
                go(.add, .large)
            case "medium":
                if let quakes = Library.items.first(where: { $0.id == "earthquakes" }) { addLibrary(quakes) }
                try? await Task.sleep(nanoseconds: 2_500_000_000)
                go(.home, .medium)
            case "list":
                if let id = countries?.id { go(.list(id), .large) }
            case "remove":
                if let id = countries?.id { doc.remove(id) }
            default:
                break
            }
        }
    }
}

enum SheetMode: Equatable {
    case home
    case feature(MapHit)
    case data(UUID)
    case look(UUID)
    case list(UUID)
    case add
    case pin

    static let small = PresentationDetent.height(150)
    static let look = PresentationDetent.height(270)

    var detents: Set<PresentationDetent> {
        switch self {
        case .look: return [SheetMode.look]
        case .add, .list: return [.medium, .large]
        default: return [SheetMode.small, .medium, .large]
        }
    }
}

struct ToastView: View {
    var toast: Toast
    var onUndo: () -> Void

    var body: some View {
        HStack(spacing: 14) {
            Text(toast.message).font(.subheadline.weight(.semibold))
            if toast.canUndo {
                Button("Undo", action: onUndo).font(.subheadline.weight(.bold))
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(.regularMaterial, in: Capsule())
        .transition(.move(edge: .top).combined(with: .opacity))
    }
}
