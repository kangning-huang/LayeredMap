import SwiftUI

/// **Alternative A:** A idle (map-first floating pills) → B Layers sheet → C symbology.
/// Open paths per docs/lm/ios-open-paths.md (proposed v1 stack). Tap a feature → the same card
/// as Alt B (owner, 2026-11-02), as a sheet. Bundle label: NimbleMap A.
struct ContentView: View {
    @StateObject private var doc = MapDocument()
    @State private var active: ActiveSheet?
    @State private var showFiles = false
    @State private var status: String?
    @State private var sceneRun = false
    @State private var overlap: [MapHit] = []

    var body: some View {
        ZStack {
            MapScreen(
                layers: doc.layers,
                camera: doc.camera,
                highlight: doc.outline(featureHit),
                insets: UIEdgeInsets(top: 90, left: 36, bottom: 120, right: 36),
                onTap: handleTap,
                onUserGesture: {
                    // Resting state is always A: pan or pinch collapses B and C.
                    if active == .layers { active = nil }
                }
            )
            .ignoresSafeArea()

            VStack(spacing: 0) {
                topChrome
                Spacer()
                if doc.isEmpty && doc.busy == nil {
                    OpenCard(
                        onBrowse: { active = .browse },
                        onExamples: { active = .open(.examples) },
                        onFiles: { active = .open(.phone) },
                        onURL: { active = .url },
                        onRecent: reopen
                    )
                    .padding(.horizontal, 16)
                    .padding(.bottom, 16)
                }
                bottomChrome
            }
        }
        .sheet(item: $active) { sheet in
            switch sheet {
            case .open(let start):
                OpenSheet(
                    start: start,
                    doc: doc,
                    onBrowse: { switchTo(.browse) },
                    onFiles: {
                        active = nil
                        showFiles = true
                    },
                    onURL: { switchTo(.url) },
                    onExample: { item in
                        active = nil
                        doc.importing(item.title, { await Importer.openBundled(item) }, showBy: item.showBy)
                    },
                    onPhoneFile: { f in
                        active = nil
                        doc.importingAll(f.name) { await PhoneFiles.open(f) }
                    },
                    onPicked: { urls in
                        active = nil
                        doc.importingAll(Importer.pickedLabel(urls)) { await Importer.openPicked(urls) }
                    }
                )
            case .browse:
                BrowseSheet { url in
                    active = nil
                    doc.importing(Importer.displayName(for: url)) { await Importer.download(url) }
                }
            case .url:
                OpenURLSheet { raw in
                    guard let url = URL(string: raw.trimmingCharacters(in: .whitespacesAndNewlines)) else {
                        doc.error = "That isn't a web link."
                        return
                    }
                    doc.importing(Importer.displayName(for: url)) { await Importer.download(url) }
                }
            case .layers:
                LayersSheet(doc: doc, startStyle: nil)
            case .style(let id):
                LayersSheet(doc: doc, startStyle: id)
            case .ask:
                AskSheet()
            case .feature(let hit):
                featureSheet(hit)
            case .list(let id):
                listSheet(id)
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
        .confirmationDialog("\(overlap.count) things here", isPresented: overlapBinding, titleVisibility: .visible) {
            ForEach(overlap, id: \.self) { hit in
                if case let (layer, f)? = doc.feature(hit) {
                    Button("\(layer.dataset.title(of: f)) · \(layer.name)") { show(hit) }
                }
            }
        }
        .alert("Couldn't open this", isPresented: errorBinding) {
            Button("OK", role: .cancel) { doc.error = nil }
        } message: {
            Text(doc.error ?? "")
        }
        .onChange(of: doc.layers.count) { old, new in
            if new > old, let top = doc.layers.first {
                flash("Opened \(top.dataset.countPhrase)")
            }
            runSceneIfAsked()
        }
        .onChange(of: doc.toast) { _, t in
            // Alt A has no Undo toast; Remove is the plain iOS swipe.
            if t != nil { doc.toast = nil }
        }
        .task { runSceneIfAsked() }
    }

    // MARK: Chrome (state A: floating glass pills only)

    private var topChrome: some View {
        HStack(spacing: 10) {
            pill("Open", systemImage: "plus") { active = .open(.home) }
            Spacer()
            if ProcessInfo.processInfo.environment["LM_FPS"] == "1" {
                FpsOverlay()
            }
            pill("Ask", systemImage: "sparkles") { active = .ask }
        }
        .padding(.horizontal, 16)
        .padding(.top, 40)
    }

    private var bottomChrome: some View {
        VStack(spacing: 10) {
            if let busy = doc.busy {
                capsule(busy, systemImage: "arrow.down.circle")
            } else if let status {
                capsule(status, systemImage: "checkmark.circle")
            }
            if !doc.isEmpty {
                pill("Layers", systemImage: "square.3.layers.3d") { active = .layers }
            }
        }
        .padding(.bottom, 12)
        .animation(.snappy, value: status)
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

    private func capsule(_ text: String, systemImage: String) -> some View {
        Label(text, systemImage: systemImage)
            .font(.footnote.weight(.semibold))
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(.regularMaterial, in: Capsule())
            .transition(.opacity)
    }

    // MARK: Actions

    // MARK: Tap a feature

    private var featureHit: MapHit? {
        if case .feature(let hit) = active { return hit }
        return nil
    }

    private func handleTap(_ hits: [MapHit], _ point: CGPoint) {
        if hits.count > 1 {
            // The chooser shows from the map, so a sheet in the way closes first.
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

    private func show(_ hit: MapHit) {
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
                onColorBy: { field in
                    var s = layer.style
                    s.showBy = field
                    doc.setStyle(layer.id, s)
                    switchTo(.style(layer.id))
                },
                onZoom: { doc.fly(to: f.bounds, pointZoom: 6) },
                onList: { switchTo(.list(layer.id)) }
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
                if case let (_, f)? = doc.feature(hit) { doc.fly(to: f.bounds, pointZoom: 5) }
                switchTo(.feature(hit))
            }
            .presentationDetents([.medium, .large])
        }
    }

    private var overlapBinding: Binding<Bool> {
        Binding(get: { !overlap.isEmpty }, set: { if !$0 { overlap = [] } })
    }

    private func switchTo(_ sheet: ActiveSheet) {
        active = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.45) { active = sheet }
    }

    private func reopen(_ r: Importer.Recent) {
        doc.importing(r.name) { await Importer.reopen(r) }
    }

    private func flash(_ text: String) {
        status = text
        Task {
            try? await Task.sleep(nanoseconds: 2_500_000_000)
            if status == text { status = nil }
        }
    }

    private var errorBinding: Binding<Bool> {
        Binding(get: { doc.error != nil }, set: { if !$0 { doc.error = nil } })
    }

    // MARK: Scenes (spike-only: LM_SCENE=… for headless screenshots)

    private func runSceneIfAsked() {
        guard !sceneRun, let scene = ProcessInfo.processInfo.environment["LM_SCENE"] else { return }
        let needsData = ["layers", "style", "examples-added", "medium", "bench"].contains(scene)
        if needsData && doc.isEmpty {
            if doc.busy == nil, let countries = Library.items.first {
                doc.importing(countries.title, { await Importer.openBundled(countries) }, showBy: countries.showBy)
            }
            return
        }
        sceneRun = true
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 1_200_000_000)
            switch scene {
            case "open": active = .open(.home)
            case "examples": active = .open(.examples)
            case "phone": active = .open(.phone)
            case "browse": active = .browse
            case "url": active = .url
            case "ask": active = .ask
            case "layers", "medium":
                if let quakes = Library.items.first(where: { $0.id == "earthquakes" }) {
                    doc.importing(quakes.title, { await Importer.openBundled(quakes) }, showBy: quakes.showBy)
                }
                try? await Task.sleep(nanoseconds: 3_000_000_000)
                if scene == "layers" { active = .layers }
            case "style":
                if let id = doc.layers.first?.id { active = .style(id) }
            case "feature":
                if let id = doc.layers.first?.id { active = .feature(MapHit(layerID: id, featureIndex: 0)) }
            default:
                break
            }
        }
    }
}

enum ActiveSheet: Identifiable, Equatable {
    case open(OpenSheet.Start)
    case browse
    case url
    case layers
    case style(UUID)
    case ask
    case feature(MapHit)
    case list(UUID)

    var id: String {
        switch self {
        case .open(let s): return "open-\(s)"
        case .browse: return "browse"
        case .url: return "url"
        case .layers: return "layers"
        case .style(let id): return "style-\(id)"
        case .ask: return "ask"
        case .feature(let hit): return "feature-\(hit.layerID)-\(hit.featureIndex)"
        case .list(let id): return "list-\(id)"
        }
    }
}

/// Ask is a pill, not the layers sheet. Non-functional in this spike.
struct AskSheet: View {
    @Environment(\.dismiss) private var dismiss
    @State private var draft = ""

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 14) {
                TextField("Ask about these layers…", text: $draft, axis: .vertical)
                    .padding(12)
                    .background(Color(.tertiarySystemFill), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                Text("Ask (Pro) is not built in this spike.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                Spacer()
            }
            .padding()
            .navigationTitle("Ask")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } }
            }
        }
        .presentationDetents([.medium])
    }
}
