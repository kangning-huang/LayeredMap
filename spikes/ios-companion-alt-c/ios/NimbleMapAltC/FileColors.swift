import Foundation

/// Alt C: keep the colors a My Maps / Google Earth file already has (Priya: "Keep my My Maps
/// colours"). Reads `<Style id><IconStyle|LineStyle|PolyStyle><color>` and ties them to the
/// category column that lines up with the styles one-to-one.
enum FileColors {
    /// Style id → 0xRRGGBB. KML colors are aabbggrr hex.
    static func kmlStyles(at url: URL) -> [String: UInt32] {
        guard let parser = XMLParser(contentsOf: url) else { return [:] }
        let reader = StyleReader()
        parser.delegate = reader
        parser.parse()
        return reader.colors
    }

    /// The column whose values each map to one style, and value → color. Category columns first;
    /// the raw style id column is the fallback.
    static func match(_ dataset: GeoDataset, styles: [String: UInt32]) -> (String, [String: UInt32])? {
        guard !styles.isEmpty, let styleField = dataset.fields.first(where: { $0.name.lowercased() == "style" }) else { return nil }
        let candidates = dataset.fields.filter { $0.kind == .category && $0.name != styleField.name } + [styleField]
        for field in candidates {
            var colors: [String: UInt32] = [:]
            var consistent = true
            for f in dataset.features {
                guard let value = f.properties[field.name].flatMap(GeoValue.text),
                      let id = f.properties[styleField.name].flatMap(GeoValue.text),
                      let color = styles[id] else { continue }
                if let seen = colors[value], seen != color {
                    consistent = false
                    break
                }
                colors[value] = color
            }
            if consistent && colors.count >= 2 { return (field.name, colors) }
        }
        return nil
    }

    private final class StyleReader: NSObject, XMLParserDelegate {
        var colors: [String: UInt32] = [:]
        private var styleID: String?
        private var inColorOf: String?
        private var text = ""

        func parser(_ parser: XMLParser, didStartElement name: String, namespaceURI: String?, qualifiedName: String?, attributes: [String: String] = [:]) {
            let local = name.split(separator: ":").last.map(String.init) ?? name
            switch local {
            case "Style": styleID = attributes["id"]
            case "color": inColorOf = styleID
            default: break
            }
            text = ""
        }

        func parser(_ parser: XMLParser, foundCharacters string: String) {
            text += string
        }

        func parser(_ parser: XMLParser, didEndElement name: String, namespaceURI: String?, qualifiedName: String?) {
            let local = name.split(separator: ":").last.map(String.init) ?? name
            if local == "color", let id = inColorOf, colors[id] == nil,
               let abgr = UInt32(text.trimmingCharacters(in: .whitespacesAndNewlines), radix: 16) {
                // First color in a style wins (IconStyle comes before LineStyle/PolyStyle in My Maps).
                let r = abgr & 0xFF, g = (abgr >> 8) & 0xFF, b = (abgr >> 16) & 0xFF
                colors[id] = (r << 16) | (g << 8) | b
            }
            if local == "color" { inColorOf = nil }
            if local == "Style" { styleID = nil }
        }
    }
}
