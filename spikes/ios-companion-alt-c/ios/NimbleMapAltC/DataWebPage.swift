import SafariServices
import SwiftUI
import WebKit

/// A's in-app browser on the NimbleMap site's data page, back in C under the Data library (the
/// owner: the site's Data tab is a good way to browse datasets). A tap on a link to a file NimbleMap
/// opens downloads it onto the map; anything else stays a web page.
struct DataWebPage: View {
    var onDataLink: (URL) -> Void

    @State private var safari = false
    static let page = URL(string: "https://nimblemap.com/data.html")

    var body: some View {
        Group {
            if let page = DataWebPage.page {
                DataWebView(url: page, onDataLink: onDataLink)
                    .ignoresSafeArea(edges: .bottom)
            }
        }
        .navigationTitle("NimbleMap data")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    safari = true
                } label: {
                    Image(systemName: "safari")
                }
                .accessibilityLabel("Open in Safari")
            }
        }
        .sheet(isPresented: $safari) {
            if let page = DataWebPage.page { SafariView(url: page) }
        }
    }
}

struct DataWebView: UIViewRepresentable {
    var url: URL
    var onDataLink: (URL) -> Void

    func makeUIView(context: Context) -> WKWebView {
        let web = WKWebView(frame: .zero)
        web.navigationDelegate = context.coordinator
        web.load(URLRequest(url: url))
        return web
    }

    func updateUIView(_ web: WKWebView, context: Context) {
        context.coordinator.onDataLink = onDataLink
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onDataLink: onDataLink)
    }

    final class Coordinator: NSObject, WKNavigationDelegate {
        var onDataLink: (URL) -> Void

        init(onDataLink: @escaping (URL) -> Void) {
            self.onDataLink = onDataLink
        }

        static func isData(_ url: URL) -> Bool {
            let ext = url.pathExtension.lowercased()
            if Importer.extensions.contains(ext) && ext != "txt" && ext != "json" { return true }
            let s = url.absoluteString.lowercased()
            return s.contains("rows.csv") || s.hasSuffix(".geojson") || s.contains("format=geojson")
        }

        @MainActor
        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction) async -> WKNavigationActionPolicy {
            if let url = navigationAction.request.url, navigationAction.navigationType == .linkActivated, Coordinator.isData(url) {
                onDataLink(url)
                return .cancel
            }
            return .allow
        }
    }
}

struct SafariView: UIViewControllerRepresentable {
    var url: URL

    func makeUIViewController(context: Context) -> SFSafariViewController {
        SFSafariViewController(url: url)
    }

    func updateUIViewController(_ controller: SFSafariViewController, context: Context) {}
}
