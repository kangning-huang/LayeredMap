import SwiftUI
import CoreLocation

/// **Alternative C**, from the A/B persona study (LM-tmp/personas/report.md): A's clean map, file
/// flow, recents and Layers list, with B's first run (a sample map and one hint), insight cards,
/// in-app data library and picture-preview "Change look". Bundle label: NimbleMap C.
struct ContentView: View {
    @StateObject private var doc = MapDocument()
    @State private var active: ActiveSheet?
    @State private var showFiles = false
    @State private var showHint = false
    @State private var basemap: Basemap = .light
    @State private var overlap: [MapHit] = []
    /// Where the "N things here" tap was, so map images under it can be offered too.
    @State private var overlapAt: CLLocationCoordinate2D?
    /// The look before "Change look" opened, for Cancel. Swiping the sheet away keeps the new look,
    /// like Done (nothing is ever trapped open).
    @State private var lookOriginal: (UUID, LayerStyle)?
    @State private var started = false
    @State private var cameraReady = false
    @State private var sceneRun = false

    var body: some View {
        ZStack {
            MapScreen(
                layers: doc.layers,
                camera: doc.camera,
                highlight: doc.outline(featureHit),
                basemap: basemap,
                insets: mapInsets,
                onTap: handleTap,
                onUserGesture: {
                    if active == .layers { active = nil }
                },
                onCameraIdle: { bounds in
                    doc.viewBounds = bounds
                    // Only once the saved map is back, and only for the person's own map.
                    if cameraReady && doc.sampleID == nil && !doc.isEmpty { SavedMap.view = bounds }
                }
            )
            .ignoresSafeArea()

            VStack(spacing: 8) {
                topChrome
                if showHint && !doc.isEmpty && active == nil {
                    capsule("Tap anything on the map to learn about it", systemImage: "hand.tap")
                }
                if let toast = doc.toast {
                    ToastView(toast: toast, onUndo: { doc.undo() }, onAction: {
                        doc.toastAction?()
                        doc.toast = nil
                    })
                        .task(id: toast.id) {
                            try? await Task.sleep(nanoseconds: toast.actionTitle == nil ? 5_000_000_000 : 8_000_000_000)
                            if doc.toast?.id == toast.id { doc.toast = nil }
                        }
                }
                if let busy = doc.busy {
                    capsule(busy, systemImage: "arrow.down.circle")
                }
                Spacer()
                bottomChrome
            }
            .animation(.snappy, value: doc.toast)
            .animation(.snappy, value: showHint)
        }
        .sheet(item: $active) { sheet in
            switch sheet {
            case .open:
                OpenSheet(
                    doc: doc,
                    onLibrary: addLibrary,
                    onPhoneFile: { f in
                        active = nil
                        doc.importingAll(f.name) { await PhoneFiles.open(f) }
                    },
                    onRecent: { r in
                        active = nil
                        reopen(r)
                    },
                    onPicked: { urls in
                        active = nil
                        doc.importingAll(Importer.pickedLabel(urls)) { await Importer.openPicked(urls) }
                    },
                    onLink: { url in
                        active = nil
                        doc.importing(Importer.displayName(for: url)) { await Importer.download(url) }
                    }
                )
            case .layers:
                LayersSheet(
                    doc: doc,
                    onLook: { id in startLook(id, showBy: nil) },
                    onList: { id in switchTo(.list(id)) },
                    onOpen: { switchTo(.open) }
                )
            case .look(let id):
                lookSheet(id)
            case .style(let id):
                NavigationStack { SymbologySheet(doc: doc, layerID: id) }
                    .presentationDetents([.medium, .large])
                    .presentationBackgroundInteraction(.enabled(upThrough: .medium))
            case .feature(let hit):
                featureSheet(hit)
            case .list(let id):
                listSheet(id)
            case .rasterSpot(let id, let lat, let lon):
                if let layer = doc.layer(id) {
                    RasterCard(
                        layer: layer,
                        at: CLLocationCoordinate2D(latitude: lat, longitude: lon),
                        onClose: { active = nil },
                        onLook: { startLook(id, showBy: nil) },
                        onZoom: { doc.fly(to: layer.dataset.bounds) }
                    )
                    .presentationDetents([.medium, .large])
                    .presentationBackgroundInteraction(.enabled(upThrough: .medium))
                    .presentationDragIndicator(.visible)
                }
            }
        }
        .fileImporter(isPresented: $showFiles, allowedContentTypes: Importer.contentTypes, allowsMultipleSelection: true) { result in
            guard case .success(let urls) = result, !urls.isEmpty else { return }
            doc.importingAll(Importer.pickedLabel(urls)) { await Importer.openPicked(urls) }
        }
        .onOpenURL { url in
            active = nil
            doc.importing(Importer.displayName(for: url)) { await Importer.openExternal(url) }
        }
        .confirmationDialog("\(overlap.count + overlapRasters.count) things here", isPresented: overlapBinding, titleVisibility: .visible) {
            ForEach(overlap, id: \.self) { hit in
                if case let (layer, f)? = doc.feature(hit) {
                    Button("\(Insights.shortLabel(f, in: layer.dataset)) · \(layer.name)") { show(hit) }
                }
            }
            // A map image under the finger is one of the things here too (Priya).
            ForEach(overlapRasters, id: \.0.id) { layer, label in
                Button(label) {
                    if let at = overlapAt { show(raster: layer.id, at: at) }
                }
            }
        }
        .alert("Couldn't open this", isPresented: errorBinding) {
            Button("OK", role: .cancel) { doc.error = nil }
        } message: {
            Text(doc.error ?? "")
        }
        .task { await start() }
    }

    // MARK: First run and return

    /// Last time's map if there is one (T7); otherwise B's first run: the sample map and one hint.
    private func start() async {
        guard !started else { return }
        started = true
        if ProcessInfo.processInfo.environment["LM_FRESH"] == "1" {
            SavedMap.save([])
            SavedMap.view = nil
        }
        defer {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) { cameraReady = true }
        }
        if await doc.restore() {
            runSceneIfAsked()
            return
        }
        guard doc.isEmpty, let countries = Library.items.first else { return }
        switch await Importer.openBundled(countries) {
        case .success(let ds):
            doc.add(ds, showBy: countries.showBy, sample: true)
            showHint = true
        case .failure(let e):
            doc.error = e.message
        }
        runSceneIfAsked()
    }

    /// What the chrome covers; a card or list at half height covers the lower half, so "Zoom to"
    /// puts the place in the half above it.
    private var mapInsets: UIEdgeInsets {
        switch active {
        case .feature, .list, .layers, .open, .style, .rasterSpot:
            return UIEdgeInsets(top: 110, left: 36, bottom: 480, right: 36)
        case .look:
            return UIEdgeInsets(top: 110, left: 36, bottom: 390, right: 36)
        case nil:
            return UIEdgeInsets(top: 110, left: 36, bottom: 170, right: 36)
        }
    }

    // MARK: Chrome (A's floating pills; B's two round map buttons)

    private var topChrome: some View {
        HStack(alignment: .top, spacing: 10) {
            pill("Add data", systemImage: "plus") { active = .open }
            Spacer()
            VStack(spacing: 10) {
                Menu {
                    Picker("Map style", selection: $basemap) {
                        ForEach(Basemap.allCases) { b in Text(b.label).tag(b) }
                    }
                } label: {
                    roundIcon("map")
                }
                .accessibilityLabel("Map style")
                Button {
                    var all = GeoBounds.empty
                    for l in doc.layers where l.visible { all.add(l.dataset.bounds) }
                    doc.fly(to: all)
                } label: {
                    roundIcon("scope")
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Show all data")
                if ProcessInfo.processInfo.environment["LM_FPS"] == "1" {
                    FpsOverlay()
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 52)
    }

    @ViewBuilder
    private var bottomChrome: some View {
        VStack(spacing: 10) {
            if doc.isEmpty && doc.busy == nil {
                StartCard(sample: false, onOpen: { active = .open }, onRecent: reopen)
                    .padding(.horizontal, 16)
            } else if doc.sampleID != nil && doc.layers.count == 1 && active == nil {
                StartCard(sample: true, onOpen: { active = .open }, onRecent: reopen)
                    .padding(.horizontal, 16)
            } else if !doc.isEmpty {
                // The legend is the way into Layers (the owner: a Layers pill beside it said the
                // same thing twice), so it stays even when every layer is hidden.
                HStack(alignment: .bottom) {
                    Button { active = .layers } label: { MapLegend(layers: doc.layers) }
                        .buttonStyle(.plain)
                    Spacer()
                }
                .padding(.horizontal, 16)
            }
        }
        .padding(.bottom, 24)
    }

    private func pill(_ title: String, systemImage: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(title, systemImage: systemImage)
                .font(.subheadline.weight(.semibold))
                .padding(.horizontal, 16)
                .frame(minHeight: 44)
                .background(.regularMaterial, in: Capsule())
        }
        .buttonStyle(.plain)
    }

    private func roundIcon(_ name: String) -> some View {
        Image(systemName: name)
            .font(.body.weight(.semibold))
            .frame(width: 44, height: 44)
            .background(.regularMaterial, in: Circle())
    }

    private func capsule(_ text: String, systemImage: String) -> some View {
        Label(text, systemImage: systemImage)
            .font(.subheadline.weight(.semibold))
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .background(.regularMaterial, in: Capsule())
            .transition(.opacity)
    }

    // MARK: Tap a feature

    private var featureHit: MapHit? {
        if case .feature(let hit) = active { return hit }
        return nil
    }

    private func handleTap(_ hits: [MapHit], _ at: CLLocationCoordinate2D) {
        showHint = false
        if case .look = active { return }
        // Nothing drawn here but a map image under the finger: read the image at this spot.
        if hits.isEmpty, let raster = doc.layers.first(where: { l in
            guard l.visible, let r = l.dataset.raster else { return false }
            let b = r.bounds
            return at.longitude >= b.minLon && at.longitude <= b.maxLon && at.latitude >= b.minLat && at.latitude <= b.maxLat
        }) {
            let spot = ActiveSheet.rasterSpot(raster.id, at.latitude, at.longitude)
            if active == nil { active = spot } else { switchTo(spot) }
            return
        }
        let rasterHere = rasterUnder(at)
        if hits.count > 1 || (hits.count == 1 && rasterHere != nil && hits.first.flatMap { doc.layer($0.layerID) }?.dataset.kind == .area) {
            overlapAt = at
            if active == nil {
                overlap = hits
            } else {
                active = nil
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.45) { overlap = hits }
            }
        } else if let hit = hits.first {
            show(hit)
        } else if featureHit != nil {
            active = nil
        }
    }

    /// Visible map images under a point, topmost first.
    private func rasterUnder(_ at: CLLocationCoordinate2D) -> MapLayer? {
        rastersUnder(at).first
    }

    private func rastersUnder(_ at: CLLocationCoordinate2D) -> [MapLayer] {
        doc.layers.filter { l in
            guard l.visible, let r = l.dataset.raster else { return false }
            let b = r.bounds
            return at.longitude >= b.minLon && at.longitude <= b.maxLon && at.latitude >= b.minLat && at.latitude <= b.maxLat
        }
    }

    private var overlapRasters: [(MapLayer, String)] {
        guard let at = overlapAt else { return [] }
        return rastersUnder(at).map { l in
            if let r = l.dataset.raster, r.look == .values, let v = r.value(at: at) {
                return (l, "\(r.quantity) here: \(r.format(v)) · \(l.name)")
            }
            return (l, "\(l.name) · map image")
        }
    }

    /// "Height here: 125 m · Lisbon elevation" for each grid of values under a place.
    private func groundRows(_ f: GeoFeature) -> [String] {
        rastersUnder(f.center).compactMap { l in
            guard let r = l.dataset.raster, r.look == .values, let v = r.value(at: f.center) else { return nil }
            return "\(r.quantity) here: \(r.format(v)) · \(l.name)"
        }
    }

    private func show(raster id: UUID, at: CLLocationCoordinate2D) {
        let spot = ActiveSheet.rasterSpot(id, at.latitude, at.longitude)
        if active == nil { active = spot } else { switchTo(spot) }
    }

    private func show(_ hit: MapHit) {
        showHint = false
        if active == nil {
            active = .feature(hit)
        } else {
            switchTo(.feature(hit))
        }
    }

    @ViewBuilder
    private func featureSheet(_ hit: MapHit) -> some View {
        if case let (layer, f)? = doc.feature(hit) {
            FeatureCard(
                insight: Insights.feature(f, in: layer.dataset, layerID: layer.id),
                layer: layer,
                onClose: { active = nil },
                onColorBy: { field in startLook(layer.id, showBy: field) },
                onZoom: { doc.fly(to: f.bounds, pointZoom: 14) },
                onList: { switchTo(.list(layer.id)) },
                groundRows: groundRows(f)
            )
            .presentationDetents([.medium, .large])
            .presentationBackgroundInteraction(.enabled(upThrough: .medium))
            .presentationDragIndicator(.visible)
        } else {
            Text("This layer was removed.").foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private func listSheet(_ id: UUID) -> some View {
        if let layer = doc.layer(id) {
            FeatureList(layer: layer, onClose: { active = nil }) { index in
                let hit = MapHit(layerID: id, featureIndex: index)
                if case let (_, f)? = doc.feature(hit) { doc.fly(to: f.bounds, pointZoom: 13) }
                switchTo(.feature(hit))
            }
            .presentationDetents([.medium, .large])
        }
    }

    // MARK: Change look (B's previews in a sheet that always closes)

    private func startLook(_ id: UUID, showBy: String?) {
        guard let layer = doc.layer(id) else { return }
        lookOriginal = (id, layer.style)
        if let showBy {
            var s = layer.style
            s.showBy = showBy
            doc.setStyle(id, s)
        }
        if active == nil {
            active = .look(id)
        } else {
            switchTo(.look(id))
        }
    }

    @ViewBuilder
    private func lookSheet(_ id: UUID) -> some View {
        if let layer = doc.layer(id) {
            ChangeLookSheet(
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
                },
                onCancel: {
                    if let (lid, style) = lookOriginal, lid == id { doc.setStyle(id, style) }
                    lookOriginal = nil
                    active = nil
                },
                onDone: {
                    lookOriginal = nil
                    active = nil
                },
                onMore: {
                    lookOriginal = nil
                    switchTo(.style(id))
                }
            )
        }
    }

    // MARK: Actions

    private func addLibrary(_ item: LibraryItem) {
        active = nil
        doc.importing(item.title, { await Importer.openBundled(item) }, showBy: item.showBy)
    }

    private func switchTo(_ sheet: ActiveSheet) {
        active = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.45) { active = sheet }
    }

    private func reopen(_ r: Importer.Recent) {
        doc.importing(r.name) { await Importer.reopen(r) }
    }

    private var overlapBinding: Binding<Bool> {
        Binding(get: { !overlap.isEmpty }, set: { if !$0 { overlap = [] } })
    }

    private var errorBinding: Binding<Bool> {
        Binding(get: { doc.error != nil }, set: { if !$0 { doc.error = nil } })
    }

    // MARK: Scenes (spike-only: LM_SCENE=… for headless screenshots)

    private func runSceneIfAsked() {
        guard !sceneRun, let scene = ProcessInfo.processInfo.environment["LM_SCENE"] else { return }
        sceneRun = true
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            switch scene {
            case "open": active = .open
            case "layers": active = .layers
            case "look": if let id = doc.layers.first?.id { startLook(id, showBy: nil) }
            case "feature": if let id = doc.layers.first?.id { show(MapHit(layerID: id, featureIndex: 0)) }
            default: break
            }
        }
    }
}

enum ActiveSheet: Identifiable, Equatable {
    case open
    case layers
    case look(UUID)
    case style(UUID)
    case feature(MapHit)
    case list(UUID)
    /// A tap on a map image: the layer and where.
    case rasterSpot(UUID, Double, Double)

    var id: String {
        switch self {
        case .open: return "open"
        case .layers: return "layers"
        case .look(let id): return "look-\(id)"
        case .style(let id): return "style-\(id)"
        case .feature(let hit): return "feature-\(hit.layerID)-\(hit.featureIndex)"
        case .list(let id): return "list-\(id)"
        case .rasterSpot(let id, let lat, let lon): return "raster-\(id)-\(lat)-\(lon)"
        }
    }
}

/// The start card: recents up front (A), over the sample map (B) or an empty one.
struct StartCard: View {
    var sample: Bool
    var onOpen: () -> Void
    var onRecent: (Importer.Recent) -> Void

    var body: some View {
        let recents = Importer.recents(limit: 3)
        VStack(alignment: .leading, spacing: 10) {
            Text(sample ? "This is a sample map" : "Start your map").font(.headline)
            // D: the first screen names the library and links, not just files. The persona read
            // "Open a map file" as "files only", decided she had none, and stopped there.
            Text(sample
                 ? "Tap a country to try it. Add data from the library, paste a link, or open your own file — the sample steps aside."
                 : "Add data from the library, paste a link to one you found online, or open a file from your iPhone, Google Drive or iCloud.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
            ForEach(recents) { r in
                Button {
                    onRecent(r)
                } label: {
                    Label(r.name, systemImage: "clock").frame(maxWidth: .infinity, minHeight: 36, alignment: .leading)
                }
            }
            Button(action: onOpen) {
                Label("Add data to this map", systemImage: "square.grid.2x2")
                    .font(.body.weight(.semibold))
                    .frame(maxWidth: .infinity, minHeight: 44)
            }
            .buttonStyle(.borderedProminent)
        }
        .padding(16)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
    }
}

/// A small legend on the map for every visible layer (Priya: "Which colour is Food?"; Walt: a new
/// layer must show up in it). The top colored layer lists its colors; others get one row. It is also
/// the Layers button: a "3 layers ›" header, and hidden layers stay listed, dimmed, so they can be
/// shown again. Tap → Layers.
struct MapLegend: View {
    var layers: [MapLayer]

    var body: some View {
        let shown = layers.filter(\.visible)
        let hidden = layers.filter { !$0.visible }
        let rows = Array((shown + hidden).prefix(3))
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 4) {
                Image(systemName: "square.3.layers.3d")
                Text(layers.count == 1 ? "1 layer" : "\(layers.count) layers")
                Image(systemName: "chevron.right").font(.caption2.weight(.bold))
            }
            .font(.caption2.weight(.semibold))
            .foregroundStyle(.secondary)
            ForEach(Array(rows.enumerated()), id: \.element.id) { i, layer in
                if layer.visible {
                    entry(layer, detailed: i == firstColored(rows))
                } else {
                    HStack(spacing: 6) {
                        Image(systemName: "eye.slash").font(.caption2).frame(width: 18)
                        Text(layer.name).font(.caption).lineLimit(1)
                    }
                    .foregroundStyle(.secondary)
                }
            }
            if layers.count > 3 {
                Text("+\(layers.count - 3) more").font(.caption2).foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
        .frame(maxWidth: 210, alignment: .leading)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .contentShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .accessibilityElement(children: .combine)
        .accessibilityHint("Shows the layers")
    }

    private func firstColored(_ rows: [MapLayer]) -> Int? {
        rows.firstIndex { l in
            guard l.visible else { return false }
            if case .single = l.scheme { return false } else { return true }
        }
    }

    @ViewBuilder
    private func entry(_ layer: MapLayer, detailed: Bool) -> some View {
        switch layer.scheme {
        case .single(let c):
            HStack(spacing: 6) {
                LegendSwatch(kind: layer.dataset.kind, colors: [Color(hex: c)], opacity: 1).scaleEffect(0.6).frame(width: 18, height: 18)
                Text(layer.name).font(.caption.weight(.semibold)).lineLimit(2)
            }
        case .categories:
            VStack(alignment: .leading, spacing: 3) {
                Text(caption(layer)).font(.caption.weight(.semibold)).lineLimit(2)
                if detailed {
                    ForEach(Array(layer.scheme.legend.prefix(5).enumerated()), id: \.offset) { _, item in
                        HStack(spacing: 6) {
                            Circle().fill(item.1).frame(width: 10, height: 10)
                            Text(item.0).font(.caption2).lineLimit(1)
                        }
                    }
                } else {
                    HStack(spacing: 3) {
                        ForEach(Array(layer.scheme.swatch.enumerated()), id: \.offset) { _, c in
                            Circle().fill(c).frame(width: 8, height: 8)
                        }
                    }
                }
            }
        case .ramp:
            VStack(alignment: .leading, spacing: 3) {
                Text(caption(layer)).font(.caption.weight(.semibold)).lineLimit(2)
                LegendBar(scheme: layer.scheme, ends: layer.legendEnds).frame(width: 150)
            }
        }
    }

    private func caption(_ layer: MapLayer) -> String {
        if let r = layer.dataset.raster, r.look == .values {
            return r.unit.isEmpty ? layer.name : "\(layer.name) · \(r.quantity.lowercased()) (\(r.unit))"
        }
        guard let by = layer.style.showBy.flatMap({ layer.dataset.field(named: $0) }) else { return layer.name }
        return "\(layer.name) · by \(by.label.lowercased())"
    }
}

struct ToastView: View {
    var toast: Toast
    var onUndo: () -> Void
    var onAction: () -> Void = {}

    var body: some View {
        HStack(spacing: 14) {
            Text(toast.message).font(.subheadline.weight(.semibold))
            if let title = toast.actionTitle {
                Button(title, action: onAction).font(.subheadline.weight(.bold))
            }
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
