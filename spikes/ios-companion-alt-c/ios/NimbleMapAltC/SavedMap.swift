import Foundation

/// Alt C: what was on the map last time (Priya: "When I open the app on the trip, show me my Lisbon
/// map"). Each layer is a file already copied into the app, or a library item, with its look.
enum SavedMap {
    struct Layer: Codable {
        var library: String?
        /// Relative to `Importer.dataFolder`, so a reinstall's new container path still works.
        var path: String?
        var name: String
        var visible: Bool
        var style: LayerStyle

        func open() async -> Result<GeoDataset, ImportError>? {
            if let library, let item = Library.items.first(where: { $0.id == library }) {
                return await Importer.openBundled(item)
            }
            if let path {
                let url = Importer.dataFolder.appendingPathComponent(path)
                guard FileManager.default.fileExists(atPath: url.path) else { return nil }
                return await Importer.decode(url, name: name, primary: nil, sourceName: "Your file", credit: nil)
            }
            return nil
        }
    }

    private static let key = "lm.savedMap"

    /// Top-first, like the list.
    static func save(_ layers: [MapLayer]) {
        let entries = layers.compactMap { l -> Layer? in
            guard l.dataset.libraryID != nil || l.dataset.savedPath != nil else { return nil }
            return Layer(library: l.dataset.libraryID, path: l.dataset.savedPath, name: l.name, visible: l.visible, style: l.style)
        }
        if let data = try? JSONEncoder().encode(entries) {
            UserDefaults.standard.set(data, forKey: key)
        }
    }

    static func load() -> [Layer] {
        guard let data = UserDefaults.standard.data(forKey: key) else { return [] }
        return (try? JSONDecoder().decode([Layer].self, from: data)) ?? []
    }

    private static let viewKey = "lm.savedView"

    /// Where the map was looking (T7: "where I left off in Yorkshire", not the whole Atlantic).
    static var view: GeoBounds? {
        get {
            guard let a = UserDefaults.standard.array(forKey: viewKey) as? [Double], a.count == 4 else { return nil }
            return GeoBounds(minLon: a[0], minLat: a[1], maxLon: a[2], maxLat: a[3])
        }
        set {
            if let b = newValue {
                UserDefaults.standard.set([b.minLon, b.minLat, b.maxLon, b.maxLat], forKey: viewKey)
            } else {
                UserDefaults.standard.removeObject(forKey: viewKey)
            }
        }
    }

    /// Nothing saved yet: a first run (or every layer was removed).
    static var isEmpty: Bool { load().isEmpty }
}
